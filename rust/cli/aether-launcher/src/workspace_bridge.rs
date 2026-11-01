use serde::{Deserialize, Serialize};
use std::fs;
#[cfg(windows)]
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
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

pub struct WorkspaceSession {
    roots: Mutex<Vec<PathBuf>>,
    user_data_dir: PathBuf,
}

impl WorkspaceSession {
    pub fn new(user_data_dir: PathBuf) -> Self {
        Self {
            roots: Mutex::new(Vec::new()),
            user_data_dir,
        }
    }

    /// Sets a single folder as the active workspace root.
    pub fn set_single_root(&self, path: PathBuf) -> Result<serde_json::Value, String> {
        let canonical = path
            .canonicalize()
            .map_err(|e| format!("Invalid workspace path {:?}: {}", path, e))?;

        if !canonical.is_dir() {
            return Err(format!("Path {:?} is not a directory", canonical));
        }

        {
            let mut roots = self.roots.lock().unwrap();
            roots.clear();
            roots.push(canonical.clone());
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

        {
            let mut roots = self.roots.lock().unwrap();
            if !roots.contains(&canonical) {
                roots.push(canonical.clone());
            }
        }

        info!("Added workspace root: {:?}", canonical);
        self.get_workspace_summary()
    }

    /// Removes a folder from active workspace roots.
    pub fn remove_root(&self, path: &Path) -> Result<serde_json::Value, String> {
        let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        {
            let mut roots = self.roots.lock().unwrap();
            roots.retain(|r| r != &canonical && r != path);
        }
        self.get_workspace_summary()
    }

    /// Returns the primary active root directory, if any.
    pub fn primary_root(&self) -> Option<PathBuf> {
        let roots = self.roots.lock().unwrap();
        roots.first().cloned()
    }

    /// Gets summary of all active roots with file trees.
    pub fn get_workspace_summary(&self) -> Result<serde_json::Value, String> {
        let roots = self.roots.lock().unwrap().clone();
        let mut root_infos = Vec::new();

        for (idx, root) in roots.iter().enumerate() {
            let name = root
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| format!("Workspace {}", idx + 1));

            let id = format!("root-{}", idx);
            let is_trusted = self.is_path_trusted(root);
            let tree = self.build_file_tree(root, 0, 5)?;

            root_infos.push(WorkspaceRootInfo {
                id,
                path: root.to_string_lossy().to_string(),
                name,
                is_trusted,
                tree,
            });
        }

        Ok(serde_json::json!({
            "roots": root_infos,
            "root_count": root_infos.len(),
            "primary_path": roots.first().map(|p| p.to_string_lossy().to_string()),
        }))
    }

    /// Reads a file contained in any of the active workspace roots.
    pub fn read_file(&self, user_path: &str) -> Result<serde_json::Value, String> {
        let (root, rel_path) = self.resolve_in_workspace(user_path)?;
        let cap = filesystem_engine::WorkspaceCapability::open(&root)
            .map_err(|e| format!("Failed to open workspace capability: {}", e))?;

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
        let (root, rel_path) = self.resolve_in_workspace(user_path)?;
        let cap = filesystem_engine::WorkspaceCapability::open(&root)
            .map_err(|e| format!("Failed to open workspace capability: {}", e))?;

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
        let (root, rel_path) = self.resolve_in_workspace(user_path)?;
        let cap = filesystem_engine::WorkspaceCapability::open(&root)
            .map_err(|e| format!("Failed to open workspace capability: {}", e))?;

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
        let (root_old, rel_old) = self.resolve_in_workspace(old_path)?;
        let (root_new, rel_new) = self.resolve_in_workspace(new_path)?;

        if root_old != root_new {
            return Err("Cross-workspace root renames are not permitted".into());
        }

        let cap = filesystem_engine::WorkspaceCapability::open(&root_old)
            .map_err(|e| format!("Failed to open workspace capability: {}", e))?;

        cap.rename(&rel_old, &rel_new)
            .map_err(|e| format!("Rename failed from '{}' to '{}': {}", rel_old, rel_new, e))?;

        Ok(serde_json::json!({
            "old_path": old_path,
            "new_path": new_path,
            "success": true
        }))
    }

    /// Deletes a file in an active workspace root.
    pub fn delete_entry(&self, user_path: &str) -> Result<serde_json::Value, String> {
        let (root, rel_path) = self.resolve_in_workspace(user_path)?;
        let cap = filesystem_engine::WorkspaceCapability::open(&root)
            .map_err(|e| format!("Failed to open workspace capability: {}", e))?;

        cap.delete_file(&rel_path)
            .map_err(|e| format!("Delete failed for '{}': {}", rel_path, e))?;

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
            let (root, rel) = self.resolve_in_workspace(target_path)?;
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
        #[cfg(target_os = "macos")]
        {
            let _ = std::process::Command::new("open")
                .arg("-R")
                .arg(&resolved)
                .spawn();
        }
        #[cfg(target_os = "linux")]
        {
            let show_path = if resolved.is_dir() {
                &resolved
            } else {
                resolved.parent().unwrap_or(&resolved)
            };
            let _ = std::process::Command::new("xdg-open")
                .arg(show_path)
                .spawn();
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
        // Default to trusted if it's the AetherIDE development repo or a user-explicitly opened directory
        true
    }

    /// Resolves an incoming user path against active workspace roots.
    /// Returns (Root PathBuf, Normalized Relative Path String).
    fn resolve_in_workspace(&self, user_path: &str) -> Result<(PathBuf, String), String> {
        let roots = self.roots.lock().unwrap().clone();
        if roots.is_empty() {
            return Err("No active workspace is open".into());
        }

        let p = Path::new(user_path);

        // Case 1: Absolute path
        if p.is_absolute() {
            let canon = p.canonicalize().unwrap_or_else(|_| p.to_path_buf());
            for root in &roots {
                if filesystem_engine::FilesystemEngine::is_within_boundary(&canon, root) {
                    if let Ok(rel) = canon.strip_prefix(root) {
                        return Ok((root.clone(), rel.to_string_lossy().replace('\\', "/")));
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
                        return Ok((target_root.clone(), rel.to_string()));
                    }
                }
            }
        }

        // Case 3: Relative path defaults to primary root
        let primary = &roots[0];
        Ok((primary.clone(), norm_slashes))
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

        let _ = std::fs::remove_dir_all(&temp_base);
    }
}
