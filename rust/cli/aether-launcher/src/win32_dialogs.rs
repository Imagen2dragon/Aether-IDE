#[cfg(windows)]
pub mod native {
    use std::path::{Path, PathBuf};
    use windows::core::{Result, HSTRING, PWSTR};
    use windows::Win32::Foundation::HWND;
    use windows::Win32::System::Com::{
        CoCreateInstance, CoTaskMemFree, IBindCtx, CLSCTX_INPROC_SERVER,
    };
    use windows::Win32::UI::Shell::Common::COMDLG_FILTERSPEC;
    use windows::Win32::UI::Shell::{
        FileOpenDialog, FileSaveDialog, IFileOpenDialog, IFileSaveDialog, IShellItem,
        SHCreateItemFromParsingName, FOS_FILEMUSTEXIST, FOS_FORCEFILESYSTEM, FOS_NOCHANGEDIR,
        FOS_PATHMUSTEXIST, FOS_PICKFOLDERS, SIGDN_FILESYSPATH,
    };

    /// Opens a native Windows Open Folder common dialog.
    /// Returns the selected folder's canonical PathBuf, or None if cancelled.
    pub fn pick_folder(
        parent_hwnd: HWND,
        title: Option<&str>,
        default_dir: Option<&Path>,
    ) -> Result<Option<PathBuf>> {
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

            if dialog.Show(parent_hwnd).is_err() {
                // User clicked Cancel or closed dialog
                return Ok(None);
            }

            let item = dialog.GetResult()?;
            let display_name: PWSTR = item.GetDisplayName(SIGDN_FILESYSPATH)?;
            let path_str = display_name.to_string()?;
            CoTaskMemFree(Some(display_name.0 as *const _));

            Ok(Some(PathBuf::from(path_str)))
        }
    }

    /// Opens a native Windows Open File common dialog.
    /// Returns the selected file's canonical PathBuf, or None if cancelled.
    pub fn pick_file(
        parent_hwnd: HWND,
        title: Option<&str>,
        default_dir: Option<&Path>,
        filter_name: Option<&str>,
        filter_spec: Option<&str>,
    ) -> Result<Option<PathBuf>> {
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

            if dialog.Show(parent_hwnd).is_err() {
                return Ok(None);
            }

            let item = dialog.GetResult()?;
            let display_name: PWSTR = item.GetDisplayName(SIGDN_FILESYSPATH)?;
            let path_str = display_name.to_string()?;
            CoTaskMemFree(Some(display_name.0 as *const _));

            Ok(Some(PathBuf::from(path_str)))
        }
    }

    /// Opens a native Windows Save File common dialog.
    /// Returns the selected destination PathBuf, or None if cancelled.
    pub fn save_file(
        parent_hwnd: HWND,
        title: Option<&str>,
        default_dir: Option<&Path>,
        default_name: Option<&str>,
    ) -> Result<Option<PathBuf>> {
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

            if dialog.Show(parent_hwnd).is_err() {
                return Ok(None);
            }

            let item = dialog.GetResult()?;
            let display_name: PWSTR = item.GetDisplayName(SIGDN_FILESYSPATH)?;
            let path_str = display_name.to_string()?;
            CoTaskMemFree(Some(display_name.0 as *const _));

            Ok(Some(PathBuf::from(path_str)))
        }
    }
}
