use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

pub const BUDGET_TARGET_BYTES: u64 = 6 * 1024 * 1024 * 1024; // 6 GB
pub const WARNING_THRESHOLD_BYTES: u64 = 8 * 1024 * 1024 * 1024; // 8 GB

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageAuditReport {
    pub in_tree_target_bytes: u64,
    pub in_tree_target_path: String,
    pub external_cache_bytes: u64,
    pub external_cache_path: String,
    pub node_modules_bytes: u64,
    pub dist_bytes: u64,
    pub total_consumed_bytes: u64,
    pub reclaimable_bytes: u64,
    pub budget_target_bytes: u64,
    pub warning_threshold_bytes: u64,
    pub is_over_budget: bool,
    pub is_warning: bool,
    pub active_cache_dir: String,
    pub recommendations: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageCleanupPlan {
    pub items_to_remove: Vec<String>,
    pub total_bytes_to_reclaim: u64,
    pub is_dry_run: bool,
    pub executed: bool,
}

pub struct StorageAuditor;

impl StorageAuditor {
    pub fn get_external_cache_base() -> PathBuf {
        if let Ok(appdata) = std::env::var("LOCALAPPDATA") {
            PathBuf::from(appdata).join("AetherIDE").join("build-cache")
        } else if let Ok(home) = std::env::var("HOME") {
            PathBuf::from(home)
                .join(".cache")
                .join("aetheride")
                .join("build-cache")
        } else {
            PathBuf::from(".").join(".build-cache")
        }
    }

    pub fn get_active_cargo_target_dir() -> PathBuf {
        if let Ok(target) = std::env::var("CARGO_TARGET_DIR") {
            PathBuf::from(target)
        } else {
            #[cfg(windows)]
            {
                Self::get_external_cache_base()
                    .join("aetheride")
                    .join("windows-x64")
            }
            #[cfg(target_os = "linux")]
            {
                Self::get_external_cache_base()
                    .join("aetheride")
                    .join("linux-x64")
            }
            #[cfg(target_os = "macos")]
            {
                Self::get_external_cache_base()
                    .join("aetheride")
                    .join("macos-arm64")
            }
            #[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
            {
                Self::get_external_cache_base()
                    .join("aetheride")
                    .join("default")
            }
        }
    }

    pub fn audit(workspace_root: &Path) -> StorageAuditReport {
        let in_tree_target = workspace_root.join("rust").join("target");
        let in_tree_bytes = Self::dir_size(&in_tree_target);

        let ext_cache = Self::get_external_cache_base();
        let ext_bytes = Self::dir_size(&ext_cache);

        let node_modules = workspace_root
            .join("apps")
            .join("desktop")
            .join("node_modules");
        let node_bytes = Self::dir_size(&node_modules);

        let dist = workspace_root.join("dist");
        let dist_bytes = Self::dir_size(&dist);

        let total_consumed = in_tree_bytes + ext_bytes + node_bytes + dist_bytes;
        let reclaimable = in_tree_bytes; // In-tree target can be moved/reclaimed

        let is_warning = total_consumed > WARNING_THRESHOLD_BYTES;
        let is_over_budget = total_consumed > BUDGET_TARGET_BYTES;

        let mut recommendations = Vec::new();
        if in_tree_bytes > 100 * 1024 * 1024 {
            recommendations.push(format!(
                "In-tree target directory consumes {:.2} GB inside OneDrive. Clean and redirect to external build-cache.",
                in_tree_bytes as f64 / (1024.0 * 1024.0 * 1024.0)
            ));
        }
        if is_warning {
            recommendations.push(format!(
                "Total storage ({:.2} GB) exceeds 8 GB warning threshold. Run 'aether storage clean --apply'.",
                total_consumed as f64 / (1024.0 * 1024.0 * 1024.0)
            ));
        } else if is_over_budget {
            recommendations.push(format!(
                "Total storage ({:.2} GB) exceeds 6 GB budget target.",
                total_consumed as f64 / (1024.0 * 1024.0 * 1024.0)
            ));
        } else {
            recommendations
                .push("Storage footprint is within optimal operating limits (< 6 GB).".to_string());
        }

        StorageAuditReport {
            in_tree_target_bytes: in_tree_bytes,
            in_tree_target_path: in_tree_target.to_string_lossy().to_string(),
            external_cache_bytes: ext_bytes,
            external_cache_path: ext_cache.to_string_lossy().to_string(),
            node_modules_bytes: node_bytes,
            dist_bytes,
            total_consumed_bytes: total_consumed,
            reclaimable_bytes: reclaimable,
            budget_target_bytes: BUDGET_TARGET_BYTES,
            warning_threshold_bytes: WARNING_THRESHOLD_BYTES,
            is_over_budget,
            is_warning,
            active_cache_dir: Self::get_active_cargo_target_dir()
                .to_string_lossy()
                .to_string(),
            recommendations,
        }
    }

    pub fn plan_cleanup(workspace_root: &Path) -> StorageCleanupPlan {
        let mut items = Vec::new();
        let mut total_bytes = 0u64;

        // 1. In-tree target directory
        let in_tree_target = workspace_root.join("rust").join("target");
        if in_tree_target.exists() {
            let size = Self::dir_size(&in_tree_target);
            if size > 0 {
                items.push(in_tree_target.to_string_lossy().to_string());
                total_bytes += size;
            }
        }

        // 2. Stale release tmp staging
        let tmp_dist = workspace_root.join("dist").join("tmp");
        if tmp_dist.exists() {
            let size = Self::dir_size(&tmp_dist);
            items.push(tmp_dist.to_string_lossy().to_string());
            total_bytes += size;
        }

        StorageCleanupPlan {
            items_to_remove: items,
            total_bytes_to_reclaim: total_bytes,
            is_dry_run: true,
            executed: false,
        }
    }

    pub fn execute_cleanup(plan: &mut StorageCleanupPlan) -> std::io::Result<u64> {
        let mut deleted_bytes = 0u64;
        for item in &plan.items_to_remove {
            let path = Path::new(item);
            if path.exists() {
                // Safety invariant: NEVER delete source directories or .git
                if item.contains(".git")
                    || item.ends_with("crates")
                    || item.ends_with("src")
                    || item.ends_with("cli")
                    || item.ends_with("services")
                {
                    continue;
                }
                let size = Self::dir_size(path);
                if path.is_dir() {
                    fs::remove_dir_all(path)?;
                } else {
                    fs::remove_file(path)?;
                }
                deleted_bytes += size;
            }
        }
        plan.executed = true;
        plan.is_dry_run = false;
        Ok(deleted_bytes)
    }

    pub fn dir_size(path: &Path) -> u64 {
        if !path.exists() {
            return 0;
        }
        if path.is_file() {
            return fs::metadata(path).map(|m| m.len()).unwrap_or(0);
        }
        let mut total = 0;
        if let Ok(entries) = fs::read_dir(path) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.is_dir() {
                    total += Self::dir_size(&p);
                } else if let Ok(meta) = p.metadata() {
                    total += meta.len();
                }
            }
        }
        total
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_storage_auditor_baseline() {
        let root = Path::new(".");
        let report = StorageAuditor::audit(root);
        assert_eq!(report.budget_target_bytes, BUDGET_TARGET_BYTES);
        assert_eq!(report.warning_threshold_bytes, WARNING_THRESHOLD_BYTES);
        assert!(!report.active_cache_dir.is_empty());
    }

    #[test]
    fn test_storage_cleanup_plan_dry_run_preserves_files() {
        let root = Path::new(".");
        let plan = StorageAuditor::plan_cleanup(root);
        assert!(plan.is_dry_run);
        assert!(!plan.executed);
    }
}
