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

    /// Securely resolves a user-provided relative path against a workspace root.
    /// Strictly guarantees that the resolved target cannot escape the workspace root
    /// via parent directory traversal (`..`), absolute paths, Windows drive letters,
    /// Windows device files (CON, NUL, AUX, PRN), or filesystem symlinks.
    pub fn resolve_contained_path(root: &Path, user_path: &str) -> Result<PathBuf> {
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
            if !candidate_canon.starts_with(&root_canon) {
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
                    if !parent_canon.starts_with(&root_canon) {
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
}
