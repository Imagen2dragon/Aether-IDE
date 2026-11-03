use filesystem_engine::WorkspaceCapability;
use serde::{Deserialize, Serialize};
use std::fs;
#[cfg(windows)]
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tracing::info;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceRootInfo {
    pub id: String,
    pub path: String,
    pub name: String,
    pub is_trusted: bool,
    pub tree: Vec<FileNode>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileNode {
    pub name: String,
    pub path: String,      // workspace-relative path with forward slashes
    pub full_path: String, // canonical absolute path
    pub is_dir: bool,
    pub size: u64,
    pub children: Option<Vec<FileNode>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceConfig {
    pub version: u32,
    pub folders: Vec<WorkspaceFolderConfig>,
    #[serde(default)]
    pub settings: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceFolderConfig {
    pub path: String,
    pub name: Option<String>,
}

pub struct AuthorizedWorkspaceRoot {
    pub workspace_id: String,
    pub root_id: String,
    pub canonical_path: PathBuf,
    pub name: String,
    pub is_trusted: bool,
    pub capability: Arc<WorkspaceCapability>,
}

pub type SharedWorkspace = std::sync::Arc<parking_lot::Mutex<WorkspaceSession>>;

pub struct WorkspaceSession {
    roots: Mutex<Vec<AuthorizedWorkspaceRoot>>,
    user_data_dir: PathBuf,
    active_workspace_id: Mutex<String>,
}

impl WorkspaceSession {
    pub fn new(user_data_dir: PathBuf) -> Self {
        let ws_id = format!("ws-{}", uuid::Uuid::new_v4().simple());
        Self {
            roots: Mutex::new(Vec::new()),
            user_data_dir,
            active_workspace_id: Mutex::new(ws_id),
        }
    }

    pub fn workspace_id(&self) -> String {
        self.active_workspace_id.lock().unwrap().clone()
    }

    /// Sets a single folder as the active workspace root.
    pub fn set_single_root(&self, path: PathBuf) -> Result<serde_json::Value, String> {
        let canonical = path
            .canonicalize()
            .map_err(|e| format!("Invalid workspace path {:?}: {}", path, e))?;

        if !canonical.is_dir() {
            return Err(format!("Path {:?} is not a directory", canonical));
        }

        let cap = WorkspaceCapability::open(&canonical)
            .map_err(|e| format!("Failed to initialize capability for {:?}: {}", canonical, e))?;

        let ws_id = format!("ws-{}", uuid::Uuid::new_v4().simple());
        *self.active_workspace_id.lock().unwrap() = ws_id.clone();

        let name = canonical
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "Workspace 1".to_string());

        let is_trusted = self.is_path_trusted(&canonical);

        let root_entry = AuthorizedWorkspaceRoot {
            workspace_id: ws_id,
            root_id: "root-0".to_string(),
            canonical_path: canonical.clone(),
            name,
            is_trusted,
            capability: Arc::new(cap),
        };

        {
            let mut roots = self.roots.lock().unwrap();
            roots.clear();
            roots.push(root_entry);
        }

        info!("Active workspace set to: {:?}", canonical);
        self.get_workspace_summary()
    }

    /// Adds a folder to active workspace (multi-root).
    pub fn add_root(&self, path: PathBuf) -> Result<serde_json::Value, String> {
        let canonical = path
            .canonicalize()
            .map_err(|e| format!("Invalid workspace path {:?}: {}", path, e))?;

        if !canonical.is_dir() {
            return Err(format!("Path {:?} is not a directory", canonical));
        }

        let ws_id = self.workspace_id();
        let cap = WorkspaceCapability::open(&canonical)
            .map_err(|e| format!("Failed to initialize capability for {:?}: {}", canonical, e))?;

        let mut roots = self.roots.lock().unwrap();
        if roots.iter().any(|r| r.canonical_path == canonical) {
            drop(roots);
            return self.get_workspace_summary();
        }

        let idx = roots.len();
        let root_id = format!("root-{}", idx);
        let name = canonical
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| format!("Workspace {}", idx + 1));
        let is_trusted = self.is_path_trusted(&canonical);

        roots.push(AuthorizedWorkspaceRoot {
            workspace_id: ws_id,
            root_id,
            canonical_path: canonical.clone(),
            name,
            is_trusted,
            capability: Arc::new(cap),
        });

        drop(roots);
        info!("Added workspace root: {:?}", canonical);
        self.get_workspace_summary()
    }

    /// Removes a folder from active workspace roots.
    pub fn remove_root(&self, path_or_id: &Path) -> Result<serde_json::Value, String> {
        let id_str = path_or_id.to_string_lossy();
        let canonical = path_or_id
            .canonicalize()
            .unwrap_or_else(|_| path_or_id.to_path_buf());

        {
            let mut roots = self.roots.lock().unwrap();
            roots.retain(|r| {
                r.root_id != id_str
                    && r.canonical_path != canonical
                    && r.canonical_path != path_or_id
            });

            // Re-index root IDs
            for (idx, r) in roots.iter_mut().enumerate() {
                r.root_id = format!("root-{}", idx);
            }
        }

        self.get_workspace_summary()
    }

    /// Completely revokes active workspace and drops all capabilities.
    pub fn revoke_workspace(&self) -> Result<serde_json::Value, String> {
        {
            let mut roots = self.roots.lock().unwrap();
            roots.clear();
        }
        *self.active_workspace_id.lock().unwrap() = format!("ws-{}", uuid::Uuid::new_v4().simple());
        info!("Active workspace revoked and all capabilities closed");
        self.get_workspace_summary()
    }

    /// Returns the primary active root directory, if any.
    pub fn primary_root(&self) -> Option<PathBuf> {
        let roots = self.roots.lock().unwrap();
        roots.first().map(|r| r.canonical_path.clone())
    }

    /// Returns list of canonical root paths.
    pub fn get_roots(&self) -> Vec<PathBuf> {
        let roots = self.roots.lock().unwrap();
        roots.iter().map(|r| r.canonical_path.clone()).collect()
    }

    /// Validates that a requested working directory is strictly within an authorized and trusted workspace root.
    pub fn validate_process_directory(&self, dir: &Path) -> Result<PathBuf, String> {
        let canonical = dir
            .canonicalize()
            .map_err(|e| format!("Invalid process working directory {:?}: {}", dir, e))?;

        let roots = self.roots.lock().unwrap();
        for r in roots.iter() {
            if filesystem_engine::FilesystemEngine::is_within_boundary(
                &canonical,
                &r.canonical_path,
            ) {
                if !r.is_trusted {
                    return Err(format!(
                        "Security Denial: Directory {:?} is inside untrusted workspace root '{}'. Process execution denied.",
                        canonical, r.name
                    ));
                }
                return Ok(canonical);
            }
        }

        Err(format!(
            "Security Denial: Directory {:?} is outside all authorized workspace roots.",
            canonical
        ))
    }

    /// Resolves an incoming user path against active workspace roots.
    /// Returns (Capability Arc, Canonical Root PathBuf, Relative Path String).
    pub fn resolve_in_workspace(
        &self,
        user_path: &str,
    ) -> Result<(Arc<WorkspaceCapability>, PathBuf, String), String> {
        let roots = self.roots.lock().unwrap();
        if roots.is_empty() {
            return Err("No active workspace is open".into());
        }

        let p = Path::new(user_path);

        // Case 1: Absolute path
        if p.is_absolute() {
            let canon = p.canonicalize().unwrap_or_else(|_| p.to_path_buf());
            for root in roots.iter() {
                if filesystem_engine::FilesystemEngine::is_within_boundary(
                    &canon,
                    &root.canonical_path,
                ) {
                    if let Ok(rel) = canon.strip_prefix(&root.canonical_path) {
                        let rel_str = rel.to_string_lossy().replace('\\', "/");
                        return Ok((
                            root.capability.clone(),
                            root.canonical_path.clone(),
                            rel_str,
                        ));
                    }
                }
            }
            return Err(format!(
                "Security Denial: Absolute path '{:?}' is outside active workspace roots",
                p
            ));
        }

        // Case 2: Relative path with explicit root-ID prefix (e.g. "root-0/src/main.rs")
        let norm_slashes = user_path.replace('\\', "/");
        if let Some(slash_idx) = norm_slashes.find('/') {
            let prefix = &norm_slashes[..slash_idx];
            if let Some(rest) = prefix.strip_prefix("root-") {
                if let Ok(idx) = rest.parse::<usize>() {
                    if let Some(target_root) = roots.get(idx) {
                        let rel = &norm_slashes[slash_idx + 1..];
                        return Ok((
                            target_root.capability.clone(),
                            target_root.canonical_path.clone(),
                            rel.to_string(),
                        ));
                    }
                }
            }
        }

        // Case 3: Relative path defaults to primary root
        let primary = &roots[0];
        Ok((
            primary.capability.clone(),
            primary.canonical_path.clone(),
            norm_slashes,
        ))
    }

    /// Gets summary of all active roots with file trees.
    pub fn get_workspace_summary(&self) -> Result<serde_json::Value, String> {
        let roots = self.roots.lock().unwrap();
        let mut root_infos = Vec::new();

        for root in roots.iter() {
            let tree = self.build_file_tree(&root.canonical_path, 0, 5)?;
            root_infos.push(WorkspaceRootInfo {
                id: root.root_id.clone(),
                path: root.canonical_path.to_string_lossy().to_string(),
                name: root.name.clone(),
                is_trusted: root.is_trusted,
                tree,
            });
        }

        let primary = roots
            .first()
            .map(|r| r.canonical_path.to_string_lossy().to_string());
        let count = root_infos.len();
        drop(roots);

        Ok(serde_json::json!({
            "workspace_id": self.workspace_id(),
            "roots": root_infos,
            "root_count": count,
            "primary_path": primary,
        }))
    }

    /// Reads a file contained in any of the active workspace roots.
    pub fn read_file(&self, user_path: &str) -> Result<serde_json::Value, String> {
        let (cap, root, rel_path) = self.resolve_in_workspace(user_path)?;
        let content = cap
            .read_file(&rel_path)
            .map_err(|e| format!("Read failed for '{}': {}", rel_path, e))?;

        Ok(serde_json::json!({
            "path": user_path,
            "relative_path": rel_path,
            "root": root.to_string_lossy().to_string(),
            "content": content
        }))
    }

    /// Atomically writes content to a file in an active workspace root.
    pub fn write_file(&self, user_path: &str, content: &str) -> Result<serde_json::Value, String> {
        let (cap, _, rel_path) = self.resolve_in_workspace(user_path)?;
        let saved_path = cap
            .write_file_atomic(&rel_path, content)
            .map_err(|e| format!("Atomic write failed for '{}': {}", rel_path, e))?;

        Ok(serde_json::json!({
            "path": user_path,
            "relative_path": rel_path,
            "saved_path": saved_path.to_string_lossy().to_string(),
            "success": true
        }))
    }

    /// Creates a new empty file in an active workspace root.
    pub fn create_file(&self, user_path: &str) -> Result<serde_json::Value, String> {
        self.write_file(user_path, "")
    }

    /// Creates a directory in an active workspace root.
    pub fn create_directory(&self, user_path: &str) -> Result<serde_json::Value, String> {
        let (cap, _, rel_path) = self.resolve_in_workspace(user_path)?;
        let dir_path = cap
            .create_dir(&rel_path)
            .map_err(|e| format!("Create directory failed for '{}': {}", rel_path, e))?;

        Ok(serde_json::json!({
            "path": user_path,
            "created_path": dir_path.to_string_lossy().to_string(),
            "success": true
        }))
    }

    /// Renames a file or directory in an active workspace root.
    pub fn rename_entry(
        &self,
        old_path: &str,
        new_path: &str,
    ) -> Result<serde_json::Value, String> {
        let (cap_old, root_old, rel_old) = self.resolve_in_workspace(old_path)?;
        let (_, root_new, rel_new) = self.resolve_in_workspace(new_path)?;

        if root_old != root_new {
            return Err("Cross-workspace root renames are not permitted".into());
        }

        cap_old
            .rename(&rel_old, &rel_new)
            .map_err(|e| format!("Rename failed from '{}' to '{}': {}", rel_old, rel_new, e))?;

        Ok(serde_json::json!({
            "old_path": old_path,
            "new_path": new_path,
            "success": true
        }))
    }

    /// Deletes a file in an active workspace root.
    pub fn delete_entry(&self, user_path: &str) -> Result<serde_json::Value, String> {
        let (cap, _, rel_path) = self.resolve_in_workspace(user_path)?;
        cap.delete_file(&rel_path)
            .map_err(|e| format!("Delete failed for '{}': {}", rel_path, e))?;

        Ok(serde_json::json!({
            "path": user_path,
            "success": true
        }))
    }

    /// Deletes a directory recursively in an active workspace root.
    pub fn delete_directory(&self, user_path: &str) -> Result<serde_json::Value, String> {
        let (cap, _, rel_path) = self.resolve_in_workspace(user_path)?;
        cap.delete_dir_all(&rel_path)
            .map_err(|e| format!("Delete directory failed for '{}': {}", rel_path, e))?;

        Ok(serde_json::json!({
            "path": user_path,
            "success": true
        }))
    }

    /// Reveals a file or folder in Windows File Explorer.
    pub fn reveal_in_explorer(&self, target_path: &str) -> Result<serde_json::Value, String> {
        let path = Path::new(target_path);
        let resolved = if path.is_absolute() {
            path.to_path_buf()
        } else {
            let (_, root, rel) = self.resolve_in_workspace(target_path)?;
            root.join(rel)
        };

        if !resolved.exists() {
            return Err(format!("Target path does not exist: {:?}", resolved));
        }

        #[cfg(windows)]
        {
            let arg = format!("/select,\"{}\"", resolved.to_string_lossy());
            let _ = std::process::Command::new("explorer").raw_arg(&arg).spawn();
        }

        Ok(serde_json::json!({
            "path": target_path,
            "resolved": resolved.to_string_lossy().to_string(),
            "revealed": true
        }))
    }

    /// Sets trust state for a workspace root.
    pub fn set_trust(
        &self,
        root_path: &str,
        is_trusted: bool,
    ) -> Result<serde_json::Value, String> {
        let trust_file = self.user_data_dir.join("trusted_workspaces.json");
        let mut map: serde_json::Map<String, serde_json::Value> =
            if let Ok(data) = fs::read_to_string(&trust_file) {
                serde_json::from_str(&data).unwrap_or_default()
            } else {
                serde_json::Map::new()
            };

        let norm_path = Path::new(root_path)
            .canonicalize()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| root_path.to_string());

        map.insert(
            norm_path.clone(),
            serde_json::json!({
                "trusted": is_trusted,
                "updated_at": chrono::Utc::now().to_rfc3339()
            }),
        );

        if let Some(parent) = trust_file.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let _ = fs::write(
            &trust_file,
            serde_json::to_string_pretty(&map).unwrap_or_default(),
        );

        // Update in-memory state
        {
            let mut roots = self.roots.lock().unwrap();
            for r in roots.iter_mut() {
                if r.canonical_path.to_string_lossy() == norm_path {
                    r.is_trusted = is_trusted;
                }
            }
        }

        Ok(serde_json::json!({
            "root": norm_path,
            "trusted": is_trusted
        }))
    }

    /// Checks if a root path is marked trusted.
    pub fn is_path_trusted(&self, root: &Path) -> bool {
        let trust_file = self.user_data_dir.join("trusted_workspaces.json");
        if let Ok(data) = fs::read_to_string(&trust_file) {
            if let Ok(map) =
                serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(&data)
            {
                let canon = root
                    .canonicalize()
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_else(|_| root.to_string_lossy().to_string());

                if let Some(val) = map.get(&canon) {
                    return val
                        .get("trusted")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);
                }
            }
        }
        // Default to trusted for user explicitly opened directories unless revoked
        true
    }

    /// Builds hierarchical file tree up to max_depth.
    fn build_file_tree(
        &self,
        dir: &Path,
        depth: usize,
        max_depth: usize,
    ) -> Result<Vec<FileNode>, String> {
        if depth > max_depth {
            return Ok(Vec::new());
        }

        let entries = match fs::read_dir(dir) {
            Ok(e) => e,
            Err(_) => return Ok(Vec::new()),
        };

        let mut nodes = Vec::new();
        let excluded_names = [
            ".git",
            "target",
            "node_modules",
            "dist",
            "build",
            ".idea",
            ".vscode",
            ".system_generated",
            "target_atomic",
            "tmp_atomic",
        ];

        let mut dir_entries = Vec::new();
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if excluded_names.contains(&name.as_str())
                || (name.starts_with('.') && name != ".env.example" && name != ".gitignore")
            {
                continue;
            }
            dir_entries.push(entry);
        }

        // Sort: directories first, then files alphabetically
        dir_entries.sort_by_key(|e| {
            let is_d = e.file_type().map(|t| t.is_dir()).unwrap_or(false);
            (!is_d, e.file_name().to_string_lossy().to_ascii_lowercase())
        });

        for entry in dir_entries {
            let file_name = entry.file_name().to_string_lossy().to_string();
            let entry_path = entry.path();
            let meta = match entry.metadata() {
                Ok(m) => m,
                Err(_) => continue,
            };

            let is_dir = meta.is_dir();
            let size = meta.len();
            let full_path = entry_path.to_string_lossy().to_string();

            let children = if is_dir {
                Some(self.build_file_tree(&entry_path, depth + 1, max_depth)?)
            } else {
                None
            };

            nodes.push(FileNode {
                name: file_name.clone(),
                path: file_name,
                full_path,
                is_dir,
                size,
                children,
            });
        }

        Ok(nodes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_workspace_session_single_root() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_test_ws_{}", uuid::Uuid::new_v4().simple()));
        std::fs::create_dir_all(&temp_dir).unwrap();
        let user_data = temp_dir.join("user_data");

        let session = WorkspaceSession::new(user_data);
        let summary = session
            .set_single_root(temp_dir.clone())
            .expect("Failed to set root");

        assert_eq!(summary.get("root_count").and_then(|v| v.as_u64()), Some(1));
        assert!(session.primary_root().is_some());

        // Create file
        let write_res = session
            .write_file("hello.txt", "AetherIDE Native Desktop")
            .expect("Write failed");
        assert_eq!(
            write_res.get("success").and_then(|v| v.as_bool()),
            Some(true)
        );

        // Read file
        let read_res = session.read_file("hello.txt").expect("Read failed");
        assert_eq!(
            read_res.get("content").and_then(|v| v.as_str()),
            Some("AetherIDE Native Desktop")
        );

        // Rename entry
        let ren_res = session
            .rename_entry("hello.txt", "renamed.txt")
            .expect("Rename failed");
        assert_eq!(ren_res.get("success").and_then(|v| v.as_bool()), Some(true));

        // Verify old file gone, new file exists
        assert!(session.read_file("hello.txt").is_err());
        let read_renamed = session
            .read_file("renamed.txt")
            .expect("Read renamed failed");
        assert_eq!(
            read_renamed.get("content").and_then(|v| v.as_str()),
            Some("AetherIDE Native Desktop")
        );

        // Delete entry
        session.delete_entry("renamed.txt").expect("Delete failed");
        assert!(session.read_file("renamed.txt").is_err());

        // Path traversal rejection
        let trav_err = session.read_file("../../windows/system32/cmd.exe");
        assert!(trav_err.is_err());

        // Validate process directory
        let valid_proc = session.validate_process_directory(&temp_dir);
        assert!(valid_proc.is_ok());

        let invalid_proc = session.validate_process_directory(&std::env::temp_dir());
        assert!(invalid_proc.is_err());

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_workspace_session_multi_root() {
        let temp_base = std::env::temp_dir().join(format!(
            "aether_test_multi_{}",
            uuid::Uuid::new_v4().simple()
        ));
        let root1 = temp_base.join("project-a");
        let root2 = temp_base.join("project-b");
        std::fs::create_dir_all(&root1).unwrap();
        std::fs::create_dir_all(&root2).unwrap();
        let user_data = temp_base.join("user_data");

        let session = WorkspaceSession::new(user_data);
        session.set_single_root(root1.clone()).unwrap();
        session.add_root(root2.clone()).unwrap();

        let summary = session.get_workspace_summary().unwrap();
        assert_eq!(summary.get("root_count").and_then(|v| v.as_u64()), Some(2));

        // Revoke workspace
        session.revoke_workspace().unwrap();
        let summary_after = session.get_workspace_summary().unwrap();
        assert_eq!(
            summary_after.get("root_count").and_then(|v| v.as_u64()),
            Some(0)
        );

        let _ = std::fs::remove_dir_all(&temp_base);
    }
}
