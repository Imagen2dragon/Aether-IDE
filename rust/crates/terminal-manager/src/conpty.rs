#[cfg(windows)]
pub mod win32 {
    use process_manager::win32_job::ScopedJobObject;
    use std::ffi::c_void;
    use std::io;
    use std::ptr;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use tokio::sync::broadcast;
    use tracing::{debug, info};

    pub type HANDLE = *mut c_void;
    pub type HPCON = *mut c_void;
    pub type HRESULT = i32;
    pub type BOOL = i32;
    pub type DWORD = u32;
    pub type WORD = u16;

    const S_OK: HRESULT = 0;

    const PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE: usize = 0x00020016;
    const EXTENDED_STARTUPINFO_PRESENT: DWORD = 0x00080000;

    #[repr(C)]
    #[derive(Debug, Copy, Clone)]
    pub struct COORD {
        pub x: i16,
        pub y: i16,
    }

    #[repr(C)]
    pub struct SECURITY_ATTRIBUTES {
        pub n_length: DWORD,
        pub lp_security_descriptor: *mut c_void,
        pub b_inherit_handle: BOOL,
    }

    #[repr(C)]
    pub struct STARTUPINFOW {
        pub cb: DWORD,
        pub lp_reserved: *mut u16,
        pub lp_desktop: *mut u16,
        pub lp_title: *mut u16,
        pub dw_x: DWORD,
        pub dw_y: DWORD,
        pub dw_x_size: DWORD,
        pub dw_y_size: DWORD,
        pub dw_x_count_chars: DWORD,
        pub dw_y_count_chars: DWORD,
        pub dw_fill_attribute: DWORD,
        pub dw_flags: DWORD,
        pub w_show_window: WORD,
        pub cb_reserved2: WORD,
        pub lp_reserved2: *mut u8,
        pub h_std_input: HANDLE,
        pub h_std_output: HANDLE,
        pub h_std_error: HANDLE,
    }

    #[repr(C)]
    pub struct STARTUPINFOEXW {
        pub startup_info: STARTUPINFOW,
        pub lp_attribute_list: *mut c_void,
    }

    #[repr(C)]
    pub struct PROCESS_INFORMATION {
        pub h_process: HANDLE,
        pub h_thread: HANDLE,
        pub dw_process_id: DWORD,
        pub dw_thread_id: DWORD,
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn CreatePseudoConsole(
            size: COORD,
            h_input: HANDLE,
            h_output: HANDLE,
            dw_flags: DWORD,
            ph_pc: *mut HPCON,
        ) -> HRESULT;

        fn ResizePseudoConsole(h_pc: HPCON, size: COORD) -> HRESULT;

        fn ClosePseudoConsole(h_pc: HPCON);

        fn CreatePipe(
            h_read_pipe: *mut HANDLE,
            h_write_pipe: *mut HANDLE,
            lp_pipe_attributes: *mut SECURITY_ATTRIBUTES,
            n_size: DWORD,
        ) -> BOOL;

        fn ReadFile(
            h_file: HANDLE,
            lp_buffer: *mut u8,
            n_number_of_bytes_to_read: DWORD,
            lp_number_of_bytes_read: *mut DWORD,
            lp_overlapped: *mut c_void,
        ) -> BOOL;

        fn WriteFile(
            h_file: HANDLE,
            lp_buffer: *const u8,
            n_number_of_bytes_to_write: DWORD,
            lp_number_of_bytes_written: *mut DWORD,
            lp_overlapped: *mut c_void,
        ) -> BOOL;

        fn InitializeProcThreadAttributeList(
            lp_attribute_list: *mut c_void,
            dw_attribute_count: DWORD,
            dw_flags: DWORD,
            lp_size: *mut usize,
        ) -> BOOL;

        fn UpdateProcThreadAttribute(
            lp_attribute_list: *mut c_void,
            dw_flags: DWORD,
            attribute: usize,
            lp_value: *const c_void,
            cb_size: usize,
            lp_previous_value: *mut c_void,
            lp_return_size: *mut usize,
        ) -> BOOL;

        fn DeleteProcThreadAttributeList(lp_attribute_list: *mut c_void);

        fn CreateProcessW(
            lp_application_name: *const u16,
            lp_command_line: *mut u16,
            lp_process_attributes: *mut SECURITY_ATTRIBUTES,
            lp_thread_attributes: *mut SECURITY_ATTRIBUTES,
            b_inherit_handles: BOOL,
            dw_creation_flags: DWORD,
            lp_environment: *mut c_void,
            lp_current_directory: *const u16,
            lp_startup_info: *mut STARTUPINFOEXW,
            lp_process_information: *mut PROCESS_INFORMATION,
        ) -> BOOL;

        fn CloseHandle(h_object: HANDLE) -> BOOL;
    }

    pub struct ConPtySession {
        pub id: u32,
        pub pid: u32,
        hpc: HPCON,
        in_write: HANDLE,
        h_process: HANDLE,
        _job_object: Option<ScopedJobObject>,
        output_tx: broadcast::Sender<Vec<u8>>,
        recent_output: Arc<parking_lot::Mutex<Vec<u8>>>,
        closed: Arc<AtomicBool>,
    }

    unsafe impl Send for ConPtySession {}
    unsafe impl Sync for ConPtySession {}

    impl ConPtySession {
        pub fn spawn(
            id: u32,
            command: &str,
            cwd: Option<&std::path::Path>,
            cols: u16,
            rows: u16,
        ) -> io::Result<Self> {
            info!(
                "Spawning ConPTY session {} for command: '{}' (cols: {}, rows: {})",
                id, command, cols, rows
            );

            // 1. Create Pipes with inheritance enabled for child redirection
            let mut in_read: HANDLE = ptr::null_mut();
            let mut in_write: HANDLE = ptr::null_mut();
            let mut out_read: HANDLE = ptr::null_mut();
            let mut out_write: HANDLE = ptr::null_mut();

            let mut sa = SECURITY_ATTRIBUTES {
                n_length: std::mem::size_of::<SECURITY_ATTRIBUTES>() as DWORD,
                lp_security_descriptor: ptr::null_mut(),
                b_inherit_handle: 1, // TRUE so child can inherit standard handles
            };

            unsafe {
                if CreatePipe(&mut in_read, &mut in_write, &mut sa, 0) == 0 {
                    return Err(io::Error::last_os_error());
                }
                if CreatePipe(&mut out_read, &mut out_write, &mut sa, 0) == 0 {
                    CloseHandle(in_read);
                    CloseHandle(in_write);
                    return Err(io::Error::last_os_error());
                }
            }

            // 2. Create PseudoConsole
            let size = COORD {
                x: cols as i16,
                y: rows as i16,
            };
            let mut hpc: HPCON = ptr::null_mut();

            let hr = unsafe { CreatePseudoConsole(size, in_read, out_write, 0, &mut hpc) };
            if hr != S_OK {
                unsafe {
                    CloseHandle(in_read);
                    CloseHandle(in_write);
                    CloseHandle(out_read);
                    CloseHandle(out_write);
                }
                return Err(io::Error::other(format!(
                    "CreatePseudoConsole failed with HRESULT 0x{:08X}",
                    hr
                )));
            }

            // 3. Prepare STARTUPINFOEXW with AttributeList
            let mut attr_size: usize = 0;
            unsafe {
                let _ = InitializeProcThreadAttributeList(ptr::null_mut(), 1, 0, &mut attr_size);
            }

            let layout = std::alloc::Layout::from_size_align(attr_size, 16)
                .map_err(|e| io::Error::other(e.to_string()))?;
            let attr_list = unsafe { std::alloc::alloc(layout) as *mut c_void };

            unsafe {
                if InitializeProcThreadAttributeList(attr_list, 1, 0, &mut attr_size) == 0 {
                    std::alloc::dealloc(attr_list as *mut u8, layout);
                    ClosePseudoConsole(hpc);
                    CloseHandle(in_read);
                    CloseHandle(in_write);
                    CloseHandle(out_read);
                    CloseHandle(out_write);
                    return Err(io::Error::last_os_error());
                }

                if UpdateProcThreadAttribute(
                    attr_list,
                    0,
                    PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE,
                    hpc,
                    std::mem::size_of::<HPCON>(),
                    ptr::null_mut(),
                    ptr::null_mut(),
                ) == 0
                {
                    DeleteProcThreadAttributeList(attr_list);
                    std::alloc::dealloc(attr_list as *mut u8, layout);
                    ClosePseudoConsole(hpc);
                    CloseHandle(in_read);
                    CloseHandle(in_write);
                    CloseHandle(out_read);
                    CloseHandle(out_write);
                    return Err(io::Error::last_os_error());
                }
            }

            const STARTF_USESTDHANDLES: DWORD = 0x00000100;

            let mut siex: STARTUPINFOEXW = unsafe { std::mem::zeroed() };
            siex.startup_info.cb = std::mem::size_of::<STARTUPINFOEXW>() as DWORD;
            siex.lp_attribute_list = attr_list;
            siex.startup_info.dw_flags = STARTF_USESTDHANDLES;
            siex.startup_info.h_std_input = ptr::null_mut();
            siex.startup_info.h_std_output = out_write;
            siex.startup_info.h_std_error = out_write;

            let mut cmd_utf16: Vec<u16> =
                command.encode_utf16().chain(std::iter::once(0)).collect();
            let cwd_utf16: Option<Vec<u16>> = cwd.map(|p| {
                p.as_os_str()
                    .encode_wide()
                    .chain(std::iter::once(0))
                    .collect()
            });
            let cwd_ptr = match &cwd_utf16 {
                Some(v) => v.as_ptr(),
                None => ptr::null(),
            };

            let mut pi: PROCESS_INFORMATION = unsafe { std::mem::zeroed() };
            let flags = EXTENDED_STARTUPINFO_PRESENT;

            let success = unsafe {
                CreateProcessW(
                    ptr::null(),
                    cmd_utf16.as_mut_ptr(),
                    ptr::null_mut(),
                    ptr::null_mut(),
                    1, // TRUE: inherit handles for standard streams
                    flags,
                    ptr::null_mut(),
                    cwd_ptr,
                    &mut siex,
                    &mut pi,
                )
            };

            unsafe {
                DeleteProcThreadAttributeList(attr_list);
                std::alloc::dealloc(attr_list as *mut u8, layout);
                // Close parent handles now that child has inherited them
                CloseHandle(in_read);
                CloseHandle(out_write);
            }

            if success == 0 {
                let err = io::Error::last_os_error();
                unsafe {
                    ClosePseudoConsole(hpc);
                    CloseHandle(in_write);
                    CloseHandle(out_read);
                }
                return Err(err);
            }

            // 4. Attach to Windows Job Object
            let job_object = ScopedJobObject::create();
            if let Some(ref job) = job_object {
                let attached = unsafe { job.assign_process_handle(pi.h_process) };
                debug!(
                    "ConPTY process {} assigned to ScopedJobObject: {}",
                    pi.dw_process_id, attached
                );
            }

            // 5. Close Thread Handle
            unsafe {
                CloseHandle(pi.h_thread);
            }

            let (output_tx, _) = broadcast::channel(1024);
            let closed = Arc::new(AtomicBool::new(false));
            let recent_output = Arc::new(parking_lot::Mutex::new(Vec::new()));

            // 6. Spawn Background Reader Thread
            let tx_clone = output_tx.clone();
            let closed_clone = closed.clone();
            let recent_clone = recent_output.clone();
            let out_read_raw = out_read as usize;
            std::thread::spawn(move || {
                let out_read = out_read_raw as HANDLE;
                let mut buf = vec![0u8; 8192];
                loop {
                    if closed_clone.load(Ordering::SeqCst) {
                        break;
                    }

                    let mut bytes_read: DWORD = 0;
                    let ok = unsafe {
                        ReadFile(
                            out_read,
                            buf.as_mut_ptr(),
                            buf.len() as DWORD,
                            &mut bytes_read,
                            ptr::null_mut(),
                        )
                    };

                    if ok == 0 || bytes_read == 0 {
                        debug!("ConPTY out_read EOF or pipe closed");
                        break;
                    }

                    let chunk = buf[..bytes_read as usize].to_vec();
                    {
                        let mut r = recent_clone.lock();
                        r.extend_from_slice(&chunk);
                        if r.len() > 65536 {
                            let drain_len = r.len() - 65536;
                            r.drain(..drain_len);
                        }
                    }
                    let _ = tx_clone.send(chunk);
                }

                unsafe {
                    CloseHandle(out_read);
                }
                closed_clone.store(true, Ordering::SeqCst);
            });

            Ok(Self {
                id,
                pid: pi.dw_process_id,
                hpc,
                in_write,
                h_process: pi.h_process,
                _job_object: job_object,
                output_tx,
                recent_output,
                closed,
            })
        }

        pub fn write_input(&self, data: &[u8]) -> io::Result<usize> {
            if self.closed.load(Ordering::SeqCst) {
                return Err(io::Error::new(
                    io::ErrorKind::BrokenPipe,
                    "ConPTY is closed",
                ));
            }

            let mut written: DWORD = 0;
            let ok = unsafe {
                WriteFile(
                    self.in_write,
                    data.as_ptr(),
                    data.len() as DWORD,
                    &mut written,
                    ptr::null_mut(),
                )
            };

            if ok == 0 {
                Err(io::Error::last_os_error())
            } else {
                Ok(written as usize)
            }
        }

        pub fn resize(&self, cols: u16, rows: u16) -> io::Result<()> {
            if self.closed.load(Ordering::SeqCst) {
                return Ok(());
            }

            let size = COORD {
                x: cols as i16,
                y: rows as i16,
            };

            let hr = unsafe { ResizePseudoConsole(self.hpc, size) };
            if hr == S_OK {
                debug!("Resized ConPTY session {} to {}x{}", self.id, cols, rows);
                Ok(())
            } else {
                Err(io::Error::other(format!(
                    "ResizePseudoConsole failed with 0x{:08X}",
                    hr
                )))
            }
        }

        pub fn subscribe_output(&self) -> broadcast::Receiver<Vec<u8>> {
            self.output_tx.subscribe()
        }

        pub fn get_recent_output(&self) -> Vec<u8> {
            self.recent_output.lock().clone()
        }

        pub fn is_alive(&self) -> bool {
            !self.closed.load(Ordering::SeqCst)
        }

        pub fn close(&mut self) {
            if !self.closed.swap(true, Ordering::SeqCst) {
                unsafe {
                    if !self.hpc.is_null() {
                        ClosePseudoConsole(self.hpc);
                        self.hpc = ptr::null_mut();
                    }
                    if !self.in_write.is_null() {
                        CloseHandle(self.in_write);
                        self.in_write = ptr::null_mut();
                    }
                    if !self.h_process.is_null() {
                        CloseHandle(self.h_process);
                        self.h_process = ptr::null_mut();
                    }
                }
                info!("ConPTY session {} closed cleanly", self.id);
            }
        }
    }

    impl Drop for ConPtySession {
        fn drop(&mut self) {
            self.close();
        }
    }

    use std::os::windows::ffi::OsStrExt;

    #[test]
    fn test_struct_sizes() {
        assert_eq!(std::mem::size_of::<STARTUPINFOW>(), 104);
        assert_eq!(std::mem::size_of::<STARTUPINFOEXW>(), 112);
    }
}
