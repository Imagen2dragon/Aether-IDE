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

        let entries = fs::read_dir(current)
            .map_err(|e| AetherError::Io(e))?;

        for entry in entries.flatten() {
            let path = entry.path();
            let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

            // Check exclusions
            if exclude_patterns.iter().any(|pattern| file_name == pattern || file_name.starts_with('.')) {
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
        fs::read_to_string(path).map_err(|e| AetherError::Io(e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_atomic_write_and_read() {
        let temp_dir = std::env::temp_dir().join(format!("aether_fs_test_{}", uuid::Uuid::new_v4().simple()));
        let test_file = temp_dir.join("subdir").join("test.txt");

        FilesystemEngine::write_file_atomic(&test_file, "Hello AetherIDE Atomic").expect("atomic write failed");
        let content = FilesystemEngine::read_file_string(&test_file).expect("read failed");
        assert_eq!(content, "Hello AetherIDE Atomic");

        let _ = fs::remove_dir_all(temp_dir);
    }
}
