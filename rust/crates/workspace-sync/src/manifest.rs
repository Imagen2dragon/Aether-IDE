use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs::File;
use std::io::{self, Read};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FileEntry {
    pub path: String,
    pub size: u64,
    pub sha256: String,
    pub modified_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkspaceManifest {
    pub workspace_name: String,
    pub root_hash: String,
    pub entries: HashMap<String, FileEntry>,
    pub generated_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ManifestDiff {
    pub to_upload: Vec<String>,
    pub to_download: Vec<String>,
    pub to_delete: Vec<String>,
    pub conflicts: Vec<String>,
}

impl WorkspaceManifest {
    pub fn build(
        root: &Path,
        workspace_name: &str,
        ignore_patterns: &[String],
    ) -> io::Result<Self> {
        let mut entries = HashMap::new();
        scan_directory(root, root, ignore_patterns, &mut entries)?;

        // Compute global root hash
        let mut hasher = Sha256::new();
        let mut sorted_paths: Vec<&String> = entries.keys().collect();
        sorted_paths.sort();

        for p in sorted_paths {
            let entry = &entries[p];
            hasher.update(p.as_bytes());
            hasher.update(entry.sha256.as_bytes());
        }

        let root_hash = format!("{:x}", hasher.finalize());
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        Ok(Self {
            workspace_name: workspace_name.to_string(),
            root_hash,
            entries,
            generated_at: now,
        })
    }
}

pub fn diff_manifests(local: &WorkspaceManifest, remote: &WorkspaceManifest) -> ManifestDiff {
    let mut to_upload = Vec::new();
    let mut to_download = Vec::new();
    let to_delete = Vec::new();
    let mut conflicts = Vec::new();

    // Check files on local against remote
    for (path, local_entry) in &local.entries {
        match remote.entries.get(path) {
            Some(remote_entry) => {
                if local_entry.sha256 != remote_entry.sha256 {
                    // Both files differ
                    if local_entry.modified_at != remote_entry.modified_at {
                        conflicts.push(path.clone());
                    } else {
                        to_upload.push(path.clone());
                    }
                }
            }
            None => {
                to_upload.push(path.clone());
            }
        }
    }

    // Check files on remote absent from local
    for path in remote.entries.keys() {
        if !local.entries.contains_key(path) {
            to_download.push(path.clone());
        }
    }

    ManifestDiff {
        to_upload,
        to_download,
        to_delete,
        conflicts,
    }
}

fn scan_directory(
    root: &Path,
    current: &Path,
    ignore_patterns: &[String],
    entries: &mut HashMap<String, FileEntry>,
) -> io::Result<()> {
    if !current.exists() {
        return Ok(());
    }

    for entry in std::fs::read_dir(current)? {
        let entry = entry?;
        let path = entry.path();
        let file_name = entry.file_name().to_string_lossy().to_string();

        if ignore_patterns
            .iter()
            .any(|pattern| file_name == *pattern || file_name.starts_with('.'))
        {
            continue;
        }

        let ft = entry.file_type()?;
        // By security policy, skip symbolic links and reparse points during manifest generation
        if ft.is_symlink() {
            continue;
        }

        if ft.is_dir() {
            scan_directory(root, &path, ignore_patterns, entries)?;
        } else if ft.is_file() {
            let relative = path
                .strip_prefix(root)
                .map_err(|e| io::Error::other(e.to_string()))?
                .to_string_lossy()
                .replace('\\', "/");

            let metadata = entry.metadata()?;
            let size = metadata.len();
            let modified_at = metadata
                .modified()
                .ok()
                .and_then(|m| m.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0);

            let sha256 = hash_file(&path)?;

            entries.insert(
                relative.clone(),
                FileEntry {
                    path: relative,
                    size,
                    sha256,
                    modified_at,
                },
            );
        }
    }
    Ok(())
}

fn hash_file(path: &Path) -> io::Result<String> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 8192];
    loop {
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        hasher.update(&buffer[..n]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_manifest_diff_detection() {
        let mut local_entries = HashMap::new();
        local_entries.insert(
            "src/main.rs".to_string(),
            FileEntry {
                path: "src/main.rs".to_string(),
                size: 100,
                sha256: "hash_v1".to_string(),
                modified_at: 1000,
            },
        );
        local_entries.insert(
            "Cargo.toml".to_string(),
            FileEntry {
                path: "Cargo.toml".to_string(),
                size: 50,
                sha256: "hash_toml".to_string(),
                modified_at: 1000,
            },
        );

        let local_manifest = WorkspaceManifest {
            workspace_name: "test".into(),
            root_hash: "root_1".into(),
            entries: local_entries,
            generated_at: 1000,
        };

        let mut remote_entries = HashMap::new();
        // remote has modified main.rs
        remote_entries.insert(
            "src/main.rs".to_string(),
            FileEntry {
                path: "src/main.rs".to_string(),
                size: 120,
                sha256: "hash_v2".to_string(),
                modified_at: 2000,
            },
        );
        // remote has an extra file
        remote_entries.insert(
            "README.md".to_string(),
            FileEntry {
                path: "README.md".to_string(),
                size: 200,
                sha256: "hash_readme".to_string(),
                modified_at: 2000,
            },
        );

        let remote_manifest = WorkspaceManifest {
            workspace_name: "test".into(),
            root_hash: "root_2".into(),
            entries: remote_entries,
            generated_at: 2000,
        };

        let diff = diff_manifests(&local_manifest, &remote_manifest);
        assert_eq!(diff.to_download, vec!["README.md"]);
        assert!(diff.conflicts.contains(&"src/main.rs".to_string()));
        assert_eq!(diff.to_upload, vec!["Cargo.toml"]);
    }
}
