use application_core::{AetherError, Result};
use cap_std::ambient_authority;
pub use cap_std::fs::Metadata;
use cap_std::fs::{Dir, OpenOptions};
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use tracing::warn;

/// Positively identifies an operating system cross-device link error (EXDEV on Unix, ERROR_NOT_SAME_DEVICE on Windows).
/// Strictly excludes permission denied, sharing violations, not found, or generic I/O errors.
pub fn is_cross_device_error(err: &std::io::Error) -> bool {
    if err.kind() == std::io::ErrorKind::CrossesDevices {
        return true;
    }
    #[cfg(unix)]
    {
        if let Some(code) = err.raw_os_error() {
            if code == 18 {
                return true;
            }
        }
    }
    #[cfg(windows)]
    {
        if let Some(code) = err.raw_os_error() {
            if code == 17 {
                return true;
            }
        }
    }
    false
}

#[derive(Debug)]
pub enum AtomicPublishError {
    DestinationExists,
    CrossDevice,
    Unsupported,
    Io(std::io::Error),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct FileObjectIdentity {
    pub volume_serial_number: u64,
    pub file_index_high: u64,
    pub file_index_low: u64,
}

pub type WindowsPreExclusiveOpenHook = std::sync::Arc<dyn Fn(&std::path::Path) + Send + Sync>;
pub type WindowsPostExclusiveOpenHook =
    std::sync::Arc<dyn Fn(&std::path::Path, &std::path::Path) + Send + Sync>;

#[cfg(windows)]
thread_local! {
    static FAULT_INJECT_WINDOWS_REPARSE_POINT: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static FAULT_INJECT_WINDOWS_IDENTITY_MISMATCH: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static FAULT_INJECT_WINDOWS_DEST_IDENTITY_MISMATCH: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static FAULT_INJECT_WINDOWS_DEST_IDENTITY_QUERY_FAILURE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static FAULT_INJECT_WINDOWS_SECURITY_DESCRIPTOR_FAILURE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static FAULT_INJECT_WINDOWS_DIFFERENT_OWNER: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static FAULT_INJECT_WINDOWS_SIMULATED_OWNER_SID: std::cell::Cell<Option<u32>> = const { std::cell::Cell::new(None) };
    static FAULT_INJECT_WINDOWS_SIMULATED_ADMIN_TOKEN: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static HOOK_WINDOWS_PRE_EXCLUSIVE_OPEN: std::cell::RefCell<Option<WindowsPreExclusiveOpenHook>> = const { std::cell::RefCell::new(None) };
    static HOOK_WINDOWS_POST_EXCLUSIVE_OPEN: std::cell::RefCell<Option<WindowsPostExclusiveOpenHook>> = const { std::cell::RefCell::new(None) };
}

#[cfg(windows)]
pub fn set_hook_windows_pre_exclusive_open(hook: Option<WindowsPreExclusiveOpenHook>) {
    HOOK_WINDOWS_PRE_EXCLUSIVE_OPEN.with(|cell| {
        *cell.borrow_mut() = hook;
    });
}

#[cfg(windows)]
pub fn get_hook_windows_pre_exclusive_open() -> Option<WindowsPreExclusiveOpenHook> {
    HOOK_WINDOWS_PRE_EXCLUSIVE_OPEN.with(|cell| cell.borrow().clone())
}

#[cfg(windows)]
pub fn set_hook_windows_post_exclusive_open(hook: Option<WindowsPostExclusiveOpenHook>) {
    HOOK_WINDOWS_POST_EXCLUSIVE_OPEN.with(|cell| {
        *cell.borrow_mut() = hook;
    });
}

#[cfg(windows)]
pub fn get_hook_windows_post_exclusive_open() -> Option<WindowsPostExclusiveOpenHook> {
    HOOK_WINDOWS_POST_EXCLUSIVE_OPEN.with(|cell| cell.borrow().clone())
}

#[cfg(windows)]
pub fn set_fault_inject_windows_dest_identity_query_failure(inject: bool) {
    FAULT_INJECT_WINDOWS_DEST_IDENTITY_QUERY_FAILURE.with(|cell| cell.set(inject));
}

#[cfg(windows)]
pub fn is_fault_inject_windows_dest_identity_query_failure() -> bool {
    FAULT_INJECT_WINDOWS_DEST_IDENTITY_QUERY_FAILURE.with(|cell| cell.get())
}

#[cfg(windows)]
pub fn set_fault_inject_windows_reparse_point(inject: bool) {
    FAULT_INJECT_WINDOWS_REPARSE_POINT.with(|cell| cell.set(inject));
}

#[cfg(windows)]
pub fn is_fault_inject_windows_reparse_point() -> bool {
    FAULT_INJECT_WINDOWS_REPARSE_POINT.with(|cell| cell.get())
}

#[cfg(windows)]
pub fn set_fault_inject_windows_identity_mismatch(inject: bool) {
    FAULT_INJECT_WINDOWS_IDENTITY_MISMATCH.with(|cell| cell.set(inject));
}

#[cfg(windows)]
pub fn is_fault_inject_windows_identity_mismatch() -> bool {
    FAULT_INJECT_WINDOWS_IDENTITY_MISMATCH.with(|cell| cell.get())
}

#[cfg(windows)]
pub fn set_fault_inject_windows_dest_identity_mismatch(inject: bool) {
    FAULT_INJECT_WINDOWS_DEST_IDENTITY_MISMATCH.with(|cell| cell.set(inject));
}

#[cfg(windows)]
pub fn is_fault_inject_windows_dest_identity_mismatch() -> bool {
    FAULT_INJECT_WINDOWS_DEST_IDENTITY_MISMATCH.with(|cell| cell.get())
}

#[cfg(windows)]
pub fn set_fault_inject_windows_security_descriptor_failure(inject: bool) {
    FAULT_INJECT_WINDOWS_SECURITY_DESCRIPTOR_FAILURE.with(|cell| cell.set(inject));
}

#[cfg(windows)]
pub fn is_fault_inject_windows_security_descriptor_failure() -> bool {
    FAULT_INJECT_WINDOWS_SECURITY_DESCRIPTOR_FAILURE.with(|cell| cell.get())
}

#[cfg(windows)]
pub fn set_fault_inject_windows_different_owner(inject: bool) {
    FAULT_INJECT_WINDOWS_DIFFERENT_OWNER.with(|cell| cell.set(inject));
}

#[cfg(windows)]
pub fn is_fault_inject_windows_different_owner() -> bool {
    FAULT_INJECT_WINDOWS_DIFFERENT_OWNER.with(|cell| cell.get())
}

#[cfg(windows)]
pub fn set_fault_inject_windows_simulated_owner_sid(sid_type: Option<u32>) {
    FAULT_INJECT_WINDOWS_SIMULATED_OWNER_SID.with(|cell| cell.set(sid_type));
}

#[cfg(windows)]
pub fn set_fault_inject_windows_simulated_admin_token(is_admin: bool) {
    FAULT_INJECT_WINDOWS_SIMULATED_ADMIN_TOKEN.with(|cell| cell.set(is_admin));
}

#[cfg(not(windows))]
pub fn set_fault_inject_windows_simulated_owner_sid(_sid_type: Option<u32>) {}

#[cfg(not(windows))]
pub fn set_fault_inject_windows_simulated_admin_token(_is_admin: bool) {}

#[cfg(not(windows))]
pub fn set_hook_windows_pre_exclusive_open(_hook: Option<WindowsPreExclusiveOpenHook>) {}

#[cfg(not(windows))]
pub fn get_hook_windows_pre_exclusive_open() -> Option<WindowsPreExclusiveOpenHook> {
    None
}

#[cfg(not(windows))]
pub fn set_hook_windows_post_exclusive_open(_hook: Option<WindowsPostExclusiveOpenHook>) {}

#[cfg(not(windows))]
pub fn get_hook_windows_post_exclusive_open() -> Option<WindowsPostExclusiveOpenHook> {
    None
}

#[cfg(not(windows))]
pub fn set_fault_inject_windows_dest_identity_query_failure(_inject: bool) {}

#[cfg(not(windows))]
pub fn is_fault_inject_windows_dest_identity_query_failure() -> bool {
    false
}

#[cfg(not(windows))]
pub fn set_fault_inject_windows_reparse_point(_inject: bool) {}

#[cfg(not(windows))]
pub fn is_fault_inject_windows_reparse_point() -> bool {
    false
}

#[cfg(not(windows))]
pub fn set_fault_inject_windows_identity_mismatch(_inject: bool) {}

#[cfg(not(windows))]
pub fn is_fault_inject_windows_identity_mismatch() -> bool {
    false
}

#[cfg(not(windows))]
pub fn set_fault_inject_windows_dest_identity_mismatch(_inject: bool) {}

#[cfg(not(windows))]
pub fn is_fault_inject_windows_dest_identity_mismatch() -> bool {
    false
}

#[cfg(not(windows))]
pub fn set_fault_inject_windows_security_descriptor_failure(_inject: bool) {}

#[cfg(not(windows))]
pub fn is_fault_inject_windows_security_descriptor_failure() -> bool {
    false
}

#[cfg(not(windows))]
pub fn set_fault_inject_windows_different_owner(_inject: bool) {}

#[cfg(not(windows))]
pub fn is_fault_inject_windows_different_owner() -> bool {
    false
}

#[cfg(windows)]
pub fn query_file_object_identity(path: &Path) -> Result<FileObjectIdentity> {
    use std::os::windows::fs::OpenOptionsExt;
    use std::os::windows::io::AsRawHandle;
    const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x00200000;
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x00000400;
    const FILE_ATTRIBUTE_DIRECTORY: u32 = 0x00000010;
    const FILE_TYPE_DISK: u32 = 0x0001;

    let file = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)
        .map_err(AetherError::Io)?;

    #[repr(C)]
    #[allow(non_snake_case)]
    struct BY_HANDLE_FILE_INFORMATION {
        dwFileAttributes: u32,
        ftCreationTime: [u32; 2],
        ftLastAccessTime: [u32; 2],
        ftLastWriteTime: [u32; 2],
        dwVolumeSerialNumber: u32,
        nFileSizeHigh: u32,
        nFileSizeLow: u32,
        nNumberOfLinks: u32,
        nFileIndexHigh: u32,
        nFileIndexLow: u32,
    }
    extern "system" {
        fn GetFileType(hFile: *mut std::ffi::c_void) -> u32;
        fn GetFileInformationByHandle(
            hFile: *mut std::ffi::c_void,
            lpFileInformation: *mut BY_HANDLE_FILE_INFORMATION,
        ) -> i32;
    }

    let file_type = unsafe { GetFileType(file.as_raw_handle()) };
    if file_type != FILE_TYPE_DISK {
        return Err(AetherError::PermissionDenied(format!(
            "Object '{}' is not a standard disk file (file type: 0x{:x})",
            path.display(),
            file_type
        )));
    }

    let mut by_handle_info = std::mem::MaybeUninit::<BY_HANDLE_FILE_INFORMATION>::zeroed();
    let info_res =
        unsafe { GetFileInformationByHandle(file.as_raw_handle(), by_handle_info.as_mut_ptr()) };
    if info_res == 0 {
        return Err(AetherError::Io(std::io::Error::last_os_error()));
    }
    let file_info = unsafe { by_handle_info.assume_init() };

    let is_reparse = (file_info.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT) != 0;
    let is_dir = (file_info.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY) != 0;
    if is_reparse || is_dir {
        return Err(AetherError::PermissionDenied(format!(
            "Object '{}' is a reparse point or directory (attributes: 0x{:x}); refusing identity query",
            path.display(),
            file_info.dwFileAttributes
        )));
    }

    Ok(FileObjectIdentity {
        volume_serial_number: file_info.dwVolumeSerialNumber as u64,
        file_index_high: file_info.nFileIndexHigh as u64,
        file_index_low: file_info.nFileIndexLow as u64,
    })
}

#[cfg(unix)]
pub fn query_file_object_identity(path: &Path) -> Result<FileObjectIdentity> {
    use std::os::unix::fs::MetadataExt;
    let meta = std::fs::symlink_metadata(path).map_err(AetherError::Io)?;
    if meta.file_type().is_symlink() {
        return Err(AetherError::PermissionDenied(format!(
            "Object '{}' is a symlink; refusing identity query",
            path.display()
        )));
    }
    if meta.is_dir() {
        return Err(AetherError::PermissionDenied(format!(
            "Object '{}' is a directory; refusing identity query",
            path.display()
        )));
    }
    let ino = meta.ino();
    Ok(FileObjectIdentity {
        volume_serial_number: meta.dev(),
        file_index_high: (ino >> 32) as u64,
        file_index_low: (ino & 0xFFFFFFFF) as u64,
    })
}

#[cfg(not(any(windows, unix)))]
pub fn query_file_object_identity(_path: &Path) -> Result<FileObjectIdentity> {
    Ok(FileObjectIdentity {
        volume_serial_number: 0,
        file_index_high: 0,
        file_index_low: 0,
    })
}

#[cfg(windows)]
pub fn query_directory_identity(path: &Path) -> Result<FileObjectIdentity> {
    use std::os::windows::fs::OpenOptionsExt;
    use std::os::windows::io::AsRawHandle;
    const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x02000000;
    const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x00200000;
    const FILE_SHARE_READ: u32 = 0x00000001;
    const FILE_SHARE_WRITE: u32 = 0x00000002;
    const FILE_SHARE_DELETE: u32 = 0x00000004;
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x00000400;
    const FILE_ATTRIBUTE_DIRECTORY: u32 = 0x00000010;
    const FILE_TYPE_DISK: u32 = 0x0001;

    let file = std::fs::OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)
        .map_err(AetherError::Io)?;

    #[repr(C)]
    #[allow(non_snake_case)]
    struct BY_HANDLE_FILE_INFORMATION {
        dwFileAttributes: u32,
        ftCreationTime: [u32; 2],
        ftLastAccessTime: [u32; 2],
        ftLastWriteTime: [u32; 2],
        dwVolumeSerialNumber: u32,
        nFileSizeHigh: u32,
        nFileSizeLow: u32,
        nNumberOfLinks: u32,
        nFileIndexHigh: u32,
        nFileIndexLow: u32,
    }
    extern "system" {
        fn GetFileType(hFile: *mut std::ffi::c_void) -> u32;
        fn GetFileInformationByHandle(
            hFile: *mut std::ffi::c_void,
            lpFileInformation: *mut BY_HANDLE_FILE_INFORMATION,
        ) -> i32;
    }

    let file_type = unsafe { GetFileType(file.as_raw_handle()) };
    if file_type != FILE_TYPE_DISK {
        return Err(AetherError::PermissionDenied(format!(
            "Object '{}' is not a standard disk directory (file type: 0x{:x})",
            path.display(),
            file_type
        )));
    }

    let mut by_handle_info = std::mem::MaybeUninit::<BY_HANDLE_FILE_INFORMATION>::zeroed();
    let info_res =
        unsafe { GetFileInformationByHandle(file.as_raw_handle(), by_handle_info.as_mut_ptr()) };
    if info_res == 0 {
        return Err(AetherError::Io(std::io::Error::last_os_error()));
    }
    let file_info = unsafe { by_handle_info.assume_init() };

    let is_reparse = (file_info.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT) != 0;
    let is_dir = (file_info.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY) != 0;
    if is_reparse {
        return Err(AetherError::PermissionDenied(format!(
            "Object '{}' is a reparse point (attributes: 0x{:x}); refusing directory identity query",
            path.display(),
            file_info.dwFileAttributes
        )));
    }
    if !is_dir {
        return Err(AetherError::PermissionDenied(format!(
            "Object '{}' is not a directory (attributes: 0x{:x}); refusing directory identity query",
            path.display(),
            file_info.dwFileAttributes
        )));
    }

    Ok(FileObjectIdentity {
        volume_serial_number: file_info.dwVolumeSerialNumber as u64,
        file_index_high: file_info.nFileIndexHigh as u64,
        file_index_low: file_info.nFileIndexLow as u64,
    })
}

#[cfg(unix)]
pub fn query_directory_identity(path: &Path) -> Result<FileObjectIdentity> {
    use std::os::unix::fs::MetadataExt;
    let meta = std::fs::symlink_metadata(path).map_err(AetherError::Io)?;
    if meta.file_type().is_symlink() {
        return Err(AetherError::PermissionDenied(format!(
            "Object '{}' is a symlink; refusing directory identity query",
            path.display()
        )));
    }
    if !meta.is_dir() {
        return Err(AetherError::PermissionDenied(format!(
            "Object '{}' is not a directory; refusing directory identity query",
            path.display()
        )));
    }
    let ino = meta.ino();
    Ok(FileObjectIdentity {
        volume_serial_number: meta.dev(),
        file_index_high: (ino >> 32) as u64,
        file_index_low: (ino & 0xFFFFFFFF) as u64,
    })
}

#[cfg(not(any(windows, unix)))]
pub fn query_directory_identity(_path: &Path) -> Result<FileObjectIdentity> {
    Ok(FileObjectIdentity {
        volume_serial_number: 0,
        file_index_high: 0,
        file_index_low: 0,
    })
}

#[cfg(windows)]
pub fn query_any_object_identity(path: &Path) -> Result<FileObjectIdentity> {
    use std::os::windows::fs::OpenOptionsExt;
    use std::os::windows::io::AsRawHandle;
    const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x02000000;
    const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x00200000;
    const FILE_SHARE_READ: u32 = 0x00000001;
    const FILE_SHARE_WRITE: u32 = 0x00000002;
    const FILE_SHARE_DELETE: u32 = 0x00000004;
    const FILE_TYPE_DISK: u32 = 0x0001;

    let file = std::fs::OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)
        .map_err(AetherError::Io)?;

    #[repr(C)]
    #[allow(non_snake_case)]
    struct BY_HANDLE_FILE_INFORMATION {
        dwFileAttributes: u32,
        ftCreationTime: [u32; 2],
        ftLastAccessTime: [u32; 2],
        ftLastWriteTime: [u32; 2],
        dwVolumeSerialNumber: u32,
        nFileSizeHigh: u32,
        nFileSizeLow: u32,
        nNumberOfLinks: u32,
        nFileIndexHigh: u32,
        nFileIndexLow: u32,
    }
    extern "system" {
        fn GetFileType(hFile: *mut std::ffi::c_void) -> u32;
        fn GetFileInformationByHandle(
            hFile: *mut std::ffi::c_void,
            lpFileInformation: *mut BY_HANDLE_FILE_INFORMATION,
        ) -> i32;
    }

    let file_type = unsafe { GetFileType(file.as_raw_handle()) };
    if file_type != FILE_TYPE_DISK {
        return Err(AetherError::PermissionDenied(format!(
            "Object '{}' is not a standard disk object (file type: 0x{:x})",
            path.display(),
            file_type
        )));
    }

    let mut by_handle_info = std::mem::MaybeUninit::<BY_HANDLE_FILE_INFORMATION>::zeroed();
    let info_res =
        unsafe { GetFileInformationByHandle(file.as_raw_handle(), by_handle_info.as_mut_ptr()) };
    if info_res == 0 {
        return Err(AetherError::Io(std::io::Error::last_os_error()));
    }
    let file_info = unsafe { by_handle_info.assume_init() };

    Ok(FileObjectIdentity {
        volume_serial_number: file_info.dwVolumeSerialNumber as u64,
        file_index_high: file_info.nFileIndexHigh as u64,
        file_index_low: file_info.nFileIndexLow as u64,
    })
}

#[cfg(unix)]
pub fn query_any_object_identity(path: &Path) -> Result<FileObjectIdentity> {
    use std::os::unix::fs::MetadataExt;
    let meta = std::fs::symlink_metadata(path).map_err(AetherError::Io)?;
    let ino = meta.ino();
    Ok(FileObjectIdentity {
        volume_serial_number: meta.dev(),
        file_index_high: (ino >> 32) as u64,
        file_index_low: (ino & 0xFFFFFFFF) as u64,
    })
}

#[cfg(not(any(windows, unix)))]
pub fn query_any_object_identity(_path: &Path) -> Result<FileObjectIdentity> {
    Ok(FileObjectIdentity {
        volume_serial_number: 0,
        file_index_high: 0,
        file_index_low: 0,
    })
}

pub type PathHook = std::sync::Arc<dyn Fn(&Path) + Send + Sync>;

static OWNERSHIP_PRE_CAPTURE_HOOK: std::sync::Mutex<Option<PathHook>> = std::sync::Mutex::new(None);
static STAGING_PRE_PIN_HOOK: std::sync::Mutex<Option<PathHook>> = std::sync::Mutex::new(None);
static PRE_PUBLICATION_HOOK: std::sync::Mutex<Option<PathHook>> = std::sync::Mutex::new(None);
static CHILD_OPEN_PRE_HOOK: std::sync::Mutex<Option<PathHook>> = std::sync::Mutex::new(None);
static ROOT_PRE_ENUMERATION_HOOK: std::sync::Mutex<Option<PathHook>> = std::sync::Mutex::new(None);
static NESTED_DIR_PRE_ENUMERATION_HOOK: std::sync::Mutex<Option<PathHook>> =
    std::sync::Mutex::new(None);
static NESTED_DIR_PRE_TRAVERSAL_HOOK: std::sync::Mutex<Option<PathHook>> =
    std::sync::Mutex::new(None);
static CHILD_FILE_PRE_DELETE_HOOK: std::sync::Mutex<Option<PathHook>> = std::sync::Mutex::new(None);
static CHILD_ENTRY_FINAL_PRE_DESTRUCTIVE_HOOK: std::sync::Mutex<Option<PathHook>> =
    std::sync::Mutex::new(None);
static SYMLINK_PRE_DELETE_HOOK: std::sync::Mutex<Option<PathHook>> = std::sync::Mutex::new(None);
static NESTED_DIR_PRE_REMOVE_HOOK: std::sync::Mutex<Option<PathHook>> = std::sync::Mutex::new(None);
static ROOT_PRE_REMOVE_HOOK: std::sync::Mutex<Option<PathHook>> = std::sync::Mutex::new(None);
static TEMP_ENTRY_PRE_UNLINK_HOOK: std::sync::Mutex<Option<PathHook>> = std::sync::Mutex::new(None);

static PARTIAL_TRAVERSAL_FAULT_INJECT: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);
static FAULT_INJECT_TEMP_RENAME_FAILURE: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);
static FAULT_INJECT_PIN_FAILURE: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

pub fn set_ownership_pre_capture_hook(hook: Option<PathHook>) {
    if let Ok(mut g) = OWNERSHIP_PRE_CAPTURE_HOOK.lock() {
        *g = hook;
    }
}

pub fn set_staging_pre_pin_hook(hook: Option<PathHook>) {
    if let Ok(mut g) = STAGING_PRE_PIN_HOOK.lock() {
        *g = hook;
    }
}

pub fn set_pre_publication_hook(hook: Option<PathHook>) {
    if let Ok(mut g) = PRE_PUBLICATION_HOOK.lock() {
        *g = hook;
    }
}

pub fn set_child_open_pre_hook(hook: Option<PathHook>) {
    if let Ok(mut g) = CHILD_OPEN_PRE_HOOK.lock() {
        *g = hook;
    }
}

pub fn set_root_pre_enumeration_hook(hook: Option<PathHook>) {
    if let Ok(mut g) = ROOT_PRE_ENUMERATION_HOOK.lock() {
        *g = hook;
    }
}

pub fn set_nested_dir_pre_enumeration_hook(hook: Option<PathHook>) {
    if let Ok(mut g) = NESTED_DIR_PRE_ENUMERATION_HOOK.lock() {
        *g = hook;
    }
}

pub fn set_nested_dir_pre_traversal_hook(hook: Option<PathHook>) {
    if let Ok(mut g) = NESTED_DIR_PRE_TRAVERSAL_HOOK.lock() {
        *g = hook;
    }
}

pub fn set_child_file_pre_delete_hook(hook: Option<PathHook>) {
    if let Ok(mut g) = CHILD_FILE_PRE_DELETE_HOOK.lock() {
        *g = hook;
    }
}

pub fn set_child_entry_final_pre_destructive_hook(hook: Option<PathHook>) {
    if let Ok(mut g) = CHILD_ENTRY_FINAL_PRE_DESTRUCTIVE_HOOK.lock() {
        *g = hook;
    }
}

pub fn set_symlink_pre_delete_hook(hook: Option<PathHook>) {
    if let Ok(mut g) = SYMLINK_PRE_DELETE_HOOK.lock() {
        *g = hook;
    }
}

pub fn set_nested_dir_pre_remove_hook(hook: Option<PathHook>) {
    if let Ok(mut g) = NESTED_DIR_PRE_REMOVE_HOOK.lock() {
        *g = hook;
    }
}

pub fn set_root_pre_remove_hook(hook: Option<PathHook>) {
    if let Ok(mut g) = ROOT_PRE_REMOVE_HOOK.lock() {
        *g = hook;
    }
}

pub fn set_temp_entry_pre_unlink_hook(hook: Option<PathHook>) {
    if let Ok(mut g) = TEMP_ENTRY_PRE_UNLINK_HOOK.lock() {
        *g = hook;
    }
}

pub fn set_partial_traversal_fault_inject(val: bool) {
    PARTIAL_TRAVERSAL_FAULT_INJECT.store(val, std::sync::atomic::Ordering::SeqCst);
}

pub fn set_fault_inject_temp_rename_failure(val: bool) {
    FAULT_INJECT_TEMP_RENAME_FAILURE.store(val, std::sync::atomic::Ordering::SeqCst);
}

pub fn set_fault_inject_pin_failure(val: bool) {
    FAULT_INJECT_PIN_FAILURE.store(val, std::sync::atomic::Ordering::SeqCst);
}

/// Threat-Model & Security Boundary:
/// - Cross-user local isolation is enforced on Unix by verifying parent directory ownership against the
///   running process UID and restricting POSIX mode to 0700.
/// - On Windows, cross-user isolation is enforced by verifying parent directory ownership against the
///   current process token (checking TokenUser, TokenOwner, and token membership for elevated/admin contexts)
///   and inspecting explicit and inherited ACEs in the DACL to verify or establish restricted private permissions,
///   rejecting any dangerous access (creation, modification, deletion, inheritance) granted to unauthorized principals
///   (such as Everyone, BUILTIN\Users, Authenticated Users, or third-party accounts).
/// - Threat-Model Boundary Limitation: Local filesystem permissions do not isolate against malicious concurrent
///   processes or threads running under the IDENTICAL UID / user security token. Protection against same-user
///   concurrent namespace substitution during snapshot creation is established by Isolated Staging Creation
///   & Atomic Adoption (using an unguessable 128-bit random nonce in a private parent) and OS-level atomic
///   no-replace publication (RENAME_NOREPLACE on Linux, renameatx_np(RENAME_EXCL) on macOS, MoveFileExW without replace on Windows).
pub fn verify_and_secure_storage_parent(parent: &Path) -> Result<()> {
    if !parent.is_dir() {
        return Err(AetherError::PermissionDenied(format!(
            "Parent storage path '{:?}' is not a directory",
            parent
        )));
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        use std::os::unix::fs::PermissionsExt;
        let meta = std::fs::symlink_metadata(parent).map_err(AetherError::Io)?;
        if meta.file_type().is_symlink() {
            return Err(AetherError::PermissionDenied(format!(
                "Parent storage path '{:?}' is a symlink; refusing operation",
                parent
            )));
        }
        let current_uid = unsafe { libc::getuid() };
        if meta.uid() != current_uid {
            return Err(AetherError::PermissionDenied(format!(
                "Parent storage directory '{:?}' is owned by UID {}, but current process is UID {}; refusing operation",
                parent,
                meta.uid(),
                current_uid
            )));
        }
        let mode = meta.permissions().mode();
        if (mode & 0o077) != 0 {
            std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700)).map_err(
                |e| {
                    AetherError::Io(std::io::Error::new(
                        e.kind(),
                        format!(
                            "Failed to enforce 0700 permissions on storage parent '{:?}': {}",
                            parent, e
                        ),
                    ))
                },
            )?;
            let meta_after = std::fs::symlink_metadata(parent).map_err(AetherError::Io)?;
            if (meta_after.permissions().mode() & 0o077) != 0 {
                return Err(AetherError::PermissionDenied(format!(
                    "Failed to verify restricted 0700 permissions on storage parent '{:?}'",
                    parent
                )));
            }
        }
        Ok(())
    }

    #[cfg(windows)]
    {
        match verify_storage_parent_acl(parent) {
            Ok(()) => Ok(()),
            Err(AetherError::PermissionDenied(ref msg))
                if msg.contains("unsafe permissions") || msg.contains("NULL DACL") =>
            {
                secure_storage_parent_private_acl(parent)?;
                verify_storage_parent_acl(parent)
            }
            Err(e) => Err(e),
        }
    }

    #[cfg(not(any(unix, windows)))]
    {
        Ok(())
    }
}

#[cfg(windows)]
pub fn verify_storage_parent_acl(parent: &Path) -> Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x00000400;
    const FILE_ATTRIBUTE_DIRECTORY: u32 = 0x00000010;

    let meta = std::fs::symlink_metadata(parent).map_err(AetherError::Io)?;
    let attrs = meta.file_attributes();
    if (attrs & FILE_ATTRIBUTE_REPARSE_POINT) != 0 {
        return Err(AetherError::PermissionDenied(format!(
            "Parent storage path '{:?}' is a reparse point or junction; refusing operation",
            parent
        )));
    }
    if (attrs & FILE_ATTRIBUTE_DIRECTORY) == 0 {
        return Err(AetherError::PermissionDenied(format!(
            "Parent storage path '{:?}' is not a directory (attrs 0x{:x})",
            parent, attrs
        )));
    }

    if FAULT_INJECT_WINDOWS_SECURITY_DESCRIPTOR_FAILURE.with(|c| c.get()) {
        return Err(AetherError::PermissionDenied(format!(
            "Injected fault: failed to inspect security descriptor on storage parent '{:?}'",
            parent
        )));
    }

    let wide_parent: Vec<u16> = parent.as_os_str().encode_wide().chain(Some(0)).collect();

    unsafe {
        const SE_FILE_OBJECT: u32 = 1;
        const OWNER_SECURITY_INFORMATION: u32 = 0x00000001;
        const DACL_SECURITY_INFORMATION: u32 = 0x00000004;
        const TOKEN_QUERY: u32 = 0x0008;

        extern "system" {
            fn GetNamedSecurityInfoW(
                pObjectName: *const u16,
                ObjectType: u32,
                SecurityInfo: u32,
                ppsidOwner: *mut *mut std::ffi::c_void,
                ppsidGroup: *mut *mut std::ffi::c_void,
                ppDacl: *mut *mut std::ffi::c_void,
                ppSacl: *mut *mut std::ffi::c_void,
                ppSecurityDescriptor: *mut *mut std::ffi::c_void,
            ) -> u32;
            fn OpenProcessToken(
                ProcessHandle: *mut std::ffi::c_void,
                DesiredAccess: u32,
                TokenHandle: *mut *mut std::ffi::c_void,
            ) -> i32;
            fn GetTokenInformation(
                TokenHandle: *mut std::ffi::c_void,
                TokenInformationClass: u32,
                TokenInformation: *mut std::ffi::c_void,
                TokenInformationLength: u32,
                ReturnLength: *mut u32,
            ) -> i32;
            fn CheckTokenMembership(
                TokenHandle: *mut std::ffi::c_void,
                SidToCheck: *mut std::ffi::c_void,
                IsMember: *mut i32,
            ) -> i32;
            fn CreateWellKnownSid(
                WellKnownSidType: u32,
                DomainSid: *mut std::ffi::c_void,
                pSid: *mut std::ffi::c_void,
                cbSid: *mut u32,
            ) -> i32;
            fn EqualSid(pSid1: *mut std::ffi::c_void, pSid2: *mut std::ffi::c_void) -> i32;
            fn GetAclInformation(
                pAcl: *mut std::ffi::c_void,
                pAclInformation: *mut std::ffi::c_void,
                nAclInformationLength: u32,
                dwAclInformationClass: u32,
            ) -> i32;
            fn GetAce(
                pAcl: *mut std::ffi::c_void,
                dwAceIndex: u32,
                pAce: *mut *mut std::ffi::c_void,
            ) -> i32;
            fn ConvertSidToStringSidW(Sid: *mut std::ffi::c_void, StringSid: *mut *mut u16) -> i32;
            fn LocalFree(hMem: *mut std::ffi::c_void) -> *mut std::ffi::c_void;
            fn CloseHandle(hObject: *mut std::ffi::c_void) -> i32;
            fn GetCurrentProcess() -> *mut std::ffi::c_void;
        }

        let mut p_owner_sid: *mut std::ffi::c_void = std::ptr::null_mut();
        let mut p_dacl: *mut std::ffi::c_void = std::ptr::null_mut();
        let mut p_sec_desc: *mut std::ffi::c_void = std::ptr::null_mut();

        let sec_res = GetNamedSecurityInfoW(
            wide_parent.as_ptr(),
            SE_FILE_OBJECT,
            OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION,
            &mut p_owner_sid,
            std::ptr::null_mut(),
            &mut p_dacl,
            std::ptr::null_mut(),
            &mut p_sec_desc,
        );

        if sec_res != 0 {
            return Err(AetherError::PermissionDenied(format!(
                "Failed to query security descriptor on storage parent '{:?}': Win32 error {}",
                parent, sec_res
            )));
        }

        struct SecDescGuard(*mut std::ffi::c_void);
        impl Drop for SecDescGuard {
            fn drop(&mut self) {
                if !self.0.is_null() {
                    unsafe {
                        LocalFree(self.0);
                    }
                }
            }
        }
        let _desc_guard = SecDescGuard(p_sec_desc);

        let mut token: *mut std::ffi::c_void = std::ptr::null_mut();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
            return Err(AetherError::Io(std::io::Error::last_os_error()));
        }
        struct TokenGuard(*mut std::ffi::c_void);
        impl Drop for TokenGuard {
            fn drop(&mut self) {
                if !self.0.is_null() {
                    unsafe {
                        CloseHandle(self.0);
                    }
                }
            }
        }
        let _tok_guard = TokenGuard(token);

        #[repr(C)]
        #[allow(non_snake_case)]
        struct SID_AND_ATTRIBUTES {
            Sid: *mut std::ffi::c_void,
            Attributes: u32,
        }
        #[repr(C)]
        #[allow(non_snake_case)]
        struct TOKEN_USER {
            User: SID_AND_ATTRIBUTES,
        }
        #[repr(C)]
        #[allow(non_snake_case)]
        struct TOKEN_OWNER {
            Owner: *mut std::ffi::c_void,
        }

        #[repr(align(8))]
        struct AlignedBuffer([u8; 512]);
        let mut user_buf = AlignedBuffer([0u8; 512]);
        let mut ret_len = 0u32;
        #[allow(non_upper_case_globals)]
        const TokenUser: u32 = 1;
        if GetTokenInformation(
            token,
            TokenUser,
            user_buf.0.as_mut_ptr() as *mut _,
            user_buf.0.len() as u32,
            &mut ret_len,
        ) == 0
        {
            return Err(AetherError::Io(std::io::Error::last_os_error()));
        }
        let token_user = &*(user_buf.0.as_ptr() as *const TOKEN_USER);

        let mut owner_buf = AlignedBuffer([0u8; 512]);
        #[allow(non_upper_case_globals)]
        const TokenOwner: u32 = 4;
        let mut tok_owner_sid: *mut std::ffi::c_void = std::ptr::null_mut();
        if GetTokenInformation(
            token,
            TokenOwner,
            owner_buf.0.as_mut_ptr() as *mut _,
            owner_buf.0.len() as u32,
            &mut ret_len,
        ) != 0
        {
            let token_owner = &*(owner_buf.0.as_ptr() as *const TOKEN_OWNER);
            tok_owner_sid = token_owner.Owner;
        }

        // Test seam for simulating different owner
        if FAULT_INJECT_WINDOWS_DIFFERENT_OWNER.with(|c| c.get()) {
            return Err(AetherError::PermissionDenied(format!(
                "Parent storage directory '{:?}' is not owned by an authorized private owner (simulated different user)",
                parent
            )));
        }

        // Allow simulated owner SID (e.g. BUILTIN\Users or BUILTIN\Administrators) for testing
        const WIN_LOCAL_SYSTEM_SID: u32 = 22;
        const WIN_BUILTIN_ADMINISTRATORS_SID: u32 = 26;
        let mut sim_owner_sid = vec![0u8; 68];
        let mut effective_owner_sid = p_owner_sid;
        if let Some(well_known_type) = FAULT_INJECT_WINDOWS_SIMULATED_OWNER_SID.with(|c| c.get()) {
            let mut cb_sim = sim_owner_sid.len() as u32;
            if CreateWellKnownSid(
                well_known_type,
                std::ptr::null_mut(),
                sim_owner_sid.as_mut_ptr() as *mut _,
                &mut cb_sim,
            ) != 0
            {
                effective_owner_sid = sim_owner_sid.as_mut_ptr() as *mut _;
            }
        }

        // Narrow accepted-owner policy:
        // 1. Current individual user (TokenUser):
        let is_current_user = EqualSid(token_user.User.Sid, effective_owner_sid) != 0;

        // 2. Elevated administrative storage (BUILTIN\Administrators S-1-5-32-544):
        let mut is_authorized_admin_owner = false;
        let mut admin_sid = vec![0u8; 68];
        let mut cb_admin = admin_sid.len() as u32;
        if CreateWellKnownSid(
            WIN_BUILTIN_ADMINISTRATORS_SID,
            std::ptr::null_mut(),
            admin_sid.as_mut_ptr() as *mut _,
            &mut cb_admin,
        ) != 0
            && EqualSid(admin_sid.as_mut_ptr() as *mut _, effective_owner_sid) != 0
        {
            let mut is_member = 0i32;
            let token_is_admin_member = (CheckTokenMembership(
                std::ptr::null_mut(),
                admin_sid.as_mut_ptr() as *mut _,
                &mut is_member,
            ) != 0
                && is_member != 0)
                || FAULT_INJECT_WINDOWS_SIMULATED_ADMIN_TOKEN.with(|c| c.get());

            if token_is_admin_member {
                let matches_tok_owner = (!tok_owner_sid.is_null()
                    && EqualSid(tok_owner_sid, admin_sid.as_mut_ptr() as *mut _) != 0)
                    || FAULT_INJECT_WINDOWS_SIMULATED_ADMIN_TOKEN.with(|c| c.get());

                #[repr(C)]
                #[allow(non_snake_case)]
                struct TOKEN_ELEVATION {
                    TokenIsElevated: u32,
                }
                const TOKEN_ELEVATION_CLASS: u32 = 20;
                let mut elev = TOKEN_ELEVATION { TokenIsElevated: 0 };
                let mut elev_ret_len = 0u32;
                let is_elevated = (GetTokenInformation(
                    token,
                    TOKEN_ELEVATION_CLASS,
                    &mut elev as *mut _ as *mut _,
                    std::mem::size_of::<TOKEN_ELEVATION>() as u32,
                    &mut elev_ret_len,
                ) != 0
                    && elev.TokenIsElevated != 0)
                    || FAULT_INJECT_WINDOWS_SIMULATED_ADMIN_TOKEN.with(|c| c.get());

                if matches_tok_owner || is_elevated {
                    is_authorized_admin_owner = true;
                }
            }
        }

        // 3. SYSTEM (S-1-5-18):
        let mut is_authorized_system_owner = false;
        let mut sys_sid = vec![0u8; 68];
        let mut cb_sys = sys_sid.len() as u32;
        if CreateWellKnownSid(
            WIN_LOCAL_SYSTEM_SID,
            std::ptr::null_mut(),
            sys_sid.as_mut_ptr() as *mut _,
            &mut cb_sys,
        ) != 0
            && EqualSid(sys_sid.as_mut_ptr() as *mut _, effective_owner_sid) != 0
            && EqualSid(token_user.User.Sid, sys_sid.as_mut_ptr() as *mut _) != 0
        {
            is_authorized_system_owner = true;
        }

        let is_valid_owner =
            is_current_user || is_authorized_admin_owner || is_authorized_system_owner;
        if !is_valid_owner {
            return Err(AetherError::PermissionDenied(format!(
                "Parent storage directory '{:?}' is not owned by an authorized private owner (individual TokenUser or authorized elevated Administrators); refusing operation",
                parent
            )));
        }

        // Inspect DACL
        if p_dacl.is_null() {
            return Err(AetherError::PermissionDenied(format!(
                "Parent storage directory '{:?}' has a NULL DACL (unprotected); refusing operation",
                parent
            )));
        }

        #[repr(C)]
        #[allow(non_snake_case)]
        struct ACL_SIZE_INFORMATION {
            AceCount: u32,
            AclBytesInUse: u32,
            AclBytesFree: u32,
        }
        const ACL_SIZE_INFO_CLASS: u32 = 2;
        let mut acl_size = ACL_SIZE_INFORMATION {
            AceCount: 0,
            AclBytesInUse: 0,
            AclBytesFree: 0,
        };
        if GetAclInformation(
            p_dacl,
            &mut acl_size as *mut _ as *mut std::ffi::c_void,
            std::mem::size_of::<ACL_SIZE_INFORMATION>() as u32,
            ACL_SIZE_INFO_CLASS,
        ) == 0
        {
            return Err(AetherError::Io(std::io::Error::last_os_error()));
        }

        #[repr(C)]
        #[allow(non_snake_case)]
        struct ACE_HEADER {
            AceType: u8,
            AceFlags: u8,
            AceSize: u16,
        }
        #[repr(C)]
        #[allow(non_snake_case)]
        struct ACCESS_ALLOWED_ACE {
            Header: ACE_HEADER,
            Mask: u32,
            SidStart: u32,
        }

        const ACCESS_ALLOWED_ACE_TYPE: u8 = 0;
        const ACCESS_ALLOWED_CALLBACK_ACE_TYPE: u8 = 9;

        const FILE_ADD_FILE: u32 = 0x0002;
        const FILE_ADD_SUBDIRECTORY: u32 = 0x0004;
        const FILE_WRITE_DATA: u32 = 0x0002;
        const FILE_APPEND_DATA: u32 = 0x0004;
        const FILE_WRITE_EA: u32 = 0x0010;
        const FILE_WRITE_ATTRIBUTES: u32 = 0x0100;
        const FILE_DELETE_CHILD: u32 = 0x0040;
        const DELETE: u32 = 0x00010000;
        const WRITE_DAC: u32 = 0x00040000;
        const WRITE_OWNER: u32 = 0x00080000;
        const GENERIC_WRITE: u32 = 0x40000000;
        const GENERIC_ALL: u32 = 0x10000000;

        const DANGEROUS_PERMISSIONS: u32 = FILE_ADD_FILE
            | FILE_ADD_SUBDIRECTORY
            | FILE_WRITE_DATA
            | FILE_APPEND_DATA
            | FILE_WRITE_EA
            | FILE_WRITE_ATTRIBUTES
            | FILE_DELETE_CHILD
            | DELETE
            | WRITE_DAC
            | WRITE_OWNER
            | GENERIC_WRITE
            | GENERIC_ALL;

        for i in 0..acl_size.AceCount {
            let mut p_ace: *mut std::ffi::c_void = std::ptr::null_mut();
            if GetAce(p_dacl, i, &mut p_ace) == 0 {
                return Err(AetherError::Io(std::io::Error::last_os_error()));
            }
            let ace_hdr = &*(p_ace as *const ACE_HEADER);
            if ace_hdr.AceType == ACCESS_ALLOWED_ACE_TYPE
                || ace_hdr.AceType == ACCESS_ALLOWED_CALLBACK_ACE_TYPE
            {
                let allowed_ace = &*(p_ace as *const ACCESS_ALLOWED_ACE);
                let mask = allowed_ace.Mask;
                if (mask & DANGEROUS_PERMISSIONS) != 0 {
                    let ace_sid = &allowed_ace.SidStart as *const u32 as *mut std::ffi::c_void;
                    let is_current_user_ace = EqualSid(token_user.User.Sid, ace_sid) != 0;

                    if is_current_user_ace {
                        continue;
                    }

                    let mut p_str: *mut u16 = std::ptr::null_mut();
                    if ConvertSidToStringSidW(ace_sid, &mut p_str) != 0 && !p_str.is_null() {
                        let mut len = 0;
                        while *p_str.add(len) != 0 {
                            len += 1;
                        }
                        let s = String::from_utf16_lossy(std::slice::from_raw_parts(p_str, len));
                        LocalFree(p_str as *mut _);

                        // S-1-5-18 = NT AUTHORITY\SYSTEM
                        // S-1-5-32-544 = BUILTIN\Administrators
                        if s == "S-1-5-18" || s == "S-1-5-32-544" {
                            continue;
                        }

                        return Err(AetherError::PermissionDenied(format!(
                            "Parent storage directory '{:?}' has unsafe permissions: ACE {} grants dangerous permissions (0x{:08X}) to unauthorized principal {}; refusing operation",
                            parent, i, mask, s
                        )));
                    } else {
                        return Err(AetherError::PermissionDenied(format!(
                            "Parent storage directory '{:?}' has unsafe permissions: ACE {} grants dangerous permissions (0x{:08X}) to unknown principal; refusing operation",
                            parent, i, mask
                        )));
                    }
                }
            }
        }
    }

    Ok(())
}

#[cfg(windows)]
pub fn secure_storage_parent_private_acl(parent: &Path) -> Result<()> {
    use std::os::windows::ffi::OsStrExt;

    let wide_parent: Vec<u16> = parent.as_os_str().encode_wide().chain(Some(0)).collect();

    unsafe {
        const SE_FILE_OBJECT: u32 = 1;
        const DACL_SECURITY_INFORMATION: u32 = 0x00000004;
        const PROTECTED_DACL_SECURITY_INFORMATION: u32 = 0x80000000;
        const TOKEN_QUERY: u32 = 0x0008;
        const ACL_REVISION: u32 = 2;
        const CONTAINER_INHERIT_ACE: u32 = 0x02;
        const OBJECT_INHERIT_ACE: u32 = 0x01;
        const GENERIC_ALL: u32 = 0x10000000;
        const WIN_LOCAL_SYSTEM_SID: u32 = 22;
        const WIN_BUILTIN_ADMINISTRATORS_SID: u32 = 26;

        extern "system" {
            fn OpenProcessToken(
                ProcessHandle: *mut std::ffi::c_void,
                DesiredAccess: u32,
                TokenHandle: *mut *mut std::ffi::c_void,
            ) -> i32;
            fn GetTokenInformation(
                TokenHandle: *mut std::ffi::c_void,
                TokenInformationClass: u32,
                TokenInformation: *mut std::ffi::c_void,
                TokenInformationLength: u32,
                ReturnLength: *mut u32,
            ) -> i32;
            fn CreateWellKnownSid(
                WellKnownSidType: u32,
                DomainSid: *mut std::ffi::c_void,
                pSid: *mut std::ffi::c_void,
                cbSid: *mut u32,
            ) -> i32;
            fn InitializeAcl(
                pAcl: *mut std::ffi::c_void,
                nAclLength: u32,
                dwAclRevision: u32,
            ) -> i32;
            fn AddAccessAllowedAceEx(
                pAcl: *mut std::ffi::c_void,
                dwAceRevision: u32,
                AceFlags: u32,
                AccessMask: u32,
                pSid: *mut std::ffi::c_void,
            ) -> i32;
            fn SetNamedSecurityInfoW(
                pObjectName: *const u16,
                ObjectType: u32,
                SecurityInfo: u32,
                ppsidOwner: *mut std::ffi::c_void,
                ppsidGroup: *mut std::ffi::c_void,
                ppDacl: *mut std::ffi::c_void,
                ppSacl: *mut std::ffi::c_void,
            ) -> u32;
            fn CloseHandle(hObject: *mut std::ffi::c_void) -> i32;
            fn GetCurrentProcess() -> *mut std::ffi::c_void;
        }

        let mut token: *mut std::ffi::c_void = std::ptr::null_mut();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
            return Err(AetherError::Io(std::io::Error::last_os_error()));
        }

        #[repr(C)]
        #[allow(non_snake_case)]
        struct SID_AND_ATTRIBUTES {
            Sid: *mut std::ffi::c_void,
            Attributes: u32,
        }
        #[repr(C)]
        #[allow(non_snake_case)]
        struct TOKEN_USER {
            User: SID_AND_ATTRIBUTES,
        }

        #[repr(align(8))]
        struct AlignedBuffer([u8; 512]);
        let mut user_buf = AlignedBuffer([0u8; 512]);
        let mut ret_len = 0u32;
        #[allow(non_upper_case_globals)]
        const TokenUser: u32 = 1;
        if GetTokenInformation(
            token,
            TokenUser,
            user_buf.0.as_mut_ptr() as *mut _,
            user_buf.0.len() as u32,
            &mut ret_len,
        ) == 0
        {
            CloseHandle(token);
            return Err(AetherError::Io(std::io::Error::last_os_error()));
        }
        CloseHandle(token);
        let token_user = &*(user_buf.0.as_ptr() as *const TOKEN_USER);

        let mut system_sid = vec![0u8; 68];
        let mut cb_system = system_sid.len() as u32;
        if CreateWellKnownSid(
            WIN_LOCAL_SYSTEM_SID,
            std::ptr::null_mut(),
            system_sid.as_mut_ptr() as *mut _,
            &mut cb_system,
        ) == 0
        {
            return Err(AetherError::Io(std::io::Error::last_os_error()));
        }

        let mut admin_sid = vec![0u8; 68];
        let mut cb_admin = admin_sid.len() as u32;
        if CreateWellKnownSid(
            WIN_BUILTIN_ADMINISTRATORS_SID,
            std::ptr::null_mut(),
            admin_sid.as_mut_ptr() as *mut _,
            &mut cb_admin,
        ) == 0
        {
            return Err(AetherError::Io(std::io::Error::last_os_error()));
        }

        let mut acl_buf = vec![0u8; 1024];
        if InitializeAcl(
            acl_buf.as_mut_ptr() as *mut _,
            acl_buf.len() as u32,
            ACL_REVISION,
        ) == 0
        {
            return Err(AetherError::Io(std::io::Error::last_os_error()));
        }

        let ace_flags = CONTAINER_INHERIT_ACE | OBJECT_INHERIT_ACE;
        if AddAccessAllowedAceEx(
            acl_buf.as_mut_ptr() as *mut _,
            ACL_REVISION,
            ace_flags,
            GENERIC_ALL,
            token_user.User.Sid,
        ) == 0
        {
            return Err(AetherError::Io(std::io::Error::last_os_error()));
        }

        if AddAccessAllowedAceEx(
            acl_buf.as_mut_ptr() as *mut _,
            ACL_REVISION,
            ace_flags,
            GENERIC_ALL,
            system_sid.as_mut_ptr() as *mut _,
        ) == 0
        {
            return Err(AetherError::Io(std::io::Error::last_os_error()));
        }

        if AddAccessAllowedAceEx(
            acl_buf.as_mut_ptr() as *mut _,
            ACL_REVISION,
            ace_flags,
            GENERIC_ALL,
            admin_sid.as_mut_ptr() as *mut _,
        ) == 0
        {
            return Err(AetherError::Io(std::io::Error::last_os_error()));
        }

        let set_res = SetNamedSecurityInfoW(
            wide_parent.as_ptr(),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            acl_buf.as_mut_ptr() as *mut _,
            std::ptr::null_mut(),
        );

        if set_res != 0 {
            return Err(AetherError::PermissionDenied(format!(
                "Failed to set private ACL on storage parent '{:?}': Win32 error {}",
                parent, set_res
            )));
        }
    }

    Ok(())
}

#[cfg(not(windows))]
pub fn verify_storage_parent_acl(_parent: &Path) -> Result<()> {
    Ok(())
}

#[cfg(not(windows))]
pub fn secure_storage_parent_private_acl(_parent: &Path) -> Result<()> {
    Ok(())
}

pub fn create_and_pin_directory(path: &Path) -> Result<PinnedDirectoryHandle> {
    create_and_pin_directory_with_hook(path, None)
}

pub fn create_and_pin_directory_with_hook(
    path: &Path,
    hook: Option<PathHook>,
) -> Result<PinnedDirectoryHandle> {
    let parent = path.parent().ok_or_else(|| {
        AetherError::PermissionDenied(format!("Directory '{:?}' has no parent", path))
    })?;
    verify_and_secure_storage_parent(parent)?;

    // Check if destination path already exists prior to operation (standard collision check)
    let initial_dest_exists = match std::fs::symlink_metadata(path) {
        Ok(_) => true,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => false,
        Err(e) => return Err(AetherError::Io(e)),
    };
    if initial_dest_exists {
        return Err(AetherError::Io(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            format!(
                "Failed to create snapshot directory '{:?}': directory already exists",
                path
            ),
        )));
    }

    // Isolated Staging Creation:
    // Create an unpredictable, cryptographically random staging directory inside the verified parent.
    // Because the name is a 128-bit random nonce inside a private parent,
    // no concurrent process can predict or pre-substitute it.
    let staging_nonce = format!(
        ".stage_{}_{}",
        uuid::Uuid::new_v4().simple(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    );
    let staging_path = parent.join(&staging_nonce);

    std::fs::create_dir(&staging_path).map_err(AetherError::Io)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&staging_path, std::fs::Permissions::from_mode(0o700));
    }

    let initial_staging_id = query_directory_identity(&staging_path)?;

    // Seam: Test 1 & 2 pre-pin hook (replace staging directory before pin attempt)
    if let Ok(guard) = STAGING_PRE_PIN_HOOK.lock() {
        if let Some(ref h) = *guard {
            h(&staging_path);
        }
    }

    if FAULT_INJECT_PIN_FAILURE.load(std::sync::atomic::Ordering::SeqCst) {
        return Err(AetherError::PermissionDenied(format!(
            "Injected fault: pinning failed for staging directory '{:?}'",
            staging_path
        )));
    }

    // Immediately open and pin the staging directory handle.
    // This is our verified, immutable first ownership witness.
    let mut pinned = match pin_created_directory_internal(&staging_path, initial_staging_id) {
        Ok(p) => p,
        Err(e) => {
            // NEVER perform best-effort removal of an unpinned pathname when ownership has not been established!
            // Preserve the potentially replaced object, record the failure, and fail closed.
            return Err(AetherError::PermissionDenied(format!(
                "Failed to pin created staging directory '{:?}': {}; preserving object without deletion",
                staging_path, e
            )));
        }
    };

    // Adversarial Hook Seam (Test 1):
    // Simulates an adversary attempting to substitute `path` before ownership capture.
    if let Some(ref h) = hook {
        h(path);
    } else if let Ok(guard) = OWNERSHIP_PRE_CAPTURE_HOOK.lock() {
        if let Some(ref h) = *guard {
            h(path);
        }
    }

    // If destination `path` already exists (e.g. pre-created by adversary or test hook),
    // NEVER adopt it! Refuse adoption, preserve the replacement object, clean staging, and fail closed.
    let dest_exists = match std::fs::symlink_metadata(path) {
        Ok(_) => true,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => false,
        Err(e) => {
            let clean_res = pinned.clean_contents();
            let rm_res = pinned.verify_and_remove_entry();
            if let Err(cr) = clean_res {
                return Err(AetherError::Internal(format!(
                    "Destination metadata check failed ({}) and staging cleanup failed: {}",
                    e, cr
                )));
            }
            if let Err(rr) = rm_res {
                return Err(AetherError::Internal(format!(
                    "Destination metadata check failed ({}) and staging removal failed: {}",
                    e, rr
                )));
            }
            return Err(AetherError::Io(e));
        }
    };

    if dest_exists {
        // Clean staging dir through its pinned handle
        let clean_res = pinned.clean_contents();
        let rm_res = pinned.verify_and_remove_entry();
        if let Err(cr) = clean_res {
            return Err(AetherError::Internal(format!(
                "Refusing adoption of '{:?}': destination already exists, and staging cleanup failed: {}",
                path, cr
            )));
        }
        if let Err(rr) = rm_res {
            return Err(AetherError::Internal(format!(
                "Refusing adoption of '{:?}': destination already exists, and staging removal failed: {}",
                path, rr
            )));
        }
        return Err(AetherError::PermissionDenied(format!(
            "Refusing to adopt directory '{:?}': destination was substituted before ownership capture",
            path
        )));
    }

    // Seam: Test 3 & 4 pre-publication hook
    // Invoked after final absence check and immediately before atomic no-replace publication
    if let Ok(guard) = PRE_PUBLICATION_HOOK.lock() {
        if let Some(ref h) = *guard {
            h(path);
        }
    }

    // Publish the pinned staging directory to `path` with no-replace semantics
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        let wide_stage: Vec<u16> = staging_path
            .as_os_str()
            .encode_wide()
            .chain(Some(0))
            .collect();
        let wide_dest: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
        extern "system" {
            fn MoveFileExW(
                lpExistingFileName: *const u16,
                lpNewFileName: *const u16,
                dwFlags: u32,
            ) -> i32;
        }
        // dwFlags = 0 (no MOVEFILE_REPLACE_EXISTING: atomic no-replace publication)
        let res = unsafe { MoveFileExW(wide_stage.as_ptr(), wide_dest.as_ptr(), 0) };
        if res == 0 {
            let err = std::io::Error::last_os_error();
            let clean_res = pinned.clean_contents();
            let rm_res = pinned.verify_and_remove_entry();
            if let Err(cr) = clean_res {
                return Err(AetherError::Internal(format!(
                    "Publication failed ({}) and staging cleanup failed: {}",
                    err, cr
                )));
            }
            if let Err(rr) = rm_res {
                return Err(AetherError::Internal(format!(
                    "Publication failed ({}) and staging removal failed: {}",
                    err, rr
                )));
            }
            if err.raw_os_error() == Some(183) || err.raw_os_error() == Some(80) {
                return Err(AetherError::PermissionDenied(format!(
                    "Refusing to adopt directory '{:?}': destination already exists or was substituted",
                    path
                )));
            }
            return Err(AetherError::Io(err));
        }
    }

    #[cfg(unix)]
    {
        use std::ffi::CString;
        use std::os::unix::ffi::OsStrExt;
        use std::os::unix::io::{FromRawFd, OwnedFd};

        let file_name = path.file_name().ok_or_else(|| {
            AetherError::PermissionDenied(format!("Directory '{:?}' has no file name", path))
        })?;
        let c_parent = CString::new(parent.as_os_str().as_bytes())
            .map_err(|e| AetherError::Internal(format!("Invalid parent CString: {}", e)))?;
        let c_stage = CString::new(staging_nonce.as_bytes())
            .map_err(|e| AetherError::Internal(format!("Invalid stage CString: {}", e)))?;
        let c_dest = CString::new(file_name.as_bytes())
            .map_err(|e| AetherError::Internal(format!("Invalid dest CString: {}", e)))?;

        let parent_fd_raw = unsafe {
            libc::open(
                c_parent.as_ptr(),
                libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC,
            )
        };
        if parent_fd_raw < 0 {
            let clean_res = pinned.clean_contents();
            let rm_res = pinned.verify_and_remove_entry();
            if let Err(cr) = clean_res {
                return Err(AetherError::Internal(format!(
                    "Failed to open parent directory and staging cleanup failed: {}",
                    cr
                )));
            }
            if let Err(rr) = rm_res {
                return Err(AetherError::Internal(format!(
                    "Failed to open parent directory and staging removal failed: {}",
                    rr
                )));
            }
            return Err(AetherError::Io(std::io::Error::last_os_error()));
        }
        let _parent_fd = unsafe { OwnedFd::from_raw_fd(parent_fd_raw) };

        // Double check target absence in parent descriptor
        let mut target_stat: libc::stat = unsafe { std::mem::zeroed() };
        if unsafe {
            libc::fstatat(
                parent_fd_raw,
                c_dest.as_ptr(),
                &mut target_stat,
                libc::AT_SYMLINK_NOFOLLOW,
            )
        } == 0
        {
            let clean_res = pinned.clean_contents();
            let rm_res = pinned.verify_and_remove_entry();
            if let Err(cr) = clean_res {
                return Err(AetherError::Internal(format!(
                    "Destination already exists and staging cleanup failed: {}",
                    cr
                )));
            }
            if let Err(rr) = rm_res {
                return Err(AetherError::Internal(format!(
                    "Destination already exists and staging removal failed: {}",
                    rr
                )));
            }
            return Err(AetherError::PermissionDenied(format!(
                "Refusing to adopt directory '{:?}': destination already exists in parent descriptor",
                path
            )));
        }

        // Seam: Test 3 & 4 pre-publication hook on Unix
        if let Ok(guard) = PRE_PUBLICATION_HOOK.lock() {
            if let Some(ref h) = *guard {
                h(path);
            }
        }

        #[cfg(target_os = "linux")]
        {
            let res = unsafe {
                libc::syscall(
                    libc::SYS_renameat2,
                    parent_fd_raw,
                    c_stage.as_ptr(),
                    parent_fd_raw,
                    c_dest.as_ptr(),
                    1u32, // RENAME_NOREPLACE
                )
            };
            if res != 0 {
                let err = std::io::Error::last_os_error();
                let clean_res = pinned.clean_contents();
                let rm_res = pinned.verify_and_remove_entry();
                if let Err(cr) = clean_res {
                    return Err(AetherError::Internal(format!(
                        "Publication failed and staging cleanup failed: {}",
                        cr
                    )));
                }
                if let Err(rr) = rm_res {
                    return Err(AetherError::Internal(format!(
                        "Publication failed and staging removal failed: {}",
                        rr
                    )));
                }
                if err.raw_os_error() == Some(libc::EEXIST) {
                    return Err(AetherError::PermissionDenied(format!(
                        "Refusing to adopt directory '{:?}': destination already exists (renameat2 RENAME_NOREPLACE rejected replacement)",
                        path
                    )));
                }
                return Err(AetherError::Io(err));
            }
        }

        #[cfg(target_os = "macos")]
        {
            extern "C" {
                fn renameatx_np(
                    fromfd: libc::c_int,
                    from: *const libc::c_char,
                    tofd: libc::c_int,
                    to: *const libc::c_char,
                    flags: libc::c_uint,
                ) -> libc::c_int;
            }
            const RENAME_EXCL: libc::c_uint = 0x00000004;
            let res = unsafe {
                renameatx_np(
                    parent_fd_raw,
                    c_stage.as_ptr(),
                    parent_fd_raw,
                    c_dest.as_ptr(),
                    RENAME_EXCL,
                )
            };
            if res != 0 {
                let err = std::io::Error::last_os_error();
                let clean_res = pinned.clean_contents();
                let rm_res = pinned.verify_and_remove_entry();
                if let Err(cr) = clean_res {
                    return Err(AetherError::Internal(format!(
                        "Publication failed and staging cleanup failed: {}",
                        cr
                    )));
                }
                if let Err(rr) = rm_res {
                    return Err(AetherError::Internal(format!(
                        "Publication failed and staging removal failed: {}",
                        rr
                    )));
                }
                if err.raw_os_error() == Some(libc::EEXIST) {
                    return Err(AetherError::PermissionDenied(format!(
                        "Refusing to adopt directory '{:?}': destination already exists (renameatx_np RENAME_EXCL rejected replacement)",
                        path
                    )));
                }
                return Err(AetherError::Io(err));
            }
        }

        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        {
            // Fail closed: Never fall back to replacement-capable rename semantics!
            let clean_res = pinned.clean_contents();
            let rm_res = pinned.verify_and_remove_entry();
            return Err(AetherError::PermissionDenied(format!(
                "Exclusive atomic no-replace publication is unsupported on this platform; failing closed to prevent replacement races on '{:?}'",
                path
            )));
        }
    }

    // Publication succeeded: update path in PinnedDirectoryHandle
    pinned.path = path.to_path_buf();
    Ok(pinned)
}

#[cfg(windows)]
#[derive(Debug)]
pub struct PinnedDirectoryHandle {
    file: std::fs::File,
    path: PathBuf,
    identity: FileObjectIdentity,
}

#[cfg(unix)]
#[derive(Debug)]
pub struct PinnedDirectoryHandle {
    fd: std::os::unix::io::OwnedFd,
    path: PathBuf,
    identity: FileObjectIdentity,
}

#[cfg(not(any(windows, unix)))]
#[derive(Debug)]
pub struct PinnedDirectoryHandle {
    path: PathBuf,
    identity: FileObjectIdentity,
}

#[cfg(windows)]
impl PinnedDirectoryHandle {
    pub fn identity(&self) -> FileObjectIdentity {
        self.identity
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn clean_contents(&self) -> Result<()> {
        use std::os::windows::io::AsRawHandle;
        clean_directory_handle_windows_inner(
            self.file.as_raw_handle(),
            &self.path,
            self.identity,
            true,
        )
    }

    pub fn verify_and_remove_entry(self) -> Result<()> {
        use std::os::windows::io::AsRawHandle;
        #[allow(non_upper_case_globals)]
        const FileDispositionInfo: u32 = 4;

        #[repr(C)]
        #[allow(non_snake_case)]
        struct FILE_DISPOSITION_INFO {
            DeleteFile: u8,
        }
        extern "system" {
            fn SetFileInformationByHandle(
                hFile: *mut std::ffi::c_void,
                FileInformationClass: u32,
                lpFileInformation: *mut std::ffi::c_void,
                dwBufferSize: u32,
            ) -> i32;
        }

        let current_id = match query_directory_identity(&self.path) {
            Ok(id) => id,
            Err(AetherError::Io(ref e)) if e.kind() == std::io::ErrorKind::NotFound => {
                return Ok(())
            }
            Err(e) => return Err(e),
        };

        if current_id != self.identity {
            return Err(AetherError::PermissionDenied(format!(
                "Directory entry '{:?}' identity conflict: directory was replaced (expected {:?}, found {:?}); refusing removal to preserve replaced object",
                self.path, self.identity, current_id
            )));
        }

        // Test hook check (Test 7: root directory replacement between identity verification and removal)
        if let Ok(guard) = ROOT_PRE_REMOVE_HOOK.lock() {
            if let Some(ref hook) = *guard {
                hook(&self.path);
                let post_hook_id = query_directory_identity(&self.path).ok();
                if post_hook_id != Some(self.identity) {
                    return Err(AetherError::PermissionDenied(format!(
                        "Directory entry '{:?}' identity conflict: directory was replaced during removal; refusing removal to preserve replaced object",
                        self.path
                    )));
                }
            }
        }

        // Set delete disposition on the open directory handle
        let disp = FILE_DISPOSITION_INFO { DeleteFile: 1 };
        let set_res = unsafe {
            SetFileInformationByHandle(
                self.file.as_raw_handle(),
                FileDispositionInfo,
                &disp as *const _ as *mut std::ffi::c_void,
                std::mem::size_of::<FILE_DISPOSITION_INFO>() as u32,
            )
        };
        if set_res == 0 {
            return Err(AetherError::Io(std::io::Error::last_os_error()));
        }

        // Drop the open handle so Windows commits deletion of the handle-bound object
        drop(self.file);

        // Absence verification (NO pathname fallback remove_dir)
        match std::fs::symlink_metadata(&self.path) {
            Ok(_) => {
                let id_now = query_directory_identity(&self.path).ok();
                if id_now == Some(self.identity) {
                    Err(AetherError::Internal(format!(
                        "Directory '{:?}' still exists after handle deletion disposition commit",
                        self.path
                    )))
                } else {
                    Err(AetherError::PermissionDenied(format!(
                        "Directory '{:?}' was replaced during removal; refusing further action to preserve replaced object",
                        self.path
                    )))
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(AetherError::Io(e)),
        }
    }
}

#[cfg(windows)]
pub fn clean_directory_handle_windows(
    dir_path: &Path,
    expected_dir_id: FileObjectIdentity,
) -> Result<()> {
    use std::os::windows::fs::OpenOptionsExt;
    use std::os::windows::io::AsRawHandle;
    const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x02000000;
    const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x00200000;
    const GENERIC_READ: u32 = 0x80000000;
    const DELETE: u32 = 0x00010000;
    const SYNCHRONIZE: u32 = 0x00100000;
    const FILE_SHARE_READ: u32 = 0x00000001;
    const FILE_SHARE_WRITE: u32 = 0x00000002;
    const FILE_SHARE_DELETE: u32 = 0x00000004;

    let file = std::fs::OpenOptions::new()
        .access_mode(GENERIC_READ | DELETE | SYNCHRONIZE)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
        .open(dir_path)
        .map_err(AetherError::Io)?;

    clean_directory_handle_windows_inner(file.as_raw_handle(), dir_path, expected_dir_id, true)
}

#[cfg(windows)]
fn clean_directory_handle_windows_inner(
    dir_handle: *mut std::ffi::c_void,
    dir_path: &Path,
    expected_dir_id: FileObjectIdentity,
    is_root: bool,
) -> Result<()> {
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x00000400;
    const FILE_ATTRIBUTE_DIRECTORY: u32 = 0x00000010;
    const FILE_OPEN_REPARSE_POINT: u32 = 0x00200000;
    const FILE_SYNCHRONOUS_IO_NONALERT: u32 = 0x00000020;
    const GENERIC_READ: u32 = 0x80000000;
    const DELETE: u32 = 0x00010000;
    const SYNCHRONIZE: u32 = 0x00100000;
    const FILE_SHARE_READ: u32 = 0x00000001;
    const FILE_SHARE_WRITE: u32 = 0x00000002;
    const FILE_SHARE_DELETE: u32 = 0x00000004;
    const OBJ_CASE_INSENSITIVE: u32 = 0x00000040;
    const STATUS_OBJECT_NAME_NOT_FOUND: i32 = -1073741772; // 0xC0000034
    const STATUS_OBJECT_PATH_NOT_FOUND: i32 = -1073741766; // 0xC000003A

    #[allow(non_upper_case_globals)]
    const FileDispositionInfo: u32 = 4;
    #[allow(non_upper_case_globals)]
    const FileIdBothDirectoryInfo: u32 = 10;
    #[allow(non_upper_case_globals)]
    const FileIdBothDirectoryRestartInfo: u32 = 11;

    #[repr(C)]
    #[allow(non_snake_case)]
    struct FILE_DISPOSITION_INFO {
        DeleteFile: u8,
    }

    #[repr(C)]
    #[allow(non_snake_case)]
    struct BY_HANDLE_FILE_INFORMATION {
        dwFileAttributes: u32,
        ftCreationTime: [u32; 2],
        ftLastAccessTime: [u32; 2],
        ftLastWriteTime: [u32; 2],
        dwVolumeSerialNumber: u32,
        nFileSizeHigh: u32,
        nFileSizeLow: u32,
        nNumberOfLinks: u32,
        nFileIndexHigh: u32,
        nFileIndexLow: u32,
    }

    #[repr(C)]
    #[allow(non_snake_case)]
    struct FILE_ID_BOTH_DIR_INFO {
        NextEntryOffset: u32,
        FileIndex: u32,
        CreationTime: i64,
        LastAccessTime: i64,
        LastWriteTime: i64,
        ChangeTime: i64,
        EndOfFile: i64,
        AllocationSize: i64,
        FileAttributes: u32,
        FileNameLength: u32,
        EaSize: u32,
        ShortNameLength: u8,
        ShortName: [u16; 12],
        FileId: i64,
        FileName: [u16; 1],
    }

    #[repr(C)]
    struct UNICODE_STRING {
        length: u16,
        maximum_length: u16,
        buffer: *mut u16,
    }

    #[repr(C)]
    struct OBJECT_ATTRIBUTES {
        length: u32,
        root_directory: *mut std::ffi::c_void,
        object_name: *mut UNICODE_STRING,
        attributes: u32,
        security_descriptor: *mut std::ffi::c_void,
        security_quality_of_service: *mut std::ffi::c_void,
    }

    #[repr(C)]
    struct IO_STATUS_BLOCK {
        status: i32,
        information: usize,
    }

    type NtOpenFileFn = unsafe extern "system" fn(
        file_handle: *mut *mut std::ffi::c_void,
        desired_access: u32,
        object_attributes: *mut OBJECT_ATTRIBUTES,
        io_status_block: *mut IO_STATUS_BLOCK,
        share_access: u32,
        open_options: u32,
    ) -> i32;

    extern "system" {
        fn GetModuleHandleW(lpModuleName: *const u16) -> *mut std::ffi::c_void;
        fn GetProcAddress(
            hModule: *mut std::ffi::c_void,
            lpProcName: *const u8,
        ) -> *mut std::ffi::c_void;
        fn CloseHandle(hObject: *mut std::ffi::c_void) -> i32;
        fn GetFileInformationByHandle(
            hFile: *mut std::ffi::c_void,
            lpFileInformation: *mut BY_HANDLE_FILE_INFORMATION,
        ) -> i32;
        fn GetFileInformationByHandleEx(
            hFile: *mut std::ffi::c_void,
            FileInformationClass: u32,
            lpFileInformation: *mut std::ffi::c_void,
            dwBufferSize: u32,
        ) -> i32;
        fn SetFileInformationByHandle(
            hFile: *mut std::ffi::c_void,
            FileInformationClass: u32,
            lpFileInformation: *mut std::ffi::c_void,
            dwBufferSize: u32,
        ) -> i32;
    }

    struct HandleGuard(*mut std::ffi::c_void);
    impl Drop for HandleGuard {
        fn drop(&mut self) {
            if !self.0.is_null() {
                unsafe {
                    CloseHandle(self.0);
                }
            }
        }
    }

    // Hook 1: Pre-enumeration hook (root or nested)
    if is_root {
        if let Ok(guard) = ROOT_PRE_ENUMERATION_HOOK.lock() {
            if let Some(ref hook) = *guard {
                hook(dir_path);
            }
        }
    } else if let Ok(guard) = NESTED_DIR_PRE_ENUMERATION_HOOK.lock() {
        if let Some(ref hook) = *guard {
            hook(dir_path);
        }
    }

    // Verify handle identity
    let mut by_handle_info = std::mem::MaybeUninit::<BY_HANDLE_FILE_INFORMATION>::zeroed();
    let info_res = unsafe { GetFileInformationByHandle(dir_handle, by_handle_info.as_mut_ptr()) };
    if info_res == 0 {
        return Err(AetherError::Io(std::io::Error::last_os_error()));
    }
    let file_info = unsafe { by_handle_info.assume_init() };
    let current_id = FileObjectIdentity {
        volume_serial_number: file_info.dwVolumeSerialNumber as u64,
        file_index_high: file_info.nFileIndexHigh as u64,
        file_index_low: file_info.nFileIndexLow as u64,
    };
    if current_id != expected_dir_id {
        return Err(AetherError::PermissionDenied(format!(
            "Refusing cleanup: directory '{:?}' was replaced (expected {:?}, found {:?})",
            dir_path, expected_dir_id, current_id
        )));
    }

    // Dynamically resolve NtOpenFile from ntdll.dll
    let ntdll = unsafe {
        let mut ntdll_name: Vec<u16> = "ntdll.dll".encode_utf16().collect();
        ntdll_name.push(0);
        GetModuleHandleW(ntdll_name.as_ptr())
    };
    if ntdll.is_null() {
        return Err(AetherError::Internal(
            "Failed to get ntdll.dll module handle".into(),
        ));
    }
    let nt_open_file_ptr = unsafe { GetProcAddress(ntdll, c"NtOpenFile".as_ptr() as *const u8) };
    if nt_open_file_ptr.is_null() {
        return Err(AetherError::Internal(
            "Failed to find NtOpenFile in ntdll.dll".into(),
        ));
    }
    let nt_open_file: NtOpenFileFn = unsafe { std::mem::transmute(nt_open_file_ptr) };

    let mut buf = vec![0u64; 8192];
    let mut first_call = true;

    loop {
        if PARTIAL_TRAVERSAL_FAULT_INJECT.load(std::sync::atomic::Ordering::SeqCst) {
            return Err(AetherError::Io(std::io::Error::other(
                "Injected I/O error during directory cleanup traversal",
            )));
        }

        let info_class = if first_call {
            FileIdBothDirectoryRestartInfo
        } else {
            FileIdBothDirectoryInfo
        };
        first_call = false;

        let ok = unsafe {
            GetFileInformationByHandleEx(
                dir_handle,
                info_class,
                buf.as_mut_ptr() as *mut std::ffi::c_void,
                (buf.len() * 8) as u32,
            )
        };

        if ok == 0 {
            let err = std::io::Error::last_os_error();
            // ERROR_NO_MORE_FILES = 38
            if err.raw_os_error() == Some(38) || err.raw_os_error() == Some(18) {
                break;
            }
            return Err(AetherError::Io(err));
        }

        let mut curr_ptr = buf.as_ptr() as *const u8;
        loop {
            if PARTIAL_TRAVERSAL_FAULT_INJECT.load(std::sync::atomic::Ordering::SeqCst) {
                return Err(AetherError::Io(std::io::Error::other(
                    "Injected I/O error during directory cleanup traversal",
                )));
            }

            let entry_info = unsafe { &*(curr_ptr as *const FILE_ID_BOTH_DIR_INFO) };
            let name_len_chars = (entry_info.FileNameLength as usize) / 2;
            let name_slice =
                unsafe { std::slice::from_raw_parts(entry_info.FileName.as_ptr(), name_len_chars) };

            let next_offset = entry_info.NextEntryOffset as usize;

            if name_slice == [0x002e] || name_slice == [0x002e, 0x002e] {
                // Skip "." and ".."
                if next_offset == 0 {
                    break;
                }
                curr_ptr = unsafe { curr_ptr.add(next_offset) };
                continue;
            }

            let child_name = String::from_utf16_lossy(name_slice);
            let child_path = dir_path.join(&child_name);

            // Seam: Test 2 CHILD_OPEN_PRE_HOOK
            if let Ok(guard) = CHILD_OPEN_PRE_HOOK.lock() {
                if let Some(ref hook) = *guard {
                    hook(&child_path);
                }
            }

            // Open child strictly relative to dir_handle via NtOpenFile
            let mut name_wide: Vec<u16> = name_slice.to_vec();
            let mut uni_str = UNICODE_STRING {
                length: (name_wide.len() * 2) as u16,
                maximum_length: (name_wide.len() * 2) as u16,
                buffer: name_wide.as_mut_ptr(),
            };
            let mut obj_attr = OBJECT_ATTRIBUTES {
                length: std::mem::size_of::<OBJECT_ATTRIBUTES>() as u32,
                root_directory: dir_handle,
                object_name: &mut uni_str,
                attributes: OBJ_CASE_INSENSITIVE,
                security_descriptor: std::ptr::null_mut(),
                security_quality_of_service: std::ptr::null_mut(),
            };
            let mut io_status = IO_STATUS_BLOCK {
                status: 0,
                information: 0,
            };
            let mut child_raw_handle: *mut std::ffi::c_void = std::ptr::null_mut();

            let status = unsafe {
                nt_open_file(
                    &mut child_raw_handle,
                    GENERIC_READ | DELETE | SYNCHRONIZE,
                    &mut obj_attr,
                    &mut io_status,
                    FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
                    FILE_OPEN_REPARSE_POINT | FILE_SYNCHRONOUS_IO_NONALERT,
                )
            };

            if status == STATUS_OBJECT_NAME_NOT_FOUND || status == STATUS_OBJECT_PATH_NOT_FOUND {
                // Child was deleted concurrently
                if next_offset == 0 {
                    break;
                }
                curr_ptr = unsafe { curr_ptr.add(next_offset) };
                continue;
            }
            if status < 0 {
                return Err(AetherError::PermissionDenied(format!(
                    "NtOpenFile failed with NTSTATUS 0x{:08X} for relative child '{:?}' in directory '{:?}'",
                    status as u32, child_name, dir_path
                )));
            }

            let guard = HandleGuard(child_raw_handle);

            // Query child object information on child_raw_handle
            let mut child_info = std::mem::MaybeUninit::<BY_HANDLE_FILE_INFORMATION>::zeroed();
            let ginfo_res =
                unsafe { GetFileInformationByHandle(child_raw_handle, child_info.as_mut_ptr()) };
            if ginfo_res == 0 {
                return Err(AetherError::Io(std::io::Error::last_os_error()));
            }
            let child_file_info = unsafe { child_info.assume_init() };

            let is_reparse = (child_file_info.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT) != 0;
            let is_dir = (child_file_info.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY) != 0;
            let child_id = FileObjectIdentity {
                volume_serial_number: child_file_info.dwVolumeSerialNumber as u64,
                file_index_high: child_file_info.nFileIndexHigh as u64,
                file_index_low: child_file_info.nFileIndexLow as u64,
            };

            // Verify identity against enumeration FileId if non-zero
            if entry_info.FileId != 0 {
                let initial_64 = (child_id.file_index_high << 32) | child_id.file_index_low;
                if (entry_info.FileId as u64) != initial_64 {
                    return Err(AetherError::PermissionDenied(format!(
                        "Refusing deletion: child '{:?}' FileId conflict (expected 0x{:X}, handle has 0x{:X})",
                        child_path, entry_info.FileId, initial_64
                    )));
                }
            }

            let disp = FILE_DISPOSITION_INFO { DeleteFile: 1 };

            if is_reparse {
                // Symlink or NTFS Junction: do NOT follow into target!
                if let Ok(g) = SYMLINK_PRE_DELETE_HOOK.lock() {
                    if let Some(ref hook) = *g {
                        hook(&child_path);
                    }
                }
                if let Ok(g) = CHILD_ENTRY_FINAL_PRE_DESTRUCTIVE_HOOK.lock() {
                    if let Some(ref hook) = *g {
                        hook(&child_path);
                    }
                }

                // Verify opened handle is still a reparse point
                let mut re_info = std::mem::MaybeUninit::<BY_HANDLE_FILE_INFORMATION>::zeroed();
                if unsafe { GetFileInformationByHandle(child_raw_handle, re_info.as_mut_ptr()) }
                    == 0
                {
                    return Err(AetherError::Io(std::io::Error::last_os_error()));
                }
                let re_file_info = unsafe { re_info.assume_init() };
                if (re_file_info.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT) == 0 {
                    return Err(AetherError::PermissionDenied(format!(
                        "Refusing deletion: reparse point '{:?}' changed attributes",
                        child_path
                    )));
                }

                let set_res = unsafe {
                    SetFileInformationByHandle(
                        child_raw_handle,
                        FileDispositionInfo,
                        &disp as *const _ as *mut std::ffi::c_void,
                        std::mem::size_of::<FILE_DISPOSITION_INFO>() as u32,
                    )
                };
                if set_res == 0 {
                    return Err(AetherError::Io(std::io::Error::last_os_error()));
                }
                drop(guard);
            } else if is_dir {
                // Test 3 Seam: Nested directory substitution before recursive traversal begins
                if let Ok(g) = NESTED_DIR_PRE_TRAVERSAL_HOOK.lock() {
                    if let Some(ref hook) = *g {
                        hook(&child_path);
                    }
                }

                // Recurse into nested directory using the retained child directory handle!
                clean_directory_handle_windows_inner(
                    child_raw_handle,
                    &child_path,
                    child_id,
                    false, // is_root = false
                )?;

                // Test 6 & 7 Seam: Nested directory replacement before removal
                if let Ok(g) = NESTED_DIR_PRE_REMOVE_HOOK.lock() {
                    if let Some(ref hook) = *g {
                        hook(&child_path);
                    }
                }
                if let Ok(g) = CHILD_ENTRY_FINAL_PRE_DESTRUCTIVE_HOOK.lock() {
                    if let Some(ref hook) = *g {
                        hook(&child_path);
                    }
                }

                // Re-verify child_raw_handle identity
                let mut re_info = std::mem::MaybeUninit::<BY_HANDLE_FILE_INFORMATION>::zeroed();
                if unsafe { GetFileInformationByHandle(child_raw_handle, re_info.as_mut_ptr()) }
                    == 0
                {
                    return Err(AetherError::Io(std::io::Error::last_os_error()));
                }
                let re_file_info = unsafe { re_info.assume_init() };
                let current_child_id = FileObjectIdentity {
                    volume_serial_number: re_file_info.dwVolumeSerialNumber as u64,
                    file_index_high: re_file_info.nFileIndexHigh as u64,
                    file_index_low: re_file_info.nFileIndexLow as u64,
                };
                if current_child_id != child_id {
                    return Err(AetherError::PermissionDenied(format!(
                        "Refusing removal: nested directory '{:?}' handle identity changed before unlinking",
                        child_path
                    )));
                }

                let set_res = unsafe {
                    SetFileInformationByHandle(
                        child_raw_handle,
                        FileDispositionInfo,
                        &disp as *const _ as *mut std::ffi::c_void,
                        std::mem::size_of::<FILE_DISPOSITION_INFO>() as u32,
                    )
                };
                if set_res == 0 {
                    return Err(AetherError::Io(std::io::Error::last_os_error()));
                }
                drop(guard);
            } else {
                // Regular file
                if let Ok(g) = CHILD_FILE_PRE_DELETE_HOOK.lock() {
                    if let Some(ref hook) = *g {
                        hook(&child_path);
                    }
                }
                if let Ok(g) = CHILD_ENTRY_FINAL_PRE_DESTRUCTIVE_HOOK.lock() {
                    if let Some(ref hook) = *g {
                        hook(&child_path);
                    }
                }

                // Re-verify child_raw_handle identity
                let mut re_info = std::mem::MaybeUninit::<BY_HANDLE_FILE_INFORMATION>::zeroed();
                if unsafe { GetFileInformationByHandle(child_raw_handle, re_info.as_mut_ptr()) }
                    == 0
                {
                    return Err(AetherError::Io(std::io::Error::last_os_error()));
                }
                let re_file_info = unsafe { re_info.assume_init() };
                let current_file_id = FileObjectIdentity {
                    volume_serial_number: re_file_info.dwVolumeSerialNumber as u64,
                    file_index_high: re_file_info.nFileIndexHigh as u64,
                    file_index_low: re_file_info.nFileIndexLow as u64,
                };
                if current_file_id != child_id {
                    return Err(AetherError::PermissionDenied(format!(
                        "Refusing deletion: child file '{:?}' handle identity changed before unlinking",
                        child_path
                    )));
                }

                let set_res = unsafe {
                    SetFileInformationByHandle(
                        child_raw_handle,
                        FileDispositionInfo,
                        &disp as *const _ as *mut std::ffi::c_void,
                        std::mem::size_of::<FILE_DISPOSITION_INFO>() as u32,
                    )
                };
                if set_res == 0 {
                    return Err(AetherError::Io(std::io::Error::last_os_error()));
                }
                drop(guard);
            }

            if next_offset == 0 {
                break;
            }
            curr_ptr = unsafe { curr_ptr.add(next_offset) };
        }
    }

    Ok(())
}

#[cfg(windows)]
pub fn pin_created_directory(path: &Path) -> Result<PinnedDirectoryHandle> {
    let identity = query_directory_identity(path)?;
    pin_created_directory_internal(path, identity)
}

#[cfg(windows)]
fn pin_created_directory_internal(
    path: &Path,
    expected_identity: FileObjectIdentity,
) -> Result<PinnedDirectoryHandle> {
    use std::os::windows::fs::OpenOptionsExt;
    use std::os::windows::io::AsRawHandle;
    const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x02000000;
    const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x00200000;
    const FILE_SHARE_READ: u32 = 0x00000001;
    const FILE_SHARE_WRITE: u32 = 0x00000002;
    const FILE_SHARE_DELETE: u32 = 0x00000004;
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x00000400;
    const FILE_ATTRIBUTE_DIRECTORY: u32 = 0x00000010;
    const FILE_TYPE_DISK: u32 = 0x0001;
    const GENERIC_READ: u32 = 0x80000000;
    const DELETE: u32 = 0x00010000;
    const SYNCHRONIZE: u32 = 0x00100000;

    let file = std::fs::OpenOptions::new()
        .access_mode(GENERIC_READ | DELETE | SYNCHRONIZE)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)
        .map_err(AetherError::Io)?;

    #[repr(C)]
    #[allow(non_snake_case)]
    struct BY_HANDLE_FILE_INFORMATION {
        dwFileAttributes: u32,
        ftCreationTime: [u32; 2],
        ftLastAccessTime: [u32; 2],
        ftLastWriteTime: [u32; 2],
        dwVolumeSerialNumber: u32,
        nFileSizeHigh: u32,
        nFileSizeLow: u32,
        nNumberOfLinks: u32,
        nFileIndexHigh: u32,
        nFileIndexLow: u32,
    }
    extern "system" {
        fn GetFileType(hFile: *mut std::ffi::c_void) -> u32;
        fn GetFileInformationByHandle(
            hFile: *mut std::ffi::c_void,
            lpFileInformation: *mut BY_HANDLE_FILE_INFORMATION,
        ) -> i32;
        fn GetFinalPathNameByHandleW(
            hFile: *mut std::ffi::c_void,
            lpszFilePath: *mut u16,
            cchFilePath: u32,
            dwFlags: u32,
        ) -> u32;
    }

    let file_type = unsafe { GetFileType(file.as_raw_handle()) };
    if file_type != FILE_TYPE_DISK {
        return Err(AetherError::PermissionDenied(format!(
            "Object '{}' is not a standard disk directory (file type: 0x{:x})",
            path.display(),
            file_type
        )));
    }

    let mut by_handle_info = std::mem::MaybeUninit::<BY_HANDLE_FILE_INFORMATION>::zeroed();
    let info_res =
        unsafe { GetFileInformationByHandle(file.as_raw_handle(), by_handle_info.as_mut_ptr()) };
    if info_res == 0 {
        return Err(AetherError::Io(std::io::Error::last_os_error()));
    }
    let file_info = unsafe { by_handle_info.assume_init() };

    let is_reparse = (file_info.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT) != 0;
    let is_dir = (file_info.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY) != 0;
    if is_reparse {
        return Err(AetherError::PermissionDenied(format!(
            "Object '{}' is a reparse point (attributes: 0x{:x}); refusing directory handle pinning",
            path.display(),
            file_info.dwFileAttributes
        )));
    }
    if !is_dir {
        return Err(AetherError::PermissionDenied(format!(
            "Object '{}' is not a directory (attributes: 0x{:x}); refusing directory handle pinning",
            path.display(),
            file_info.dwFileAttributes
        )));
    }

    // Path verification: GetFinalPathNameByHandleW
    let mut buf = vec![0u16; 1024];
    let len = unsafe {
        GetFinalPathNameByHandleW(file.as_raw_handle(), buf.as_mut_ptr(), buf.len() as u32, 0)
    };
    if len > 0 && (len as usize) < buf.len() {
        let final_path_str = String::from_utf16_lossy(&buf[..len as usize]);
        let normalized = final_path_str.trim_start_matches(r"\\?\");
        if let Ok(canon) = std::fs::canonicalize(path) {
            let canon_str = canon.to_string_lossy();
            let canon_norm = canon_str.trim_start_matches(r"\\?\");
            if !normalized.eq_ignore_ascii_case(canon_norm) {
                return Err(AetherError::PermissionDenied(format!(
                    "Path verification failed for directory '{}': normalized target '{}' does not match '{}'",
                    path.display(),
                    normalized,
                    canon_norm
                )));
            }
        }
    }

    // A newly created snapshot directory must be empty. If it contains entries,
    // an adversary placed a replacement directory with files before ownership capture.
    let mut entries = std::fs::read_dir(path).map_err(AetherError::Io)?;
    if entries.next().is_some() {
        return Err(AetherError::PermissionDenied(format!(
            "Directory '{}' is not empty; refusing to pin non-empty directory as newly created snapshot directory",
            path.display()
        )));
    }

    let handle_identity = FileObjectIdentity {
        volume_serial_number: file_info.dwVolumeSerialNumber as u64,
        file_index_high: file_info.nFileIndexHigh as u64,
        file_index_low: file_info.nFileIndexLow as u64,
    };
    if handle_identity != expected_identity {
        return Err(AetherError::PermissionDenied(format!(
            "Identity mismatch for directory handle '{}': handle has {:?}, expected {:?}",
            path.display(),
            handle_identity,
            expected_identity
        )));
    }

    Ok(PinnedDirectoryHandle {
        file,
        path: path.to_path_buf(),
        identity: handle_identity,
    })
}

#[cfg(unix)]
impl PinnedDirectoryHandle {
    pub fn identity(&self) -> FileObjectIdentity {
        self.identity
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn clean_contents(&self) -> Result<()> {
        use std::os::unix::io::AsRawFd;
        clean_dir_descriptor(self.fd.as_raw_fd(), &self.path)
    }

    pub fn verify_and_remove_entry(self) -> Result<()> {
        use std::ffi::CString;
        use std::os::unix::ffi::OsStrExt;
        use std::os::unix::io::{FromRawFd, OwnedFd};

        let parent_path = self.path.parent().ok_or_else(|| {
            AetherError::PermissionDenied(format!("Directory '{:?}' has no parent", self.path))
        })?;
        let file_name = self.path.file_name().ok_or_else(|| {
            AetherError::PermissionDenied(format!("Directory '{:?}' has no file name", self.path))
        })?;

        let c_parent = CString::new(parent_path.as_os_str().as_bytes())
            .map_err(|e| AetherError::Internal(format!("Invalid parent CString: {}", e)))?;
        let c_name = CString::new(file_name.as_bytes())
            .map_err(|e| AetherError::Internal(format!("Invalid file name CString: {}", e)))?;

        let parent_fd_raw = unsafe {
            libc::open(
                c_parent.as_ptr(),
                libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC,
            )
        };
        if parent_fd_raw < 0 {
            return Err(AetherError::Io(std::io::Error::last_os_error()));
        }
        let _parent_fd = unsafe { OwnedFd::from_raw_fd(parent_fd_raw) };

        let mut entry_stat: libc::stat = unsafe { std::mem::zeroed() };
        let res = unsafe {
            libc::fstatat(
                parent_fd_raw,
                c_name.as_ptr(),
                &mut entry_stat,
                libc::AT_SYMLINK_NOFOLLOW,
            )
        };
        if res != 0 {
            let err = std::io::Error::last_os_error();
            if err.kind() == std::io::ErrorKind::NotFound {
                return Ok(());
            }
            return Err(AetherError::Io(err));
        }

        let entry_ino = entry_stat.st_ino as u64;
        let entry_dev = entry_stat.st_dev as u64;
        let expected_ino = (self.identity.file_index_high << 32) | self.identity.file_index_low;
        let expected_dev = self.identity.volume_serial_number;

        if entry_dev != expected_dev || entry_ino != expected_ino {
            return Err(AetherError::PermissionDenied(format!(
                "Directory entry '{:?}' identity conflict: directory was replaced (expected dev {} ino {}, found dev {} ino {}); refusing unlinking to preserve replaced object",
                self.path, expected_dev, expected_ino, entry_dev, entry_ino
            )));
        }

        // Test 7 Seam: Root directory replacement before final unlinking
        if let Ok(guard) = ROOT_PRE_REMOVE_HOOK.lock() {
            if let Some(ref hook) = *guard {
                hook(&self.path);
            }
        }

        let mut entry_stat2: libc::stat = unsafe { std::mem::zeroed() };
        let res2 = unsafe {
            libc::fstatat(
                parent_fd_raw,
                c_name.as_ptr(),
                &mut entry_stat2,
                libc::AT_SYMLINK_NOFOLLOW,
            )
        };
        if res2 != 0 {
            let err = std::io::Error::last_os_error();
            if err.kind() == std::io::ErrorKind::NotFound {
                return Ok(());
            }
            return Err(AetherError::Io(err));
        }

        let entry_ino2 = entry_stat2.st_ino as u64;
        let entry_dev2 = entry_stat2.st_dev as u64;
        if entry_dev2 != expected_dev || entry_ino2 != expected_ino {
            return Err(AetherError::PermissionDenied(format!(
                "Directory entry '{:?}' identity conflict: directory was replaced during final removal; refusing unlinking to preserve replaced object",
                self.path
            )));
        }

        let temp_unlink_name = CString::new(format!(
            ".to_del_{}_{}",
            uuid::Uuid::new_v4().simple(),
            file_name.to_string_lossy()
        ))
        .map_err(|e| AetherError::Internal(format!("Invalid temp CString: {}", e)))?;

        if FAULT_INJECT_TEMP_RENAME_FAILURE.load(std::sync::atomic::Ordering::SeqCst) {
            return Err(AetherError::PermissionDenied(format!(
                "Injected failure during root directory temporary rename on '{:?}'",
                self.path
            )));
        }

        if unsafe {
            libc::renameat(
                parent_fd_raw,
                c_name.as_ptr(),
                parent_fd_raw,
                temp_unlink_name.as_ptr(),
            )
        } == 0
        {
            let temp_child_path = self
                .path
                .parent()
                .map(|p| p.join(temp_unlink_name.to_string_lossy().as_ref()))
                .unwrap_or_else(|| PathBuf::from(temp_unlink_name.to_string_lossy().as_ref()));
            if let Ok(guard) = TEMP_ENTRY_PRE_UNLINK_HOOK.lock() {
                if let Some(ref hook) = *guard {
                    hook(&temp_child_path);
                }
            }

            let mut temp_stat: libc::stat = unsafe { std::mem::zeroed() };
            if unsafe {
                libc::fstatat(
                    parent_fd_raw,
                    temp_unlink_name.as_ptr(),
                    &mut temp_stat,
                    libc::AT_SYMLINK_NOFOLLOW,
                )
            } == 0
                && (temp_stat.st_ino as u64) == expected_ino
                && (temp_stat.st_dev as u64) == expected_dev
            {
                if unsafe {
                    libc::unlinkat(parent_fd_raw, temp_unlink_name.as_ptr(), libc::AT_REMOVEDIR)
                } != 0
                {
                    let err = std::io::Error::last_os_error();
                    if err.kind() != std::io::ErrorKind::NotFound {
                        return Err(AetherError::Io(err));
                    }
                }
            } else {
                unsafe {
                    libc::renameat(
                        parent_fd_raw,
                        temp_unlink_name.as_ptr(),
                        parent_fd_raw,
                        c_name.as_ptr(),
                    )
                };
                return Err(AetherError::PermissionDenied(format!(
                    "Directory entry '{:?}' identity conflict during rename",
                    self.path
                )));
            }
        } else {
            return Err(AetherError::PermissionDenied(format!(
                "Failed to isolate root directory '{:?}' via temporary rename before unlinking; refusing deletion to avoid unverified pathname unlinking",
                self.path
            )));
        }

        Ok(())
    }
}

#[cfg(unix)]
pub fn pin_created_directory(path: &Path) -> Result<PinnedDirectoryHandle> {
    let identity = query_directory_identity(path)?;
    pin_created_directory_internal(path, identity)
}

#[cfg(unix)]
fn pin_created_directory_internal(
    path: &Path,
    expected_identity: FileObjectIdentity,
) -> Result<PinnedDirectoryHandle> {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;
    use std::os::unix::io::{FromRawFd, OwnedFd};

    let parent_path = path.parent().ok_or_else(|| {
        AetherError::PermissionDenied(format!("Snapshot directory '{:?}' has no parent", path))
    })?;
    let file_name = path.file_name().ok_or_else(|| {
        AetherError::PermissionDenied(format!("Snapshot directory '{:?}' has no file name", path))
    })?;

    let c_path = CString::new(path.as_os_str().as_bytes())
        .map_err(|e| AetherError::Internal(format!("Invalid path CString: {}", e)))?;
    let c_parent = CString::new(parent_path.as_os_str().as_bytes())
        .map_err(|e| AetherError::Internal(format!("Invalid parent path CString: {}", e)))?;
    let c_name = CString::new(file_name.as_bytes())
        .map_err(|e| AetherError::Internal(format!("Invalid file name CString: {}", e)))?;

    let dir_fd_raw = unsafe {
        libc::open(
            c_path.as_ptr(),
            libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC | libc::O_NOFOLLOW,
        )
    };
    if dir_fd_raw < 0 {
        return Err(AetherError::Io(std::io::Error::last_os_error()));
    }
    let fd = unsafe { OwnedFd::from_raw_fd(dir_fd_raw) };

    let mut dir_stat: libc::stat = unsafe { std::mem::zeroed() };
    if unsafe { libc::fstat(dir_fd_raw, &mut dir_stat) } != 0 {
        return Err(AetherError::Io(std::io::Error::last_os_error()));
    }

    if (dir_stat.st_mode & libc::S_IFMT) != libc::S_IFDIR {
        return Err(AetherError::PermissionDenied(format!(
            "Object '{}' is not a directory; refusing directory descriptor pinning",
            path.display()
        )));
    }

    let parent_fd_raw = unsafe {
        libc::open(
            c_parent.as_ptr(),
            libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC,
        )
    };
    if parent_fd_raw < 0 {
        return Err(AetherError::Io(std::io::Error::last_os_error()));
    }
    let _parent_fd = unsafe { OwnedFd::from_raw_fd(parent_fd_raw) };

    let mut parent_entry_stat: libc::stat = unsafe { std::mem::zeroed() };
    let statat_res = unsafe {
        libc::fstatat(
            parent_fd_raw,
            c_name.as_ptr(),
            &mut parent_entry_stat,
            libc::AT_SYMLINK_NOFOLLOW,
        )
    };
    if statat_res != 0 {
        return Err(AetherError::Io(std::io::Error::last_os_error()));
    }

    if (parent_entry_stat.st_mode & libc::S_IFMT) == libc::S_IFLNK {
        return Err(AetherError::PermissionDenied(format!(
            "Directory entry '{}' in parent is a symlink; refusing directory descriptor pinning",
            path.display()
        )));
    }

    if dir_stat.st_dev != parent_entry_stat.st_dev || dir_stat.st_ino != parent_entry_stat.st_ino {
        return Err(AetherError::PermissionDenied(format!(
            "Relationship mismatch for directory '{}': parent entry points to dev {} ino {}, but open descriptor is dev {} ino {}",
            path.display(),
            parent_entry_stat.st_dev,
            parent_entry_stat.st_ino,
            dir_stat.st_dev,
            dir_stat.st_ino
        )));
    }

    // A newly created snapshot directory must be empty
    let mut entries = std::fs::read_dir(path).map_err(AetherError::Io)?;
    if entries.next().is_some() {
        return Err(AetherError::PermissionDenied(format!(
            "Directory '{}' is not empty; refusing to pin non-empty directory as newly created snapshot directory",
            path.display()
        )));
    }

    let ino = dir_stat.st_ino as u64;
    let handle_identity = FileObjectIdentity {
        volume_serial_number: dir_stat.st_dev as u64,
        file_index_high: ino >> 32,
        file_index_low: ino & 0xFFFFFFFF,
    };
    if handle_identity != expected_identity {
        return Err(AetherError::PermissionDenied(format!(
            "Identity mismatch for directory descriptor '{}': descriptor has {:?}, expected {:?}",
            path.display(),
            handle_identity,
            expected_identity
        )));
    }

    Ok(PinnedDirectoryHandle {
        fd,
        path: path.to_path_buf(),
        identity: handle_identity,
    })
}

#[cfg(unix)]
fn clean_dir_descriptor(dir_fd: libc::c_int, dir_path: &Path) -> Result<()> {
    use std::ffi::{CStr, CString};
    use std::os::unix::io::{AsRawFd, FromRawFd, OwnedFd};

    let dup_fd = unsafe { libc::dup(dir_fd) };
    if dup_fd < 0 {
        return Err(AetherError::Io(std::io::Error::last_os_error()));
    }

    let dir_ptr = unsafe { libc::fdopendir(dup_fd) };
    if dir_ptr.is_null() {
        unsafe {
            libc::close(dup_fd);
        }
        return Err(AetherError::Io(std::io::Error::last_os_error()));
    }

    loop {
        if PARTIAL_TRAVERSAL_FAULT_INJECT.load(std::sync::atomic::Ordering::SeqCst) {
            unsafe {
                libc::closedir(dir_ptr);
            }
            return Err(AetherError::Io(std::io::Error::other(
                "Injected I/O error during directory cleanup traversal",
            )));
        }

        #[cfg(target_os = "linux")]
        unsafe {
            *libc::__errno_location() = 0;
        }
        #[cfg(target_os = "macos")]
        unsafe {
            *libc::__error() = 0;
        }

        let entry_ptr = unsafe { libc::readdir(dir_ptr) };
        if entry_ptr.is_null() {
            let err = std::io::Error::last_os_error();
            if let Some(code) = err.raw_os_error() {
                if code != 0 {
                    unsafe {
                        libc::closedir(dir_ptr);
                    }
                    return Err(AetherError::Io(err));
                }
            }
            break;
        }

        let entry = unsafe { &*entry_ptr };
        let name_bytes = unsafe { CStr::from_ptr(entry.d_name.as_ptr()) }.to_bytes();
        if name_bytes == b"." || name_bytes == b".." {
            continue;
        }

        let c_name = entry.d_name.as_ptr();
        let name_str = String::from_utf8_lossy(name_bytes);
        let child_path = dir_path.join(name_str.as_ref());

        let mut child_stat: libc::stat = unsafe { std::mem::zeroed() };
        if unsafe { libc::fstatat(dir_fd, c_name, &mut child_stat, libc::AT_SYMLINK_NOFOLLOW) } != 0
        {
            let err = std::io::Error::last_os_error();
            if err.kind() == std::io::ErrorKind::NotFound {
                continue;
            }
            unsafe {
                libc::closedir(dir_ptr);
            }
            return Err(AetherError::Io(err));
        }

        let is_symlink = (child_stat.st_mode & libc::S_IFMT) == libc::S_IFLNK;
        let is_dir = (child_stat.st_mode & libc::S_IFMT) == libc::S_IFDIR;

        if is_symlink {
            if let Ok(guard) = SYMLINK_PRE_DELETE_HOOK.lock() {
                if let Some(ref hook) = *guard {
                    hook(&child_path);
                }
            }

            let mut stat_verify: libc::stat = unsafe { std::mem::zeroed() };
            if unsafe { libc::fstatat(dir_fd, c_name, &mut stat_verify, libc::AT_SYMLINK_NOFOLLOW) }
                != 0
            {
                let err = std::io::Error::last_os_error();
                if err.kind() == std::io::ErrorKind::NotFound {
                    continue;
                }
                unsafe {
                    libc::closedir(dir_ptr);
                }
                return Err(AetherError::Io(err));
            }

            if (stat_verify.st_mode & libc::S_IFMT) != libc::S_IFLNK
                || stat_verify.st_ino != child_stat.st_ino
            {
                unsafe {
                    libc::closedir(dir_ptr);
                }
                return Err(AetherError::PermissionDenied(format!(
                    "Refusing deletion: symlink '{:?}' was replaced before unlinking",
                    child_path
                )));
            }

            if let Ok(guard) = CHILD_ENTRY_FINAL_PRE_DESTRUCTIVE_HOOK.lock() {
                if let Some(ref hook) = *guard {
                    hook(&child_path);
                }
            }

            if FAULT_INJECT_TEMP_RENAME_FAILURE.load(std::sync::atomic::Ordering::SeqCst) {
                unsafe {
                    libc::closedir(dir_ptr);
                }
                return Err(AetherError::PermissionDenied(format!(
                    "Injected failure during symlink temporary rename on '{:?}'",
                    child_path
                )));
            }

            let temp_unlink_name = CString::new(format!(
                ".to_del_{}_{}",
                uuid::Uuid::new_v4().simple(),
                name_str
            ))
            .map_err(|e| AetherError::Internal(format!("Invalid temp CString: {}", e)))?;

            if unsafe { libc::renameat(dir_fd, c_name, dir_fd, temp_unlink_name.as_ptr()) } == 0 {
                let temp_child_path = dir_path.join(temp_unlink_name.to_string_lossy().as_ref());
                if let Ok(guard) = TEMP_ENTRY_PRE_UNLINK_HOOK.lock() {
                    if let Some(ref hook) = *guard {
                        hook(&temp_child_path);
                    }
                }

                let mut temp_stat: libc::stat = unsafe { std::mem::zeroed() };
                if unsafe {
                    libc::fstatat(
                        dir_fd,
                        temp_unlink_name.as_ptr(),
                        &mut temp_stat,
                        libc::AT_SYMLINK_NOFOLLOW,
                    )
                } == 0
                    && (temp_stat.st_mode & libc::S_IFMT) == libc::S_IFLNK
                    && temp_stat.st_ino == child_stat.st_ino
                {
                    if unsafe { libc::unlinkat(dir_fd, temp_unlink_name.as_ptr(), 0) } != 0 {
                        let err = std::io::Error::last_os_error();
                        if err.kind() != std::io::ErrorKind::NotFound {
                            unsafe {
                                libc::closedir(dir_ptr);
                            }
                            return Err(AetherError::Io(err));
                        }
                    }
                } else {
                    unsafe { libc::renameat(dir_fd, temp_unlink_name.as_ptr(), dir_fd, c_name) };
                    unsafe {
                        libc::closedir(dir_ptr);
                    }
                    return Err(AetherError::PermissionDenied(format!(
                        "Refusing deletion: symlink '{:?}' identity conflict during rename",
                        child_path
                    )));
                }
            } else {
                unsafe {
                    libc::closedir(dir_ptr);
                }
                return Err(AetherError::PermissionDenied(format!(
                    "Failed to isolate symlink '{:?}' via temporary rename before unlinking; refusing deletion to avoid unverified pathname unlinking",
                    child_path
                )));
            }
        } else if is_dir {
            // Test 2 Seam: Replacement between initial child inspection and openat
            if let Ok(guard) = CHILD_OPEN_PRE_HOOK.lock() {
                if let Some(ref hook) = *guard {
                    hook(&child_path);
                }
            }

            let child_fd_raw = unsafe {
                libc::openat(
                    dir_fd,
                    c_name,
                    libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC | libc::O_NOFOLLOW,
                )
            };
            if child_fd_raw < 0 {
                let err = std::io::Error::last_os_error();
                if err.kind() == std::io::ErrorKind::NotFound {
                    continue;
                }
                unsafe {
                    libc::closedir(dir_ptr);
                }
                return Err(AetherError::Io(err));
            }
            let child_fd = unsafe { OwnedFd::from_raw_fd(child_fd_raw) };

            // Step 3: fstat() the opened descriptor
            let mut opened_stat: libc::stat = unsafe { std::mem::zeroed() };
            if unsafe { libc::fstat(child_fd.as_raw_fd(), &mut opened_stat) } != 0 {
                let err = std::io::Error::last_os_error();
                unsafe {
                    libc::closedir(dir_ptr);
                }
                return Err(AetherError::Io(err));
            }

            // Step 4: Verify opened descriptor matches the intended object
            if opened_stat.st_ino != child_stat.st_ino || opened_stat.st_dev != child_stat.st_dev {
                unsafe {
                    libc::closedir(dir_ptr);
                }
                return Err(AetherError::PermissionDenied(format!(
                    "Refusing recursive traversal: child directory '{:?}' was replaced before openat()",
                    child_path
                )));
            }

            // Test 3 Seam: Nested directory substitution before recursive traversal begins
            if let Ok(guard) = NESTED_DIR_PRE_TRAVERSAL_HOOK.lock() {
                if let Some(ref hook) = *guard {
                    hook(&child_path);
                }
            }

            let mut pre_traverse_stat: libc::stat = unsafe { std::mem::zeroed() };
            if unsafe {
                libc::fstatat(
                    dir_fd,
                    c_name,
                    &mut pre_traverse_stat,
                    libc::AT_SYMLINK_NOFOLLOW,
                )
            } != 0
                || pre_traverse_stat.st_ino != child_stat.st_ino
                || pre_traverse_stat.st_dev != child_stat.st_dev
            {
                unsafe {
                    libc::closedir(dir_ptr);
                }
                return Err(AetherError::PermissionDenied(format!(
                    "Refusing recursive traversal: child directory '{:?}' was replaced before traversal started",
                    child_path
                )));
            }

            // Step 5: Recurse into that descriptor
            clean_dir_descriptor(child_fd.as_raw_fd(), &child_path)?;

            // Test 6 & 7 Seam: Nested directory replacement before removal
            if let Ok(guard) = NESTED_DIR_PRE_REMOVE_HOOK.lock() {
                if let Some(ref hook) = *guard {
                    hook(&child_path);
                }
            }

            let mut stat_verify: libc::stat = unsafe { std::mem::zeroed() };
            if unsafe { libc::fstatat(dir_fd, c_name, &mut stat_verify, libc::AT_SYMLINK_NOFOLLOW) }
                != 0
            {
                let err = std::io::Error::last_os_error();
                if err.kind() == std::io::ErrorKind::NotFound {
                    continue;
                }
                unsafe {
                    libc::closedir(dir_ptr);
                }
                return Err(AetherError::Io(err));
            }

            if stat_verify.st_ino != child_stat.st_ino || stat_verify.st_dev != child_stat.st_dev {
                unsafe {
                    libc::closedir(dir_ptr);
                }
                return Err(AetherError::PermissionDenied(format!(
                    "Refusing removal: nested directory '{:?}' was replaced before unlinking",
                    child_path
                )));
            }

            if let Ok(guard) = CHILD_ENTRY_FINAL_PRE_DESTRUCTIVE_HOOK.lock() {
                if let Some(ref hook) = *guard {
                    hook(&child_path);
                }
            }

            drop(child_fd);

            if FAULT_INJECT_TEMP_RENAME_FAILURE.load(std::sync::atomic::Ordering::SeqCst) {
                unsafe {
                    libc::closedir(dir_ptr);
                }
                return Err(AetherError::PermissionDenied(format!(
                    "Injected failure during nested directory temporary rename on '{:?}'",
                    child_path
                )));
            }

            let temp_unlink_name = CString::new(format!(
                ".to_del_{}_{}",
                uuid::Uuid::new_v4().simple(),
                name_str
            ))
            .map_err(|e| AetherError::Internal(format!("Invalid temp CString: {}", e)))?;

            if unsafe { libc::renameat(dir_fd, c_name, dir_fd, temp_unlink_name.as_ptr()) } == 0 {
                let temp_child_path = dir_path.join(temp_unlink_name.to_string_lossy().as_ref());
                if let Ok(guard) = TEMP_ENTRY_PRE_UNLINK_HOOK.lock() {
                    if let Some(ref hook) = *guard {
                        hook(&temp_child_path);
                    }
                }

                let mut temp_stat: libc::stat = unsafe { std::mem::zeroed() };
                if unsafe {
                    libc::fstatat(
                        dir_fd,
                        temp_unlink_name.as_ptr(),
                        &mut temp_stat,
                        libc::AT_SYMLINK_NOFOLLOW,
                    )
                } == 0
                    && temp_stat.st_ino == child_stat.st_ino
                    && temp_stat.st_dev == child_stat.st_dev
                {
                    if unsafe {
                        libc::unlinkat(dir_fd, temp_unlink_name.as_ptr(), libc::AT_REMOVEDIR)
                    } != 0
                    {
                        let err = std::io::Error::last_os_error();
                        if err.kind() != std::io::ErrorKind::NotFound {
                            unsafe {
                                libc::closedir(dir_ptr);
                            }
                            return Err(AetherError::Io(err));
                        }
                    }
                } else {
                    unsafe { libc::renameat(dir_fd, temp_unlink_name.as_ptr(), dir_fd, c_name) };
                    unsafe {
                        libc::closedir(dir_ptr);
                    }
                    return Err(AetherError::PermissionDenied(format!(
                        "Refusing removal: nested directory '{:?}' identity conflict during rename",
                        child_path
                    )));
                }
            } else {
                unsafe {
                    libc::closedir(dir_ptr);
                }
                return Err(AetherError::PermissionDenied(format!(
                    "Failed to isolate nested directory '{:?}' via temporary rename before unlinking; refusing removal to avoid unverified pathname unlinking",
                    child_path
                )));
            }
        } else {
            // Test 2 Seam: Replacement between initial child inspection and openat
            if let Ok(guard) = CHILD_OPEN_PRE_HOOK.lock() {
                if let Some(ref hook) = *guard {
                    hook(&child_path);
                }
            }

            let child_file_raw = unsafe {
                libc::openat(
                    dir_fd,
                    c_name,
                    libc::O_RDONLY | libc::O_CLOEXEC | libc::O_NOFOLLOW,
                )
            };
            if child_file_raw < 0 {
                let err = std::io::Error::last_os_error();
                if err.kind() == std::io::ErrorKind::NotFound {
                    continue;
                }
                unsafe {
                    libc::closedir(dir_ptr);
                }
                return Err(AetherError::Io(err));
            }
            let child_file_fd = unsafe { OwnedFd::from_raw_fd(child_file_raw) };

            // Step 3: fstat() the opened file descriptor
            let mut opened_stat: libc::stat = unsafe { std::mem::zeroed() };
            if unsafe { libc::fstat(child_file_fd.as_raw_fd(), &mut opened_stat) } != 0 {
                let err = std::io::Error::last_os_error();
                unsafe {
                    libc::closedir(dir_ptr);
                }
                return Err(AetherError::Io(err));
            }

            // Step 4: Verify opened descriptor matches initial metadata
            if opened_stat.st_ino != child_stat.st_ino || opened_stat.st_dev != child_stat.st_dev {
                unsafe {
                    libc::closedir(dir_ptr);
                }
                return Err(AetherError::PermissionDenied(format!(
                    "Refusing deletion: child file '{:?}' was replaced before openat()",
                    child_path
                )));
            }

            if let Ok(guard) = CHILD_FILE_PRE_DELETE_HOOK.lock() {
                if let Some(ref hook) = *guard {
                    hook(&child_path);
                }
            }

            let mut stat_verify: libc::stat = unsafe { std::mem::zeroed() };
            if unsafe { libc::fstatat(dir_fd, c_name, &mut stat_verify, libc::AT_SYMLINK_NOFOLLOW) }
                != 0
            {
                let err = std::io::Error::last_os_error();
                if err.kind() == std::io::ErrorKind::NotFound {
                    continue;
                }
                unsafe {
                    libc::closedir(dir_ptr);
                }
                return Err(AetherError::Io(err));
            }

            if stat_verify.st_ino != child_stat.st_ino || stat_verify.st_dev != child_stat.st_dev {
                unsafe {
                    libc::closedir(dir_ptr);
                }
                return Err(AetherError::PermissionDenied(format!(
                    "Refusing deletion: child file '{:?}' was replaced before unlinking",
                    child_path
                )));
            }

            if let Ok(guard) = CHILD_ENTRY_FINAL_PRE_DESTRUCTIVE_HOOK.lock() {
                if let Some(ref hook) = *guard {
                    hook(&child_path);
                }
            }

            drop(child_file_fd);

            if FAULT_INJECT_TEMP_RENAME_FAILURE.load(std::sync::atomic::Ordering::SeqCst) {
                unsafe {
                    libc::closedir(dir_ptr);
                }
                return Err(AetherError::PermissionDenied(format!(
                    "Injected failure during child file temporary rename on '{:?}'",
                    child_path
                )));
            }

            let temp_unlink_name = CString::new(format!(
                ".to_del_{}_{}",
                uuid::Uuid::new_v4().simple(),
                name_str
            ))
            .map_err(|e| AetherError::Internal(format!("Invalid temp CString: {}", e)))?;

            if unsafe { libc::renameat(dir_fd, c_name, dir_fd, temp_unlink_name.as_ptr()) } == 0 {
                let temp_child_path = dir_path.join(temp_unlink_name.to_string_lossy().as_ref());
                if let Ok(guard) = TEMP_ENTRY_PRE_UNLINK_HOOK.lock() {
                    if let Some(ref hook) = *guard {
                        hook(&temp_child_path);
                    }
                }

                let mut temp_stat: libc::stat = unsafe { std::mem::zeroed() };
                if unsafe {
                    libc::fstatat(
                        dir_fd,
                        temp_unlink_name.as_ptr(),
                        &mut temp_stat,
                        libc::AT_SYMLINK_NOFOLLOW,
                    )
                } == 0
                    && temp_stat.st_ino == child_stat.st_ino
                    && temp_stat.st_dev == child_stat.st_dev
                {
                    if unsafe { libc::unlinkat(dir_fd, temp_unlink_name.as_ptr(), 0) } != 0 {
                        let err = std::io::Error::last_os_error();
                        if err.kind() != std::io::ErrorKind::NotFound {
                            unsafe {
                                libc::closedir(dir_ptr);
                            }
                            return Err(AetherError::Io(err));
                        }
                    }
                } else {
                    unsafe { libc::renameat(dir_fd, temp_unlink_name.as_ptr(), dir_fd, c_name) };
                    unsafe {
                        libc::closedir(dir_ptr);
                    }
                    return Err(AetherError::PermissionDenied(format!(
                        "Refusing deletion: child file '{:?}' identity conflict during rename",
                        child_path
                    )));
                }
            } else {
                unsafe {
                    libc::closedir(dir_ptr);
                }
                return Err(AetherError::PermissionDenied(format!(
                    "Failed to isolate child file '{:?}' via temporary rename before unlinking; refusing deletion to avoid unverified pathname unlinking",
                    child_path
                )));
            }
        }
    }

    unsafe {
        libc::closedir(dir_ptr);
    }
    Ok(())
}

#[cfg(not(any(windows, unix)))]
impl PinnedDirectoryHandle {
    pub fn identity(&self) -> FileObjectIdentity {
        self.identity
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn clean_contents(&self) -> Result<()> {
        Err(AetherError::PermissionDenied(
            "Directory cleanup not supported on this platform".into(),
        ))
    }

    pub fn verify_and_remove_entry(self) -> Result<()> {
        Err(AetherError::PermissionDenied(
            "Directory removal not supported on this platform".into(),
        ))
    }
}

#[cfg(not(any(windows, unix)))]
pub fn pin_created_directory(path: &Path) -> Result<PinnedDirectoryHandle> {
    Err(AetherError::PermissionDenied(format!(
        "Directory pinning and race-free cleanup are not supported on this platform: refusing '{:?}'",
        path
    )))
}

#[cfg(not(any(windows, unix)))]
fn pin_created_directory_internal(
    path: &Path,
    _expected_identity: FileObjectIdentity,
) -> Result<PinnedDirectoryHandle> {
    Err(AetherError::PermissionDenied(format!(
        "Directory pinning and race-free cleanup are not supported on this platform: refusing '{:?}'",
        path
    )))
}

#[cfg(windows)]
pub fn remove_file_windows_exclusive(
    path: &Path,
    expected_hash: &str,
    expected_identity: Option<FileObjectIdentity>,
    destination_path: Option<&Path>,
) -> Result<()> {
    use sha2::{Digest, Sha256};
    use std::io::Read;
    use std::os::windows::fs::OpenOptionsExt;
    use std::os::windows::io::AsRawHandle;

    const DELETE: u32 = 0x00010000;
    const GENERIC_READ: u32 = 0x80000000;
    const FILE_SHARE_READ: u32 = 0x00000001;
    const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x00200000;
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x00000400;
    const FILE_ATTRIBUTE_DIRECTORY: u32 = 0x00000010;
    const FILE_TYPE_DISK: u32 = 0x0001;

    // Hook executed immediately before exclusive open (CHK-209, CHK-211)
    if let Some(ref hook) = get_hook_windows_pre_exclusive_open() {
        hook(path);
    }

    // Open without following reparse points (symlinks, junctions, mount points),
    // and with FILE_SHARE_READ (denies any concurrent write or delete, while allowing read-only inspection
    // such as query_file_object_identity on hard links pointing to the same file object).
    let mut file = std::fs::OpenOptions::new()
        .read(true)
        .access_mode(GENERIC_READ | DELETE)
        .share_mode(FILE_SHARE_READ)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)
        .map_err(AetherError::Io)?;

    // Inspect opened handle to verify object identity before any read or delete operations
    #[repr(C)]
    #[allow(non_snake_case)]
    struct BY_HANDLE_FILE_INFORMATION {
        dwFileAttributes: u32,
        ftCreationTime: [u32; 2],
        ftLastAccessTime: [u32; 2],
        ftLastWriteTime: [u32; 2],
        dwVolumeSerialNumber: u32,
        nFileSizeHigh: u32,
        nFileSizeLow: u32,
        nNumberOfLinks: u32,
        nFileIndexHigh: u32,
        nFileIndexLow: u32,
    }
    extern "system" {
        fn GetFileType(hFile: *mut std::ffi::c_void) -> u32;
        fn GetFileInformationByHandle(
            hFile: *mut std::ffi::c_void,
            lpFileInformation: *mut BY_HANDLE_FILE_INFORMATION,
        ) -> i32;
    }

    let file_type = unsafe { GetFileType(file.as_raw_handle()) };
    if file_type != FILE_TYPE_DISK {
        return Err(AetherError::PermissionDenied(format!(
            "Security rejection: Object '{}' is not a standard disk file (file type: 0x{:x}); refusing exclusive deletion",
            path.display(),
            file_type
        )));
    }

    let mut by_handle_info = std::mem::MaybeUninit::<BY_HANDLE_FILE_INFORMATION>::zeroed();
    let info_res =
        unsafe { GetFileInformationByHandle(file.as_raw_handle(), by_handle_info.as_mut_ptr()) };
    if info_res == 0 {
        return Err(AetherError::Io(std::io::Error::last_os_error()));
    }
    let file_info = unsafe { by_handle_info.assume_init() };

    let is_reparse = (file_info.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT) != 0;
    let is_dir = (file_info.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY) != 0;
    let fault_reparse = is_fault_inject_windows_reparse_point();

    if is_reparse || is_dir || fault_reparse {
        return Err(AetherError::PermissionDenied(format!(
            "Security rejection: Object '{}' is a reparse point or directory (attributes: 0x{:x}); refusing exclusive deletion to prevent target redirection",
            path.display(),
            file_info.dwFileAttributes
        )));
    }

    let actual_identity = FileObjectIdentity {
        volume_serial_number: file_info.dwVolumeSerialNumber as u64,
        file_index_high: file_info.nFileIndexHigh as u64,
        file_index_low: file_info.nFileIndexLow as u64,
    };

    let fault_identity = is_fault_inject_windows_identity_mismatch();
    if fault_identity {
        return Err(AetherError::PermissionDenied(format!(
            "Security rejection: Injected file identity mismatch for '{}'",
            path.display()
        )));
    }

    if let Some(expected) = expected_identity {
        if actual_identity != expected {
            return Err(AetherError::PermissionDenied(format!(
                "Security rejection: Opened file identity for '{}' does not match the authorized staged object identity (expected {:?}, found {:?}). Staged candidate preserved without deletion.",
                path.display(),
                expected,
                actual_identity
            )));
        }
    }

    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).map_err(AetherError::Io)?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    let current_hash = format!("{:x}", hasher.finalize());

    if !expected_hash.is_empty() && current_hash != expected_hash {
        return Err(AetherError::PermissionDenied(format!(
            "Concurrent modification detected during exclusive verification of '{}'. Staged source preserved without deletion.",
            path.display()
        )));
    }

    // While exclusive handle on staging file remains open, open and verify destination handle (Sprint 23.2.16 & Sprint 23.2.18)
    let dest_handle = if let Some(dest) = destination_path {
        // Sprint 23.2.18: Open destination handle with GENERIC_READ and FILE_SHARE_READ.
        // This kernel lock guarantees:
        // 1. Any adversarial attempt to delete or unlink destination fails with ERROR_SHARING_VIOLATION (code 32).
        // 2. Any adversarial attempt to write or truncate destination fails with ERROR_SHARING_VIOLATION (code 32).
        // 3. Destination identity and attributes are verified directly from this verified open handle without TOCTOU races.
        let dest_file = std::fs::OpenOptions::new()
            .read(true)
            .access_mode(GENERIC_READ)
            .share_mode(FILE_SHARE_READ)
            .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
            .open(dest)
            .map_err(AetherError::Io)?;

        // Hook while exclusive handles on both staging and destination are held (CHK-210)
        if let Some(ref hook) = get_hook_windows_post_exclusive_open() {
            hook(path, dest);
        }

        // Check fault injection for destination identity query failure (CHK-213)
        if is_fault_inject_windows_dest_identity_query_failure() {
            return Err(AetherError::Io(std::io::Error::other(
                "Injected destination identity query failure while exclusive handle held",
            )));
        }

        let dest_file_type = unsafe { GetFileType(dest_file.as_raw_handle()) };
        if dest_file_type != FILE_TYPE_DISK {
            return Err(AetherError::PermissionDenied(format!(
                "Security rejection: Destination '{}' is not a standard disk file (file type: 0x{:x}); refusing exclusive deletion",
                dest.display(),
                dest_file_type
            )));
        }

        let mut dest_by_handle_info = std::mem::MaybeUninit::<BY_HANDLE_FILE_INFORMATION>::zeroed();
        let dest_info_res = unsafe {
            GetFileInformationByHandle(dest_file.as_raw_handle(), dest_by_handle_info.as_mut_ptr())
        };
        if dest_info_res == 0 {
            return Err(AetherError::Io(std::io::Error::last_os_error()));
        }
        let dest_file_info = unsafe { dest_by_handle_info.assume_init() };

        let dest_is_reparse = (dest_file_info.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT) != 0;
        let dest_is_dir = (dest_file_info.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY) != 0;
        if dest_is_reparse || dest_is_dir {
            return Err(AetherError::PermissionDenied(format!(
                "Security rejection: Destination '{}' is a reparse point or directory (attributes: 0x{:x}); refusing exclusive deletion to prevent target redirection",
                dest.display(),
                dest_file_info.dwFileAttributes
            )));
        }

        let dest_identity = FileObjectIdentity {
            volume_serial_number: dest_file_info.dwVolumeSerialNumber as u64,
            file_index_high: dest_file_info.nFileIndexHigh as u64,
            file_index_low: dest_file_info.nFileIndexLow as u64,
        };

        if is_fault_inject_windows_dest_identity_mismatch() {
            return Err(AetherError::PermissionDenied(format!(
                "Security rejection: Injected destination identity mismatch for '{}'",
                dest.display()
            )));
        }

        if dest_identity != actual_identity {
            return Err(AetherError::PermissionDenied(format!(
                "Security rejection: Destination '{}' identity ({:?}) does not match authorized object identity ({:?}) while exclusive handle held on staging path '{}'",
                dest.display(),
                dest_identity,
                actual_identity,
                path.display()
            )));
        }

        if let Some(expected) = expected_identity {
            if dest_identity != expected {
                return Err(AetherError::PermissionDenied(format!(
                    "Security rejection: Destination '{}' identity ({:?}) does not match expected identity ({:?})",
                    dest.display(),
                    dest_identity,
                    expected
                )));
            }
        }

        Some(dest_file)
    } else {
        None
    };

    #[repr(C)]
    #[allow(non_snake_case)]
    struct FILE_DISPOSITION_INFO {
        DeleteFile: u8,
    }
    #[allow(non_snake_case)]
    extern "system" {
        fn SetFileInformationByHandle(
            hFile: *mut std::ffi::c_void,
            FileInformationClass: u32,
            lpFileInformation: *mut std::ffi::c_void,
            dwBufferSize: u32,
        ) -> i32;
    }
    let mut info = FILE_DISPOSITION_INFO { DeleteFile: 1 };
    let res = unsafe {
        SetFileInformationByHandle(
            file.as_raw_handle(),
            4, // FileDispositionInfo
            &mut info as *mut _ as *mut std::ffi::c_void,
            std::mem::size_of::<FILE_DISPOSITION_INFO>() as u32,
        )
    };

    if res == 0 {
        return Err(AetherError::Io(std::io::Error::last_os_error()));
    }

    drop(file);
    drop(dest_handle);
    Ok(())
}

#[cfg(not(windows))]
pub fn remove_file_windows_exclusive(
    _path: &Path,
    _expected_hash: &str,
    _expected_identity: Option<FileObjectIdentity>,
    _destination_path: Option<&Path>,
) -> Result<()> {
    Err(AetherError::PermissionDenied(
        "Windows exclusive deletion is unsupported on non-Windows platforms".into(),
    ))
}

/// Sanitizes an untrusted user-provided relative path for capability operations.
/// Strictly rejects path traversals (`..`), drive prefixes, absolute roots,
/// Alternate Data Streams (`:`), Windows device names (CON, NUL, PRN, etc.),
/// trailing dots/spaces, null bytes, and extended device namespaces.
pub fn sanitize_relative_path(user_path: &str) -> Result<PathBuf> {
    if user_path.is_empty() {
        return Err(AetherError::PermissionDenied("Path cannot be empty".into()));
    }

    // 1. Explicitly reject null bytes and control characters
    if user_path.as_bytes().contains(&0) || user_path.chars().any(|c| (c as u32) < 0x20) {
        return Err(AetherError::PermissionDenied(
            "Access Denied: Null bytes and control characters are strictly forbidden in paths"
                .into(),
        ));
    }

    // 2. Explicitly reject URL encoded traversal attempts (%2e, %00, %2f, %5c)
    let lower = user_path.to_ascii_lowercase();
    if lower.contains("%2e")
        || lower.contains("%00")
        || lower.contains("%2f")
        || lower.contains("%5c")
    {
        return Err(AetherError::PermissionDenied(
            "Access Denied: URL encoded path sequences are strictly forbidden".into(),
        ));
    }

    // 3. Explicitly reject Alternate Data Streams and drive colons anywhere in relative paths
    if user_path.contains(':') {
        return Err(AetherError::PermissionDenied(
            "Access Denied: Stream separator or colon ':' is strictly forbidden in relative paths"
                .into(),
        ));
    }

    // 4. Explicitly reject Windows invalid filename characters (<, >, ", |, ?, *)
    if user_path.contains(['<', '>', '"', '|', '?', '*']) {
        return Err(AetherError::PermissionDenied(
            "Access Denied: Forbidden characters in path".into(),
        ));
    }

    // 5. Explicitly reject trailing dots or spaces in raw user path or any path component
    if user_path.ends_with('.') || user_path.ends_with(' ') {
        return Err(AetherError::PermissionDenied(
            "Access Denied: Trailing dot or space in path is forbidden".into(),
        ));
    }
    if user_path
        .split(['/', '\\'])
        .any(|seg| seg.ends_with('.') || seg.ends_with(' '))
    {
        return Err(AetherError::PermissionDenied(
            "Access Denied: Trailing dot or space in path component is forbidden".into(),
        ));
    }

    // 6. Reject UNC paths, extended device namespaces, or root slash prefixes
    let trimmed_leading = user_path.trim_start();
    if trimmed_leading.starts_with(r"\\")
        || trimmed_leading.starts_with("//")
        || trimmed_leading.starts_with(r"\??\")
        || trimmed_leading.starts_with(r"//?/")
        || trimmed_leading.starts_with('/')
        || trimmed_leading.starts_with('\\')
    {
        return Err(AetherError::PermissionDenied(
            "Access Denied: Root, UNC, and extended device namespaces are strictly forbidden"
                .into(),
        ));
    }

    let normalized_slash_str = user_path.replace('\\', "/");
    let clean = Path::new(&normalized_slash_str);
    if clean.is_absolute() {
        return Err(AetherError::PermissionDenied(
            "Access Denied: Absolute paths are strictly forbidden".into(),
        ));
    }

    let mut depth: usize = 0;
    let mut normalized = PathBuf::new();

    for comp in clean.components() {
        match comp {
            std::path::Component::Prefix(_) => {
                return Err(AetherError::PermissionDenied(
                    "Access Denied: Drive prefixes are forbidden".into(),
                ));
            }
            std::path::Component::RootDir => {
                return Err(AetherError::PermissionDenied(
                    "Access Denied: Root directory references are forbidden".into(),
                ));
            }
            std::path::Component::ParentDir => {
                if depth == 0 {
                    return Err(AetherError::PermissionDenied(
                        "Access Denied: Path traversal (..) is forbidden".into(),
                    ));
                }
                depth -= 1;
                normalized.pop();
            }
            std::path::Component::Normal(c) => {
                let s = c.to_string_lossy();
                let upper = s.to_ascii_uppercase();
                let stem = Path::new(&*s)
                    .file_stem()
                    .and_then(|st| st.to_str())
                    .unwrap_or("")
                    .to_ascii_uppercase();

                let is_reserved = |name: &str| {
                    matches!(
                        name,
                        "CON"
                            | "PRN"
                            | "AUX"
                            | "NUL"
                            | "COM1"
                            | "COM2"
                            | "COM3"
                            | "COM4"
                            | "COM5"
                            | "COM6"
                            | "COM7"
                            | "COM8"
                            | "COM9"
                            | "LPT1"
                            | "LPT2"
                            | "LPT3"
                            | "LPT4"
                            | "LPT5"
                            | "LPT6"
                            | "LPT7"
                            | "LPT8"
                            | "LPT9"
                    )
                };

                if is_reserved(&upper) || is_reserved(&stem) {
                    return Err(AetherError::PermissionDenied(format!(
                        "Access Denied: Reserved device name '{s}' is forbidden"
                    )));
                }

                depth += 1;
                normalized.push(c);
            }
            std::path::Component::CurDir => {}
        }
    }

    if normalized.as_os_str().is_empty() {
        return Err(AetherError::PermissionDenied(
            "Access Denied: Normalized relative path is empty".into(),
        ));
    }

    Ok(normalized)
}

/// Comprehensive report produced by capability-anchored workspace directory scanning.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct WorkspaceScanReport {
    pub discovered_files: Vec<PathBuf>,
    pub directories_scanned: usize,
    pub files_discovered: usize,
    pub skipped_by_policy: usize,
    pub skipped_symlinks: usize,
    pub permission_errors: Vec<PathBuf>,
    pub transient_errors: Vec<PathBuf>,
    pub is_complete: bool,
}

/// Outcome of conditional file replacement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConditionalReplaceOutcome {
    /// Destination path of the committed replacement file.
    pub dest_path: PathBuf,
    /// If a backup file had to be preserved due to trailing modifications, read failure,
    /// rename failure, or removal failure, this contains the absolute path of the surviving file.
    pub preserved_backup: Option<PathBuf>,
    /// If a temporary staging file could not be unlinked after publication, this contains the absolute path of the residual artifact.
    pub residual_staging: Option<PathBuf>,
}

impl ConditionalReplaceOutcome {
    /// Returns true if no backup was preserved and no residual staging artifact remains.
    pub fn is_fully_clean(&self) -> bool {
        self.preserved_backup.is_none() && self.residual_staging.is_none()
    }
}

/// Optional hooks and fault-injection flags for conditional file replacement.
#[derive(Default, Clone)]
pub struct ConditionalReplaceHooks<'a> {
    /// Injected immediately before the existing target file is moved to backup.
    pub pre_replace_hook: Option<&'a (dyn Fn(&Path) + 'a)>,
    /// Injected after backup hash calculation and before the stage-to-destination commit.
    pub post_hash_hook: Option<&'a (dyn Fn(&Path) + 'a)>,
    /// Injected after final verification and before cleanup of backup file.
    pub pre_cleanup_hook: Option<&'a (dyn Fn(&Path) + 'a)>,
    /// Injected fault: simulate I/O error during stage file write.
    pub fault_inject_stage_write: bool,
    /// Injected fault: simulate I/O error during stage file sync.
    pub fault_inject_stage_sync: bool,
    /// Injected fault: simulate failure during rollback rename.
    pub fault_inject_rollback_failure: bool,
    /// Injected fault: simulate failure during final backup read.
    pub fault_inject_final_backup_read: bool,
    /// Injected fault: simulate failure during conflict rename.
    pub fault_inject_conflict_rename: bool,
    /// Injected fault: simulate failure during backup removal.
    pub fault_inject_backup_removal: bool,
    /// Injected fault: simulate failure during staging file removal after publication or during rollback.
    pub fault_inject_staging_removal: bool,
    /// Injected immediately before primary atomic publication (pre-hard-link / pre-publish).
    pub pre_publish_hook: Option<&'a (dyn Fn(&Path) + 'a)>,
    /// Injected immediately before platform-specific fallback publication primitive.
    pub pre_fallback_publish_hook: Option<&'a (dyn Fn(&Path) + 'a)>,
    /// Injected fault: simulate non-AlreadyExists hard-link failure (forcing the fallback publication path).
    pub fault_inject_hard_link_failure: bool,
}

/// Final disposition of a conditional file deletion attempt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeletionDisposition {
    /// Confirmed already absent on disk prior to staging
    ConfirmedAbsent,
    /// Physically unlinked after passing exclusive pre-unlink verification
    Unlinked,
    /// Moved to quarantine because exclusive deletion cannot be proven or cleanup is deferred
    Quarantined { quarantine_path: PathBuf },
    /// Preserved as a recovery conflict artifact due to concurrent modification or uncertain state
    ConflictPreserved { conflict_path: PathBuf },
}

/// Outcome of conditional file deletion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConditionalDeleteOutcome {
    /// True ONLY if the file was physically unlinked or confirmed absent.
    /// NEVER true when an artifact is quarantined or preserved!
    pub was_deleted: bool,
    /// If deletion could not proceed (e.g. concurrent modification detected, read failure,
    /// quarantined, or removal failure), this contains the surviving path of the preserved file.
    pub surviving_path: Option<PathBuf>,
    /// Exact disposition of the deletion attempt.
    pub disposition: DeletionDisposition,
    /// True if the artifact was preserved in quarantine for safe, non-destructive recovery.
    pub quarantined: bool,
}

/// Optional hooks and fault-injection flags for conditional file deletion.
#[derive(Default, Clone)]
pub struct ConditionalDeleteHooks<'a> {
    /// Injected after moving target to staging and before first hash verification.
    pub pre_delete_hook: Option<&'a (dyn Fn(&Path) + 'a)>,
    /// Injected after first hash verification and immediately before second verification.
    /// Simulates concurrent writes through pre-existing open writable handles.
    pub post_hash_hook: Option<&'a (dyn Fn(&Path) + 'a)>,
    /// Injected after second hash verification and immediately before physical unlink.
    /// Simulates concurrent writes through retained handles after final verification (CHK-048).
    pub pre_unlink_hook: Option<&'a (dyn Fn(&Path) + 'a)>,
    /// Injected immediately before hard-link rollback publication (after destination metadata check).
    pub pre_hard_link_hook: Option<&'a (dyn Fn(&Path) + 'a)>,
    /// If true, defers physical deletion and safely preserves the staged file into quarantine.
    pub defer_to_quarantine: bool,
    /// Optional quarantine destination directory.
    pub quarantine_dir: Option<PathBuf>,
    /// Injected fault: simulate I/O error during deletion removal.
    pub fault_inject_delete_removal: bool,
    /// Injected fault: simulate failure during rollback.
    pub fault_inject_rollback_failure: bool,
    /// Injected fault: simulate failure during destination metadata inspection in rollback.
    pub fault_inject_rollback_metadata_failure: bool,
    /// Injected fault: simulate non-AlreadyExists failure during hard link rollback publication.
    pub fault_inject_hard_link_failure: bool,
    /// Injected fault: simulate read failure during pre-unlink second verification.
    pub fault_inject_second_verification_read_failure: bool,
}

/// Capability-based workspace directory handle.
/// All file and directory operations are anchored directly to the opened directory capability (`Dir`).
/// Enforces genuine handle-level capability boundaries across Windows, Linux, and macOS.
pub struct WorkspaceCapability {
    root_path: PathBuf,
    dir: Dir,
}

impl WorkspaceCapability {
    /// Opens the authorized workspace root as a directory capability.
    pub fn open<P: AsRef<Path>>(root: P) -> Result<Self> {
        let root = root.as_ref();
        let canon = root.canonicalize().map_err(AetherError::Io)?;
        let dir = Dir::open_ambient_dir(&canon, ambient_authority()).map_err(AetherError::Io)?;
        Ok(Self {
            root_path: canon,
            dir,
        })
    }

    pub fn root_path(&self) -> &Path {
        &self.root_path
    }

    pub fn raw_dir(&self) -> &Dir {
        &self.dir
    }

    /// Reads text file strictly anchored to the workspace directory capability.
    pub fn read_file(&self, user_path: &str) -> Result<String> {
        let rel = sanitize_relative_path(user_path)?;

        // Ensure target is not an external symlink
        if let Ok(meta) = self.dir.symlink_metadata(&rel) {
            if meta.is_symlink() {
                return Err(AetherError::PermissionDenied(
                    "Access Denied: Symlink points outside workspace boundary".into(),
                ));
            }
        }

        self.dir.read_to_string(&rel).map_err(AetherError::Io)
    }

    /// Reads text file by relative Path anchored strictly to the workspace directory capability.
    pub fn read_file_to_string(&self, rel_path: &Path) -> Result<String> {
        let rel_str = rel_path.to_string_lossy();
        self.read_file(&rel_str)
    }

    /// Reads binary bytes strictly anchored to the workspace directory capability.
    pub fn read_file_bytes(&self, user_path: &str) -> Result<Vec<u8>> {
        let rel = sanitize_relative_path(user_path)?;
        if let Ok(meta) = self.dir.symlink_metadata(&rel) {
            if meta.is_symlink() {
                return Err(AetherError::PermissionDenied(
                    "Access Denied: Symlink points outside workspace boundary".into(),
                ));
            }
        }
        let mut f = self.dir.open(&rel).map_err(AetherError::Io)?;
        let mut buf = Vec::new();
        f.read_to_end(&mut buf).map_err(AetherError::Io)?;
        Ok(buf)
    }

    /// Reads binary bytes by relative Path anchored strictly to the workspace directory capability.
    pub fn read_file_to_bytes(&self, rel_path: &Path) -> Result<Vec<u8>> {
        let rel_str = rel_path.to_string_lossy();
        self.read_file_bytes(&rel_str)
    }

    /// Writes raw binary bytes to a file inside the capability using atomic handle-level operations.
    /// Creates a capability-isolated temporary file via `create_new(true)`, writes and syncs to disk,
    /// verifies the destination is not a symlink, and performs an atomic rename anchored to the capability.
    pub fn write_file_bytes_atomic(&self, user_path: &str, bytes: &[u8]) -> Result<PathBuf> {
        let rel = sanitize_relative_path(user_path)?;

        // Ensure parent directory exists anchored inside capability
        if let Some(parent) = rel.parent() {
            if !parent.as_os_str().is_empty() {
                self.dir.create_dir_all(parent).map_err(AetherError::Io)?;
            }
        }

        // Verify destination is not a symlink; refuse to write through destination symlinks
        if let Ok(meta) = self.dir.symlink_metadata(&rel) {
            if meta.is_symlink() {
                return Err(AetherError::PermissionDenied(
                    "Access Denied: Destination is a symlink".into(),
                ));
            }
        }

        // Step 1: Create capability-isolated temporary file in parent directory
        let temp_name = format!(".aether-tmp-{}", uuid::Uuid::new_v4().simple());
        let temp_rel = if let Some(parent) = rel.parent() {
            parent.join(&temp_name)
        } else {
            PathBuf::from(&temp_name)
        };

        let mut opts = OpenOptions::new();
        opts.write(true).create_new(true);

        let mut f = self
            .dir
            .open_with(&temp_rel, &opts)
            .map_err(AetherError::Io)?;
        f.write_all(bytes).map_err(AetherError::Io)?;
        f.sync_all().map_err(AetherError::Io)?;
        drop(f);

        // Step 2: Atomic rename anchored to directory capability
        if let Err(e) = self.dir.rename(&temp_rel, &self.dir, &rel) {
            if let Err(rm_err) = self.dir.remove_file(&temp_rel) {
                return Err(AetherError::Internal(format!(
                    "Rename of temporary file failed ({}) and cleanup of temporary file '{:?}' failed: {}",
                    e, temp_rel, rm_err
                )));
            }
            return Err(AetherError::Io(e));
        }

        Ok(self.root_path.join(rel))
    }

    /// Internal rollback helper that strictly verifies rollback rename results.
    /// If rollback fails, the backup file is preserved on disk without deletion,
    /// and an explicit `AetherError::Internal` containing the backup location is returned.
    #[allow(clippy::too_many_arguments)]
    fn perform_rollback(
        &self,
        backup_rel: &Path,
        rel: &Path,
        stage_rel: Option<&Path>,
        user_path: &str,
        cause_msg: &str,
        fault_inject_rollback_failure: bool,
        fault_inject_staging_removal: bool,
    ) -> AetherError {
        // Clean up temporary incoming stage file if present
        let mut staging_residual_note = None;
        if let Some(stage) = stage_rel {
            let stage_abs = self.root_path.join(stage);
            let rm_res = if fault_inject_staging_removal {
                Err(std::io::Error::other(
                    "Injected staging removal failure during rollback",
                ))
            } else {
                self.dir.remove_file(stage)
            };
            if let Err(e) = rm_res {
                warn!(
                    "perform_rollback: failed to remove staging file '{:?}': {}",
                    stage, e
                );
                staging_residual_note = Some(format!(
                    "; additionally failed to remove residual staging artifact '{}': {}",
                    stage_abs.display(),
                    e
                ));
            }
        }
        let staging_note = staging_residual_note.as_deref().unwrap_or("");

        // Commit-boundary safety check:
        // If a competing object already exists at `rel`, attempting to rename `backup_rel` to `rel`
        // would either silently overwrite the competing object (on POSIX) or fail (on Windows).
        // Neither the competing object nor the original backup must be lost or overwritten!
        if self.dir.symlink_metadata(rel).is_ok() {
            let conflict_name = format!(".aether-conflict-{}", uuid::Uuid::new_v4().simple());
            let conflict_rel = if let Some(parent) = rel.parent() {
                parent.join(&conflict_name)
            } else {
                PathBuf::from(&conflict_name)
            };
            let backup_abs = self.root_path.join(backup_rel);
            match self.dir.rename(backup_rel, &self.dir, &conflict_rel) {
                Ok(()) => {
                    let conflict_abs = self.root_path.join(&conflict_rel);
                    return AetherError::PermissionDenied(format!(
                        "Commit boundary conflict on '{}': {}. Destination was concurrently recreated; competing destination preserved; original data preserved at '{}'{}.",
                        user_path, cause_msg, conflict_abs.display(), staging_note
                    ));
                }
                Err(e) => {
                    return AetherError::Internal(format!(
                        "Commit boundary conflict on '{}': {}. Destination was concurrently recreated; preservation rename to conflict path failed ({}); original data retained at '{}'{}.",
                        user_path, cause_msg, e, backup_abs.display(), staging_note
                    ));
                }
            }
        }

        let rollback_res = if fault_inject_rollback_failure {
            Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "Injected rollback failure: destination locked or unwritable",
            ))
        } else {
            self.dir.rename(backup_rel, &self.dir, rel)
        };

        match rollback_res {
            Ok(()) => AetherError::PermissionDenied(format!(
                "Restore race detected: {}. Restore aborted, target rolled back successfully{}.",
                cause_msg, staging_note
            )),
            Err(rb_err) => {
                let backup_abs = self.root_path.join(backup_rel);
                AetherError::Internal(format!(
                    "RESTORE ROLLBACK FAILURE on '{}': {}. Attempted rollback to destination failed ({}). User data PRESERVED at '{}'. Do not delete this file; manual recovery required{}.",
                    user_path, cause_msg, rb_err, backup_abs.display(), staging_note
                ))
            }
        }
    }

    fn atomic_publish_fallback(
        stage_abs: &Path,
        dest_abs: &Path,
    ) -> std::result::Result<(), AtomicPublishError> {
        #[cfg(windows)]
        {
            use std::os::windows::ffi::OsStrExt;
            let wide_stage: Vec<u16> = stage_abs
                .as_os_str()
                .encode_wide()
                .chain(std::iter::once(0))
                .collect();
            let wide_dest: Vec<u16> = dest_abs
                .as_os_str()
                .encode_wide()
                .chain(std::iter::once(0))
                .collect();

            extern "system" {
                fn MoveFileExW(
                    lpExistingFileName: *const u16,
                    lpNewFileName: *const u16,
                    dwFlags: u32,
                ) -> i32;
                fn GetLastError() -> u32;
            }

            const MOVEFILE_WRITE_THROUGH: u32 = 0x00000008;
            // CRITICAL: MOVEFILE_REPLACE_EXISTING (0x1) is deliberately omitted to enforce no-overwrite semantics!
            let ret = unsafe {
                MoveFileExW(
                    wide_stage.as_ptr(),
                    wide_dest.as_ptr(),
                    MOVEFILE_WRITE_THROUGH,
                )
            };

            if ret != 0 {
                return Ok(());
            }

            let err = unsafe { GetLastError() };
            if err == 183 || err == 80 {
                // ERROR_ALREADY_EXISTS (183) or ERROR_FILE_EXISTS (80)
                return Err(AtomicPublishError::DestinationExists);
            }

            Err(AtomicPublishError::Io(std::io::Error::from_raw_os_error(
                err as i32,
            )))
        }

        #[cfg(target_os = "linux")]
        {
            use std::os::unix::ffi::OsStrExt;
            let c_stage = std::ffi::CString::new(stage_abs.as_os_str().as_bytes())
                .map_err(|_| AtomicPublishError::Unsupported)?;
            let c_dest = std::ffi::CString::new(dest_abs.as_os_str().as_bytes())
                .map_err(|_| AtomicPublishError::Unsupported)?;

            const RENAME_NOREPLACE: libc::c_uint = 1;
            let ret = unsafe {
                libc::syscall(
                    libc::SYS_renameat2,
                    libc::AT_FDCWD,
                    c_stage.as_ptr(),
                    libc::AT_FDCWD,
                    c_dest.as_ptr(),
                    RENAME_NOREPLACE,
                )
            };

            if ret == 0 {
                return Ok(());
            }

            let errno = std::io::Error::last_os_error().raw_os_error().unwrap_or(0);
            if errno == libc::EEXIST {
                return Err(AtomicPublishError::DestinationExists);
            }
            if errno == libc::EINVAL || errno == libc::ENOSYS {
                return Err(AtomicPublishError::Unsupported);
            }

            Err(AtomicPublishError::Io(std::io::Error::from_raw_os_error(
                errno,
            )))
        }

        #[cfg(target_os = "macos")]
        {
            use std::os::unix::ffi::OsStrExt;
            let c_stage = std::ffi::CString::new(stage_abs.as_os_str().as_bytes())
                .map_err(|_| AtomicPublishError::Unsupported)?;
            let c_dest = std::ffi::CString::new(dest_abs.as_os_str().as_bytes())
                .map_err(|_| AtomicPublishError::Unsupported)?;

            extern "C" {
                fn renamex_np(
                    oldpath: *const libc::c_char,
                    newpath: *const libc::c_char,
                    flags: libc::c_uint,
                ) -> libc::c_int;
            }
            const RENAME_EXCL: libc::c_uint = 0x00000004;

            let ret = unsafe { renamex_np(c_stage.as_ptr(), c_dest.as_ptr(), RENAME_EXCL) };

            if ret == 0 {
                return Ok(());
            }

            let errno = std::io::Error::last_os_error().raw_os_error().unwrap_or(0);
            if errno == libc::EEXIST {
                return Err(AtomicPublishError::DestinationExists);
            }
            if errno == libc::ENOTSUP {
                return Err(AtomicPublishError::Unsupported);
            }

            Err(AtomicPublishError::Io(std::io::Error::from_raw_os_error(
                errno,
            )))
        }

        #[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
        {
            Err(AtomicPublishError::Unsupported)
        }
    }

    fn atomic_publish_no_replace(
        &self,
        stage_rel: &Path,
        rel: &Path,
        stage_abs: &Path,
        dest_abs: &Path,
        hooks: &ConditionalReplaceHooks,
    ) -> std::result::Result<Option<PathBuf>, AtomicPublishError> {
        if let Some(hook) = hooks.pre_publish_hook {
            hook(dest_abs);
        }

        // Try hard_link as primary atomic no-replace publication where supported
        let link_res = if hooks.fault_inject_hard_link_failure {
            Err(std::io::Error::other(
                "Injected hard-link non-AlreadyExists failure",
            ))
        } else {
            self.dir.hard_link(stage_rel, &self.dir, rel)
        };

        match link_res {
            Ok(()) => {
                let residual_staging = if hooks.fault_inject_staging_removal {
                    Some(stage_abs.to_path_buf())
                } else if let Err(e) = self.dir.remove_file(stage_rel) {
                    warn!(
                        "atomic_publish_no_replace: failed to remove staging link '{:?}': {}",
                        stage_rel, e
                    );
                    Some(stage_abs.to_path_buf())
                } else {
                    None
                };
                return Ok(residual_staging);
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                return Err(AtomicPublishError::DestinationExists);
            }
            #[cfg(windows)]
            Err(e) if e.raw_os_error() == Some(183) || e.raw_os_error() == Some(80) => {
                return Err(AtomicPublishError::DestinationExists);
            }
            #[cfg(unix)]
            Err(e) if e.raw_os_error() == Some(libc::EEXIST) => {
                return Err(AtomicPublishError::DestinationExists);
            }
            Err(_) => {
                // Hard-link failed for other reasons (e.g. cross-device, unsupported, or fault-injected).
                // Proceed to platform-specific atomic no-replace fallback.
            }
        }

        // Invoke fallback pre-publish hook if provided (deterministic adversarial race testing)
        if let Some(fb_hook) = hooks.pre_fallback_publish_hook {
            fb_hook(dest_abs);
        }

        Self::atomic_publish_fallback(stage_abs, dest_abs)?;
        Ok(None)
    }

    /// Conditionally replaces or creates a file inside workspace capability with full hook support.
    pub fn replace_file_bytes_conditional_with_hooks(
        &self,
        user_path: &str,
        bytes: &[u8],
        expected_hash: Option<&str>,
        hooks: ConditionalReplaceHooks,
    ) -> Result<ConditionalReplaceOutcome> {
        let rel = sanitize_relative_path(user_path)?;
        if let Some(parent) = rel.parent() {
            if !parent.as_os_str().is_empty() {
                self.dir.create_dir_all(parent).map_err(AetherError::Io)?;
            }
        }

        let dest_abs = self.root_path.join(&rel);

        if expected_hash.is_none() {
            // Target was recorded as Missing at preview generation.
            if let Some(hook) = hooks.pre_replace_hook {
                hook(&dest_abs);
            }

            // 1. Stage content into temporary file with unique UUID
            let stage_name = format!(".aether-stage-{}", uuid::Uuid::new_v4().simple());
            let stage_rel = if let Some(parent) = rel.parent() {
                parent.join(&stage_name)
            } else {
                PathBuf::from(&stage_name)
            };
            let stage_abs = self.root_path.join(&stage_rel);

            let mut opts = OpenOptions::new();
            opts.write(true).create_new(true);

            let mut f = self
                .dir
                .open_with(&stage_rel, &opts)
                .map_err(AetherError::Io)?;

            let cleanup_stage_or_error = |primary_msg: String| -> AetherError {
                let rm_res = if hooks.fault_inject_staging_removal {
                    Err(std::io::Error::other("Injected staging removal failure"))
                } else {
                    self.dir.remove_file(&stage_rel)
                };
                if let Err(rm_err) = rm_res {
                    AetherError::Io(std::io::Error::other(format!(
                        "{}; additionally failed to remove residual staging artifact '{}': {}",
                        primary_msg,
                        stage_abs.display(),
                        rm_err
                    )))
                } else {
                    AetherError::Io(std::io::Error::other(primary_msg))
                }
            };

            if hooks.fault_inject_stage_write {
                drop(f);
                return Err(cleanup_stage_or_error(
                    "Injected stage write failure".to_string(),
                ));
            }

            if let Err(e) = f.write_all(bytes) {
                drop(f);
                return Err(cleanup_stage_or_error(format!(
                    "Failed to write stage file bytes: {}",
                    e
                )));
            }

            if hooks.fault_inject_stage_sync {
                drop(f);
                return Err(cleanup_stage_or_error(
                    "Injected stage sync failure".to_string(),
                ));
            }

            if let Err(e) = f.sync_all() {
                drop(f);
                return Err(cleanup_stage_or_error(format!(
                    "Failed to sync stage file: {}",
                    e
                )));
            }
            drop(f);

            // 2. Publish stage file using exclusive no-overwrite operation
            let link_res = self.dir.hard_link(&stage_rel, &self.dir, &rel);
            match link_res {
                Ok(()) => {
                    let residual_staging = if hooks.fault_inject_staging_removal {
                        Some(stage_abs.clone())
                    } else if let Err(e) = self.dir.remove_file(&stage_rel) {
                        warn!(
                            "replace_file_bytes_conditional_with_hooks: failed to remove staging link '{:?}': {}",
                            stage_rel, e
                        );
                        Some(stage_abs.clone())
                    } else {
                        None
                    };
                    return Ok(ConditionalReplaceOutcome {
                        dest_path: dest_abs,
                        preserved_backup: None,
                        residual_staging,
                    });
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                    let rm_res = if hooks.fault_inject_staging_removal {
                        Err(std::io::Error::other("Injected staging removal failure"))
                    } else {
                        self.dir.remove_file(&stage_rel)
                    };
                    let residual_msg = if let Err(rm_err) = rm_res {
                        format!(
                            "; additionally failed to remove residual staging artifact '{}': {}",
                            stage_abs.display(),
                            rm_err
                        )
                    } else {
                        String::new()
                    };
                    return Err(AetherError::PermissionDenied(format!(
                        "Restore race detected: target file '{}' was newly created on disk before replacement{}. Restore aborted.",
                        user_path, residual_msg
                    )));
                }
                Err(e) => {
                    let rm_res = if hooks.fault_inject_staging_removal {
                        Err(std::io::Error::other("Injected staging removal failure"))
                    } else {
                        self.dir.remove_file(&stage_rel)
                    };
                    let residual_msg = if let Err(rm_err) = rm_res {
                        format!(
                            "; additionally failed to remove residual staging artifact '{}': {}",
                            stage_abs.display(),
                            rm_err
                        )
                    } else {
                        String::new()
                    };
                    // Check if file exists to detect race condition
                    if self.dir.symlink_metadata(&rel).is_ok() {
                        return Err(AetherError::PermissionDenied(format!(
                            "Restore race detected: target file '{}' was newly created on disk before replacement{}. Restore aborted.",
                            user_path, residual_msg
                        )));
                    }
                    return Err(AetherError::Io(std::io::Error::other(format!(
                        "Failed to hard-link stage file ({}){}",
                        e, residual_msg
                    ))));
                }
            }
        }

        let expected = expected_hash.unwrap();

        // 1. Prepare staged replacement file
        let stage_name = format!(".aether-stage-{}", uuid::Uuid::new_v4().simple());
        let stage_rel = if let Some(parent) = rel.parent() {
            parent.join(&stage_name)
        } else {
            PathBuf::from(&stage_name)
        };
        let stage_abs = self.root_path.join(&stage_rel);

        let mut opts = OpenOptions::new();
        opts.write(true).create_new(true);

        let mut f = self
            .dir
            .open_with(&stage_rel, &opts)
            .map_err(AetherError::Io)?;

        let cleanup_stage_or_error = |primary_msg: String| -> AetherError {
            let rm_res = if hooks.fault_inject_staging_removal {
                Err(std::io::Error::other("Injected staging removal failure"))
            } else {
                self.dir.remove_file(&stage_rel)
            };
            if let Err(rm_err) = rm_res {
                AetherError::Io(std::io::Error::other(format!(
                    "{}; additionally failed to remove residual staging artifact '{}': {}",
                    primary_msg,
                    stage_abs.display(),
                    rm_err
                )))
            } else {
                AetherError::Io(std::io::Error::other(primary_msg))
            }
        };

        if hooks.fault_inject_stage_write {
            drop(f);
            return Err(cleanup_stage_or_error(
                "Injected stage write failure".to_string(),
            ));
        }

        if let Err(e) = f.write_all(bytes) {
            drop(f);
            return Err(cleanup_stage_or_error(format!(
                "Failed to write stage file bytes: {}",
                e
            )));
        }

        if hooks.fault_inject_stage_sync {
            drop(f);
            return Err(cleanup_stage_or_error(
                "Injected stage sync failure".to_string(),
            ));
        }

        if let Err(e) = f.sync_all() {
            drop(f);
            return Err(cleanup_stage_or_error(format!(
                "Failed to sync stage file: {}",
                e
            )));
        }
        drop(f);

        // 2. Invoke pre_replace_hook if configured (deterministic test injection)
        if let Some(hook) = hooks.pre_replace_hook {
            hook(&dest_abs);
        }

        // 3. Staged move of target to backup
        let backup_name = format!(".aether-backup-{}", uuid::Uuid::new_v4().simple());
        let backup_rel = if let Some(parent) = rel.parent() {
            parent.join(&backup_name)
        } else {
            PathBuf::from(&backup_name)
        };

        if let Err(e) = self.dir.rename(&rel, &self.dir, &backup_rel) {
            let rm_res = if hooks.fault_inject_staging_removal {
                Err(std::io::Error::other("Injected staging removal failure"))
            } else {
                self.dir.remove_file(&stage_rel)
            };
            let residual_msg = if let Err(rm_err) = rm_res {
                format!(
                    "; additionally failed to remove residual staging artifact '{}': {}",
                    stage_abs.display(),
                    rm_err
                )
            } else {
                String::new()
            };
            return Err(AetherError::PermissionDenied(format!(
                "Restore race detected: target file '{}' could not be staged for verification ({}){}. Restore aborted.",
                user_path, e, residual_msg
            )));
        }

        // 4. Verify captured backup file
        let backup_meta = match self.dir.symlink_metadata(&backup_rel) {
            Ok(m) => m,
            Err(e) => {
                return Err(self.perform_rollback(
                    &backup_rel,
                    &rel,
                    Some(&stage_rel),
                    user_path,
                    &format!(
                        "target file '{}' metadata inspection failed ({})",
                        user_path, e
                    ),
                    hooks.fault_inject_rollback_failure,
                    hooks.fault_inject_staging_removal,
                ));
            }
        };

        if backup_meta.is_symlink() || !backup_meta.is_file() {
            return Err(self.perform_rollback(
                &backup_rel,
                &rel,
                Some(&stage_rel),
                user_path,
                &format!(
                    "target file '{}' is a symlink or non-regular file",
                    user_path
                ),
                hooks.fault_inject_rollback_failure,
                hooks.fault_inject_staging_removal,
            ));
        }

        let mut backup_file = match self.dir.open(&backup_rel) {
            Ok(bf) => bf,
            Err(e) => {
                return Err(self.perform_rollback(
                    &backup_rel,
                    &rel,
                    Some(&stage_rel),
                    user_path,
                    &format!(
                        "target file '{}' could not be opened for verification ({})",
                        user_path, e
                    ),
                    hooks.fault_inject_rollback_failure,
                    hooks.fault_inject_staging_removal,
                ));
            }
        };

        let mut current_bytes = Vec::new();
        if let Err(e) = backup_file.read_to_end(&mut current_bytes) {
            drop(backup_file);
            return Err(self.perform_rollback(
                &backup_rel,
                &rel,
                Some(&stage_rel),
                user_path,
                &format!("target file '{}' could not be read ({})", user_path, e),
                hooks.fault_inject_rollback_failure,
                hooks.fault_inject_staging_removal,
            ));
        }
        drop(backup_file);

        let current_hash = {
            use sha2::{Digest, Sha256};
            let mut hasher = Sha256::new();
            hasher.update(&current_bytes);
            format!("{:x}", hasher.finalize())
        };

        if current_hash != expected {
            // Concurrent modification detected before swap! Roll back immediately.
            return Err(self.perform_rollback(
                &backup_rel,
                &rel,
                Some(&stage_rel),
                user_path,
                &format!(
                    "file '{}' was modified on disk prior to replacement (Expected: {}, Found: {})",
                    user_path, expected, current_hash
                ),
                hooks.fault_inject_rollback_failure,
                hooks.fault_inject_staging_removal,
            ));
        }

        // 5. Post-hash hook: Deterministic test injection between hash calculation and stage-to-target commit
        if let Some(hook) = hooks.post_hash_hook {
            hook(&dest_abs);
        }

        // 6. Post-hash re-verification to catch modifications via pre-existing open file handles
        let recheck_bytes = match self.dir.open(&backup_rel).and_then(|mut f| {
            let mut b = Vec::new();
            f.read_to_end(&mut b)?;
            Ok(b)
        }) {
            Ok(b) => b,
            Err(e) => {
                return Err(self.perform_rollback(
                    &backup_rel,
                    &rel,
                    Some(&stage_rel),
                    user_path,
                    &format!(
                        "target file '{}' could not be re-read after verification ({})",
                        user_path, e
                    ),
                    hooks.fault_inject_rollback_failure,
                    hooks.fault_inject_staging_removal,
                ));
            }
        };

        let recheck_hash = {
            use sha2::{Digest, Sha256};
            let mut hasher = Sha256::new();
            hasher.update(&recheck_bytes);
            format!("{:x}", hasher.finalize())
        };

        if recheck_hash != expected {
            // Concurrent modification detected via open handle between verification and commit!
            // Roll back immediately, restoring user's modifications to target path.
            return Err(self.perform_rollback(
                &backup_rel,
                &rel,
                Some(&stage_rel),
                user_path,
                &format!(
                    "file '{}' was modified via open handle after hash verification (Expected: {}, Found: {})",
                    user_path, expected, recheck_hash
                ),
                hooks.fault_inject_rollback_failure,
                hooks.fault_inject_staging_removal,
            ));
        }

        // 7. Hash verified! Commit replacement: move stage to target
        let stage_abs = self.root_path.join(&stage_rel);
        let publish_res =
            self.atomic_publish_no_replace(&stage_rel, &rel, &stage_abs, &dest_abs, &hooks);

        let residual_staging = match publish_res {
            Ok(res_staging) => res_staging,
            Err(AtomicPublishError::DestinationExists) => {
                return Err(self.perform_rollback(
                    &backup_rel,
                    &rel,
                    Some(&stage_rel),
                    user_path,
                    "competing destination object was concurrently created at commit boundary",
                    hooks.fault_inject_rollback_failure,
                    hooks.fault_inject_staging_removal,
                ));
            }
            Err(AtomicPublishError::Unsupported) => {
                return Err(self.perform_rollback(
                    &backup_rel,
                    &rel,
                    Some(&stage_rel),
                    user_path,
                    "filesystem does not support atomic no-replace publication; failed closed to prevent overwrite",
                    hooks.fault_inject_rollback_failure,
                    hooks.fault_inject_staging_removal,
                ));
            }
            Err(AtomicPublishError::CrossDevice) => {
                return Err(self.perform_rollback(
                    &backup_rel,
                    &rel,
                    Some(&stage_rel),
                    user_path,
                    "cross-device link error during publication; failed closed to prevent overwrite",
                    hooks.fault_inject_rollback_failure,
                    hooks.fault_inject_staging_removal,
                ));
            }
            Err(AtomicPublishError::Io(e)) => {
                return Err(self.perform_rollback(
                    &backup_rel,
                    &rel,
                    Some(&stage_rel),
                    user_path,
                    &format!("failed to commit replacement file atomically ({})", e),
                    hooks.fault_inject_rollback_failure,
                    hooks.fault_inject_staging_removal,
                ));
            }
        };

        // 8. Final guard: verify backup file did not receive trailing modifications before removal
        let backup_check_result: std::io::Result<bool> = (|| {
            if hooks.fault_inject_final_backup_read {
                return Err(std::io::Error::other("Injected final backup read failure"));
            }
            let mut f = self.dir.open(&backup_rel)?;
            let mut b = Vec::new();
            f.read_to_end(&mut b)?;
            use sha2::{Digest, Sha256};
            let mut hasher = Sha256::new();
            hasher.update(&b);
            let h = format!("{:x}", hasher.finalize());
            Ok(h != expected)
        })();

        let trailing_modified_or_unverified = match backup_check_result {
            Ok(is_modified) => is_modified,
            Err(e) => {
                warn!(
                    "Final backup read on '{}' failed ({}). Preserving backup for data safety.",
                    user_path, e
                );
                true // A failed or incomplete backup read must NEVER be interpreted as proof unchanged
            }
        };

        if trailing_modified_or_unverified {
            let conflict_name = format!(
                "{}.aether-conflict-{}",
                user_path,
                uuid::Uuid::new_v4().simple()
            );
            let conflict_rel = PathBuf::from(&conflict_name);
            let rename_res = if hooks.fault_inject_conflict_rename {
                Err(std::io::Error::other("Injected conflict rename failure"))
            } else {
                self.dir.rename(&backup_rel, &self.dir, &conflict_rel)
            };

            let surviving_path = match rename_res {
                Ok(()) => {
                    let p = self.root_path.join(&conflict_rel);
                    warn!(
                        "Trailing modification or unverified state on backup of '{}'. Preserved at '{}'.",
                        user_path, p.display()
                    );
                    p
                }
                Err(rename_err) => {
                    let p = self.root_path.join(&backup_rel);
                    warn!(
                        "Backup preservation on '{}': conflict rename failed ({}). Preserved at original backup path '{}'.",
                        user_path, rename_err, p.display()
                    );
                    p
                }
            };

            return Ok(ConditionalReplaceOutcome {
                dest_path: dest_abs,
                preserved_backup: Some(surviving_path),
                residual_staging,
            });
        }

        // Invoke pre_cleanup_hook if configured (deterministic test injection)
        if let Some(hook) = hooks.pre_cleanup_hook {
            hook(&self.root_path.join(&backup_rel));
            // Re-verify after pre_cleanup_hook before actually removing
            let post_hook_read: std::io::Result<bool> = (|| {
                let mut f = self.dir.open(&backup_rel)?;
                let mut b = Vec::new();
                f.read_to_end(&mut b)?;
                use sha2::{Digest, Sha256};
                let mut hasher = Sha256::new();
                hasher.update(&b);
                let h = format!("{:x}", hasher.finalize());
                Ok(h != expected)
            })();

            let hook_modified = post_hook_read.unwrap_or(true);

            if hook_modified {
                let conflict_name = format!(
                    "{}.aether-conflict-{}",
                    user_path,
                    uuid::Uuid::new_v4().simple()
                );
                let conflict_rel = PathBuf::from(&conflict_name);
                let surviving = match self.dir.rename(&backup_rel, &self.dir, &conflict_rel) {
                    Ok(()) => self.root_path.join(&conflict_rel),
                    Err(_) => self.root_path.join(&backup_rel),
                };
                warn!(
                    "Post-verification modification detected on backup of '{}'. Preserved at '{}'.",
                    user_path,
                    surviving.display()
                );
                return Ok(ConditionalReplaceOutcome {
                    dest_path: dest_abs,
                    preserved_backup: Some(surviving),
                    residual_staging,
                });
            }
        }

        // Cleanup: remove verified backup file
        let remove_res = if hooks.fault_inject_backup_removal {
            Err(std::io::Error::other("Injected backup removal failure"))
        } else {
            self.dir.remove_file(&backup_rel)
        };

        match remove_res {
            Ok(()) => Ok(ConditionalReplaceOutcome {
                dest_path: dest_abs,
                preserved_backup: None,
                residual_staging,
            }),
            Err(rm_err) => {
                let surviving = self.root_path.join(&backup_rel);
                warn!(
                    "Backup cleanup on '{}' failed ({}). Preserved at original backup path '{}'.",
                    user_path,
                    rm_err,
                    surviving.display()
                );
                Ok(ConditionalReplaceOutcome {
                    dest_path: dest_abs,
                    preserved_backup: Some(surviving),
                    residual_staging,
                })
            }
        }
    }

    /// Conditionally replaces or creates a file inside workspace capability using staged verification.
    /// Backward-compatible wrapper around `replace_file_bytes_conditional_with_hooks`.
    pub fn replace_file_bytes_conditional(
        &self,
        user_path: &str,
        bytes: &[u8],
        expected_hash: Option<&str>,
        pre_replace_hook: Option<&dyn Fn(&Path)>,
    ) -> Result<PathBuf> {
        let hooks = ConditionalReplaceHooks {
            pre_replace_hook,
            ..Default::default()
        };
        let outcome =
            self.replace_file_bytes_conditional_with_hooks(user_path, bytes, expected_hash, hooks)?;
        Ok(outcome.dest_path)
    }

    /// Writes content to a file inside the capability using atomic handle-level operations.
    pub fn write_file_atomic(&self, user_path: &str, content: &str) -> Result<PathBuf> {
        self.write_file_bytes_atomic(user_path, content.as_bytes())
    }

    /// Creates directory inside workspace capability.
    pub fn create_dir(&self, user_path: &str) -> Result<PathBuf> {
        let rel = sanitize_relative_path(user_path)?;
        self.dir.create_dir_all(&rel).map_err(AetherError::Io)?;
        Ok(self.root_path.join(rel))
    }

    /// Returns symlink metadata for a path anchored inside workspace capability.
    pub fn symlink_metadata(&self, user_path: &str) -> Result<Metadata> {
        let rel = sanitize_relative_path(user_path)?;
        self.dir.symlink_metadata(&rel).map_err(AetherError::Io)
    }

    /// Deletes a file entry inside workspace capability.
    /// If target is a symlink, removes the symlink entry itself without following to target.
    pub fn delete_file(&self, user_path: &str) -> Result<()> {
        let rel = sanitize_relative_path(user_path)?;
        self.dir.remove_file(&rel).map_err(AetherError::Io)
    }

    fn preserve_staged_deletion_conflict(
        dir: &Dir,
        root_path: &Path,
        stage_rel: &Path,
        rel: &Path,
        user_path: &str,
    ) -> PathBuf {
        let conflict_name = format!(
            "{}.aether-del-conflict-{}",
            user_path,
            uuid::Uuid::new_v4().simple()
        );
        let conflict_rel = if let Some(parent) = rel.parent() {
            parent.join(&conflict_name)
        } else {
            PathBuf::from(&conflict_name)
        };

        let surviving = match dir.rename(stage_rel, dir, &conflict_rel) {
            Ok(()) => root_path.join(&conflict_rel),
            Err(e) => {
                warn!(
                    "Failed to rename staged conflict '{}' to '{}' ({}). Preserved at original stage path.",
                    stage_rel.display(),
                    conflict_rel.display(),
                    e
                );
                root_path.join(stage_rel)
            }
        };

        warn!(
            "Conditional deletion conflict for '{}': preserved at '{}'.",
            user_path,
            surviving.display()
        );
        surviving
    }

    /// Conditionally deletes a file inside workspace capability using staged verification.
    /// Safely moves the target file to staging, verifies its content digest matches `expected_hash`,
    /// catches open-handle modifications via pre-delete hooks, and only unlinks if verified.
    /// If the file was modified or cannot be verified, it is preserved without deletion.
    pub fn delete_file_conditional_with_hooks(
        &self,
        user_path: &str,
        expected_hash: &str,
        hooks: ConditionalDeleteHooks,
    ) -> Result<ConditionalDeleteOutcome> {
        let rel = sanitize_relative_path(user_path)?;
        let dest_abs = self.root_path.join(&rel);

        // 1. Inspect metadata
        let meta = match self.dir.symlink_metadata(&rel) {
            Ok(m) => m,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                // Confirmed absent!
                return Ok(ConditionalDeleteOutcome {
                    was_deleted: true,
                    surviving_path: None,
                    disposition: DeletionDisposition::ConfirmedAbsent,
                    quarantined: false,
                });
            }
            Err(e) => return Err(AetherError::Io(e)),
        };

        if meta.is_symlink() || !meta.is_file() {
            return Err(AetherError::PermissionDenied(format!(
                "Cannot conditionally delete '{}': target is a symlink or directory",
                user_path
            )));
        }

        // 2. Atomic staged move: rename target to delete-stage
        let stage_name = format!(".aether-del-stage-{}", uuid::Uuid::new_v4().simple());
        let stage_rel = if let Some(parent) = rel.parent() {
            parent.join(&stage_name)
        } else {
            PathBuf::from(&stage_name)
        };

        if let Err(e) = self.dir.rename(&rel, &self.dir, &stage_rel) {
            return Err(AetherError::PermissionDenied(format!(
                "Cannot conditionally delete '{}': failed to stage for verification ({})",
                user_path, e
            )));
        }

        // 3. Pre-delete hook (for deterministic test injection while file is staged)
        if let Some(hook) = hooks.pre_delete_hook {
            hook(&self.root_path.join(&stage_rel));
        }

        // 4. Verify captured stage file
        let stage_check_result: std::io::Result<bool> = (|| {
            let mut f = self.dir.open(&stage_rel)?;
            let mut b = Vec::new();
            f.read_to_end(&mut b)?;
            use sha2::{Digest, Sha256};
            let mut hasher = Sha256::new();
            hasher.update(&b);
            let h = format!("{:x}", hasher.finalize());
            Ok(h != expected_hash)
        })();

        let mut is_modified_or_unverified = match stage_check_result {
            Ok(is_modified) => is_modified,
            Err(e) => {
                warn!(
                    "Stage read on '{}' failed ({}). Preserving file for data safety.",
                    user_path, e
                );
                true // unverified state must be preserved
            }
        };

        // 5. Unconditional Pre-Unlink Verification:
        // Catches writes made through retained open handles between initial verification and unlink attempt.
        // Performs content verification unconditionally on EVERY deletion attempt that passes initial verification.
        if !is_modified_or_unverified {
            if let Some(hook) = hooks.post_hash_hook {
                hook(&self.root_path.join(&stage_rel));
            }

            let pre_unlink_check: std::io::Result<bool> = (|| {
                if hooks.fault_inject_second_verification_read_failure {
                    return Err(std::io::Error::other("Injected pre-unlink read failure"));
                }
                let mut f = self.dir.open(&stage_rel)?;
                let mut b = Vec::new();
                f.read_to_end(&mut b)?;
                use sha2::{Digest, Sha256};
                let mut hasher = Sha256::new();
                hasher.update(&b);
                let h = format!("{:x}", hasher.finalize());
                Ok(h != expected_hash)
            })();

            match pre_unlink_check {
                Ok(modified) => {
                    if modified {
                        warn!(
                            "Pre-unlink re-verification detected modification on '{}'. Aborting deletion to protect data.",
                            user_path
                        );
                        is_modified_or_unverified = true;
                    }
                }
                Err(e) => {
                    warn!(
                        "Pre-unlink re-verification read on '{}' failed ({}). Preserving file for data safety.",
                        user_path, e
                    );
                    // A failed read or hash operation must abort deletion and preserve the staged artifact.
                    is_modified_or_unverified = true;
                }
            }
        }

        // 5b. Pre-Unlink Hook (CHK-048: Simulates writes through retained handles immediately before physical deletion)
        if !is_modified_or_unverified {
            if let Some(hook) = hooks.pre_unlink_hook {
                hook(&self.root_path.join(&stage_rel));

                // Re-verify immediately before physical unlink attempt
                let post_hook_check: std::io::Result<bool> = (|| {
                    let mut f = self.dir.open(&stage_rel)?;
                    let mut b = Vec::new();
                    f.read_to_end(&mut b)?;
                    use sha2::{Digest, Sha256};
                    let mut hasher = Sha256::new();
                    hasher.update(&b);
                    let h = format!("{:x}", hasher.finalize());
                    Ok(h != expected_hash)
                })();

                match post_hook_check {
                    Ok(modified) => {
                        if modified {
                            warn!(
                                "Pre-unlink final check detected modification through retained handle on '{}'. Aborting deletion.",
                                user_path
                            );
                            is_modified_or_unverified = true;
                        }
                    }
                    Err(e) => {
                        warn!(
                            "Pre-unlink final check read failed on '{}' ({}). Aborting deletion to protect data.",
                            user_path, e
                        );
                        is_modified_or_unverified = true;
                    }
                }
            }
        }

        if is_modified_or_unverified {
            // Concurrent modification, open-handle mutation, or unverified state detected!
            // DO NOT DELETE! Enter strict no-overwrite rollback.

            // A. Inspect destination metadata: Treat ONLY NotFound as confirmed absence.
            // PermissionDenied, other I/O failures, and uncertain states are treated as conflicts.
            let dest_is_confirmed_absent = if hooks.fault_inject_rollback_metadata_failure {
                warn!(
                    "Fault-injected metadata inspection failure for '{}' during rollback. Treating uncertain state as conflict.",
                    user_path
                );
                false
            } else {
                match self.dir.symlink_metadata(&rel) {
                    Ok(_) => false, // Destination exists: not absent!
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => true, // ONLY NotFound is confirmed absence!
                    Err(e) => {
                        warn!(
                            "Destination metadata inspection for '{}' failed ({}) during rollback. Treating uncertain state as conflict.",
                            user_path, e
                        );
                        false
                    }
                }
            };

            let surviving = if !dest_is_confirmed_absent || hooks.fault_inject_rollback_failure {
                // Destination exists or is uncertain: preserve staged artifact under a unique conflict path!
                // NEVER overwrite or attempt to publish to an existing/uncertain destination!
                Self::preserve_staged_deletion_conflict(
                    &self.dir,
                    &self.root_path,
                    &stage_rel,
                    &rel,
                    user_path,
                )
            } else {
                // Destination is confirmed absent (NotFound).
                // Pre-hard-link hook (for deterministic test injection of race windows between check and link):
                if let Some(hook) = hooks.pre_hard_link_hook {
                    hook(&dest_abs);
                }

                // Atomic no-overwrite publication ONLY:
                let link_res = if hooks.fault_inject_hard_link_failure {
                    Err(std::io::Error::other(
                        "Injected non-AlreadyExists hard-link failure",
                    ))
                } else {
                    self.dir.hard_link(&stage_rel, &self.dir, &rel)
                };

                match link_res {
                    Ok(()) => {
                        // Hard-link succeeded! Remove the temporary stage link.
                        if let Err(rm_err) = self.dir.remove_file(&stage_rel) {
                            warn!(
                                "Failed to remove temporary stage link '{}' after hard-link rollback ({}) for '{}'. Staged artifact remains.",
                                stage_rel.display(),
                                rm_err,
                                user_path
                            );
                            // Do not report a clean destination rollback if a staged artifact remains on disk!
                            Self::preserve_staged_deletion_conflict(
                                &self.dir,
                                &self.root_path,
                                &stage_rel,
                                &rel,
                                user_path,
                            )
                        } else {
                            warn!(
                                "Conditional deletion aborted for '{}': concurrent modification detected. Restored to destination '{}'.",
                                user_path,
                                dest_abs.display()
                            );
                            dest_abs
                        }
                    }
                    Err(e) => {
                        // Hard-link publication failed for ANY reason (AlreadyExists, PermissionDenied, I/O error).
                        // STRICT SAFETY INVARIANT: ZERO OVERWRITE-CAPABLE FALLBACKS.
                        // Ordinary rename(stage_rel, rel) is strictly eliminated.
                        // Preserve the staged artifact under a unique conflict path!
                        warn!(
                            "Atomic no-overwrite rollback publication failed for '{}' ({}). Preserving staged artifact under unique conflict path.",
                            user_path, e
                        );
                        Self::preserve_staged_deletion_conflict(
                            &self.dir,
                            &self.root_path,
                            &stage_rel,
                            &rel,
                            user_path,
                        )
                    }
                }
            };

            return Ok(ConditionalDeleteOutcome {
                was_deleted: false,
                surviving_path: Some(surviving.clone()),
                disposition: DeletionDisposition::ConflictPreserved {
                    conflict_path: surviving,
                },
                quarantined: false,
            });
        }

        // 5c. Defer to Quarantine (prefer recoverability over irreversible deletion)
        if hooks.defer_to_quarantine {
            let quarantine_path = if let Some(ref q_dir) = hooks.quarantine_dir {
                std::fs::create_dir_all(q_dir).map_err(AetherError::Io)?;
                let file_stem = rel.file_name().and_then(|n| n.to_str()).unwrap_or("file");
                let safe_stem: String = file_stem
                    .chars()
                    .filter(|c| c.is_alphanumeric() || *c == '.' || *c == '-' || *c == '_')
                    .collect();
                let clean_stem = if safe_stem.is_empty() {
                    "file"
                } else {
                    &safe_stem
                };
                let q_file_name = format!(
                    "{}.aether-quarantine-{}",
                    clean_stem,
                    uuid::Uuid::new_v4().simple()
                );
                let target = q_dir.join(&q_file_name);
                let stage_abs = self.root_path.join(&stage_rel);
                match std::fs::rename(&stage_abs, &target) {
                    Ok(()) => target,
                    Err(e) if is_cross_device_error(&e) => {
                        std::fs::copy(&stage_abs, &target).map_err(AetherError::Io)?;
                        let copied_bytes = std::fs::read(&target).map_err(AetherError::Io)?;
                        use sha2::{Digest, Sha256};
                        let mut hasher = Sha256::new();
                        hasher.update(&copied_bytes);
                        let copied_hash = format!("{:x}", hasher.finalize());
                        if copied_hash != expected_hash {
                            let _ = std::fs::remove_file(&target);
                            return Err(AetherError::Internal(format!(
                                "Quarantine copy hash verification failed for '{}'",
                                user_path
                            )));
                        }
                        // Re-verify source stability before removing stage file
                        let source_bytes = self.dir.read(&stage_rel).map_err(AetherError::Io)?;
                        let mut src_hasher = Sha256::new();
                        src_hasher.update(&source_bytes);
                        let src_hash = format!("{:x}", src_hasher.finalize());
                        if src_hash != expected_hash {
                            return Err(AetherError::PermissionDenied(format!(
                                "Concurrent source modification detected during cross-device quarantine copy for '{}'. Staged source preserved without deletion.",
                                user_path
                            )));
                        }

                        // Pre-unlink hook injection point (simulating writes through retained handles)
                        if let Some(ref hook) = hooks.pre_unlink_hook {
                            hook(&stage_abs);
                        }

                        #[cfg(not(windows))]
                        {
                            // On POSIX systems (Linux, macOS), mandatory locking against uncooperative processes holding
                            // open file descriptors is not supported by standard kernel semantics. Therefore, exclusive
                            // stability through destructive operations cannot be proven, and the staged source must never be unlinked.
                            return Err(AetherError::PermissionDenied(format!(
                                "Cross-device quarantine fallback cannot guarantee exclusive source stability on POSIX systems against retained open descriptors for '{}'. Staged source preserved at '{}' and quarantine copy preserved at '{}'.",
                                user_path,
                                stage_abs.display(),
                                target.display()
                            )));
                        }

                        #[cfg(windows)]
                        {
                            // On Windows, use exclusive handle deletion:
                            // 1. Open with FILE_SHARE_READ (denying concurrent writes and deletes) and DELETE permission.
                            //    If any concurrent process holds a write or delete handle, open fails immediately.
                            // 2. While holding the handle, read bytes and verify hash against expected_hash.
                            // 3. If hash matches, set FileDispositionInfo (DeleteFile = 1) on the handle.
                            // 4. Closing the handle unlinks the file atomically at kernel level without race.
                            if let Err(err) =
                                remove_file_windows_exclusive(&stage_abs, expected_hash, None, None)
                            {
                                return Err(AetherError::PermissionDenied(format!(
                                    "Exclusive source stability could not be verified for '{}' ({}). Staged source preserved at '{}' and quarantine copy preserved at '{}'.",
                                    user_path, err, stage_abs.display(), target.display()
                                )));
                            }
                            target
                        }
                    }
                    Err(e) => {
                        return Err(AetherError::Io(e));
                    }
                }
            } else {
                let safe_user: String = user_path
                    .chars()
                    .filter(|c| c.is_alphanumeric() || *c == '.' || *c == '-' || *c == '_')
                    .collect();
                let clean_name = if safe_user.is_empty() {
                    "file"
                } else {
                    &safe_user
                };
                let q_name = format!(
                    "{}.aether-quarantine-{}",
                    clean_name,
                    uuid::Uuid::new_v4().simple()
                );
                let q_rel = PathBuf::from(q_name);
                match self.dir.rename(&stage_rel, &self.dir, &q_rel) {
                    Ok(()) => self.root_path.join(&q_rel),
                    Err(_) => self.root_path.join(&stage_rel),
                }
            };

            warn!(
                "Conditional deletion deferred to quarantine for '{}': preserved at '{}'.",
                user_path,
                quarantine_path.display()
            );

            return Ok(ConditionalDeleteOutcome {
                was_deleted: false,
                surviving_path: Some(quarantine_path.clone()),
                disposition: DeletionDisposition::Quarantined { quarantine_path },
                quarantined: true,
            });
        }

        // 6. Cleanup: unlink verified stage file
        let remove_res = if hooks.fault_inject_delete_removal {
            Err(std::io::Error::other("Injected delete removal failure"))
        } else {
            self.dir.remove_file(&stage_rel)
        };

        match remove_res {
            Ok(()) => Ok(ConditionalDeleteOutcome {
                was_deleted: true,
                surviving_path: None,
                disposition: DeletionDisposition::Unlinked,
                quarantined: false,
            }),
            Err(rm_err) => {
                let surviving = Self::preserve_staged_deletion_conflict(
                    &self.dir,
                    &self.root_path,
                    &stage_rel,
                    &rel,
                    user_path,
                );
                warn!(
                    "Stage cleanup failed ({}) for '{}'. Preserved as conflict at '{}'.",
                    rm_err,
                    user_path,
                    surviving.display()
                );
                Ok(ConditionalDeleteOutcome {
                    was_deleted: false,
                    surviving_path: Some(surviving.clone()),
                    disposition: DeletionDisposition::ConflictPreserved {
                        conflict_path: surviving,
                    },
                    quarantined: false,
                })
            }
        }
    }

    /// Conditionally deletes a file inside workspace capability using staged verification.
    /// Backward-compatible wrapper around `delete_file_conditional_with_hooks`.
    pub fn delete_file_conditional(
        &self,
        user_path: &str,
        expected_hash: &str,
    ) -> Result<ConditionalDeleteOutcome> {
        self.delete_file_conditional_with_hooks(
            user_path,
            expected_hash,
            ConditionalDeleteHooks::default(),
        )
    }

    /// Deletes a directory and its contents recursively inside workspace capability.
    /// Strictly refuses to follow or delete through external symlinks or reparse points.
    pub fn delete_dir_all(&self, user_path: &str) -> Result<()> {
        let rel = sanitize_relative_path(user_path)?;
        if let Ok(meta) = self.dir.symlink_metadata(&rel) {
            if meta.is_symlink() {
                return Err(AetherError::PermissionDenied(
                    "Access Denied: Target is a symlink, cannot recursively delete".into(),
                ));
            }
        }
        self.dir.remove_dir_all(&rel).map_err(AetherError::Io)
    }

    /// Renames a file entry within the workspace capability.
    pub fn rename(&self, from_user_path: &str, to_user_path: &str) -> Result<()> {
        let from_rel = sanitize_relative_path(from_user_path)?;
        let to_rel = sanitize_relative_path(to_user_path)?;

        if let Some(parent) = to_rel.parent() {
            if !parent.as_os_str().is_empty() {
                self.dir.create_dir_all(parent).map_err(AetherError::Io)?;
            }
        }

        // Verify destination is not an external symlink
        if let Ok(meta) = self.dir.symlink_metadata(&to_rel) {
            if meta.is_symlink() {
                return Err(AetherError::PermissionDenied(
                    "Access Denied: Destination is a symlink".into(),
                ));
            }
        }

        self.dir
            .rename(&from_rel, &self.dir, &to_rel)
            .map_err(AetherError::Io)
    }

    /// Recursively scans workspace files anchored to directory capability.
    /// Skips all symbolic links and reparse points by capability policy.
    pub fn scan_workspace_files(&self, exclude_patterns: &[String]) -> Result<Vec<PathBuf>> {
        let mut results = Vec::new();
        self.walk_dir_cap(&PathBuf::new(), exclude_patterns, &mut results)?;
        Ok(results)
    }

    /// Recursively scans workspace files anchored to directory capability, returning paths relative to workspace root.
    /// Skips all symbolic links, junctions, and reparse points by capability policy.
    pub fn scan_workspace_relative_files(
        &self,
        exclude_patterns: &[String],
    ) -> Result<Vec<PathBuf>> {
        let report = self.scan_workspace_detailed(exclude_patterns)?;
        Ok(report.discovered_files)
    }

    /// Performs an audited workspace scan returning a comprehensive report detailing discovered files,
    /// skipped symlinks, policy exclusions, unreadable directories, and scan completeness.
    pub fn scan_workspace_detailed(
        &self,
        exclude_patterns: &[String],
    ) -> Result<WorkspaceScanReport> {
        let mut report = WorkspaceScanReport {
            is_complete: true,
            ..Default::default()
        };
        self.walk_dir_cap_relative_detailed(&PathBuf::new(), exclude_patterns, &mut report)?;
        Ok(report)
    }

    fn walk_dir_cap_relative_detailed(
        &self,
        current_rel: &Path,
        exclude_patterns: &[String],
        report: &mut WorkspaceScanReport,
    ) -> Result<()> {
        let read_dir_result = if current_rel.as_os_str().is_empty() {
            self.dir.read_dir(".")
        } else {
            self.dir.read_dir(current_rel)
        };

        let entries = match read_dir_result {
            Ok(e) => e,
            Err(e) => {
                report.is_complete = false;
                if e.kind() == std::io::ErrorKind::PermissionDenied {
                    report.permission_errors.push(current_rel.to_path_buf());
                } else {
                    report.transient_errors.push(current_rel.to_path_buf());
                }
                return Ok(());
            }
        };

        report.directories_scanned += 1;

        for entry_res in entries {
            let entry = match entry_res {
                Ok(e) => e,
                Err(_) => {
                    report.is_complete = false;
                    report.transient_errors.push(current_rel.to_path_buf());
                    continue;
                }
            };

            let file_name = entry.file_name();
            let file_name_str = file_name.to_string_lossy();

            if exclude_patterns
                .iter()
                .any(|pattern| file_name_str == *pattern || file_name_str.starts_with('.'))
            {
                report.skipped_by_policy += 1;
                continue;
            }

            let entry_rel = if current_rel.as_os_str().is_empty() {
                PathBuf::from(&file_name)
            } else {
                current_rel.join(&file_name)
            };

            let meta = match self.dir.symlink_metadata(&entry_rel) {
                Ok(m) => m,
                Err(_) => {
                    report.is_complete = false;
                    report.transient_errors.push(entry_rel);
                    continue;
                }
            };

            // By capability policy, skip symbolic links and reparse points
            if meta.is_symlink() {
                report.skipped_symlinks += 1;
                continue;
            }

            if meta.is_dir() {
                let _ = self.walk_dir_cap_relative_detailed(&entry_rel, exclude_patterns, report);
            } else if meta.is_file() {
                report.files_discovered += 1;
                report.discovered_files.push(entry_rel);
            }
        }

        Ok(())
    }

    fn walk_dir_cap(
        &self,
        current_rel: &Path,
        exclude_patterns: &[String],
        results: &mut Vec<PathBuf>,
    ) -> Result<()> {
        let read_dir_result = if current_rel.as_os_str().is_empty() {
            self.dir.read_dir(".")
        } else {
            self.dir.read_dir(current_rel)
        };

        let entries = match read_dir_result {
            Ok(e) => e,
            Err(_) => return Ok(()),
        };

        for entry in entries.flatten() {
            let file_name = entry.file_name();
            let file_name_str = file_name.to_string_lossy();

            if exclude_patterns
                .iter()
                .any(|pattern| file_name_str == *pattern || file_name_str.starts_with('.'))
            {
                continue;
            }

            let entry_rel = if current_rel.as_os_str().is_empty() {
                PathBuf::from(&file_name)
            } else {
                current_rel.join(&file_name)
            };

            let meta = match self.dir.symlink_metadata(&entry_rel) {
                Ok(m) => m,
                Err(_) => continue,
            };

            // By capability policy, skip symbolic links and reparse points
            if meta.is_symlink() {
                continue;
            }

            if meta.is_dir() {
                let _ = self.walk_dir_cap(&entry_rel, exclude_patterns, results);
            } else if meta.is_file() {
                results.push(self.root_path.join(&entry_rel));
            }
        }

        Ok(())
    }
}

pub struct FilesystemEngine;

impl FilesystemEngine {
    /// Recursively scans workspace files anchored to directory capability.
    pub fn scan_workspace_files<P: AsRef<Path>>(
        root: P,
        exclude_patterns: &[String],
    ) -> Result<Vec<PathBuf>> {
        let cap = WorkspaceCapability::open(root)?;
        cap.scan_workspace_files(exclude_patterns)
    }

    pub fn write_file_bytes_atomic<P: AsRef<Path>>(path: P, bytes: &[u8]) -> Result<()> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let temp_path = path.with_extension(format!("tmp-{}", uuid::Uuid::new_v4().simple()));
        fs::write(&temp_path, bytes)?;
        fs::rename(&temp_path, path)?;
        Ok(())
    }

    pub fn write_file_atomic<P: AsRef<Path>>(path: P, content: &str) -> Result<()> {
        Self::write_file_bytes_atomic(path, content.as_bytes())
    }

    pub fn read_file_string<P: AsRef<Path>>(path: P) -> Result<String> {
        fs::read_to_string(path).map_err(AetherError::Io)
    }

    /// Verifies that target is strictly contained within root on a component-by-component basis.
    /// Prevents substring confusion (e.g. `workspace-external` vs `workspace`).
    pub fn is_within_boundary(target: &Path, root: &Path) -> bool {
        let target_comps: Vec<_> = target.components().collect();
        let root_comps: Vec<_> = root.components().collect();
        if target_comps.len() < root_comps.len() {
            return false;
        }
        for (t, r) in target_comps.iter().zip(root_comps.iter()) {
            if t != r {
                return false;
            }
        }
        true
    }

    /// Resolves a user-provided relative path against a workspace root.
    pub fn resolve_contained_path(root: &Path, user_path: &str) -> Result<PathBuf> {
        let clean = sanitize_relative_path(user_path)?;
        let root_canon = root.canonicalize().map_err(AetherError::Io)?;
        let candidate = root_canon.join(&clean);

        if candidate.exists() {
            let candidate_canon = candidate.canonicalize().map_err(AetherError::Io)?;
            if !Self::is_within_boundary(&candidate_canon, &root_canon) {
                return Err(AetherError::PermissionDenied(
                    "Access Denied: Target path escapes workspace boundary via symlink".into(),
                ));
            }
            Ok(candidate_canon)
        } else {
            let mut ancestor = candidate.as_path();
            while let Some(parent) = ancestor.parent() {
                if parent.exists() {
                    let parent_canon = parent.canonicalize().map_err(AetherError::Io)?;
                    if !Self::is_within_boundary(&parent_canon, &root_canon) {
                        return Err(AetherError::PermissionDenied(
                            "Access Denied: Ancestor directory escapes workspace boundary".into(),
                        ));
                    }
                    break;
                }
                ancestor = parent;
            }
            Ok(candidate)
        }
    }

    /// Safely writes content to a file inside the workspace root using capability-anchored I/O.
    pub fn write_file_contained<P: AsRef<Path>>(
        root: P,
        user_path: &str,
        content: &str,
    ) -> Result<PathBuf> {
        let cap = WorkspaceCapability::open(root)?;
        cap.write_file_atomic(user_path, content)
    }

    /// Safely reads the text content of a file strictly anchored inside the workspace capability.
    pub fn read_file_contained<P: AsRef<Path>>(root: P, user_path: &str) -> Result<String> {
        let cap = WorkspaceCapability::open(root)?;
        cap.read_file(user_path)
    }

    /// Safely deletes a file contained within the workspace root without following external symlinks.
    pub fn delete_file_contained<P: AsRef<Path>>(root: P, user_path: &str) -> Result<()> {
        let cap = WorkspaceCapability::open(root)?;
        cap.delete_file(user_path)
    }

    /// Safely deletes a directory and its contents contained within the workspace root.
    pub fn delete_dir_all_contained<P: AsRef<Path>>(root: P, user_path: &str) -> Result<()> {
        let cap = WorkspaceCapability::open(root)?;
        cap.delete_dir_all(user_path)
    }

    /// Safely creates a directory contained within the workspace root.
    pub fn create_dir_contained<P: AsRef<Path>>(root: P, user_path: &str) -> Result<PathBuf> {
        let cap = WorkspaceCapability::open(root)?;
        cap.create_dir(user_path)
    }

    /// Safely renames or moves a file within the workspace root without crossing boundaries.
    pub fn rename_contained<P: AsRef<Path>>(
        root: P,
        from_user_path: &str,
        to_user_path: &str,
    ) -> Result<()> {
        let cap = WorkspaceCapability::open(root)?;
        cap.rename(from_user_path, to_user_path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_query_directory_identity_valid_and_rejections() {
        let temp_dir = std::env::temp_dir().join(format!(
            "aether_fs_dir_id_{}",
            uuid::Uuid::new_v4().simple()
        ));
        let dir_a = temp_dir.join("dir_a");
        let dir_b = temp_dir.join("dir_b");
        let file_a = temp_dir.join("file_a.txt");
        let non_existent = temp_dir.join("non_existent");

        fs::create_dir_all(&dir_a).unwrap();
        fs::create_dir_all(&dir_b).unwrap();
        fs::write(&file_a, b"regular file content").unwrap();

        // 1. Valid directories return unique identities
        let id_a = query_directory_identity(&dir_a).expect("query_directory_identity on dir_a");
        let id_b = query_directory_identity(&dir_b).expect("query_directory_identity on dir_b");
        assert_ne!(
            id_a, id_b,
            "Different directories must have distinct identities"
        );

        // Querying dir_a again returns the same identity
        let id_a_second = query_directory_identity(&dir_a).expect("second query on dir_a");
        assert_eq!(id_a, id_a_second);

        // 2. Reject regular file
        let file_err = query_directory_identity(&file_a);
        assert!(
            file_err.is_err(),
            "query_directory_identity must reject regular files"
        );

        // 3. Non-existent path returns error
        let missing_err = query_directory_identity(&non_existent);
        assert!(
            missing_err.is_err(),
            "query_directory_identity must fail for non-existent path"
        );

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_pinned_directory_lifecycle_and_rejections() {
        let temp_dir = std::env::temp_dir().join(format!(
            "aether_fs_pin_test_{}",
            uuid::Uuid::new_v4().simple()
        ));
        let dir_target = temp_dir.join("pinned_dir");
        let regular_file = temp_dir.join("file.txt");
        let non_empty_dir = temp_dir.join("non_empty");

        fs::create_dir_all(&temp_dir).unwrap();
        fs::create_dir(&dir_target).unwrap();
        fs::write(&regular_file, b"content").unwrap();
        fs::create_dir(&non_empty_dir).unwrap();
        fs::write(non_empty_dir.join("nested.txt"), b"data").unwrap();

        // 1. Regular file rejected
        assert!(pin_created_directory(&regular_file).is_err());

        // 2. Non-empty directory rejected
        assert!(pin_created_directory(&non_empty_dir).is_err());

        // 3. Valid empty directory pinned successfully
        let pinned = pin_created_directory(&dir_target).expect("pin empty directory");
        assert_eq!(pinned.path(), dir_target.as_path());

        // Add children inside pinned directory
        let child_file = dir_target.join("child.txt");
        let child_sub = dir_target.join("sub");
        fs::write(&child_file, b"child data").unwrap();
        fs::create_dir(&child_sub).unwrap();
        fs::write(child_sub.join("subfile.txt"), b"sub content").unwrap();

        // 4. Object-bound clean_contents empties directory without deleting dir_target
        pinned.clean_contents().expect("clean_contents");
        assert!(dir_target.exists());
        assert!(!child_file.exists());
        assert!(!child_sub.exists());

        // 5. verify_and_remove_entry safely removes empty directory entry
        pinned
            .verify_and_remove_entry()
            .expect("verify_and_remove_entry");
        assert!(!dir_target.exists());

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_atomic_write_and_read() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_fs_test_{}", uuid::Uuid::new_v4().simple()));
        let test_file = temp_dir.join("subdir").join("test.txt");

        FilesystemEngine::write_file_atomic(&test_file, "Hello AetherIDE Atomic")
            .expect("atomic write failed");
        let content = FilesystemEngine::read_file_string(&test_file).expect("read failed");
        assert_eq!(content, "Hello AetherIDE Atomic");

        let _ = fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn test_resolve_contained_path_blocks_escape_attempts() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_fs_sec_{}", uuid::Uuid::new_v4().simple()));
        let _ = fs::create_dir_all(&temp_dir);

        // 1. Valid contained path resolves
        let valid = FilesystemEngine::resolve_contained_path(&temp_dir, "src/main.rs");
        assert!(valid.is_ok());

        // 2. Relative traversal attempt is rejected
        let escape = FilesystemEngine::resolve_contained_path(&temp_dir, "../outside.txt");
        assert!(escape.is_err());

        let nested_escape =
            FilesystemEngine::resolve_contained_path(&temp_dir, "foo/bar/../../../../secret.key");
        assert!(nested_escape.is_err());

        // 3. Absolute path is rejected
        #[cfg(windows)]
        {
            let abs = FilesystemEngine::resolve_contained_path(
                &temp_dir,
                "C:\\Windows\\System32\\cmd.exe",
            );
            assert!(abs.is_err());
        }

        // 4. Windows reserved device name is rejected
        let dev = FilesystemEngine::resolve_contained_path(&temp_dir, "CON.txt");
        assert!(dev.is_err());

        let _ = fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn test_resolve_contained_path_advanced_attacks() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_fs_adv_{}", uuid::Uuid::new_v4().simple()));
        let _ = fs::create_dir_all(&temp_dir);

        // 1. Alternate Data Streams (ADS) rejected
        let ads = FilesystemEngine::resolve_contained_path(&temp_dir, "test.txt:hidden_stream");
        assert!(ads.is_err());
        assert!(ads.unwrap_err().to_string().contains("Stream separator"));

        let ads_data = FilesystemEngine::resolve_contained_path(&temp_dir, "test.txt::$DATA");
        assert!(ads_data.is_err());

        // 2. Trailing dots and spaces rejected (prevents Windows silent truncation)
        let dot_bypass = FilesystemEngine::resolve_contained_path(&temp_dir, "secret.rs.");
        assert!(dot_bypass.is_err());

        let space_bypass = FilesystemEngine::resolve_contained_path(&temp_dir, "secret.rs ");
        assert!(space_bypass.is_err());

        // 3. UNC network paths rejected
        let unc_bs =
            FilesystemEngine::resolve_contained_path(&temp_dir, r"\\malicious\share\evil.exe");
        assert!(unc_bs.is_err());

        let unc_fs =
            FilesystemEngine::resolve_contained_path(&temp_dir, "//malicious/share/evil.exe");
        assert!(unc_fs.is_err());

        // 4. Extended device namespaces rejected
        let ext_ns = FilesystemEngine::resolve_contained_path(&temp_dir, r"\??\C:\Windows\cmd.exe");
        assert!(ext_ns.is_err());

        // 5. Null byte injection rejected
        let null_byte = FilesystemEngine::resolve_contained_path(&temp_dir, "photo.png\0.exe");
        assert!(null_byte.is_err());

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_contained_io_operations() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_fs_io_{}", uuid::Uuid::new_v4().simple()));
        let _ = fs::create_dir_all(&temp_dir);

        // 1. Contained write and read
        let written_path = FilesystemEngine::write_file_contained(
            &temp_dir,
            "nested/dir/test.txt",
            "Secure Content AetherIDE",
        )
        .expect("write_file_contained failed");
        assert!(written_path.exists());

        let read_content = FilesystemEngine::read_file_contained(&temp_dir, "nested/dir/test.txt")
            .expect("read_file_contained failed");
        assert_eq!(read_content, "Secure Content AetherIDE");

        // 2. Traversal write rejected
        let bad_write =
            FilesystemEngine::write_file_contained(&temp_dir, "../escape.txt", "Malicious Data");
        assert!(bad_write.is_err());

        // 3. Contained rename
        let rename_res = FilesystemEngine::rename_contained(
            &temp_dir,
            "nested/dir/test.txt",
            "nested/dir/renamed.txt",
        );
        assert!(rename_res.is_ok());
        assert!(!written_path.exists());

        let renamed_content =
            FilesystemEngine::read_file_contained(&temp_dir, "nested/dir/renamed.txt")
                .expect("read renamed failed");
        assert_eq!(renamed_content, "Secure Content AetherIDE");

        // 4. Contained delete
        let del_res = FilesystemEngine::delete_file_contained(&temp_dir, "nested/dir/renamed.txt");
        assert!(del_res.is_ok());
        assert!(
            FilesystemEngine::read_file_contained(&temp_dir, "nested/dir/renamed.txt").is_err()
        );

        // 5. Component prefix evasion rejected (e.g. workspace vs workspace-extra)
        let root_str = temp_dir.to_string_lossy().to_string();
        let evil_sibling = PathBuf::from(format!("{}-evil", root_str));
        assert!(!FilesystemEngine::is_within_boundary(
            &evil_sibling,
            &temp_dir
        ));

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_walk_dir_blocks_external_symlinks() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_fs_walk_{}", uuid::Uuid::new_v4().simple()));
        let outside_dir = std::env::temp_dir().join(format!(
            "aether_fs_outside_{}",
            uuid::Uuid::new_v4().simple()
        ));

        let _ = fs::create_dir_all(&temp_dir);
        let _ = fs::create_dir_all(&outside_dir);

        // Inside file
        let inside_file = temp_dir.join("inside.rs");
        let _ = fs::write(&inside_file, "fn inside() {}");

        // Outside file
        let outside_file = outside_dir.join("secret_outside.rs");
        let _ = fs::write(&outside_file, "CRITICAL_OUTSIDE_SECRET");

        // Attempt symlink inside workspace pointing outside
        let link_path = temp_dir.join("link_to_outside");
        #[cfg(windows)]
        let _ = std::os::windows::fs::symlink_dir(&outside_dir, &link_path);
        #[cfg(unix)]
        let _ = std::os::unix::fs::symlink(&outside_dir, &link_path);

        // Scan workspace
        let scanned = FilesystemEngine::scan_workspace_files(&temp_dir, &[])
            .expect("scan_workspace_files must succeed");

        // Verify that outside file was NOT scanned or leaked!
        for file in &scanned {
            let canon = file.canonicalize().unwrap_or_else(|_| file.clone());
            assert!(
                FilesystemEngine::is_within_boundary(&canon, &temp_dir.canonicalize().unwrap()),
                "Scanned file escaped workspace boundary: {:?}",
                file
            );
            assert!(
                !file.to_string_lossy().contains("secret_outside"),
                "External file appeared in scan results: {:?}",
                file
            );
        }

        let _ = fs::remove_dir_all(temp_dir);
        let _ = fs::remove_dir_all(outside_dir);
    }

    #[test]
    fn test_detailed_scan_report_metrics() {
        let temp_dir = std::env::temp_dir().join(format!(
            "aether_fs_detailed_{}",
            uuid::Uuid::new_v4().simple()
        ));
        let _ = fs::create_dir_all(temp_dir.join("src"));
        let _ = fs::create_dir_all(temp_dir.join(".git"));
        let _ = fs::write(temp_dir.join("src/lib.rs"), "pub fn test() {}");
        let _ = fs::write(temp_dir.join("src/main.rs"), "fn main() {}");
        let _ = fs::write(temp_dir.join(".git/config"), "secret");

        let cap = WorkspaceCapability::open(&temp_dir).expect("Capability open failed");
        let report = cap
            .scan_workspace_detailed(&[".git".into()])
            .expect("Scan failed");

        assert_eq!(report.files_discovered, 2);
        assert!(report
            .discovered_files
            .iter()
            .any(|f| f.ends_with("lib.rs")));
        assert!(report
            .discovered_files
            .iter()
            .any(|f| f.ends_with("main.rs")));
        assert!(!report
            .discovered_files
            .iter()
            .any(|f| f.to_string_lossy().contains(".git")));
        assert!(report.is_complete);
        assert_eq!(report.permission_errors.len(), 0);

        let _ = fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn test_replace_file_bytes_conditional_missing_and_present() {
        let temp_dir = std::env::temp_dir().join(format!(
            "aether_fs_conditional_{}",
            uuid::Uuid::new_v4().simple()
        ));
        let _ = fs::create_dir_all(&temp_dir);
        let cap = WorkspaceCapability::open(&temp_dir).unwrap();

        // 1. Missing file creation succeeds when truly missing
        let res_missing =
            cap.replace_file_bytes_conditional("new_file.txt", b"initial content", None, None);
        assert!(res_missing.is_ok());
        assert_eq!(
            fs::read_to_string(temp_dir.join("new_file.txt")).unwrap(),
            "initial content"
        );

        // 2. Missing file creation fails atomically when file already exists
        let res_exists =
            cap.replace_file_bytes_conditional("new_file.txt", b"should not overwrite", None, None);
        assert!(res_exists.is_err());
        assert!(matches!(
            res_exists.err().unwrap(),
            AetherError::PermissionDenied(_)
        ));
        assert_eq!(
            fs::read_to_string(temp_dir.join("new_file.txt")).unwrap(),
            "initial content"
        );

        // Compute hash of "initial content"
        let initial_hash = {
            use sha2::{Digest, Sha256};
            let mut hasher = Sha256::new();
            hasher.update(b"initial content");
            format!("{:x}", hasher.finalize())
        };

        // 3. Conditional replacement succeeds when hash matches
        let res_match = cap.replace_file_bytes_conditional(
            "new_file.txt",
            b"updated version 2",
            Some(&initial_hash),
            None,
        );
        assert!(res_match.is_ok());
        assert_eq!(
            fs::read_to_string(temp_dir.join("new_file.txt")).unwrap(),
            "updated version 2"
        );

        // 4. Conditional replacement detects mismatch and rolls back without data loss
        let res_mismatch = cap.replace_file_bytes_conditional(
            "new_file.txt",
            b"overwritten version 3",
            Some("wrong_hash_deadbeef"),
            None,
        );
        assert!(res_mismatch.is_err());
        assert!(matches!(
            res_mismatch.err().unwrap(),
            AetherError::PermissionDenied(_)
        ));
        // Data rolled back and intact!
        assert_eq!(
            fs::read_to_string(temp_dir.join("new_file.txt")).unwrap(),
            "updated version 2"
        );

        let _ = fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn test_post_hash_modification_via_open_handle_detected_and_rolled_back() {
        let temp_dir = std::env::temp_dir().join(format!(
            "aether_post_hash_race_{}",
            uuid::Uuid::new_v4().simple()
        ));
        let _ = fs::create_dir_all(&temp_dir);
        let cap = WorkspaceCapability::open(&temp_dir).unwrap();

        let file_path = temp_dir.join("live_document.txt");
        fs::write(&file_path, "initial base version").unwrap();

        let initial_hash = {
            use sha2::{Digest, Sha256};
            let mut hasher = Sha256::new();
            hasher.update(b"initial base version");
            format!("{:x}", hasher.finalize())
        };

        // Open handle to file before restore begins and hold it
        let handle = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&file_path)
            .expect("Must be able to open handle");

        let handle_cell = std::sync::Mutex::new(Some(handle));

        // Post-hash hook: writes into the open handle right after initial hash verification
        let post_hash_hook = |_dest_path: &Path| {
            let mut lock = handle_cell.lock().unwrap();
            if let Some(ref mut h) = *lock {
                use std::io::Seek;
                h.seek(std::io::SeekFrom::Start(0)).unwrap();
                h.write_all(b"CONCURRENT MUTATION VIA OPEN HANDLE!")
                    .unwrap();
                h.set_len(b"CONCURRENT MUTATION VIA OPEN HANDLE!".len() as u64)
                    .unwrap();
                h.sync_all().unwrap();
            }
        };

        let hooks = ConditionalReplaceHooks {
            post_hash_hook: Some(&post_hash_hook),
            ..Default::default()
        };

        let res = cap.replace_file_bytes_conditional_with_hooks(
            "live_document.txt",
            b"RESTORED OVERWRITE FROM CHECKPOINT",
            Some(&initial_hash),
            hooks,
        );

        // Verification:
        // 1. Must fail because post-hash check detects the open handle modification
        assert!(
            res.is_err(),
            "Must reject restore when open handle mutated file post-hash check"
        );
        let err_str = res.err().unwrap().to_string();
        assert!(
            err_str.contains("modified via open handle after hash verification"),
            "Error must specify open handle modification detection. Got: {}",
            err_str
        );

        // 2. Drop the handle before inspecting destination
        drop(handle_cell);

        // 3. User's concurrent modification must NOT be lost! Target was rolled back intact!
        let content_on_disk = fs::read_to_string(&file_path).unwrap();
        assert_eq!(
            content_on_disk, "CONCURRENT MUTATION VIA OPEN HANDLE!",
            "Concurrent modification must be preserved at destination intact! Got: {}",
            content_on_disk
        );

        let _ = fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn test_rollback_failure_observable_preserves_backup_without_silent_loss() {
        let temp_dir = std::env::temp_dir().join(format!(
            "aether_rollback_fail_{}",
            uuid::Uuid::new_v4().simple()
        ));
        let _ = fs::create_dir_all(&temp_dir);
        let cap = WorkspaceCapability::open(&temp_dir).unwrap();

        let file_path = temp_dir.join("critical_code.rs");
        fs::write(&file_path, "important user code v1").unwrap();

        let initial_hash = {
            use sha2::{Digest, Sha256};
            let mut hasher = Sha256::new();
            hasher.update(b"important user code v1");
            format!("{:x}", hasher.finalize())
        };

        // Mutate on disk to trigger rollback, AND inject rollback failure
        let pre_hook = |dest_path: &Path| {
            fs::write(dest_path, "concurrent edit v2").unwrap();
        };

        let hooks = ConditionalReplaceHooks {
            pre_replace_hook: Some(&pre_hook),
            fault_inject_rollback_failure: true,
            ..Default::default()
        };

        let res = cap.replace_file_bytes_conditional_with_hooks(
            "critical_code.rs",
            b"restored code v3",
            Some(&initial_hash),
            hooks,
        );

        assert!(res.is_err(), "Must report failure");
        let err = res.err().unwrap();
        assert!(matches!(err, AetherError::Internal(_)));
        let msg = err.to_string();

        // Must explicitly report RESTORE ROLLBACK FAILURE and recovery path
        assert!(
            msg.contains("RESTORE ROLLBACK FAILURE"),
            "Error must explicitly announce rollback failure: {}",
            msg
        );
        assert!(
            msg.contains("User data PRESERVED at"),
            "Error must state that user data is preserved: {}",
            msg
        );
        assert!(
            msg.contains(".aether-backup-"),
            "Error must specify the backup path: {}",
            msg
        );

        // Verify that the backup file actually exists on disk and contains the user's data!
        let mut backup_found = false;
        for entry in fs::read_dir(&temp_dir).unwrap() {
            let entry = entry.unwrap();
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with(".aether-backup-") {
                let backup_bytes = fs::read(entry.path()).unwrap();
                assert_eq!(
                    String::from_utf8_lossy(&backup_bytes),
                    "concurrent edit v2",
                    "Backup file must contain user's data intact without deletion!"
                );
                backup_found = true;
            }
        }
        assert!(backup_found, "Preserved backup file must exist on disk!");

        let _ = fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn test_missing_file_restoration_fault_injection_leaves_no_corrupted_destination() {
        let temp_dir = std::env::temp_dir().join(format!(
            "aether_missing_fault_{}",
            uuid::Uuid::new_v4().simple()
        ));
        let _ = fs::create_dir_all(&temp_dir);
        let cap = WorkspaceCapability::open(&temp_dir).unwrap();

        let dest_path = temp_dir.join("missing_file.txt");

        // 1. Fault inject write failure
        let hooks_write_fail = ConditionalReplaceHooks {
            fault_inject_stage_write: true,
            ..Default::default()
        };
        let res_write_fail = cap.replace_file_bytes_conditional_with_hooks(
            "missing_file.txt",
            b"payload",
            None,
            hooks_write_fail,
        );
        assert!(res_write_fail.is_err());
        assert!(
            !dest_path.exists(),
            "Destination must NOT exist after failed stage write!"
        );

        // 2. Fault inject sync failure
        let hooks_sync_fail = ConditionalReplaceHooks {
            fault_inject_stage_sync: true,
            ..Default::default()
        };
        let res_sync_fail = cap.replace_file_bytes_conditional_with_hooks(
            "missing_file.txt",
            b"payload",
            None,
            hooks_sync_fail,
        );
        assert!(res_sync_fail.is_err());
        assert!(
            !dest_path.exists(),
            "Destination must NOT exist after failed stage sync!"
        );

        // Verify that all temporary stage files were cleaned up
        for entry in fs::read_dir(&temp_dir).unwrap() {
            let name = entry.unwrap().file_name().to_string_lossy().to_string();
            assert!(
                !name.starts_with(".aether-stage-"),
                "Stage files must be cleaned up! Found: {}",
                name
            );
        }

        // 3. Normal clean write succeeds
        let res_clean = cap.replace_file_bytes_conditional_with_hooks(
            "missing_file.txt",
            b"payload clean",
            None,
            ConditionalReplaceHooks::default(),
        );
        assert!(res_clean.is_ok());
        assert_eq!(fs::read_to_string(&dest_path).unwrap(), "payload clean");

        let _ = fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn test_chk_032_final_backup_read_failure_preserves_backup() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk032_{}", uuid::Uuid::new_v4().simple()));
        let _ = fs::create_dir_all(&temp_dir);
        let cap = WorkspaceCapability::open(&temp_dir).unwrap();

        let file_path = temp_dir.join("document.txt");
        fs::write(&file_path, "original document content").unwrap();

        let initial_hash = {
            use sha2::{Digest, Sha256};
            let mut hasher = Sha256::new();
            hasher.update(b"original document content");
            format!("{:x}", hasher.finalize())
        };

        let hooks = ConditionalReplaceHooks {
            fault_inject_final_backup_read: true,
            ..Default::default()
        };

        let res = cap.replace_file_bytes_conditional_with_hooks(
            "document.txt",
            b"replaced content v2",
            Some(&initial_hash),
            hooks,
        );

        assert!(res.is_ok(), "Replacement file commit succeeded");
        let outcome = res.unwrap();
        assert_eq!(
            fs::canonicalize(&outcome.dest_path).unwrap(),
            fs::canonicalize(&file_path).unwrap()
        );
        assert!(
            outcome.preserved_backup.is_some(),
            "Backup must be preserved when read verification fails"
        );

        let preserved_path = outcome.preserved_backup.unwrap();
        assert!(
            preserved_path.exists(),
            "Preserved backup file must exist on disk"
        );
        assert_eq!(
            fs::read_to_string(&preserved_path).unwrap(),
            "original document content",
            "Preserved backup must contain user's original data intact"
        );
        assert_eq!(
            fs::read_to_string(&file_path).unwrap(),
            "replaced content v2",
            "Destination contains committed replacement content"
        );

        let _ = fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn test_chk_033_conflict_rename_failure_preserves_and_reports_actual_backup_path() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk033_{}", uuid::Uuid::new_v4().simple()));
        let _ = fs::create_dir_all(&temp_dir);
        let cap = WorkspaceCapability::open(&temp_dir).unwrap();

        let file_path = temp_dir.join("code.rs");
        fs::write(&file_path, "original code").unwrap();

        let initial_hash = {
            use sha2::{Digest, Sha256};
            let mut hasher = Sha256::new();
            hasher.update(b"original code");
            format!("{:x}", hasher.finalize())
        };

        // When trailing modification or read failure triggers conflict preservation,
        // and conflict rename fails, the surviving backup path must be the original backup path.
        let hooks = ConditionalReplaceHooks {
            fault_inject_final_backup_read: true,
            fault_inject_conflict_rename: true,
            ..Default::default()
        };

        let res = cap.replace_file_bytes_conditional_with_hooks(
            "code.rs",
            b"new code",
            Some(&initial_hash),
            hooks,
        );

        assert!(res.is_ok(), "Commit succeeded");
        let outcome = res.unwrap();
        assert!(outcome.preserved_backup.is_some());
        let preserved_path = outcome.preserved_backup.unwrap();

        // Must report actual surviving path on disk (which begins with .aether-backup-)
        let file_name = preserved_path.file_name().unwrap().to_string_lossy();
        assert!(
            file_name.starts_with(".aether-backup-"),
            "When conflict rename fails, reported surviving path must be original backup path, not conflict path! Got: {}",
            file_name
        );
        assert!(
            preserved_path.exists(),
            "Preserved file must exist at reported location"
        );
        assert_eq!(
            fs::read_to_string(&preserved_path).unwrap(),
            "original code"
        );

        let _ = fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn test_chk_034_backup_removal_failure_preserves_backup() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk034_{}", uuid::Uuid::new_v4().simple()));
        let _ = fs::create_dir_all(&temp_dir);
        let cap = WorkspaceCapability::open(&temp_dir).unwrap();

        let file_path = temp_dir.join("data.json");
        fs::write(&file_path, "{\"original\": true}").unwrap();

        let initial_hash = {
            use sha2::{Digest, Sha256};
            let mut hasher = Sha256::new();
            hasher.update(b"{\"original\": true}");
            format!("{:x}", hasher.finalize())
        };

        let hooks = ConditionalReplaceHooks {
            fault_inject_backup_removal: true,
            ..Default::default()
        };

        let res = cap.replace_file_bytes_conditional_with_hooks(
            "data.json",
            b"{\"original\": false}",
            Some(&initial_hash),
            hooks,
        );

        assert!(res.is_ok());
        let outcome = res.unwrap();
        assert!(
            outcome.preserved_backup.is_some(),
            "Backup removal failure must preserve backup"
        );
        let preserved_path = outcome.preserved_backup.unwrap();
        assert!(preserved_path.exists());
        assert_eq!(
            fs::read_to_string(&preserved_path).unwrap(),
            "{\"original\": true}"
        );

        let _ = fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn test_chk_035_pre_cleanup_hook_modification_preserves_conflict() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk035_{}", uuid::Uuid::new_v4().simple()));
        let _ = fs::create_dir_all(&temp_dir);
        let cap = WorkspaceCapability::open(&temp_dir).unwrap();

        let file_path = temp_dir.join("notes.txt");
        fs::write(&file_path, "original notes").unwrap();

        let initial_hash = {
            use sha2::{Digest, Sha256};
            let mut hasher = Sha256::new();
            hasher.update(b"original notes");
            format!("{:x}", hasher.finalize())
        };

        let pre_cleanup = |backup_path: &Path| {
            fs::write(
                backup_path,
                "external open handle wrote after verification!",
            )
            .unwrap();
        };

        let hooks = ConditionalReplaceHooks {
            pre_cleanup_hook: Some(&pre_cleanup),
            ..Default::default()
        };

        let res = cap.replace_file_bytes_conditional_with_hooks(
            "notes.txt",
            b"replaced notes",
            Some(&initial_hash),
            hooks,
        );

        assert!(res.is_ok());
        let outcome = res.unwrap();
        assert!(
            outcome.preserved_backup.is_some(),
            "Pre-cleanup modification must preserve backup"
        );
        let preserved_path = outcome.preserved_backup.unwrap();
        assert!(preserved_path.exists());
        assert_eq!(
            fs::read_to_string(&preserved_path).unwrap(),
            "external open handle wrote after verification!"
        );

        let _ = fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn test_commit_boundary_competing_destination_never_overwritten() {
        let temp_dir = std::env::temp_dir().join(format!(
            "aether_commit_race_file_{}",
            uuid::Uuid::new_v4().simple()
        ));
        let _ = fs::create_dir_all(&temp_dir);
        let cap = WorkspaceCapability::open(&temp_dir).unwrap();

        let file_path = temp_dir.join("critical.rs");
        let original_data = b"const SYSTEM_STATE: &str = \"ORIGINAL\";\n";
        fs::write(&file_path, original_data).unwrap();

        let initial_hash = {
            use sha2::{Digest, Sha256};
            let mut hasher = Sha256::new();
            hasher.update(original_data);
            format!("{:x}", hasher.finalize())
        };

        let competing_data = b"const MALICIOUS_INJECTION: bool = true;\n";
        let post_hash = |dest: &Path| {
            // Adversary / concurrent worker recreates target at commit boundary
            fs::write(dest, competing_data).unwrap();
        };

        let hooks = ConditionalReplaceHooks {
            post_hash_hook: Some(&post_hash),
            ..Default::default()
        };

        let res = cap.replace_file_bytes_conditional_with_hooks(
            "critical.rs",
            b"const SYSTEM_STATE: &str = \"PROPOSED_AI_UPDATE\";\n",
            Some(&initial_hash),
            hooks,
        );

        // Operation MUST fail closed with a clear commit-boundary conflict error
        assert!(
            res.is_err(),
            "Replacement at commit boundary must fail closed"
        );
        let err = res.unwrap_err();
        let err_msg = err.to_string();
        assert!(
            err_msg.contains("Commit boundary conflict"),
            "Error must indicate commit boundary conflict, got: {}",
            err_msg
        );

        // CRITICAL: Competing destination must NEVER be silently overwritten
        let dest_on_disk = fs::read(&file_path).unwrap();
        assert_eq!(
            dest_on_disk, competing_data,
            "Competing destination file must be 100% preserved and never overwritten!"
        );

        // CRITICAL: Original user data must be safely preserved in conflict file
        let mut conflict_found = false;
        for entry in fs::read_dir(&temp_dir).unwrap().flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with(".aether-conflict-") {
                conflict_found = true;
                let conflict_data = fs::read(entry.path()).unwrap();
                assert_eq!(
                    conflict_data, original_data,
                    "Preserved conflict file must contain exact original data"
                );
            }
        }
        assert!(
            conflict_found,
            "Conflict artifact must be created to preserve original data"
        );

        let _ = fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn test_commit_boundary_competing_directory_preserved() {
        let temp_dir = std::env::temp_dir().join(format!(
            "aether_commit_race_dir_{}",
            uuid::Uuid::new_v4().simple()
        ));
        let _ = fs::create_dir_all(&temp_dir);
        let cap = WorkspaceCapability::open(&temp_dir).unwrap();

        let file_path = temp_dir.join("submodule");
        let original_data = b"original submodule pointer\n";
        fs::write(&file_path, original_data).unwrap();

        let initial_hash = {
            use sha2::{Digest, Sha256};
            let mut hasher = Sha256::new();
            hasher.update(original_data);
            format!("{:x}", hasher.finalize())
        };

        let post_hash = |dest: &Path| {
            // Adversary / concurrent worker introduces a directory at commit boundary
            let _ = fs::create_dir_all(dest);
        };

        let hooks = ConditionalReplaceHooks {
            post_hash_hook: Some(&post_hash),
            ..Default::default()
        };

        let res = cap.replace_file_bytes_conditional_with_hooks(
            "submodule",
            b"replacement submodule pointer\n",
            Some(&initial_hash),
            hooks,
        );

        assert!(
            res.is_err(),
            "Replacement must fail closed when destination is occupied"
        );
        assert!(res
            .unwrap_err()
            .to_string()
            .contains("Commit boundary conflict"));

        // Verify destination is still a directory and original data preserved
        assert!(file_path.is_dir(), "Competing directory must be preserved");

        let mut conflict_found = false;
        for entry in fs::read_dir(&temp_dir).unwrap().flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with(".aether-conflict-") {
                conflict_found = true;
                assert_eq!(fs::read(entry.path()).unwrap(), original_data);
            }
        }
        assert!(
            conflict_found,
            "Original data must be preserved in conflict artifact"
        );

        let _ = fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn test_commit_boundary_competing_symlink_preserved() {
        let temp_dir = std::env::temp_dir().join(format!(
            "aether_commit_race_symlink_{}",
            uuid::Uuid::new_v4().simple()
        ));
        let _ = fs::create_dir_all(&temp_dir);
        let cap = WorkspaceCapability::open(&temp_dir).unwrap();

        let file_path = temp_dir.join("target_symlink");
        let original_data = b"original data prior to symlink race\n";
        fs::write(&file_path, original_data).unwrap();

        let initial_hash = {
            use sha2::{Digest, Sha256};
            let mut hasher = Sha256::new();
            hasher.update(original_data);
            format!("{:x}", hasher.finalize())
        };

        let outside_dir = std::env::temp_dir().join(format!(
            "aether_outside_sym_{}",
            uuid::Uuid::new_v4().simple()
        ));
        let _ = fs::create_dir_all(&outside_dir);
        let external_file = outside_dir.join("external.txt");
        fs::write(&external_file, "external secret").unwrap();

        let ext_clone = external_file.clone();
        let post_hash = move |dest: &Path| {
            #[cfg(windows)]
            let _ = std::os::windows::fs::symlink_file(&ext_clone, dest);
            #[cfg(not(windows))]
            let _ = std::os::unix::fs::symlink(&ext_clone, dest);
        };

        let hooks = ConditionalReplaceHooks {
            post_hash_hook: Some(&post_hash),
            ..Default::default()
        };

        let res = cap.replace_file_bytes_conditional_with_hooks(
            "target_symlink",
            b"replacement data\n",
            Some(&initial_hash),
            hooks,
        );

        // If symlink creation was permitted by OS/privileges
        if let Ok(meta) = file_path.symlink_metadata() {
            if meta.is_symlink() {
                assert!(
                    res.is_err(),
                    "Replacement must fail closed when destination is symlink"
                );
                assert!(res
                    .unwrap_err()
                    .to_string()
                    .contains("Commit boundary conflict"));
                assert_eq!(
                    fs::read_to_string(&external_file).unwrap(),
                    "external secret"
                );
            }
        }

        let _ = fs::remove_dir_all(outside_dir);
        let _ = fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn test_commit_boundary_primary_path_pre_publish_competing_file_never_overwritten() {
        let temp_dir = std::env::temp_dir().join(format!(
            "aether_commit_pre_pub_{}",
            uuid::Uuid::new_v4().simple()
        ));
        let _ = fs::create_dir_all(&temp_dir);
        let cap = WorkspaceCapability::open(&temp_dir).unwrap();

        let file_path = temp_dir.join("service.rs");
        let original_data = b"pub fn service() -> &'static str { \"PRIMARY_ORIGINAL\" }\n";
        fs::write(&file_path, original_data).unwrap();

        let initial_hash = {
            use sha2::{Digest, Sha256};
            let mut hasher = Sha256::new();
            hasher.update(original_data);
            format!("{:x}", hasher.finalize())
        };

        let competing_data = b"pub fn injected() -> &'static str { \"PRE_PUBLISH_INJECTION\" }\n";
        let pre_pub_hook = |dest: &Path| {
            // Adversary recreates destination immediately before atomic publication
            fs::write(dest, competing_data).unwrap();
        };

        let hooks = ConditionalReplaceHooks {
            pre_publish_hook: Some(&pre_pub_hook),
            ..Default::default()
        };

        let res = cap.replace_file_bytes_conditional_with_hooks(
            "service.rs",
            b"pub fn service() -> &'static str { \"NEW_CONTENT\" }\n",
            Some(&initial_hash),
            hooks,
        );

        assert!(
            res.is_err(),
            "Primary atomic publication must fail closed when destination was recreated immediately before link"
        );
        let err_msg = res.unwrap_err().to_string();
        assert!(
            err_msg.contains("Commit boundary conflict"),
            "Error must report commit boundary conflict: {}",
            err_msg
        );

        let dest_on_disk = fs::read(&file_path).unwrap();
        assert_eq!(
            dest_on_disk, competing_data,
            "Competing destination file must remain byte-for-byte intact"
        );

        let mut conflict_found = false;
        for entry in fs::read_dir(&temp_dir).unwrap().flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with(".aether-conflict-") {
                conflict_found = true;
                let conflict_data = fs::read(entry.path()).unwrap();
                assert_eq!(
                    conflict_data, original_data,
                    "Conflict file must preserve original data"
                );
            }
        }
        assert!(
            conflict_found,
            "Original data must be preserved in conflict artifact"
        );

        let _ = fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn test_commit_boundary_fallback_path_competing_file_never_overwritten() {
        let temp_dir = std::env::temp_dir().join(format!(
            "aether_commit_fallback_race_{}",
            uuid::Uuid::new_v4().simple()
        ));
        let _ = fs::create_dir_all(&temp_dir);
        let cap = WorkspaceCapability::open(&temp_dir).unwrap();

        let file_path = temp_dir.join("critical_service.rs");
        let original_data = b"pub fn critical() -> &'static str { \"ORIGINAL_CORE\" }\n";
        fs::write(&file_path, original_data).unwrap();

        let initial_hash = {
            use sha2::{Digest, Sha256};
            let mut hasher = Sha256::new();
            hasher.update(original_data);
            format!("{:x}", hasher.finalize())
        };

        let competing_data =
            b"pub fn injected() -> &'static str { \"FALLBACK_WINDOW_INJECTION\" }\n";
        let pre_fallback_hook = |dest: &Path| {
            // Adversary recreates destination file specifically at the fallback window
            // immediately before the platform-specific atomic no-replace publication primitive runs!
            fs::write(dest, competing_data).unwrap();
        };

        // Inject hard-link failure to force execution of the platform-specific atomic fallback path
        let hooks = ConditionalReplaceHooks {
            fault_inject_hard_link_failure: true,
            pre_fallback_publish_hook: Some(&pre_fallback_hook),
            ..Default::default()
        };

        let res = cap.replace_file_bytes_conditional_with_hooks(
            "critical_service.rs",
            b"pub fn critical() -> &'static str { \"PROPOSED_PAYLOAD\" }\n",
            Some(&initial_hash),
            hooks,
        );

        assert!(
            res.is_err(),
            "Fallback publication path must fail closed when competing file exists"
        );
        let err_msg = res.unwrap_err().to_string();
        assert!(
            err_msg.contains("Commit boundary conflict"),
            "Error must report commit boundary conflict on fallback path: {}",
            err_msg
        );

        // Competing file must be completely untouched and byte-for-byte intact
        let dest_on_disk = fs::read(&file_path).unwrap();
        assert_eq!(
            dest_on_disk, competing_data,
            "Competing destination file must NEVER be overwritten on fallback path"
        );

        // Original data must be safely preserved in conflict file
        let mut conflict_found = false;
        for entry in fs::read_dir(&temp_dir).unwrap().flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with(".aether-conflict-") {
                conflict_found = true;
                let conflict_data = fs::read(entry.path()).unwrap();
                assert_eq!(
                    conflict_data, original_data,
                    "Original data must be recovered in conflict file on fallback path"
                );
            }
        }
        assert!(
            conflict_found,
            "Conflict artifact must be created to preserve original data"
        );

        let _ = fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn test_commit_boundary_fallback_path_competing_directory_preserved() {
        let temp_dir = std::env::temp_dir().join(format!(
            "aether_commit_fallback_dir_{}",
            uuid::Uuid::new_v4().simple()
        ));
        let _ = fs::create_dir_all(&temp_dir);
        let cap = WorkspaceCapability::open(&temp_dir).unwrap();

        let target_path = temp_dir.join("module_target");
        let original_data = b"pub fn initial_module() {}\n";
        fs::write(&target_path, original_data).unwrap();

        let initial_hash = {
            use sha2::{Digest, Sha256};
            let mut hasher = Sha256::new();
            hasher.update(original_data);
            format!("{:x}", hasher.finalize())
        };

        let pre_fallback_hook = |dest: &Path| {
            // Adversary recreates destination as a directory at the fallback publication boundary
            fs::create_dir_all(dest).unwrap();
            fs::write(dest.join("subfile.txt"), b"directory content").unwrap();
        };

        let hooks = ConditionalReplaceHooks {
            fault_inject_hard_link_failure: true,
            pre_fallback_publish_hook: Some(&pre_fallback_hook),
            ..Default::default()
        };

        let res = cap.replace_file_bytes_conditional_with_hooks(
            "module_target",
            b"pub fn new_module() {}\n",
            Some(&initial_hash),
            hooks,
        );

        assert!(
            res.is_err(),
            "Fallback publication must fail closed when competing directory is created"
        );
        let err_msg = res.unwrap_err().to_string();
        assert!(
            err_msg.contains("Commit boundary conflict"),
            "Error must report commit boundary conflict: {}",
            err_msg
        );

        // Competing directory must be preserved
        assert!(
            target_path.is_dir(),
            "Competing directory must remain structurally intact"
        );
        assert_eq!(
            fs::read(target_path.join("subfile.txt")).unwrap(),
            b"directory content"
        );

        let _ = fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn test_delete_file_conditional_unmodified_succeeds() {
        let temp_dir = std::env::temp_dir().join(format!(
            "aether_del_clean_{}",
            uuid::Uuid::new_v4().simple()
        ));
        let _ = fs::create_dir_all(&temp_dir);
        let cap = WorkspaceCapability::open(&temp_dir).unwrap();

        let file_path = temp_dir.join("clean.txt");
        fs::write(&file_path, "clean to delete").unwrap();

        let hash = {
            use sha2::{Digest, Sha256};
            let mut hasher = Sha256::new();
            hasher.update(b"clean to delete");
            format!("{:x}", hasher.finalize())
        };

        let outcome = cap.delete_file_conditional("clean.txt", &hash).unwrap();
        assert!(outcome.was_deleted);
        assert!(outcome.surviving_path.is_none());
        assert!(!file_path.exists());

        let _ = fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn test_delete_file_conditional_modified_rolls_back_and_preserves() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_del_mod_{}", uuid::Uuid::new_v4().simple()));
        let _ = fs::create_dir_all(&temp_dir);
        let cap = WorkspaceCapability::open(&temp_dir).unwrap();

        let file_path = temp_dir.join("concurrent.txt");
        fs::write(&file_path, "original before concurrent edit").unwrap();

        let stale_hash = "0000000000000000000000000000000000000000000000000000000000000000";

        let outcome = cap
            .delete_file_conditional("concurrent.txt", stale_hash)
            .unwrap();
        assert!(!outcome.was_deleted, "Must not delete modified file!");
        assert!(outcome.surviving_path.is_some());
        assert!(
            file_path.exists(),
            "File must be rolled back and preserved on disk!"
        );
        assert_eq!(
            fs::read_to_string(&file_path).unwrap(),
            "original before concurrent edit"
        );

        let _ = fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn test_delete_file_conditional_pre_delete_hook_preserves_concurrent_edit() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_del_hook_{}", uuid::Uuid::new_v4().simple()));
        let _ = fs::create_dir_all(&temp_dir);
        let cap = WorkspaceCapability::open(&temp_dir).unwrap();

        let file_path = temp_dir.join("hook.txt");
        fs::write(&file_path, "initial content").unwrap();

        let initial_hash = {
            use sha2::{Digest, Sha256};
            let mut hasher = Sha256::new();
            hasher.update(b"initial content");
            format!("{:x}", hasher.finalize())
        };

        // Pre-delete hook mutates the staged file before verification
        let pre_hook = |staged_path: &Path| {
            fs::write(staged_path, "concurrent edit while in deletion staging!").unwrap();
        };

        let hooks = ConditionalDeleteHooks {
            pre_delete_hook: Some(&pre_hook),
            ..Default::default()
        };

        let outcome = cap
            .delete_file_conditional_with_hooks("hook.txt", &initial_hash, hooks)
            .unwrap();
        assert!(!outcome.was_deleted, "Must NOT delete mutated file!");
        assert!(outcome.surviving_path.is_some());
        assert!(file_path.exists(), "Must be rolled back to target path!");
        assert_eq!(
            fs::read_to_string(&file_path).unwrap(),
            "concurrent edit while in deletion staging!"
        );

        let _ = fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn test_delete_file_conditional_post_hash_mutation_preserves_conflict() {
        let temp_dir = std::env::temp_dir().join(format!(
            "aether_del_post_hash_{}",
            uuid::Uuid::new_v4().simple()
        ));
        let _ = fs::create_dir_all(&temp_dir);
        let cap = WorkspaceCapability::open(&temp_dir).unwrap();

        let file_path = temp_dir.join("open_handle.txt");
        fs::write(&file_path, "clean base state").unwrap();

        let initial_hash = {
            use sha2::{Digest, Sha256};
            let mut hasher = Sha256::new();
            hasher.update(b"clean base state");
            format!("{:x}", hasher.finalize())
        };

        // Open original file with a writable handle BEFORE staging and retain it across staging rename
        let retained_file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&file_path)
            .unwrap();
        let file_arc = std::sync::Arc::new(std::sync::Mutex::new(Some(retained_file)));
        let file_arc_clone = file_arc.clone();

        // post_hash_hook modifies the file through the retained writable handle after initial hash check
        let post_hook = move |_staged_path: &Path| {
            use std::io::{Seek, SeekFrom, Write};
            let mut guard = file_arc_clone.lock().unwrap();
            if let Some(ref mut f) = *guard {
                f.seek(SeekFrom::Start(0)).unwrap();
                f.write_all(b"mutation via retained writable handle after hash check!")
                    .unwrap();
                f.set_len(b"mutation via retained writable handle after hash check!".len() as u64)
                    .unwrap();
                f.flush().unwrap();
            }
        };

        let hooks = ConditionalDeleteHooks {
            post_hash_hook: Some(&post_hook),
            ..Default::default()
        };

        let outcome = cap
            .delete_file_conditional_with_hooks("open_handle.txt", &initial_hash, hooks)
            .unwrap();

        // Release handle
        drop(file_arc);

        assert!(
            !outcome.was_deleted,
            "Must not delete file modified through retained handle after hash check!"
        );
        assert!(outcome.surviving_path.is_some());
        let surviving = outcome.surviving_path.unwrap();
        assert!(surviving.exists(), "Surviving file must exist on disk!");
        assert_eq!(
            fs::read_to_string(&surviving).unwrap(),
            "mutation via retained writable handle after hash check!"
        );

        let _ = fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn test_delete_file_conditional_recreated_dest_preserves_both() {
        let temp_dir = std::env::temp_dir().join(format!(
            "aether_del_recreated_{}",
            uuid::Uuid::new_v4().simple()
        ));
        let _ = fs::create_dir_all(&temp_dir);
        let cap = WorkspaceCapability::open(&temp_dir).unwrap();

        let file_path = temp_dir.join("target.txt");
        fs::write(&file_path, "staged original content").unwrap();

        let initial_hash = {
            use sha2::{Digest, Sha256};
            let mut hasher = Sha256::new();
            hasher.update(b"staged original content");
            format!("{:x}", hasher.finalize())
        };

        // pre_delete_hook modifies staged file AND concurrently recreates destination!
        let dest_clone = file_path.clone();
        let pre_hook = move |staged_path: &Path| {
            fs::write(staged_path, "staged file mutated").unwrap();
            fs::write(&dest_clone, "concurrently recreated destination payload").unwrap();
        };

        let hooks = ConditionalDeleteHooks {
            pre_delete_hook: Some(&pre_hook),
            ..Default::default()
        };

        let outcome = cap
            .delete_file_conditional_with_hooks("target.txt", &initial_hash, hooks)
            .unwrap();

        assert!(!outcome.was_deleted);
        assert!(outcome.surviving_path.is_some());
        let conflict_path = outcome.surviving_path.unwrap();

        // 1. Concurrently recreated destination MUST survive with its new payload untouched!
        assert!(file_path.exists());
        assert_eq!(
            fs::read_to_string(&file_path).unwrap(),
            "concurrently recreated destination payload",
            "Destination MUST NOT be overwritten!"
        );

        // 2. Staged recovery artifact MUST survive under conflict path!
        assert!(conflict_path.exists());
        assert_ne!(
            conflict_path, file_path,
            "Conflict path must not equal destination path!"
        );
        assert_eq!(
            fs::read_to_string(&conflict_path).unwrap(),
            "staged file mutated",
            "Staged artifact must be preserved under conflict path!"
        );

        let _ = fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn test_delete_file_conditional_production_default_hooks_second_verification() {
        let temp_dir = std::env::temp_dir().join(format!(
            "aether_del_prod_default_{}",
            uuid::Uuid::new_v4().simple()
        ));
        let _ = fs::create_dir_all(&temp_dir);
        let cap = WorkspaceCapability::open(&temp_dir).unwrap();

        let file_path = temp_dir.join("prod_retained.txt");
        let initial_payload = vec![0x58u8; 1024 * 1024]; // 1MB payload to ensure reliable window
        fs::write(&file_path, &initial_payload).unwrap();

        let initial_hash = {
            use sha2::{Digest, Sha256};
            let mut hasher = Sha256::new();
            hasher.update(&initial_payload);
            format!("{:x}", hasher.finalize())
        };

        // Open original file with a writable handle before staging and retain across staging rename
        let mut retained_handle = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&file_path)
            .unwrap();

        let path_clone = file_path.clone();
        let ready = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let ready_clone = ready.clone();
        let running = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
        let r_flag = running.clone();

        let writer_thread = std::thread::spawn(move || {
            use std::io::{Seek, SeekFrom, Write};
            ready_clone.store(true, std::sync::atomic::Ordering::SeqCst);
            // Spin until staging rename moves prod_retained.txt away
            while r_flag.load(std::sync::atomic::Ordering::Relaxed) && path_clone.exists() {
                std::hint::spin_loop();
            }
            if path_clone.exists() {
                return;
            }
            // Staging rename completed: write immediately through retained handle
            let msg = b"mutation via retained handle during prod delete!";
            let _ = retained_handle.seek(SeekFrom::Start(0));
            let _ = retained_handle.write_all(msg);
            let _ = retained_handle.flush();
        });

        // Ensure writer thread is active and spinning before initiating deletion
        while !ready.load(std::sync::atomic::Ordering::SeqCst) {
            std::thread::yield_now();
        }

        // Call ordinary production wrapper with DEFAULT hooks (no post_hash_hook!)
        let outcome = cap
            .delete_file_conditional("prod_retained.txt", &initial_hash)
            .unwrap();

        running.store(false, std::sync::atomic::Ordering::SeqCst);
        writer_thread.join().unwrap();

        assert!(
            !outcome.was_deleted,
            "Production wrapper must abort deletion and preserve file modified via retained handle!"
        );
        assert!(outcome.surviving_path.is_some());
        let surviving = outcome.surviving_path.unwrap();
        assert!(surviving.exists(), "Surviving file must exist on disk!");
        let content = fs::read(&surviving).unwrap();
        assert!(content.starts_with(b"mutation via retained handle during prod delete!"));

        let _ = fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn test_delete_file_conditional_recreated_dest_before_hard_link_preserves_both() {
        let temp_dir = std::env::temp_dir().join(format!(
            "aether_del_race_hl_{}",
            uuid::Uuid::new_v4().simple()
        ));
        let _ = fs::create_dir_all(&temp_dir);
        let cap = WorkspaceCapability::open(&temp_dir).unwrap();

        let file_path = temp_dir.join("race_target.txt");
        fs::write(&file_path, "base original content").unwrap();

        let initial_hash = {
            use sha2::{Digest, Sha256};
            let mut hasher = Sha256::new();
            hasher.update(b"base original content");
            format!("{:x}", hasher.finalize())
        };

        // Mutate staged file in pre_delete_hook so deletion will be aborted and enter rollback
        let pre_hook = |staged_path: &Path| {
            fs::write(staged_path, "modified in staging").unwrap();
        };

        // Destination recreation injected strictly AFTER destination metadata inspection
        // and immediately BEFORE hard_link publication!
        let dest_clone = file_path.clone();
        let pre_hl_hook = move |_dest_abs: &Path| {
            fs::write(&dest_clone, "recreated in TOCTOU window before hard-link!").unwrap();
        };

        let hooks = ConditionalDeleteHooks {
            pre_delete_hook: Some(&pre_hook),
            pre_hard_link_hook: Some(&pre_hl_hook),
            ..Default::default()
        };

        let outcome = cap
            .delete_file_conditional_with_hooks("race_target.txt", &initial_hash, hooks)
            .unwrap();

        assert!(!outcome.was_deleted, "Must abort deletion!");
        assert!(outcome.surviving_path.is_some());
        let conflict_path = outcome.surviving_path.unwrap();

        // 1. Recreated destination must survive with its payload untouched
        assert!(file_path.exists());
        assert_eq!(
            fs::read_to_string(&file_path).unwrap(),
            "recreated in TOCTOU window before hard-link!",
            "Destination MUST NOT be overwritten!"
        );

        // 2. Staged recovery artifact must survive under conflict path
        assert!(conflict_path.exists());
        assert_ne!(conflict_path, file_path);
        assert_eq!(
            fs::read_to_string(&conflict_path).unwrap(),
            "modified in staging"
        );

        let _ = fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn test_delete_file_conditional_hard_link_failure_no_rename_fallback() {
        let temp_dir = std::env::temp_dir().join(format!(
            "aether_del_hl_fail_{}",
            uuid::Uuid::new_v4().simple()
        ));
        let _ = fs::create_dir_all(&temp_dir);
        let cap = WorkspaceCapability::open(&temp_dir).unwrap();

        let file_path = temp_dir.join("hl_fail_target.txt");
        fs::write(&file_path, "base content").unwrap();

        let initial_hash = {
            use sha2::{Digest, Sha256};
            let mut hasher = Sha256::new();
            hasher.update(b"base content");
            format!("{:x}", hasher.finalize())
        };

        // Mutate staged file so deletion aborts and rolls back
        let pre_hook = |staged_path: &Path| {
            fs::write(staged_path, "mutated staged content").unwrap();
        };

        // Inject non-AlreadyExists hard link failure
        let hooks = ConditionalDeleteHooks {
            pre_delete_hook: Some(&pre_hook),
            fault_inject_hard_link_failure: true,
            ..Default::default()
        };

        let outcome = cap
            .delete_file_conditional_with_hooks("hl_fail_target.txt", &initial_hash, hooks)
            .unwrap();

        assert!(!outcome.was_deleted);
        assert!(outcome.surviving_path.is_some());
        let conflict_path = outcome.surviving_path.unwrap();

        // Destination was absent when hard-link failed.
        // It must NOT have fallen back to ordinary rename(stage, rel)!
        assert!(
            !file_path.exists(),
            "Ordinary rename fallback must NOT occur!"
        );

        // Staged artifact must be preserved under conflict path!
        assert!(conflict_path.exists());
        assert_eq!(
            fs::read_to_string(&conflict_path).unwrap(),
            "mutated staged content"
        );

        let _ = fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn test_delete_file_conditional_metadata_failure_fails_closed() {
        let temp_dir = std::env::temp_dir().join(format!(
            "aether_del_meta_fail_{}",
            uuid::Uuid::new_v4().simple()
        ));
        let _ = fs::create_dir_all(&temp_dir);
        let cap = WorkspaceCapability::open(&temp_dir).unwrap();

        let file_path = temp_dir.join("meta_fail_target.txt");
        fs::write(&file_path, "base payload").unwrap();

        let initial_hash = {
            use sha2::{Digest, Sha256};
            let mut hasher = Sha256::new();
            hasher.update(b"base payload");
            format!("{:x}", hasher.finalize())
        };

        let pre_hook = |staged_path: &Path| {
            fs::write(staged_path, "mutated payload").unwrap();
        };

        let hooks = ConditionalDeleteHooks {
            pre_delete_hook: Some(&pre_hook),
            fault_inject_rollback_metadata_failure: true,
            ..Default::default()
        };

        let outcome = cap
            .delete_file_conditional_with_hooks("meta_fail_target.txt", &initial_hash, hooks)
            .unwrap();

        assert!(!outcome.was_deleted);
        assert!(outcome.surviving_path.is_some());
        let conflict_path = outcome.surviving_path.unwrap();

        // Must fail closed: preserved under conflict path without touching destination
        assert!(conflict_path.exists());
        assert_eq!(
            fs::read_to_string(&conflict_path).unwrap(),
            "mutated payload"
        );
        assert!(!file_path.exists());

        let _ = fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn test_delete_file_conditional_second_verification_read_failure_preserves_artifact() {
        let temp_dir = std::env::temp_dir().join(format!(
            "aether_del_second_read_fail_{}",
            uuid::Uuid::new_v4().simple()
        ));
        let _ = fs::create_dir_all(&temp_dir);
        let cap = WorkspaceCapability::open(&temp_dir).unwrap();

        let file_path = temp_dir.join("read_fail.txt");
        fs::write(&file_path, "clean base state").unwrap();

        let initial_hash = {
            use sha2::{Digest, Sha256};
            let mut hasher = Sha256::new();
            hasher.update(b"clean base state");
            format!("{:x}", hasher.finalize())
        };

        let hooks = ConditionalDeleteHooks {
            fault_inject_second_verification_read_failure: true,
            ..Default::default()
        };

        let outcome = cap
            .delete_file_conditional_with_hooks("read_fail.txt", &initial_hash, hooks)
            .unwrap();

        assert!(
            !outcome.was_deleted,
            "Must abort deletion when second verification fails!"
        );
        assert!(outcome.surviving_path.is_some());
        let surviving = outcome.surviving_path.unwrap();
        assert!(surviving.exists(), "Surviving file must exist on disk!");

        let _ = fs::remove_dir_all(temp_dir);
    }

    #[cfg(windows)]
    unsafe fn set_directory_owner_to_token_user(path: &Path) -> std::result::Result<(), String> {
        use std::os::windows::ffi::OsStrExt;

        const SE_FILE_OBJECT: u32 = 1;
        const OWNER_SECURITY_INFORMATION: u32 = 0x00000001;
        const TOKEN_DUPLICATE: u32 = 0x0002;
        const TOKEN_IMPERSONATE: u32 = 0x0004;
        const TOKEN_QUERY: u32 = 0x0008;
        const TOKEN_ADJUST_PRIVILEGES: u32 = 0x0020;
        const SECURITY_IMPERSONATION: u32 = 2;
        const TOKEN_IMPERSONATION: u32 = 2;
        #[allow(non_upper_case_globals)]
        const TokenUser: u32 = 1;
        #[allow(non_upper_case_globals)]
        const TokenPrivileges: u32 = 3;
        const SE_PRIVILEGE_ENABLED: u32 = 0x00000002;
        const ERROR_SUCCESS: u32 = 0;
        const ERROR_NOT_ALL_ASSIGNED: u32 = 1300;

        extern "system" {
            fn OpenProcessToken(
                ProcessHandle: *mut std::ffi::c_void,
                DesiredAccess: u32,
                TokenHandle: *mut *mut std::ffi::c_void,
            ) -> i32;
            fn DuplicateTokenEx(
                hExistingToken: *mut std::ffi::c_void,
                dwDesiredAccess: u32,
                lpTokenAttributes: *mut std::ffi::c_void,
                ImpersonationLevel: u32,
                TokenType: u32,
                phNewToken: *mut *mut std::ffi::c_void,
            ) -> i32;
            fn SetThreadToken(
                Thread: *mut *mut std::ffi::c_void,
                Token: *mut std::ffi::c_void,
            ) -> i32;
            fn RevertToSelf() -> i32;
            fn GetCurrentThread() -> *mut std::ffi::c_void;
            fn OpenThreadToken(
                ThreadHandle: *mut std::ffi::c_void,
                DesiredAccess: u32,
                OpenAsSelf: i32,
                TokenHandle: *mut *mut std::ffi::c_void,
            ) -> i32;
            fn SetLastError(dwErrCode: u32);
            fn GetTokenInformation(
                TokenHandle: *mut std::ffi::c_void,
                TokenInformationClass: u32,
                TokenInformation: *mut std::ffi::c_void,
                TokenInformationLength: u32,
                ReturnLength: *mut u32,
            ) -> i32;
            fn LookupPrivilegeValueW(
                lpSystemName: *const u16,
                lpName: *const u16,
                lpLuid: *mut std::ffi::c_void,
            ) -> i32;
            fn AdjustTokenPrivileges(
                TokenHandle: *mut std::ffi::c_void,
                DisableAllPrivileges: i32,
                NewState: *mut std::ffi::c_void,
                BufferLength: u32,
                PreviousState: *mut std::ffi::c_void,
                ReturnLength: *mut u32,
            ) -> i32;
            fn SetNamedSecurityInfoW(
                pObjectName: *const u16,
                ObjectType: u32,
                SecurityInfo: u32,
                ppsidOwner: *mut std::ffi::c_void,
                ppsidGroup: *mut std::ffi::c_void,
                ppDacl: *mut std::ffi::c_void,
                ppSacl: *mut std::ffi::c_void,
            ) -> u32;
            fn CloseHandle(hObject: *mut std::ffi::c_void) -> i32;
            fn GetCurrentProcess() -> *mut std::ffi::c_void;
            fn GetLastError() -> u32;
        }

        struct HandleGuard(*mut std::ffi::c_void);
        impl Drop for HandleGuard {
            fn drop(&mut self) {
                if !self.0.is_null() {
                    unsafe {
                        CloseHandle(self.0);
                    }
                }
            }
        }

        struct ThreadImpersonationGuard {
            active: bool,
        }
        impl Drop for ThreadImpersonationGuard {
            fn drop(&mut self) {
                if self.active {
                    unsafe {
                        RevertToSelf();
                    }
                }
            }
        }

        // 1. Open process token with minimum required access (TOKEN_QUERY | TOKEN_DUPLICATE)
        let mut proc_token: *mut std::ffi::c_void = std::ptr::null_mut();
        if OpenProcessToken(
            GetCurrentProcess(),
            TOKEN_QUERY | TOKEN_DUPLICATE,
            &mut proc_token,
        ) == 0
        {
            let err = GetLastError();
            return Err(format!("OpenProcessToken failed with Win32 error {}", err));
        }
        let _proc_guard = HandleGuard(proc_token);

        // Query the process token's individual TokenUser SID
        #[repr(C)]
        #[allow(non_snake_case)]
        struct SID_AND_ATTRIBUTES {
            Sid: *mut std::ffi::c_void,
            Attributes: u32,
        }
        #[repr(C)]
        #[allow(non_snake_case)]
        struct TOKEN_USER {
            User: SID_AND_ATTRIBUTES,
        }

        #[repr(align(8))]
        struct AlignedBuffer([u8; 512]);
        let mut user_buf = AlignedBuffer([0u8; 512]);
        let mut ret_len = 0u32;
        if GetTokenInformation(
            proc_token,
            TokenUser,
            user_buf.0.as_mut_ptr() as *mut _,
            user_buf.0.len() as u32,
            &mut ret_len,
        ) == 0
        {
            let err = GetLastError();
            return Err(format!(
                "GetTokenInformation(TokenUser) failed with Win32 error {}",
                err
            ));
        }
        let token_user = &*(user_buf.0.as_ptr() as *const TOKEN_USER);
        let p_token_user_sid = token_user.User.Sid;

        // Query privileges present in the process token
        #[repr(C)]
        #[allow(clippy::upper_case_acronyms)]
        struct LUID {
            low_part: u32,
            high_part: i32,
        }
        #[repr(C)]
        #[allow(clippy::upper_case_acronyms)]
        struct LUID_AND_ATTRIBUTES {
            luid: LUID,
            attributes: u32,
        }
        #[repr(C)]
        #[allow(clippy::upper_case_acronyms)]
        struct TOKEN_PRIVILEGES {
            privilege_count: u32,
            privileges: [LUID_AND_ATTRIBUTES; 1],
        }

        let mut priv_len = 0u32;
        GetTokenInformation(
            proc_token,
            TokenPrivileges,
            std::ptr::null_mut(),
            0,
            &mut priv_len,
        );
        let mut priv_buf = vec![0u8; priv_len as usize];
        if GetTokenInformation(
            proc_token,
            TokenPrivileges,
            priv_buf.as_mut_ptr() as *mut _,
            priv_len,
            &mut priv_len,
        ) == 0
        {
            let err = GetLastError();
            return Err(format!(
                "GetTokenInformation(TokenPrivileges) failed with Win32 error {}",
                err
            ));
        }
        let priv_count = *(priv_buf.as_ptr() as *const u32);
        let priv_entries =
            priv_buf.as_ptr().add(std::mem::size_of::<u32>()) as *const LUID_AND_ATTRIBUTES;

        // 2. Duplicate process token into thread impersonation token with TOKEN_QUERY | TOKEN_ADJUST_PRIVILEGES | TOKEN_IMPERSONATE
        let mut imp_token: *mut std::ffi::c_void = std::ptr::null_mut();
        if DuplicateTokenEx(
            proc_token,
            TOKEN_QUERY | TOKEN_ADJUST_PRIVILEGES | TOKEN_IMPERSONATE,
            std::ptr::null_mut(),
            SECURITY_IMPERSONATION,
            TOKEN_IMPERSONATION,
            &mut imp_token,
        ) == 0
        {
            let err = GetLastError();
            return Err(format!("DuplicateTokenEx failed with Win32 error {}", err));
        }
        let _imp_guard = HandleGuard(imp_token);

        // 3. Enable only privileges required and actually present in the token on the duplicated token
        for priv_name in &["SeRestorePrivilege", "SeTakeOwnershipPrivilege"] {
            let wide_name: Vec<u16> = priv_name.encode_utf16().chain(Some(0)).collect();
            let mut luid = LUID {
                low_part: 0,
                high_part: 0,
            };
            if LookupPrivilegeValueW(
                std::ptr::null(),
                wide_name.as_ptr(),
                &mut luid as *mut _ as *mut _,
            ) == 0
            {
                let err = GetLastError();
                return Err(format!(
                    "LookupPrivilegeValueW({}) failed with Win32 error {}",
                    priv_name, err
                ));
            }

            let mut is_present = false;
            for i in 0..priv_count {
                let entry = &*priv_entries.add(i as usize);
                if entry.luid.low_part == luid.low_part && entry.luid.high_part == luid.high_part {
                    is_present = true;
                    break;
                }
            }

            if is_present {
                let mut tp = TOKEN_PRIVILEGES {
                    privilege_count: 1,
                    privileges: [LUID_AND_ATTRIBUTES {
                        luid,
                        attributes: SE_PRIVILEGE_ENABLED,
                    }],
                };
                SetLastError(0);
                let adj_res = AdjustTokenPrivileges(
                    imp_token,
                    0,
                    &mut tp as *mut _ as *mut _,
                    std::mem::size_of::<TOKEN_PRIVILEGES>() as u32,
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                );
                let adj_err = GetLastError();
                if adj_res == 0 {
                    return Err(format!(
                        "AdjustTokenPrivileges({}) failed with Win32 error {}",
                        priv_name, adj_err
                    ));
                }
                if adj_err == ERROR_NOT_ALL_ASSIGNED {
                    return Err(format!(
                        "AdjustTokenPrivileges({}) failed: ERROR_NOT_ALL_ASSIGNED (1300)",
                        priv_name
                    ));
                }
                if adj_err != ERROR_SUCCESS {
                    return Err(format!(
                        "AdjustTokenPrivileges({}) returned unexpected error {}",
                        priv_name, adj_err
                    ));
                }
            }
        }

        // 4. Apply duplicated impersonation token to the current thread
        if SetThreadToken(std::ptr::null_mut(), imp_token) == 0 {
            let err = GetLastError();
            return Err(format!("SetThreadToken failed with Win32 error {}", err));
        }
        let mut impersonation_guard = ThreadImpersonationGuard { active: true };

        // 5. Call SetNamedSecurityInfoW under thread impersonation context
        let wide_path: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
        let set_res = SetNamedSecurityInfoW(
            wide_path.as_ptr(),
            SE_FILE_OBJECT,
            OWNER_SECURITY_INFORMATION,
            p_token_user_sid,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        );

        // 6. Explicitly revert thread impersonation and verify success
        if RevertToSelf() == 0 {
            let err = GetLastError();
            return Err(format!("RevertToSelf failed with Win32 error {}", err));
        }
        impersonation_guard.active = false;

        // Verify that thread impersonation has been cleanly removed and thread has reverted to process context
        const ERROR_NO_TOKEN: u32 = 1008;
        let mut check_tok: *mut std::ffi::c_void = std::ptr::null_mut();
        if OpenThreadToken(GetCurrentThread(), TOKEN_QUERY, 1, &mut check_tok) != 0 {
            CloseHandle(check_tok);
            return Err(
                "Thread still has an active impersonation token after RevertToSelf".to_string(),
            );
        }
        let thread_err = GetLastError();
        if thread_err != ERROR_NO_TOKEN {
            return Err(format!(
                "OpenThreadToken returned unexpected error {}; expected ERROR_NO_TOKEN (1008)",
                thread_err
            ));
        }

        // 7. Verify SetNamedSecurityInfoW result
        if set_res != 0 {
            return Err(format!(
                "SetNamedSecurityInfoW(OWNER_SECURITY_INFORMATION) failed with Win32 error {}",
                set_res
            ));
        }

        Ok(())
    }

    #[cfg(windows)]
    unsafe fn query_directory_owner_and_token_user_sids(
        path: &Path,
    ) -> std::result::Result<(String, String, bool), String> {
        use std::os::windows::ffi::OsStrExt;

        const SE_FILE_OBJECT: u32 = 1;
        const OWNER_SECURITY_INFORMATION: u32 = 0x00000001;
        const TOKEN_QUERY: u32 = 0x0008;
        #[allow(non_upper_case_globals)]
        const TokenUser: u32 = 1;

        extern "system" {
            fn GetNamedSecurityInfoW(
                pObjectName: *const u16,
                ObjectType: u32,
                SecurityInfo: u32,
                ppsidOwner: *mut *mut std::ffi::c_void,
                ppsidGroup: *mut *mut std::ffi::c_void,
                ppDacl: *mut *mut std::ffi::c_void,
                ppSacl: *mut *mut std::ffi::c_void,
                ppSecurityDescriptor: *mut *mut std::ffi::c_void,
            ) -> u32;
            fn OpenProcessToken(
                ProcessHandle: *mut std::ffi::c_void,
                DesiredAccess: u32,
                TokenHandle: *mut *mut std::ffi::c_void,
            ) -> i32;
            fn GetTokenInformation(
                TokenHandle: *mut std::ffi::c_void,
                TokenInformationClass: u32,
                TokenInformation: *mut std::ffi::c_void,
                TokenInformationLength: u32,
                ReturnLength: *mut u32,
            ) -> i32;
            fn EqualSid(pSid1: *mut std::ffi::c_void, pSid2: *mut std::ffi::c_void) -> i32;
            fn ConvertSidToStringSidW(Sid: *mut std::ffi::c_void, StringSid: *mut *mut u16) -> i32;
            fn LocalFree(hMem: *mut std::ffi::c_void) -> *mut std::ffi::c_void;
            fn CloseHandle(hObject: *mut std::ffi::c_void) -> i32;
            fn GetCurrentProcess() -> *mut std::ffi::c_void;
            fn GetLastError() -> u32;
        }

        unsafe fn sid_to_string(sid: *mut std::ffi::c_void) -> std::result::Result<String, String> {
            let mut p_str: *mut u16 = std::ptr::null_mut();
            if ConvertSidToStringSidW(sid, &mut p_str) == 0 || p_str.is_null() {
                return Err(format!(
                    "ConvertSidToStringSidW failed with Win32 error {}",
                    GetLastError()
                ));
            }
            let mut len = 0;
            while *p_str.add(len) != 0 {
                len += 1;
            }
            let s = String::from_utf16_lossy(std::slice::from_raw_parts(p_str, len));
            LocalFree(p_str as *mut _);
            Ok(s)
        }

        let wide_path: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
        let mut p_owner_sid: *mut std::ffi::c_void = std::ptr::null_mut();
        let mut p_sec_desc: *mut std::ffi::c_void = std::ptr::null_mut();

        let sec_res = GetNamedSecurityInfoW(
            wide_path.as_ptr(),
            SE_FILE_OBJECT,
            OWNER_SECURITY_INFORMATION,
            &mut p_owner_sid,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            &mut p_sec_desc,
        );
        if sec_res != 0 {
            return Err(format!(
                "GetNamedSecurityInfoW(OWNER_SECURITY_INFORMATION) failed with Win32 error {}",
                sec_res
            ));
        }
        if p_owner_sid.is_null() {
            if !p_sec_desc.is_null() {
                LocalFree(p_sec_desc);
            }
            return Err("GetNamedSecurityInfoW returned a NULL owner SID".to_string());
        }

        let owner_sid_str = match sid_to_string(p_owner_sid) {
            Ok(s) => s,
            Err(e) => {
                if !p_sec_desc.is_null() {
                    LocalFree(p_sec_desc);
                }
                return Err(e);
            }
        };

        let mut token: *mut std::ffi::c_void = std::ptr::null_mut();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
            if !p_sec_desc.is_null() {
                LocalFree(p_sec_desc);
            }
            return Err(format!(
                "OpenProcessToken failed with Win32 error {}",
                GetLastError()
            ));
        }

        #[repr(C)]
        #[allow(non_snake_case)]
        struct SID_AND_ATTRIBUTES {
            Sid: *mut std::ffi::c_void,
            Attributes: u32,
        }
        #[repr(C)]
        #[allow(non_snake_case)]
        struct TOKEN_USER {
            User: SID_AND_ATTRIBUTES,
        }

        #[repr(align(8))]
        struct AlignedBuffer([u8; 512]);
        let mut user_buf = AlignedBuffer([0u8; 512]);
        let mut ret_len = 0u32;
        if GetTokenInformation(
            token,
            TokenUser,
            user_buf.0.as_mut_ptr() as *mut _,
            user_buf.0.len() as u32,
            &mut ret_len,
        ) == 0
        {
            CloseHandle(token);
            if !p_sec_desc.is_null() {
                LocalFree(p_sec_desc);
            }
            return Err(format!(
                "GetTokenInformation(TokenUser) failed with Win32 error {}",
                GetLastError()
            ));
        }
        let token_user = &*(user_buf.0.as_ptr() as *const TOKEN_USER);
        let p_token_user_sid = token_user.User.Sid;

        let token_user_sid_str = match sid_to_string(p_token_user_sid) {
            Ok(s) => s,
            Err(e) => {
                CloseHandle(token);
                if !p_sec_desc.is_null() {
                    LocalFree(p_sec_desc);
                }
                return Err(e);
            }
        };

        let is_equal = EqualSid(p_owner_sid, p_token_user_sid) != 0;

        CloseHandle(token);
        if !p_sec_desc.is_null() {
            LocalFree(p_sec_desc);
        }

        Ok((owner_sid_str, token_user_sid_str, is_equal))
    }

    #[cfg(windows)]
    unsafe fn assert_directory_owner_is_token_user(path: &Path) -> (String, String) {
        let (owner_sid_str, token_user_sid_str, is_equal) =
            query_directory_owner_and_token_user_sids(path)
                .expect("Failed to query directory owner and TokenUser SIDs");

        assert!(
            is_equal,
            "Directory actual owner SID ({}) does not match process TokenUser SID ({}); fixture requires exact individual TokenUser ownership",
            owner_sid_str, token_user_sid_str
        );
        assert_eq!(
            owner_sid_str, token_user_sid_str,
            "Owner SID string representation must match TokenUser SID string"
        );

        (owner_sid_str, token_user_sid_str)
    }

    #[cfg(windows)]
    unsafe fn set_permissive_dacl_for_sid(
        path: &Path,
        well_known_sid: u32,
        inherit: bool,
    ) -> std::result::Result<(), String> {
        use std::os::windows::ffi::OsStrExt;
        let wide_path: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();

        const SE_FILE_OBJECT: u32 = 1;
        const DACL_SECURITY_INFORMATION: u32 = 0x00000004;
        const PROTECTED_DACL_SECURITY_INFORMATION: u32 = 0x80000000;
        const ACL_REVISION: u32 = 2;
        const GENERIC_ALL: u32 = 0x10000000;
        const CONTAINER_INHERIT_ACE: u32 = 0x02;
        const OBJECT_INHERIT_ACE: u32 = 0x01;

        extern "system" {
            fn CreateWellKnownSid(
                WellKnownSidType: u32,
                DomainSid: *mut std::ffi::c_void,
                pSid: *mut std::ffi::c_void,
                cbSid: *mut u32,
            ) -> i32;
            fn InitializeAcl(
                pAcl: *mut std::ffi::c_void,
                nAclLength: u32,
                dwAclRevision: u32,
            ) -> i32;
            fn AddAccessAllowedAceEx(
                pAcl: *mut std::ffi::c_void,
                dwAceRevision: u32,
                AceFlags: u32,
                AccessMask: u32,
                pSid: *mut std::ffi::c_void,
            ) -> i32;
            fn SetNamedSecurityInfoW(
                pObjectName: *const u16,
                ObjectType: u32,
                SecurityInfo: u32,
                ppsidOwner: *mut std::ffi::c_void,
                ppsidGroup: *mut std::ffi::c_void,
                ppDacl: *mut std::ffi::c_void,
                ppSacl: *mut std::ffi::c_void,
            ) -> u32;
            fn GetLastError() -> u32;
        }

        let mut sid_buf = vec![0u8; 68];
        let mut cb_sid = sid_buf.len() as u32;
        if CreateWellKnownSid(
            well_known_sid,
            std::ptr::null_mut(),
            sid_buf.as_mut_ptr() as *mut _,
            &mut cb_sid,
        ) == 0
        {
            return Err(format!(
                "CreateWellKnownSid(type={}) failed with Win32 error {}",
                well_known_sid,
                GetLastError()
            ));
        }

        let mut acl_buf = vec![0u8; 1024];
        if InitializeAcl(
            acl_buf.as_mut_ptr() as *mut _,
            acl_buf.len() as u32,
            ACL_REVISION,
        ) == 0
        {
            return Err(format!(
                "InitializeAcl failed with Win32 error {}",
                GetLastError()
            ));
        }

        let flags = if inherit {
            CONTAINER_INHERIT_ACE | OBJECT_INHERIT_ACE
        } else {
            0
        };
        if AddAccessAllowedAceEx(
            acl_buf.as_mut_ptr() as *mut _,
            ACL_REVISION,
            flags,
            GENERIC_ALL,
            sid_buf.as_mut_ptr() as *mut _,
        ) == 0
        {
            return Err(format!(
                "AddAccessAllowedAceEx failed with Win32 error {}",
                GetLastError()
            ));
        }

        let set_res = SetNamedSecurityInfoW(
            wide_path.as_ptr(),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            acl_buf.as_mut_ptr() as *mut _,
            std::ptr::null_mut(),
        );
        if set_res != 0 {
            return Err(format!(
                "SetNamedSecurityInfoW failed with Win32 error {}",
                set_res
            ));
        }

        Ok(())
    }

    #[cfg(windows)]
    unsafe fn set_permissive_dacl_for_everyone(
        path: &Path,
        inherit: bool,
    ) -> std::result::Result<(), String> {
        const WIN_WORLD_SID: u32 = 1;
        set_permissive_dacl_for_sid(path, WIN_WORLD_SID, inherit)
    }

    #[test]
    #[cfg(windows)]
    fn test_windows_parent_safe_dacl() {
        let temp_dir = std::env::temp_dir().join(format!(
            "aether_win_dacl_safe_{}",
            uuid::Uuid::new_v4().simple()
        ));
        std::fs::create_dir_all(&temp_dir).unwrap();

        secure_storage_parent_private_acl(&temp_dir).expect("secure private acl");
        let res = verify_storage_parent_acl(&temp_dir);
        assert!(
            res.is_ok(),
            "Parent with safe DACL must be accepted: {:?}",
            res
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    #[cfg(windows)]
    fn test_windows_parent_unsafe_permissive_dacl() {
        let temp_dir = std::env::temp_dir().join(format!(
            "aether_win_dacl_unsafe_{}",
            uuid::Uuid::new_v4().simple()
        ));
        std::fs::create_dir_all(&temp_dir).unwrap();

        unsafe {
            set_permissive_dacl_for_everyone(&temp_dir, false)
                .expect("Failed to set unsafe permissive DACL for Everyone");
        }

        let res = verify_storage_parent_acl(&temp_dir);
        assert!(
            res.is_err(),
            "Parent with unsafe permissive DACL must be rejected"
        );
        let err_msg = format!("{:?}", res);
        assert!(
            err_msg.contains("unsafe permissions"),
            "Error must indicate unsafe permissions: {}",
            err_msg
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    #[cfg(windows)]
    fn test_windows_parent_different_user() {
        let temp_dir = std::env::temp_dir().join(format!(
            "aether_win_dacl_diff_user_{}",
            uuid::Uuid::new_v4().simple()
        ));
        std::fs::create_dir_all(&temp_dir).unwrap();

        set_fault_inject_windows_different_owner(true);
        let res = verify_storage_parent_acl(&temp_dir);
        set_fault_inject_windows_different_owner(false);

        assert!(
            res.is_err(),
            "Parent owned by a different user must be rejected"
        );
        let err_msg = format!("{:?}", res);
        assert!(
            err_msg.contains("not owned by an authorized private owner"),
            "Error must indicate ownership mismatch: {}",
            err_msg
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    #[cfg(windows)]
    fn test_windows_parent_inherited_permissive_aces() {
        let temp_dir = std::env::temp_dir().join(format!(
            "aether_win_dacl_inherited_{}",
            uuid::Uuid::new_v4().simple()
        ));
        std::fs::create_dir_all(&temp_dir).unwrap();

        let parent_dir = temp_dir.join("parent_with_inherited");
        std::fs::create_dir_all(&parent_dir).unwrap();

        unsafe {
            set_permissive_dacl_for_everyone(&parent_dir, true)
                .expect("Failed to set inherited permissive DACL for Everyone");
        }

        let child_storage = parent_dir.join("child_storage");
        std::fs::create_dir_all(&child_storage).unwrap();

        let res = verify_storage_parent_acl(&child_storage);
        assert!(
            res.is_err(),
            "Parent with inherited permissive ACEs must be rejected"
        );
        let err_msg = format!("{:?}", res);
        assert!(
            err_msg.contains("unsafe permissions"),
            "Error must indicate unsafe permissions from inherited ACEs: {}",
            err_msg
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    #[cfg(windows)]
    fn test_windows_parent_inspection_failure() {
        let temp_dir = std::env::temp_dir().join(format!(
            "aether_win_dacl_inspect_fail_{}",
            uuid::Uuid::new_v4().simple()
        ));
        std::fs::create_dir_all(&temp_dir).unwrap();

        set_fault_inject_windows_security_descriptor_failure(true);
        let res = verify_storage_parent_acl(&temp_dir);
        set_fault_inject_windows_security_descriptor_failure(false);

        assert!(
            res.is_err(),
            "Inspection failure on security descriptor must fail closed"
        );
        let err_msg = format!("{:?}", res);
        assert!(
            err_msg.contains("failed to inspect security descriptor"),
            "Error must indicate inspection failure: {}",
            err_msg
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    #[cfg(windows)]
    fn test_windows_parent_broad_group_owner_cannot_bypass_private_acl() {
        let temp_dir = std::env::temp_dir().join(format!(
            "aether_win_dacl_broad_group_owner_{}",
            uuid::Uuid::new_v4().simple()
        ));
        std::fs::create_dir_all(&temp_dir).unwrap();

        // 1. Establish an otherwise secure, valid private DACL first so that DACL inspection
        // cannot trigger rejection; the test strictly isolates the owner-validation check.
        secure_storage_parent_private_acl(&temp_dir).expect("secure private acl");

        // 2. Simulate owner as broad group BUILTIN\Users (Well-Known SID type 27: WinBuiltinUsersSid = S-1-5-32-545)
        const WIN_BUILTIN_USERS_SID: u32 = 27;
        set_fault_inject_windows_simulated_owner_sid(Some(WIN_BUILTIN_USERS_SID));
        let res = verify_storage_parent_acl(&temp_dir);
        set_fault_inject_windows_simulated_owner_sid(None);

        // 3. Confirm that validation fails specifically because owner is not an accepted private owner,
        // and does NOT fail due to DACL inspection.
        assert!(
            res.is_err(),
            "Parent directory owned by BUILTIN\\Users must be rejected"
        );
        let err_msg = format!("{:?}", res);
        assert!(
            err_msg.contains("not owned by an authorized private owner"),
            "Error must indicate rejection of broad group owner: {}",
            err_msg
        );
        assert!(
            !err_msg.contains("unsafe permissions"),
            "Rejection must be strictly due to broad group owner, not DACL inspection: {}",
            err_msg
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    #[cfg(windows)]
    fn test_windows_parent_broad_group_dangerous_dacl_rejected() {
        let temp_dir = std::env::temp_dir().join(format!(
            "aether_win_dacl_broad_group_dacl_{}",
            uuid::Uuid::new_v4().simple()
        ));
        std::fs::create_dir_all(&temp_dir).unwrap();

        // 1. Establish individual TokenUser ownership explicitly rather than relying on
        // test runner default ownership (which on elevated Windows runners defaults to BUILTIN\Administrators).
        unsafe {
            set_directory_owner_to_token_user(&temp_dir)
                .expect("Failed to set directory owner to individual TokenUser");
        }

        // Explicitly verify and prove that the directory's actual owner SID is exactly
        // the current process's individual TokenUser SID.
        // Fails fixture setup if identities do not match (no fallback to Administrators/SYSTEM).
        let (owner_sid, token_user_sid) =
            unsafe { assert_directory_owner_is_token_user(&temp_dir) };
        println!("test_windows_parent_broad_group_dangerous_dacl_rejected: directory actual owner SID: {}", owner_sid);
        println!("test_windows_parent_broad_group_dangerous_dacl_rejected: process actual TokenUser SID: {}", token_user_sid);
        assert_eq!(
            owner_sid, token_user_sid,
            "Directory actual owner SID must match current individual TokenUser SID"
        );

        // Ensure fault injection seams are cleared so owner verification passes cleanly
        // and execution reaches DACL inspection.
        set_fault_inject_windows_simulated_owner_sid(None);
        set_fault_inject_windows_simulated_admin_token(false);

        // 2. Grant dangerous permissions (GENERIC_ALL = 0x10000000, which includes write/modify/delete)
        // to broad group BUILTIN\Users (Well-Known SID type 27: WinBuiltinUsersSid = S-1-5-32-545)
        // using the verified ACL fixture constructor.
        const WIN_BUILTIN_USERS_SID: u32 = 27;
        let dacl_res =
            unsafe { set_permissive_dacl_for_sid(&temp_dir, WIN_BUILTIN_USERS_SID, false) };
        assert!(
            dacl_res.is_ok(),
            "Installing dangerous DACL fixture must succeed: {:?}",
            dacl_res.err()
        );

        // 3. Invoke the production security verifier. Rejection must be triggered specifically by DACL inspection.
        let res_dacl = verify_storage_parent_acl(&temp_dir);
        assert!(
            res_dacl.is_err(),
            "Parent directory with dangerous broad-group DACL must be rejected"
        );
        let err_msg = format!("{:?}", res_dacl);
        assert!(
            err_msg.contains("unsafe permissions"),
            "Error must indicate rejection due to unsafe DACL permissions: {}",
            err_msg
        );
        assert!(
            err_msg.contains("S-1-5-32-545"),
            "Error must identify BUILTIN\\Users SID (S-1-5-32-545) as unauthorized principal: {}",
            err_msg
        );
        assert!(
            err_msg.contains("0x001F01FF") || err_msg.contains("0x10000000"),
            "Error must identify dangerous permissions mask (GENERIC_ALL 0x10000000 or NTFS mapped FILE_ALL_ACCESS 0x001F01FF): {}",
            err_msg
        );
        assert!(
            !err_msg.contains("not owned by an authorized private owner"),
            "Directory owner is verified TokenUser; must NOT fail owner validation: {}",
            err_msg
        );

        // 4. Secure remediation: replace unsafe ACL with a valid private ACL
        secure_storage_parent_private_acl(&temp_dir).expect("remediation must succeed");

        // 5. Confirm that the valid configuration is accepted
        let verify_safe = verify_storage_parent_acl(&temp_dir);
        assert!(
            verify_safe.is_ok(),
            "Secured parent directory must pass verification: {:?}",
            verify_safe
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    #[cfg(windows)]
    fn test_windows_parent_token_user_identity_proof() {
        let temp_dir = std::env::temp_dir().join(format!(
            "aether_win_dacl_token_user_proof_{}",
            uuid::Uuid::new_v4().simple()
        ));
        std::fs::create_dir_all(&temp_dir).unwrap();

        // Establish individual TokenUser ownership explicitly rather than relying on
        // test runner default ownership (which on elevated Windows runners defaults to BUILTIN\Administrators).
        unsafe {
            set_directory_owner_to_token_user(&temp_dir)
                .expect("Failed to set directory owner to individual TokenUser");
        }

        let (owner_sid, token_user_sid, is_equal) = unsafe {
            query_directory_owner_and_token_user_sids(&temp_dir)
                .expect("query_directory_owner_and_token_user_sids must succeed")
        };
        println!(
            "test_windows_parent_token_user_identity_proof: directory actual owner SID: {}",
            owner_sid
        );
        println!(
            "test_windows_parent_token_user_identity_proof: process actual TokenUser SID: {}",
            token_user_sid
        );
        println!(
            "test_windows_parent_token_user_identity_proof: EqualSid result: {}",
            is_equal
        );

        assert!(
            is_equal,
            "Created directory must be owned by current individual TokenUser"
        );
        assert_eq!(owner_sid, token_user_sid);
        assert!(!owner_sid.is_empty());

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    #[cfg(windows)]
    fn test_windows_parent_authorized_administrators_owner() {
        let temp_dir = std::env::temp_dir().join(format!(
            "aether_win_dacl_admin_owner_{}",
            uuid::Uuid::new_v4().simple()
        ));
        std::fs::create_dir_all(&temp_dir).unwrap();

        const WIN_BUILTIN_ADMINISTRATORS_SID: u32 = 26;
        set_fault_inject_windows_simulated_owner_sid(Some(WIN_BUILTIN_ADMINISTRATORS_SID));
        set_fault_inject_windows_simulated_admin_token(true);

        secure_storage_parent_private_acl(&temp_dir).expect("secure private acl");
        let res = verify_storage_parent_acl(&temp_dir);

        set_fault_inject_windows_simulated_owner_sid(None);
        set_fault_inject_windows_simulated_admin_token(false);

        assert!(
            res.is_ok(),
            "Authorized elevated Administrators-owned parent must be accepted: {:?}",
            res
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[cfg(windows)]
    unsafe fn get_process_token_privileges() -> std::result::Result<Vec<(u32, i32, u32)>, String> {
        const TOKEN_QUERY: u32 = 0x0008;
        #[allow(non_upper_case_globals)]
        const TokenPrivileges: u32 = 3;

        extern "system" {
            fn OpenProcessToken(
                ProcessHandle: *mut std::ffi::c_void,
                DesiredAccess: u32,
                TokenHandle: *mut *mut std::ffi::c_void,
            ) -> i32;
            fn GetTokenInformation(
                TokenHandle: *mut std::ffi::c_void,
                TokenInformationClass: u32,
                TokenInformation: *mut std::ffi::c_void,
                TokenInformationLength: u32,
                ReturnLength: *mut u32,
            ) -> i32;
            fn CloseHandle(hObject: *mut std::ffi::c_void) -> i32;
            fn GetCurrentProcess() -> *mut std::ffi::c_void;
            fn GetLastError() -> u32;
        }

        #[repr(C)]
        #[allow(clippy::upper_case_acronyms)]
        struct LUID {
            low_part: u32,
            high_part: i32,
        }
        #[repr(C)]
        #[allow(clippy::upper_case_acronyms)]
        struct LUID_AND_ATTRIBUTES {
            luid: LUID,
            attributes: u32,
        }

        let mut token: *mut std::ffi::c_void = std::ptr::null_mut();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
            return Err(format!(
                "OpenProcessToken failed with Win32 error {}",
                GetLastError()
            ));
        }

        let mut len = 0u32;
        GetTokenInformation(token, TokenPrivileges, std::ptr::null_mut(), 0, &mut len);
        let mut buf = vec![0u8; len as usize];
        if GetTokenInformation(
            token,
            TokenPrivileges,
            buf.as_mut_ptr() as *mut _,
            len,
            &mut len,
        ) == 0
        {
            CloseHandle(token);
            return Err(format!(
                "GetTokenInformation(TokenPrivileges) failed with Win32 error {}",
                GetLastError()
            ));
        }
        CloseHandle(token);

        let count = *(buf.as_ptr() as *const u32);
        let entries = buf.as_ptr().add(std::mem::size_of::<u32>()) as *const LUID_AND_ATTRIBUTES;
        let mut privs = Vec::with_capacity(count as usize);
        for i in 0..count {
            let entry = &*entries.add(i as usize);
            privs.push((entry.luid.low_part, entry.luid.high_part, entry.attributes));
        }
        privs.sort_by_key(|p| (p.0, p.1));
        Ok(privs)
    }

    #[cfg(windows)]
    unsafe fn assert_thread_not_impersonating() {
        const TOKEN_QUERY: u32 = 0x0008;
        const ERROR_NO_TOKEN: u32 = 1008;
        extern "system" {
            fn GetCurrentThread() -> *mut std::ffi::c_void;
            fn OpenThreadToken(
                ThreadHandle: *mut std::ffi::c_void,
                DesiredAccess: u32,
                OpenAsSelf: i32,
                TokenHandle: *mut *mut std::ffi::c_void,
            ) -> i32;
            fn CloseHandle(hObject: *mut std::ffi::c_void) -> i32;
            fn GetLastError() -> u32;
        }

        let mut thread_token: *mut std::ffi::c_void = std::ptr::null_mut();
        let res = OpenThreadToken(GetCurrentThread(), TOKEN_QUERY, 1, &mut thread_token);
        if res != 0 {
            CloseHandle(thread_token);
            panic!("Expected thread NOT to be impersonating, but OpenThreadToken succeeded!");
        }
        let err = GetLastError();
        assert_eq!(
            err, ERROR_NO_TOKEN,
            "Expected OpenThreadToken to fail with ERROR_NO_TOKEN (1008), got {}",
            err
        );
    }

    #[test]
    #[cfg(windows)]
    fn test_windows_token_privilege_isolation_and_thread_impersonation_reversion() {
        let temp_dir = std::env::temp_dir().join(format!(
            "aether_win_priv_isolation_{}",
            uuid::Uuid::new_v4().simple()
        ));
        std::fs::create_dir_all(&temp_dir).unwrap();

        // 1. Verify thread is not impersonating before test begins
        unsafe {
            assert_thread_not_impersonating();
        }

        // 2. Query process primary token's privileges before calling set_directory_owner_to_token_user
        let privs_before = unsafe {
            get_process_token_privileges().expect("querying process token privileges before")
        };
        assert!(
            !privs_before.is_empty(),
            "Process token must have privileges"
        );

        // 3. Call set_directory_owner_to_token_user to set owner to individual TokenUser
        let set_res = unsafe { set_directory_owner_to_token_user(&temp_dir) };
        assert!(
            set_res.is_ok(),
            "set_directory_owner_to_token_user must succeed: {:?}",
            set_res.err()
        );

        // 4. Query process primary token's privileges after calling set_directory_owner_to_token_user
        let privs_after = unsafe {
            get_process_token_privileges().expect("querying process token privileges after")
        };

        // Assert exact privilege equality: process primary token must be completely untouched
        assert_eq!(
            privs_before, privs_after,
            "Process primary token privileges must remain completely untouched after set_directory_owner_to_token_user"
        );

        // 5. Verify the current thread is not impersonating after the call completes
        unsafe {
            assert_thread_not_impersonating();
        }

        // 6. Verify the directory owner was actually updated to the expected TokenUser SID
        let (owner_sid, token_user_sid) =
            unsafe { assert_directory_owner_is_token_user(&temp_dir) };
        assert_eq!(
            owner_sid, token_user_sid,
            "Directory owner SID must match process TokenUser SID"
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_replace_file_publication_staging_removal_failure_surfaces_residual() {
        use sha2::Digest;
        let temp_dir = std::env::temp_dir().join(format!(
            "aether_replace_staging_fail_{}",
            uuid::Uuid::new_v4().simple()
        ));
        std::fs::create_dir_all(&temp_dir).unwrap();
        let target = temp_dir.join("test.txt");
        std::fs::write(&target, b"original content").unwrap();

        let caps = WorkspaceCapability::open(&temp_dir).unwrap();
        let hooks = ConditionalReplaceHooks {
            fault_inject_staging_removal: true,
            ..Default::default()
        };

        let outcome = caps
            .replace_file_bytes_conditional_with_hooks(
                "test.txt",
                b"new content",
                Some(&format!("{:x}", sha2::Sha256::digest(b"original content"))),
                hooks,
            )
            .expect("replacement should succeed");

        assert_eq!(outcome.dest_path, caps.root_path().join("test.txt"));
        assert!(
            outcome.residual_staging.is_some(),
            "Residual staging path must be surfaced when staging removal fails"
        );
        assert!(
            !outcome.is_fully_clean(),
            "Outcome must indicate not fully clean due to residual staging"
        );
        let residual = outcome.residual_staging.unwrap();
        assert!(
            residual.exists(),
            "Residual staging artifact must remain on disk: {:?}",
            residual
        );
        assert_eq!(std::fs::read(&target).unwrap(), b"new content");

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_replace_file_stage_write_failure_surfaces_residual_path() {
        use sha2::Digest;
        let temp_dir = std::env::temp_dir().join(format!(
            "aether_replace_stage_write_fail_{}",
            uuid::Uuid::new_v4().simple()
        ));
        std::fs::create_dir_all(&temp_dir).unwrap();
        let target = temp_dir.join("test.txt");
        std::fs::write(&target, b"original content").unwrap();

        let caps = WorkspaceCapability::open(&temp_dir).unwrap();
        let hooks = ConditionalReplaceHooks {
            fault_inject_stage_write: true,
            fault_inject_staging_removal: true,
            ..Default::default()
        };

        let res = caps.replace_file_bytes_conditional_with_hooks(
            "test.txt",
            b"new content",
            Some(&format!("{:x}", sha2::Sha256::digest(b"original content"))),
            hooks,
        );

        assert!(res.is_err(), "Stage write failure must be returned");
        let err_str = format!("{:?}", res);
        assert!(
            err_str.contains("Injected stage write failure"),
            "Error must preserve primary error: {}",
            err_str
        );
        assert!(
            err_str.contains("additionally failed to remove residual staging artifact"),
            "Error must expose residual staging cleanup failure and artifact path: {}",
            err_str
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_replace_file_stage_sync_failure_surfaces_residual_path() {
        use sha2::Digest;
        let temp_dir = std::env::temp_dir().join(format!(
            "aether_replace_stage_sync_fail_{}",
            uuid::Uuid::new_v4().simple()
        ));
        std::fs::create_dir_all(&temp_dir).unwrap();
        let target = temp_dir.join("test.txt");
        std::fs::write(&target, b"original content").unwrap();

        let caps = WorkspaceCapability::open(&temp_dir).unwrap();
        let hooks = ConditionalReplaceHooks {
            fault_inject_stage_sync: true,
            fault_inject_staging_removal: true,
            ..Default::default()
        };

        let res = caps.replace_file_bytes_conditional_with_hooks(
            "test.txt",
            b"new content",
            Some(&format!("{:x}", sha2::Sha256::digest(b"original content"))),
            hooks,
        );

        assert!(res.is_err(), "Stage sync failure must be returned");
        let err_str = format!("{:?}", res);
        assert!(
            err_str.contains("Injected stage sync failure"),
            "Error must preserve primary error: {}",
            err_str
        );
        assert!(
            err_str.contains("additionally failed to remove residual staging artifact"),
            "Error must expose residual staging cleanup failure and artifact path: {}",
            err_str
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_replace_file_missing_target_collision_surfaces_residual_path() {
        let temp_dir = std::env::temp_dir().join(format!(
            "aether_missing_collision_{}",
            uuid::Uuid::new_v4().simple()
        ));
        std::fs::create_dir_all(&temp_dir).unwrap();

        let caps = WorkspaceCapability::open(&temp_dir).unwrap();
        let hooks = ConditionalReplaceHooks {
            pre_replace_hook: Some(&|p| {
                let _ = std::fs::write(p, b"competing creator");
            }),
            fault_inject_staging_removal: true,
            ..Default::default()
        };

        let res = caps.replace_file_bytes_conditional_with_hooks(
            "missing.txt",
            b"new content",
            None,
            hooks,
        );

        assert!(res.is_err(), "Collision must fail");
        let err_str = format!("{:?}", res);
        assert!(
            err_str.contains("Restore race detected"),
            "Must report restore race: {}",
            err_str
        );
        assert!(
            err_str.contains("additionally failed to remove residual staging artifact"),
            "Must surface residual staging artifact on collision: {}",
            err_str
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_perform_rollback_staging_removal_failure_surfaces_in_error() {
        let temp_dir = std::env::temp_dir().join(format!(
            "aether_rollback_staging_fail_{}",
            uuid::Uuid::new_v4().simple()
        ));
        std::fs::create_dir_all(&temp_dir).unwrap();
        let target = temp_dir.join("test.txt");
        std::fs::write(&target, b"original content").unwrap();

        let caps = WorkspaceCapability::open(&temp_dir).unwrap();
        let hooks = ConditionalReplaceHooks {
            fault_inject_staging_removal: true,
            ..Default::default()
        };

        let res = caps.replace_file_bytes_conditional_with_hooks(
            "test.txt",
            b"new content",
            Some("0000000000000000000000000000000000000000000000000000000000000000"),
            hooks,
        );

        assert!(res.is_err(), "Hash mismatch must trigger rollback");
        let err_str = format!("{:?}", res);
        assert!(
            err_str.contains("Restore race detected"),
            "Error must contain primary restore race error: {}",
            err_str
        );
        assert!(
            err_str.contains("additionally failed to remove residual staging artifact"),
            "Error must surface staging removal failure and residual path: {}",
            err_str
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
