#[cfg(windows)]
pub use native::*;

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

    struct ComApartmentGuard(bool);

    impl ComApartmentGuard {
        fn enter() -> Self {
            unsafe {
                let hr = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
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

    fn is_cancelled_error(err: &windows::core::Error) -> bool {
        (err.code().0 as u32) == 0x800704C7
    }

    /// Opens a native Windows Open Folder common dialog.
    /// Returns the selected folder's canonical PathBuf, or None if cancelled.
    pub fn pick_folder(
        parent_hwnd: HWND,
        title: Option<&str>,
        default_dir: Option<&Path>,
    ) -> Result<Option<PathBuf>> {
        let _com_guard = ComApartmentGuard::enter();
        unsafe {
            let pfd: IFileOpenDialog =
                CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER)?;

            let mut options = pfd.GetOptions()?;
            options |= FOS_PICKFOLDERS | FOS_FORCEFILESYSTEM | FOS_PATHMUSTEXIST;
            pfd.SetOptions(options)?;

            if let Some(t) = title {
                let htitle = HSTRING::from(t);
                let _ = pfd.SetTitle(&htitle);
            }

            if let Some(dir) = default_dir {
                if dir.exists() {
                    let dir_str = dir.to_string_lossy();
                    let hdir = HSTRING::from(dir_str.as_ref());
                    if let Ok(item) =
                        SHCreateItemFromParsingName::<_, _, IShellItem>(&hdir, None::<&IBindCtx>)
                    {
                        let _ = pfd.SetFolder(&item);
                    }
                }
            }

            if let Err(e) = pfd.Show(parent_hwnd) {
                if is_cancelled_error(&e) {
                    return Ok(None);
                }
                return Err(e);
            }

            let result_item = pfd.GetResult()?;
            let psz_path: PWSTR = result_item.GetDisplayName(SIGDN_FILESYSPATH)?;
            let path_str = psz_path.to_string()?;
            CoTaskMemFree(Some(psz_path.0 as *const _));

            let pb = PathBuf::from(path_str);
            let canonical = pb.canonicalize().unwrap_or(pb);
            Ok(Some(canonical))
        }
    }

    /// Opens a native Windows Open File common dialog with configurable extension filters.
    pub fn pick_file(
        parent_hwnd: HWND,
        title: Option<&str>,
        default_dir: Option<&Path>,
        filter_name: Option<&str>,
        filter_spec: Option<&str>,
    ) -> Result<Option<PathBuf>> {
        let _com_guard = ComApartmentGuard::enter();
        unsafe {
            let pfd: IFileOpenDialog =
                CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER)?;

            let mut options = pfd.GetOptions()?;
            options |= FOS_FORCEFILESYSTEM | FOS_FILEMUSTEXIST | FOS_PATHMUSTEXIST;
            pfd.SetOptions(options)?;

            if let Some(t) = title {
                let htitle = HSTRING::from(t);
                let _ = pfd.SetTitle(&htitle);
            }

            if let (Some(name), Some(spec)) = (filter_name, filter_spec) {
                let hname = HSTRING::from(name);
                let hspec = HSTRING::from(spec);
                let filters = [COMDLG_FILTERSPEC {
                    pszName: windows::core::PCWSTR(hname.as_wide().as_ptr()),
                    pszSpec: windows::core::PCWSTR(hspec.as_wide().as_ptr()),
                }];
                let _ = pfd.SetFileTypes(&filters);
            }

            if let Some(dir) = default_dir {
                if dir.exists() {
                    let dir_str = dir.to_string_lossy();
                    let hdir = HSTRING::from(dir_str.as_ref());
                    if let Ok(item) =
                        SHCreateItemFromParsingName::<_, _, IShellItem>(&hdir, None::<&IBindCtx>)
                    {
                        let _ = pfd.SetFolder(&item);
                    }
                }
            }

            if let Err(e) = pfd.Show(parent_hwnd) {
                if is_cancelled_error(&e) {
                    return Ok(None);
                }
                return Err(e);
            }

            let result_item = pfd.GetResult()?;
            let psz_path: PWSTR = result_item.GetDisplayName(SIGDN_FILESYSPATH)?;
            let path_str = psz_path.to_string()?;
            CoTaskMemFree(Some(psz_path.0 as *const _));

            let pb = PathBuf::from(path_str);
            let canonical = pb.canonicalize().unwrap_or(pb);
            Ok(Some(canonical))
        }
    }

    /// Opens a native Windows Save File common dialog with overwrite prompt confirmation.
    pub fn save_file(
        parent_hwnd: HWND,
        title: Option<&str>,
        default_dir: Option<&Path>,
        default_name: Option<&str>,
    ) -> Result<Option<PathBuf>> {
        let _com_guard = ComApartmentGuard::enter();
        unsafe {
            let pfd: IFileSaveDialog =
                CoCreateInstance(&FileSaveDialog, None, CLSCTX_INPROC_SERVER)?;

            let mut options = pfd.GetOptions()?;
            options |= FOS_FORCEFILESYSTEM | FOS_PATHMUSTEXIST | FOS_NOCHANGEDIR;
            pfd.SetOptions(options)?;

            if let Some(t) = title {
                let htitle = HSTRING::from(t);
                let _ = pfd.SetTitle(&htitle);
            }

            if let Some(name) = default_name {
                let hname = HSTRING::from(name);
                let _ = pfd.SetFileName(&hname);
            }

            if let Some(dir) = default_dir {
                if dir.exists() {
                    let dir_str = dir.to_string_lossy();
                    let hdir = HSTRING::from(dir_str.as_ref());
                    if let Ok(item) =
                        SHCreateItemFromParsingName::<_, _, IShellItem>(&hdir, None::<&IBindCtx>)
                    {
                        let _ = pfd.SetFolder(&item);
                    }
                }
            }

            if let Err(e) = pfd.Show(parent_hwnd) {
                if is_cancelled_error(&e) {
                    return Ok(None);
                }
                return Err(e);
            }

            let result_item = pfd.GetResult()?;
            let psz_path: PWSTR = result_item.GetDisplayName(SIGDN_FILESYSPATH)?;
            let path_str = psz_path.to_string()?;
            CoTaskMemFree(Some(psz_path.0 as *const _));

            Ok(Some(PathBuf::from(path_str)))
        }
    }
}
