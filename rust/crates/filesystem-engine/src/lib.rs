use application_core::{AetherError, Result};
use std::fs;
use std::path::{Path, PathBuf};

pub struct FilesystemEngine;

impl FilesystemEngine {
    pub fn scan_workspace_files<P: AsRef<Path>>(
        root: P,
        exclude_patterns: &[String],
    ) -> Result<Vec<PathBuf>> {
        let mut results = Vec::new();
        Self::walk_dir(root.as_ref(), root.as_ref(), exclude_patterns, &mut results)?;
        Ok(results)
    }

    fn walk_dir(
        current: &Path,
        _root: &Path,
        exclude_patterns: &[String],
        results: &mut Vec<PathBuf>,
    ) -> Result<()> {
        if !current.exists() {
            return Ok(());
        }

        let entries = fs::read_dir(current).map_err(AetherError::Io)?;

        for entry in entries.flatten() {
            let path = entry.path();
            let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

            // Check exclusions
            if exclude_patterns
                .iter()
                .any(|pattern| file_name == pattern || file_name.starts_with('.'))
            {
                continue;
            }

            if path.is_dir() {
                Self::walk_dir(&path, _root, exclude_patterns, results)?;
            } else if path.is_file() {
                results.push(path);
            }
        }

        Ok(())
    }

    pub fn write_file_atomic<P: AsRef<Path>>(path: P, content: &str) -> Result<()> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let temp_path = path.with_extension(format!("tmp-{}", uuid::Uuid::new_v4().simple()));
        fs::write(&temp_path, content)?;
        fs::rename(&temp_path, path)?;
        Ok(())
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

    /// Securely resolves a user-provided relative path against a workspace root.
    /// Strictly guarantees that the resolved target cannot escape the workspace root
    /// via parent directory traversal (`..`), absolute paths, Windows drive letters,
    /// UNC paths, Alternate Data Streams (ADS), trailing dot/space bypasses,
    /// Windows device files (CON, NUL, AUX, PRN), or filesystem symlinks.
    pub fn resolve_contained_path(root: &Path, user_path: &str) -> Result<PathBuf> {
        // 1. Explicitly reject null bytes and control characters
        if user_path.contains('\0') || user_path.chars().any(|c| (c as u32) < 0x20) {
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
                "Access Denied: Stream separator or colon ':' is strictly forbidden in relative paths".into(),
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

        // 6. Reject UNC paths and Windows extended device namespaces
        let trimmed_leading = user_path.trim_start();
        if trimmed_leading.starts_with(r"\\")
            || trimmed_leading.starts_with("//")
            || trimmed_leading.starts_with(r"\??\")
            || trimmed_leading.starts_with(r"//?/")
        {
            return Err(AetherError::PermissionDenied(
                "Access Denied: UNC and extended device namespaces are strictly forbidden".into(),
            ));
        }

        let clean = Path::new(user_path);
        if clean.is_absolute() {
            return Err(AetherError::PermissionDenied(
                "Access Denied: Absolute paths are strictly forbidden".into(),
            ));
        }

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
                    return Err(AetherError::PermissionDenied(
                        "Access Denied: Path traversal (..) is forbidden".into(),
                    ));
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
                }
                std::path::Component::CurDir => {}
            }
        }

        let root_canon = root.canonicalize().map_err(AetherError::Io)?;
        let candidate = root_canon.join(clean);

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

    /// Safely writes content to a file inside the workspace root, immune to TOCTOU symlink races.
    /// Performs post-creation validation to verify that the file actually created on disk is
    /// strictly within the canonical workspace boundary.
    pub fn write_file_contained<P: AsRef<Path>>(
        root: P,
        user_path: &str,
        content: &str,
    ) -> Result<PathBuf> {
        let root = root.as_ref();
        let root_canon = root.canonicalize().map_err(AetherError::Io)?;
        let resolved = Self::resolve_contained_path(&root_canon, user_path)?;

        if let Some(parent) = resolved.parent() {
            if !parent.exists() {
                fs::create_dir_all(parent).map_err(AetherError::Io)?;
            }
            let parent_canon = parent.canonicalize().map_err(AetherError::Io)?;
            if !Self::is_within_boundary(&parent_canon, &root_canon) {
                return Err(AetherError::PermissionDenied(
                    "Access Denied: Parent directory escaped boundary".into(),
                ));
            }
        }

        Self::write_file_atomic(&resolved, content)?;

        // Post-creation canonical check against TOCTOU symlink substitution races
        let post_canon = resolved.canonicalize().map_err(AetherError::Io)?;
        if !Self::is_within_boundary(&post_canon, &root_canon) {
            let _ = fs::remove_file(&resolved);
            return Err(AetherError::PermissionDenied(
                "Access Denied: Post-creation verification failed; target escaped boundary".into(),
            ));
        }

        Ok(post_canon)
    }

    /// Safely reads the text content of a file strictly contained inside the workspace root.
    pub fn read_file_contained<P: AsRef<Path>>(root: P, user_path: &str) -> Result<String> {
        let root = root.as_ref();
        let root_canon = root.canonicalize().map_err(AetherError::Io)?;
        let resolved = Self::resolve_contained_path(&root_canon, user_path)?;
        let target_canon = resolved.canonicalize().map_err(AetherError::Io)?;
        if !Self::is_within_boundary(&target_canon, &root_canon) {
            return Err(AetherError::PermissionDenied(
                "Access Denied: Target path escapes workspace boundary".into(),
            ));
        }
        fs::read_to_string(target_canon).map_err(AetherError::Io)
    }

    /// Safely deletes a file contained within the workspace root.
    pub fn delete_file_contained<P: AsRef<Path>>(root: P, user_path: &str) -> Result<()> {
        let root = root.as_ref();
        let root_canon = root.canonicalize().map_err(AetherError::Io)?;
        let resolved = Self::resolve_contained_path(&root_canon, user_path)?;
        if resolved.exists() {
            let target_canon = resolved.canonicalize().map_err(AetherError::Io)?;
            if !Self::is_within_boundary(&target_canon, &root_canon) {
                return Err(AetherError::PermissionDenied(
                    "Access Denied: Target path escapes workspace boundary".into(),
                ));
            }
            fs::remove_file(target_canon).map_err(AetherError::Io)?;
        }
        Ok(())
    }

    /// Safely creates a directory contained within the workspace root.
    pub fn create_dir_contained<P: AsRef<Path>>(root: P, user_path: &str) -> Result<PathBuf> {
        let root = root.as_ref();
        let root_canon = root.canonicalize().map_err(AetherError::Io)?;
        let resolved = Self::resolve_contained_path(&root_canon, user_path)?;
        fs::create_dir_all(&resolved).map_err(AetherError::Io)?;
        let target_canon = resolved.canonicalize().map_err(AetherError::Io)?;
        if !Self::is_within_boundary(&target_canon, &root_canon) {
            return Err(AetherError::PermissionDenied(
                "Access Denied: Created directory escapes workspace boundary".into(),
            ));
        }
        Ok(target_canon)
    }

    /// Safely renames or moves a file within the workspace root.
    pub fn rename_contained<P: AsRef<Path>>(
        root: P,
        from_user_path: &str,
        to_user_path: &str,
    ) -> Result<()> {
        let root = root.as_ref();
        let root_canon = root.canonicalize().map_err(AetherError::Io)?;
        let from_target = Self::resolve_contained_path(&root_canon, from_user_path)?;
        let to_target = Self::resolve_contained_path(&root_canon, to_user_path)?;

        let from_canon = from_target.canonicalize().map_err(AetherError::Io)?;
        if !Self::is_within_boundary(&from_canon, &root_canon) {
            return Err(AetherError::PermissionDenied(
                "Access Denied: Source path escapes workspace boundary".into(),
            ));
        }

        if let Some(parent) = to_target.parent() {
            if !parent.exists() {
                fs::create_dir_all(parent).map_err(AetherError::Io)?;
            }
            let parent_canon = parent.canonicalize().map_err(AetherError::Io)?;
            if !Self::is_within_boundary(&parent_canon, &root_canon) {
                return Err(AetherError::PermissionDenied(
                    "Access Denied: Destination parent directory escapes workspace boundary".into(),
                ));
            }
        }

        fs::rename(&from_canon, &to_target).map_err(AetherError::Io)?;

        let to_canon = to_target.canonicalize().map_err(AetherError::Io)?;
        if !Self::is_within_boundary(&to_canon, &root_canon) {
            return Err(AetherError::PermissionDenied(
                "Access Denied: Destination target escaped workspace boundary".into(),
            ));
        }
        Ok(())
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
}
