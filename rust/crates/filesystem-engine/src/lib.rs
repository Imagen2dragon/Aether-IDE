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
    static FAULT_INJECT_WINDOWS_DEST_IDENTITY_QUERY_FAILURE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
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

    // While exclusive handle on staging file remains open, query and verify destination identity (Sprint 23.2.16 P1)
    if let Some(dest) = destination_path {
        // Hook while exclusive handle is held, before destination verification (CHK-210)
        if let Some(ref hook) = get_hook_windows_post_exclusive_open() {
            hook(path, dest);
        }

        // Check fault injection for destination identity query failure (CHK-213)
        if is_fault_inject_windows_dest_identity_query_failure() {
            return Err(AetherError::Io(std::io::Error::other(
                "Injected destination identity query failure while exclusive handle held",
            )));
        }

        let dest_identity = query_file_object_identity(dest)?;

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
    }

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
            let _ = self.dir.remove_file(&temp_rel);
            return Err(AetherError::Io(e));
        }

        Ok(self.root_path.join(rel))
    }

    /// Internal rollback helper that strictly verifies rollback rename results.
    /// If rollback fails, the backup file is preserved on disk without deletion,
    /// and an explicit `AetherError::Internal` containing the backup location is returned.
    fn perform_rollback(
        &self,
        backup_rel: &Path,
        rel: &Path,
        stage_rel: Option<&Path>,
        user_path: &str,
        cause_msg: &str,
        fault_inject_rollback_failure: bool,
    ) -> AetherError {
        // Clean up temporary incoming stage file if present
        if let Some(stage) = stage_rel {
            let _ = self.dir.remove_file(stage);
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
                "Restore race detected: {}. Restore aborted, target rolled back successfully.",
                cause_msg
            )),
            Err(rb_err) => {
                let backup_abs = self.root_path.join(backup_rel);
                AetherError::Internal(format!(
                    "RESTORE ROLLBACK FAILURE on '{}': {}. Attempted rollback to destination failed ({}). User data PRESERVED at '{}'. Do not delete this file; manual recovery required.",
                    user_path, cause_msg, rb_err, backup_abs.display()
                ))
            }
        }
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

            let mut opts = OpenOptions::new();
            opts.write(true).create_new(true);

            let mut f = self
                .dir
                .open_with(&stage_rel, &opts)
                .map_err(AetherError::Io)?;

            if hooks.fault_inject_stage_write {
                drop(f);
                let _ = self.dir.remove_file(&stage_rel);
                return Err(AetherError::Io(std::io::Error::other(
                    "Injected stage write failure",
                )));
            }

            if let Err(e) = f.write_all(bytes) {
                drop(f);
                let _ = self.dir.remove_file(&stage_rel);
                return Err(AetherError::Io(e));
            }

            if hooks.fault_inject_stage_sync {
                drop(f);
                let _ = self.dir.remove_file(&stage_rel);
                return Err(AetherError::Io(std::io::Error::other(
                    "Injected stage sync failure",
                )));
            }

            if let Err(e) = f.sync_all() {
                drop(f);
                let _ = self.dir.remove_file(&stage_rel);
                return Err(AetherError::Io(e));
            }
            drop(f);

            // 2. Publish stage file using exclusive no-overwrite operation
            let link_res = self.dir.hard_link(&stage_rel, &self.dir, &rel);
            match link_res {
                Ok(()) => {
                    let _ = self.dir.remove_file(&stage_rel);
                    return Ok(ConditionalReplaceOutcome {
                        dest_path: dest_abs,
                        preserved_backup: None,
                    });
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                    let _ = self.dir.remove_file(&stage_rel);
                    return Err(AetherError::PermissionDenied(format!(
                        "Restore race detected: target file '{}' was newly created on disk before replacement. Restore aborted.",
                        user_path
                    )));
                }
                Err(e) => {
                    // Check if file exists to detect race condition
                    if self.dir.symlink_metadata(&rel).is_ok() {
                        let _ = self.dir.remove_file(&stage_rel);
                        return Err(AetherError::PermissionDenied(format!(
                            "Restore race detected: target file '{}' was newly created on disk before replacement. Restore aborted.",
                            user_path
                        )));
                    }
                    let _ = self.dir.remove_file(&stage_rel);
                    return Err(AetherError::Io(e));
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

        let mut opts = OpenOptions::new();
        opts.write(true).create_new(true);

        let mut f = self
            .dir
            .open_with(&stage_rel, &opts)
            .map_err(AetherError::Io)?;
        f.write_all(bytes).map_err(AetherError::Io)?;
        f.sync_all().map_err(AetherError::Io)?;
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
            let _ = self.dir.remove_file(&stage_rel);
            return Err(AetherError::PermissionDenied(format!(
                "Restore race detected: target file '{}' could not be staged for verification ({}). Restore aborted.",
                user_path, e
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
            ));
        }

        // 7. Hash verified! Commit replacement: move stage to target
        if let Err(e) = self.dir.rename(&stage_rel, &self.dir, &rel) {
            return Err(self.perform_rollback(
                &backup_rel,
                &rel,
                Some(&stage_rel),
                user_path,
                &format!("failed to commit replacement file ({})", e),
                hooks.fault_inject_rollback_failure,
            ));
        }

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
}
