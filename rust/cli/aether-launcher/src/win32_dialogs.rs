#[cfg(windows)]
pub mod native {
    use std::path::{Path, PathBuf};
    use windows::core::{Result, HSTRING, PWSTR};
    use windows::Win32::Foundation::HWND;
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CoTaskMemFree, CoUninitialize, IBindCtx,
        CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED,
    };
    use windows::Win32::UI::Shell::Common::COMDLG_FILTERSPEC;
    use windows::Win32::UI::Shell::{
        FileOpenDialog, FileSaveDialog, IFileOpenDialog, IFileSaveDialog, IShellItem,
        SHCreateItemFromParsingName, FOS_FILEMUSTEXIST, FOS_FORCEFILESYSTEM, FOS_NOCHANGEDIR,
        FOS_PATHMUSTEXIST, FOS_PICKFOLDERS, SIGDN_FILESYSPATH,
    };

    /// Win32 COM Single-Threaded Apartment (STA) guard for Common File Dialogs.
    /// FileOpenDialog and FileSaveDialog require an STA apartment on the calling thread.
    struct ComApartmentGuard(bool);

    impl ComApartmentGuard {
        fn enter() -> Self {
            unsafe {
                let hr = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
                // S_OK (0) means this guard initialized COM on this thread and should uninit on drop.
                // S_FALSE (1) or RPC_E_CHANGED_MODE means COM was already initialized on this thread.
                ComApartmentGuard(hr.0 == 0)
            }
        }
    }

    impl Drop for ComApartmentGuard {
        fn drop(&mut self) {
            if self.0 {
                unsafe {
                    CoUninitialize();
                }
            }
        }
    }

    /// Checks if a HRESULT error corresponds to user cancellation (HRESULT_FROM_WIN32(ERROR_CANCELLED) = 0x800704C7).
    fn is_cancelled_error(err: &windows::core::Error) -> bool {
        (err.code().0 as u32) == 0x800704C7
    }

    /// Opens a native Windows Open Folder common dialog.
    /// Returns the selected folder's canonical PathBuf, or Ok(None) if cancelled by user.
    /// Returns Err(e) if a real COM or OS failure occurs.
    pub fn pick_folder(
        parent_hwnd: HWND,
        title: Option<&str>,
        default_dir: Option<&Path>,
    ) -> Result<Option<PathBuf>> {
        let _com_guard = ComApartmentGuard::enter();
        unsafe {
            let dialog: IFileOpenDialog =
                CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER)?;

            let mut options = dialog.GetOptions()?;
            options |= FOS_PICKFOLDERS | FOS_PATHMUSTEXIST | FOS_FORCEFILESYSTEM | FOS_NOCHANGEDIR;
            dialog.SetOptions(options)?;

            if let Some(t) = title {
                let ht = HSTRING::from(t);
                let _ = dialog.SetTitle(&ht);
            }

            if let Some(dir) = default_dir {
                if dir.exists() {
                    let dir_str = dir.to_string_lossy();
                    let hdir = HSTRING::from(&*dir_str);
                    if let Ok(item) =
                        SHCreateItemFromParsingName::<_, _, IShellItem>(&hdir, None::<&IBindCtx>)
                    {
                        let _ = dialog.SetFolder(&item);
                    }
                }
            }

            if let Err(e) = dialog.Show(parent_hwnd) {
                if is_cancelled_error(&e) {
                    return Ok(None);
                }
                return Err(e);
            }

            let item = dialog.GetResult()?;
            let display_name: PWSTR = item.GetDisplayName(SIGDN_FILESYSPATH)?;
            let path_str = display_name.to_string()?;
            CoTaskMemFree(Some(display_name.0 as *const _));

            let pb = PathBuf::from(path_str);
            let canonical = pb.canonicalize().unwrap_or(pb);
            Ok(Some(canonical))
        }
    }

    /// Opens a native Windows Open File common dialog.
    /// Returns the selected file's canonical PathBuf, or Ok(None) if cancelled by user.
    /// Returns Err(e) if a real COM or OS failure occurs.
    pub fn pick_file(
        parent_hwnd: HWND,
        title: Option<&str>,
        default_dir: Option<&Path>,
        filter_name: Option<&str>,
        filter_spec: Option<&str>,
    ) -> Result<Option<PathBuf>> {
        let _com_guard = ComApartmentGuard::enter();
        unsafe {
            let dialog: IFileOpenDialog =
                CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER)?;

            let mut options = dialog.GetOptions()?;
            options |=
                FOS_FILEMUSTEXIST | FOS_PATHMUSTEXIST | FOS_FORCEFILESYSTEM | FOS_NOCHANGEDIR;
            dialog.SetOptions(options)?;

            if let Some(t) = title {
                let ht = HSTRING::from(t);
                let _ = dialog.SetTitle(&ht);
            }

            if let (Some(name), Some(spec)) = (filter_name, filter_spec) {
                let hname = HSTRING::from(name);
                let hspec = HSTRING::from(spec);
                let filters = [COMDLG_FILTERSPEC {
                    pszName: windows::core::PCWSTR(hname.as_ptr()),
                    pszSpec: windows::core::PCWSTR(hspec.as_ptr()),
                }];
                let _ = dialog.SetFileTypes(&filters);
            }

            if let Some(dir) = default_dir {
                if dir.exists() {
                    let dir_str = dir.to_string_lossy();
                    let hdir = HSTRING::from(&*dir_str);
                    if let Ok(item) =
                        SHCreateItemFromParsingName::<_, _, IShellItem>(&hdir, None::<&IBindCtx>)
                    {
                        let _ = dialog.SetFolder(&item);
                    }
                }
            }

            if let Err(e) = dialog.Show(parent_hwnd) {
                if is_cancelled_error(&e) {
                    return Ok(None);
                }
                return Err(e);
            }

            let item = dialog.GetResult()?;
            let display_name: PWSTR = item.GetDisplayName(SIGDN_FILESYSPATH)?;
            let path_str = display_name.to_string()?;
            CoTaskMemFree(Some(display_name.0 as *const _));

            let pb = PathBuf::from(path_str);
            let canonical = pb.canonicalize().unwrap_or(pb);
            Ok(Some(canonical))
        }
    }

    /// Opens a native Windows Save File common dialog.
    /// Returns the selected destination PathBuf, or Ok(None) if cancelled by user.
    /// Returns Err(e) if a real COM or OS failure occurs.
    pub fn save_file(
        parent_hwnd: HWND,
        title: Option<&str>,
        default_dir: Option<&Path>,
        default_name: Option<&str>,
    ) -> Result<Option<PathBuf>> {
        let _com_guard = ComApartmentGuard::enter();
        unsafe {
            let dialog: IFileSaveDialog =
                CoCreateInstance(&FileSaveDialog, None, CLSCTX_INPROC_SERVER)?;

            let mut options = dialog.GetOptions()?;
            options |= FOS_PATHMUSTEXIST | FOS_FORCEFILESYSTEM | FOS_NOCHANGEDIR;
            dialog.SetOptions(options)?;

            if let Some(t) = title {
                let ht = HSTRING::from(t);
                let _ = dialog.SetTitle(&ht);
            }

            if let Some(name) = default_name {
                let hname = HSTRING::from(name);
                let _ = dialog.SetFileName(&hname);
            }

            if let Some(dir) = default_dir {
                if dir.exists() {
                    let dir_str = dir.to_string_lossy();
                    let hdir = HSTRING::from(&*dir_str);
                    if let Ok(item) =
                        SHCreateItemFromParsingName::<_, _, IShellItem>(&hdir, None::<&IBindCtx>)
                    {
                        let _ = dialog.SetFolder(&item);
                    }
                }
            }

            if let Err(e) = dialog.Show(parent_hwnd) {
                if is_cancelled_error(&e) {
                    return Ok(None);
                }
                return Err(e);
            }

            let item = dialog.GetResult()?;
            let display_name: PWSTR = item.GetDisplayName(SIGDN_FILESYSPATH)?;
            let path_str = display_name.to_string()?;
            CoTaskMemFree(Some(display_name.0 as *const _));

            Ok(Some(PathBuf::from(path_str)))
        }
    }
}
