use crate::workspace_bridge::SharedWorkspace;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager};


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageItem {
    pub name: String,
    pub path: String,
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

fn format_bytes(bytes: u64) -> String {
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

fn dir_size_recursive(path: &Path) -> (u64, usize) {
    let mut total_size = 0u64;
    let mut total_files = 0usize;

    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
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
    path: Option<String>,
) -> Result<StorageAnalysisReport, String> {
    let root = if let Some(p) = path {
        PathBuf::from(p)
    } else {
        let ws_arc = app.state::<SharedWorkspace>();
        let ws = ws_arc.lock();
        ws.get_roots()
            .into_iter()
            .next()
            .unwrap_or_else(|| PathBuf::from("."))
    };

    let candidate_dirs = [
        ("target", "Rust Build Artifacts", "Build Artifacts", true),
        ("node_modules", "Node Dependencies", "Dependencies", true),
        ("dist", "Build Output", "Build Artifacts", true),
        ("build", "Gradle / Compiler Output", "Build Artifacts", true),
        (".gradle", "Gradle Cache", "Caches", true),
        (".turbo", "Turborepo Cache", "Caches", true),
        (".cache", "Local Build Cache", "Caches", true),
        (".pytest_cache", "Pytest Cache", "Caches", true),
        ("__pycache__", "Python Bytecode", "Caches", true),
        (".next", "Next.js Build Cache", "Build Artifacts", true),
        (".aether/worktrees", "Agent Worktrees", "Agent Runtime", true),
        (".git", "Git Version Database", "Version Control", false),
    ];

    let mut items = Vec::new();
    let mut total_size = 0u64;
    let mut cleanable_size = 0u64;

    for (rel_path, label, category, cleanable) in candidate_dirs {
        let full_path = root.join(rel_path);
        if full_path.exists() {
            let (size, count) = dir_size_recursive(&full_path);
            total_size += size;
            if cleanable {
                cleanable_size += size;
            }

            items.push(StorageItem {
                name: label.to_string(),
                path: full_path.to_string_lossy().to_string(),
                category: category.to_string(),
                size_bytes: size,
                human_size: format_bytes(size),
                item_count: count,
                is_cleanable: cleanable,
            });
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
    _app: AppHandle,
    targets: Vec<String>,
    dry_run: bool,
) -> Result<StorageCleanupReport, String> {
    let mut freed = 0u64;
    let mut deleted = Vec::new();
    let mut errors = Vec::new();

    for target in targets {
        let p = PathBuf::from(&target);
        if !p.exists() {
            continue;
        }

        let (size, _) = dir_size_recursive(&p);
        freed += size;

        if !dry_run {
            if let Err(e) = std::fs::remove_dir_all(&p) {
                errors.push(format!("Failed to delete {}: {}", target, e));
            } else {
                deleted.push(target);
            }
        } else {
            deleted.push(format!("[DRY RUN] Would delete {}", target));
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
