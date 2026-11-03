use crate::workspace_bridge::SharedWorkspace;
use filesystem_engine::WorkspaceCapability;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tauri::{AppHandle, Manager};
use tracing::{info, warn};

pub const APPROVED_CLEANUP_WHITELIST: &[(&str, &str, &str)] = &[
    ("target", "Rust Build Artifacts", "Build Artifacts"),
    ("node_modules", "Node Dependencies", "Dependencies"),
    ("dist", "Build Output", "Build Artifacts"),
    ("build", "Gradle / Compiler Output", "Build Artifacts"),
    (".gradle", "Gradle Cache", "Caches"),
    (".turbo", "Turborepo Cache", "Caches"),
    (".cache", "Local Build Cache", "Caches"),
    (".pytest_cache", "Pytest Cache", "Caches"),
    ("__pycache__", "Python Bytecode", "Caches"),
    (".next", "Next.js Build Cache", "Build Artifacts"),
    (".aether/worktrees", "Agent Worktrees", "Agent Runtime"),
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageCleanupCandidate {
    pub candidate_id: String,
    pub workspace_id: String,
    pub root_path: PathBuf,
    pub relative_path: String,
    pub category: String,
    pub estimated_bytes: u64,
    pub human_size: String,
    pub file_count: usize,
    pub eligible: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageItem {
    pub candidate_id: String,
    pub name: String,
    pub path: String, // candidate_id used for client-side selection
    pub relative_path: String,
    pub category: String,
    pub size_bytes: u64,
    pub human_size: String,
    pub item_count: usize,
    pub is_cleanable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageAnalysisReport {
    pub total_size_bytes: u64,
    pub total_human_size: String,
    pub items: Vec<StorageItem>,
    pub cleanable_bytes: u64,
    pub cleanable_human_size: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageCleanupReport {
    pub dry_run: bool,
    pub freed_bytes: u64,
    pub freed_human_size: String,
    pub deleted_paths: Vec<String>,
    pub errors: Vec<String>,
}

pub type SharedStorageCandidates = Arc<Mutex<HashMap<String, StorageCleanupCandidate>>>;

pub fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

pub fn dir_size_recursive(path: &Path) -> (u64, usize) {
    let mut total_size = 0u64;
    let mut total_files = 0usize;

    // Check if target is a symlink - never traverse symlinks for size
    if let Ok(meta) = std::fs::symlink_metadata(path) {
        if meta.file_type().is_symlink() {
            return (0, 0);
        }
    }

    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            if let Ok(sym) = entry.file_type() {
                if sym.is_symlink() {
                    continue;
                }
            }
            if let Ok(meta) = entry.metadata() {
                if meta.is_dir() {
                    let (sub_size, sub_files) = dir_size_recursive(&entry.path());
                    total_size += sub_size;
                    total_files += sub_files;
                } else {
                    total_size += meta.len();
                    total_files += 1;
                }
            }
        }
    }
    (total_size, total_files)
}

#[tauri::command]
pub async fn storage_analyze(
    app: AppHandle,
    _path: Option<String>,
) -> Result<StorageAnalysisReport, String> {
    let ws_arc = app.state::<SharedWorkspace>();
    let ws = ws_arc.lock();
    let roots = ws.get_roots();
    let ws_id = ws.workspace_id();
    drop(ws);

    let candidate_registry = app.state::<SharedStorageCandidates>();
    let mut candidate_map = candidate_registry.lock();
    candidate_map.clear();

    let mut items = Vec::new();
    let mut total_size = 0u64;
    let mut cleanable_size = 0u64;

    for root in roots {
        // Measure .git (non-cleanable version control)
        let git_dir = root.join(".git");
        if git_dir.exists() {
            let (g_size, g_count) = dir_size_recursive(&git_dir);
            total_size += g_size;
            items.push(StorageItem {
                candidate_id: format!("git-{}", uuid::Uuid::new_v4().simple()),
                name: "Git Repository Database".into(),
                path: ".git".into(),
                relative_path: ".git".into(),
                category: "Version Control".into(),
                size_bytes: g_size,
                human_size: format_bytes(g_size),
                item_count: g_count,
                is_cleanable: false,
            });
        }

        // Measure approved generated-artifact directories
        for &(rel_path, label, category) in APPROVED_CLEANUP_WHITELIST {
            let full_path = root.join(rel_path);
            if full_path.exists() {
                // Ensure target is not a symlink or Windows junction
                if let Ok(meta) = std::fs::symlink_metadata(&full_path) {
                    if meta.file_type().is_symlink() {
                        continue;
                    }
                }

                let (size, count) = dir_size_recursive(&full_path);
                total_size += size;
                cleanable_size += size;

                let cand_id = format!("cand-{}", uuid::Uuid::new_v4().simple());
                let candidate = StorageCleanupCandidate {
                    candidate_id: cand_id.clone(),
                    workspace_id: ws_id.clone(),
                    root_path: root.clone(),
                    relative_path: rel_path.to_string(),
                    category: category.to_string(),
                    estimated_bytes: size,
                    human_size: format_bytes(size),
                    file_count: count,
                    eligible: true,
                };

                candidate_map.insert(cand_id.clone(), candidate);

                items.push(StorageItem {
                    candidate_id: cand_id.clone(),
                    name: label.to_string(),
                    path: cand_id, // Client uses this identifier for cleanups
                    relative_path: rel_path.to_string(),
                    category: category.to_string(),
                    size_bytes: size,
                    human_size: format_bytes(size),
                    item_count: count,
                    is_cleanable: true,
                });
            }
        }
    }

    Ok(StorageAnalysisReport {
        total_size_bytes: total_size,
        total_human_size: format_bytes(total_size),
        items,
        cleanable_bytes: cleanable_size,
        cleanable_human_size: format_bytes(cleanable_size),
    })
}

#[tauri::command]
pub async fn storage_cleanup(
    app: AppHandle,
    targets: Vec<String>,
    dry_run: bool,
) -> Result<StorageCleanupReport, String> {
    let ws_arc = app.state::<SharedWorkspace>();
    let ws = ws_arc.lock();
    let current_ws_id = ws.workspace_id();
    let authorized_roots = ws.get_roots();
    drop(ws);

    let candidate_registry = app.state::<SharedStorageCandidates>();
    let mut candidate_map = candidate_registry.lock();

    let mut freed = 0u64;
    let mut deleted = Vec::new();
    let mut errors = Vec::new();

    for target in targets {
        // 1. Resolve candidate by ID or matching relative path
        let candidate = if let Some(c) = candidate_map.get(&target).cloned() {
            c
        } else if let Some((_, c)) = candidate_map
            .iter()
            .find(|(_, c)| c.relative_path == target || c.candidate_id == target)
        {
            c.clone()
        } else {
            errors.push(format!(
                "Security Denial: Candidate identifier '{}' is not registered or expired. Run storage analysis first.",
                target
            ));
            continue;
        };

        // 2. Verify workspace identity
        if candidate.workspace_id != current_ws_id {
            errors.push(format!(
                "Security Denial: Candidate '{}' belongs to workspace '{}', but current workspace is '{}'.",
                candidate.relative_path, candidate.workspace_id, current_ws_id
            ));
            continue;
        }

        // 3. Confirm target root is currently authorized
        if !authorized_roots.contains(&candidate.root_path) {
            errors.push(format!(
                "Security Denial: Root {:?} is no longer authorized in active workspace.",
                candidate.root_path
            ));
            continue;
        }

        // 4. Confirm relative path is STILL an approved generated-artifact directory
        let is_whitelisted = APPROVED_CLEANUP_WHITELIST
            .iter()
            .any(|&(w_path, _, _)| w_path == candidate.relative_path);
        if !is_whitelisted {
            errors.push(format!(
                "Security Denial: Relative path '{}' is not an approved generated-artifact directory.",
                candidate.relative_path
            ));
            continue;
        }

        let full_target = candidate.root_path.join(&candidate.relative_path);
        if !full_target.exists() {
            errors.push(format!(
                "Directory '{}' does not exist on disk.",
                candidate.relative_path
            ));
            continue;
        }

        // 5. Revalidate symlinks and reparse points immediately before deletion
        if let Ok(meta) = std::fs::symlink_metadata(&full_target) {
            if meta.file_type().is_symlink() {
                errors.push(format!(
                    "Security Denial: Target '{}' is a symlink. Recursive deletion is forbidden.",
                    candidate.relative_path
                ));
                continue;
            }
        }

        // 6. Calculate actual current size before deletion
        let (current_size, file_count) = dir_size_recursive(&full_target);

        if dry_run {
            freed += current_size;
            deleted.push(format!(
                "[DRY RUN] Would delete {} ({} files, {})",
                candidate.relative_path,
                file_count,
                format_bytes(current_size)
            ));
        } else {
            // 7. Perform capability-anchored recursive deletion
            let cap = match WorkspaceCapability::open(&candidate.root_path) {
                Ok(c) => c,
                Err(e) => {
                    errors.push(format!(
                        "Failed to open workspace capability for {:?}: {}",
                        candidate.root_path, e
                    ));
                    continue;
                }
            };

            match cap.delete_dir_all(&candidate.relative_path) {
                Ok(_) => {
                    // 8. Confirm directory is actually gone from disk
                    if !full_target.exists() {
                        freed += current_size;
                        deleted.push(format!(
                            "Deleted {} (freed {})",
                            candidate.relative_path,
                            format_bytes(current_size)
                        ));
                        info!(
                            "Storage cleanup successfully deleted '{}' (freed {} bytes)",
                            candidate.relative_path, current_size
                        );
                        candidate_map.remove(&candidate.candidate_id);
                    } else {
                        errors.push(format!(
                            "Partial deletion: Directory '{}' still exists after deletion attempt.",
                            candidate.relative_path
                        ));
                    }
                }
                Err(e) => {
                    warn!(
                        "Storage cleanup failed for '{}': {}",
                        candidate.relative_path, e
                    );
                    errors.push(format!(
                        "Failed to delete '{}': {}",
                        candidate.relative_path, e
                    ));
                }
            }
        }
    }

    Ok(StorageCleanupReport {
        dry_run,
        freed_bytes: freed,
        freed_human_size: format_bytes(freed),
        deleted_paths: deleted,
        errors,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_storage_candidate_whitelist() {
        let valid_targets = [
            "target",
            "node_modules",
            "dist",
            "build",
            ".gradle",
            ".cache",
            "__pycache__",
        ];
        for vt in valid_targets {
            assert!(
                APPROVED_CLEANUP_WHITELIST.iter().any(|(p, _, _)| *p == vt),
                "Expected '{}' to be in cleanup whitelist",
                vt
            );
        }

        let dangerous_targets = [
            "src",
            "lib",
            "public",
            ".git",
            "Cargo.toml",
            "package.json",
            "../../etc",
        ];
        for dt in dangerous_targets {
            assert!(
                !APPROVED_CLEANUP_WHITELIST.iter().any(|(p, _, _)| *p == dt),
                "Dangerous path '{}' must NOT be in cleanup whitelist",
                dt
            );
        }
    }

    #[test]
    fn test_format_bytes() {
        assert_eq!(format_bytes(500), "500 B");
        assert_eq!(format_bytes(2048), "2.00 KB");
        assert_eq!(format_bytes(1048576 * 5), "5.00 MB");
        assert_eq!(format_bytes(1073741824 * 3), "3.00 GB");
    }
}
