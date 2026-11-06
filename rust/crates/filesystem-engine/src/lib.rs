use application_core::{AetherError, Result};
use cap_std::ambient_authority;
use cap_std::fs::{Dir, OpenOptions};
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use tracing::warn;

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

/// Optional hooks and fault-injection flags for conditional file replacement.
#[derive(Default, Clone)]
pub struct ConditionalReplaceHooks<'a> {
    /// Injected immediately before the existing target file is moved to backup.
    pub pre_replace_hook: Option<&'a (dyn Fn(&Path) + 'a)>,
    /// Injected after backup hash calculation and before the stage-to-destination commit.
    pub post_hash_hook: Option<&'a (dyn Fn(&Path) + 'a)>,
    /// Injected fault: simulate I/O error during stage file write.
    pub fault_inject_stage_write: bool,
    /// Injected fault: simulate I/O error during stage file sync.
    pub fault_inject_stage_sync: bool,
    /// Injected fault: simulate failure during rollback rename.
    pub fault_inject_rollback_failure: bool,
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
    ) -> Result<PathBuf> {
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
                    return Ok(dest_abs);
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
        let trailing_modified = match self.dir.open(&backup_rel).and_then(|mut f| {
            let mut b = Vec::new();
            f.read_to_end(&mut b)?;
            Ok(b)
        }) {
            Ok(b) => {
                use sha2::{Digest, Sha256};
                let mut hasher = Sha256::new();
                hasher.update(&b);
                format!("{:x}", hasher.finalize()) != expected
            }
            Err(_) => false,
        };

        if trailing_modified {
            // A process holding an open handle wrote to the backup after commit!
            // DO NOT DELETE backup! Keep it preserved with explicit conflict name.
            let conflict_name = format!(
                "{}.aether-conflict-{}",
                user_path,
                uuid::Uuid::new_v4().simple()
            );
            let conflict_rel = PathBuf::from(&conflict_name);
            let _ = self.dir.rename(&backup_rel, &self.dir, &conflict_rel);
            warn!(
                "Trailing modification detected on backup of '{}'. Preserved at '{}'.",
                user_path, conflict_name
            );
        } else {
            let _ = self.dir.remove_file(&backup_rel);
        }

        Ok(dest_abs)
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
        self.replace_file_bytes_conditional_with_hooks(user_path, bytes, expected_hash, hooks)
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

    /// Deletes a file entry inside workspace capability.
    /// If target is a symlink, removes the symlink entry itself without following to target.
    pub fn delete_file(&self, user_path: &str) -> Result<()> {
        let rel = sanitize_relative_path(user_path)?;
        self.dir.remove_file(&rel).map_err(AetherError::Io)
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
}
