use application_core::{AetherError, Result};
use security_core::SecretRedactor;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc;

#[cfg(windows)]
#[allow(clippy::upper_case_acronyms)]
pub mod win32_job {
    pub type HANDLE = *mut std::ffi::c_void;
    pub type BOOL = i32;
    pub type DWORD = u32;

    const JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE: DWORD = 0x00002000;
    const JOB_OBJECT_EXTENDED_LIMIT_INFORMATION: DWORD = 9;

    #[repr(C)]
    struct IOCounters {
        read_operation_count: u64,
        write_operation_count: u64,
        other_operation_count: u64,
        read_transfer_count: u64,
        write_transfer_count: u64,
        other_transfer_count: u64,
    }

    #[repr(C)]
    struct JobObjectBasicLimitInformation {
        per_process_user_time_limit: i64,
        per_job_user_time_limit: i64,
        limit_flags: DWORD,
        minimum_working_set_size: usize,
        maximum_working_set_size: usize,
        active_process_limit: DWORD,
        affinity: usize,
        priority_class: DWORD,
        scheduling_class: DWORD,
    }

    #[repr(C)]
    struct JobObjectExtendedLimitInformation {
        basic_limit_information: JobObjectBasicLimitInformation,
        io_info: IOCounters,
        process_memory_limit: usize,
        job_memory_limit: usize,
        peak_process_memory_limit: usize,
        peak_job_memory_limit: usize,
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn CreateJobObjectW(
            lp_job_attributes: *mut std::ffi::c_void,
            lp_name: *const u16,
        ) -> HANDLE;
        fn SetInformationJobObject(
            h_job: HANDLE,
            job_object_info_class: DWORD,
            lp_job_object_info: *const std::ffi::c_void,
            cb_job_object_info_length: DWORD,
        ) -> BOOL;
        fn AssignProcessToJobObject(h_job: HANDLE, h_process: HANDLE) -> BOOL;
        fn TerminateJobObject(h_job: HANDLE, u_exit_code: u32) -> BOOL;
        fn CloseHandle(h_object: HANDLE) -> BOOL;
        fn OpenProcess(
            dw_desired_access: DWORD,
            b_inherit_handle: BOOL,
            dw_process_id: DWORD,
        ) -> HANDLE;
    }

    pub struct ScopedJobObject {
        handle: HANDLE,
    }

    unsafe impl Send for ScopedJobObject {}
    unsafe impl Sync for ScopedJobObject {}

    impl ScopedJobObject {
        pub fn create() -> Option<Self> {
            unsafe {
                let handle = CreateJobObjectW(std::ptr::null_mut(), std::ptr::null());
                if handle.is_null() {
                    return None;
                }

                let mut info: JobObjectExtendedLimitInformation = std::mem::zeroed();
                info.basic_limit_information.limit_flags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;

                let ok = SetInformationJobObject(
                    handle,
                    JOB_OBJECT_EXTENDED_LIMIT_INFORMATION,
                    &info as *const _ as *const std::ffi::c_void,
                    std::mem::size_of::<JobObjectExtendedLimitInformation>() as DWORD,
                );

                if ok == 0 {
                    CloseHandle(handle);
                    return None;
                }

                Some(Self { handle })
            }
        }

        pub fn assign_process_by_id(&self, pid: u32) -> bool {
            const PROCESS_SET_QUOTA: DWORD = 0x0100;
            const PROCESS_TERMINATE: DWORD = 0x0001;
            unsafe {
                let proc_handle = OpenProcess(PROCESS_SET_QUOTA | PROCESS_TERMINATE, 0, pid);
                if proc_handle.is_null() {
                    return false;
                }
                let ok = AssignProcessToJobObject(self.handle, proc_handle);
                CloseHandle(proc_handle);
                ok != 0
            }
        }

        /// Assigns an existing process handle to this job object.
        ///
        /// # Safety
        /// `proc_handle` must be a valid Win32 process handle with PROCESS_SET_QUOTA access rights.
        pub unsafe fn assign_process_handle(&self, proc_handle: HANDLE) -> bool {
            AssignProcessToJobObject(self.handle, proc_handle) != 0
        }

        pub fn terminate(&self, exit_code: u32) -> bool {
            unsafe {
                if !self.handle.is_null() {
                    TerminateJobObject(self.handle, exit_code) != 0
                } else {
                    false
                }
            }
        }
    }

    impl Drop for ScopedJobObject {
        fn drop(&mut self) {
            unsafe {
                if !self.handle.is_null() {
                    CloseHandle(self.handle);
                }
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct ProcessOutput {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
}

pub struct ProcessManager;

impl ProcessManager {
    pub async fn run_command_streaming(
        command_str: &str,
        cwd: Option<PathBuf>,
        timeout: Option<Duration>,
        stdout_tx: Option<mpsc::Sender<String>>,
        stderr_tx: Option<mpsc::Sender<String>>,
    ) -> Result<ProcessOutput> {
        #[cfg(target_os = "windows")]
        let mut cmd = {
            let mut c = Command::new("cmd.exe");
            c.args(["/C", command_str]);
            c
        };

        #[cfg(not(target_os = "windows"))]
        let mut cmd = {
            let mut c = Command::new("sh");
            c.args(["-c", command_str]);
            c
        };

        if let Some(dir) = cwd {
            cmd.current_dir(dir);
        }

        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        let mut child = cmd
            .spawn()
            .map_err(|e| AetherError::Process(e.to_string()))?;

        // Isolate child process inside Windows Job Object to prevent orphan processes
        #[cfg(windows)]
        let job = {
            if let Some(pid) = child.id() {
                if let Some(j) = win32_job::ScopedJobObject::create() {
                    j.assign_process_by_id(pid);
                    Some(j)
                } else {
                    None
                }
            } else {
                None
            }
        };

        let stdout_handle = child.stdout.take();
        let stderr_handle = child.stderr.take();

        let stdout_task = tokio::spawn(async move {
            let mut captured = String::new();
            if let Some(pipe) = stdout_handle {
                let reader = BufReader::new(pipe);
                let mut lines = reader.lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    let sanitized = SecretRedactor::redact(&line);
                    captured.push_str(&sanitized);
                    captured.push('\n');
                    if let Some(tx) = &stdout_tx {
                        let _ = tx.send(sanitized).await;
                    }
                }
            }
            captured
        });

        let stderr_task = tokio::spawn(async move {
            let mut captured = String::new();
            if let Some(pipe) = stderr_handle {
                let reader = BufReader::new(pipe);
                let mut lines = reader.lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    let sanitized = SecretRedactor::redact(&line);
                    captured.push_str(&sanitized);
                    captured.push('\n');
                    if let Some(tx) = &stderr_tx {
                        let _ = tx.send(sanitized).await;
                    }
                }
            }
            captured
        });

        let wait_fut = async {
            let status = child.wait().await?;
            let out = stdout_task.await.unwrap_or_default();
            let err = stderr_task.await.unwrap_or_default();
            Ok::<ProcessOutput, std::io::Error>(ProcessOutput {
                exit_code: status.code().unwrap_or(-1),
                stdout: out,
                stderr: err,
            })
        };

        let result = if let Some(limit) = timeout {
            match tokio::time::timeout(limit, wait_fut).await {
                Ok(res) => res.map_err(|e| AetherError::Process(e.to_string()))?,
                Err(_) => {
                    #[cfg(windows)]
                    {
                        if let Some(ref j) = job {
                            j.terminate(1);
                        }
                    }
                    let _ = child.kill().await;
                    return Err(AetherError::Process(format!(
                        "Process timed out after {:?}",
                        limit
                    )));
                }
            }
        } else {
            wait_fut
                .await
                .map_err(|e| AetherError::Process(e.to_string()))?
        };

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_run_command_echo() {
        let out = ProcessManager::run_command_streaming(
            "echo AetherIDE-Test",
            None,
            Some(Duration::from_secs(5)),
            None,
            None,
        )
        .await
        .expect("command execution failed");

        assert_eq!(out.exit_code, 0);
        assert!(out.stdout.contains("AetherIDE-Test"));
    }

    #[tokio::test]
    async fn test_run_command_timeout_clean_termination() {
        // Run a command that takes longer than 500ms timeout
        let res = ProcessManager::run_command_streaming(
            "powershell -Command \"Start-Sleep -Seconds 10\"",
            None,
            Some(Duration::from_millis(300)),
            None,
            None,
        )
        .await;

        assert!(res.is_err());
        let err_msg = res.unwrap_err().to_string();
        assert!(err_msg.contains("Process timed out"));
    }
}
