use application_core::Result;
use filesystem_engine::FilesystemEngine;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceCheckpoint {
    pub checkpoint_id: String,
    pub description: String,
    pub created_at: String,
    pub file_hashes: HashMap<String, String>, // path -> sha256 or content
}

pub struct CheckpointEngine {
    storage_dir: PathBuf,
}

impl CheckpointEngine {
    pub fn new<P: AsRef<Path>>(storage_dir: P) -> Self {
        let p = storage_dir.as_ref().to_path_buf();
        let _ = std::fs::create_dir_all(&p);
        Self { storage_dir: p }
    }

    pub fn create_checkpoint(
        &self,
        description: &str,
        files: &[PathBuf],
    ) -> Result<WorkspaceCheckpoint> {
        let checkpoint_id = format!("ckpt-{}", uuid::Uuid::new_v4().simple());
        let mut file_hashes = HashMap::new();

        let snapshot_dir = self.storage_dir.join(&checkpoint_id);
        std::fs::create_dir_all(&snapshot_dir)?;

        for file in files {
            if let Ok(content) = FilesystemEngine::read_file_string(file) {
                let file_str = file.to_string_lossy().to_string();
                file_hashes.insert(file_str, content);
            }
        }

        let checkpoint = WorkspaceCheckpoint {
            checkpoint_id: checkpoint_id.clone(),
            description: description.to_string(),
            created_at: chrono::Utc::now().to_rfc3339(),
            file_hashes,
        };

        let meta_file = snapshot_dir.join("metadata.json");
        let json = serde_json::to_string_pretty(&checkpoint)?;
        FilesystemEngine::write_file_atomic(&meta_file, &json)?;

        Ok(checkpoint)
    }

    pub fn restore_checkpoint(&self, checkpoint: &WorkspaceCheckpoint) -> Result<()> {
        for (path_str, content) in &checkpoint.file_hashes {
            let path = PathBuf::from(path_str);
            FilesystemEngine::write_file_atomic(&path, content)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_checkpoint_create_and_restore() {
        let temp_dir = std::env::temp_dir().join(format!("aether_ckpt_test_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        let test_file = temp_dir.join("hello.txt");

        FilesystemEngine::write_file_atomic(&test_file, "Original text").unwrap();
        let engine = CheckpointEngine::new(&storage_dir);

        let ckpt = engine.create_checkpoint("Initial state", &[test_file.clone()]).unwrap();

        // Mutate file
        FilesystemEngine::write_file_atomic(&test_file, "Modified text").unwrap();
        assert_eq!(FilesystemEngine::read_file_string(&test_file).unwrap(), "Modified text");

        // Restore checkpoint
        engine.restore_checkpoint(&ckpt).unwrap();
        assert_eq!(FilesystemEngine::read_file_string(&test_file).unwrap(), "Original text");

        let _ = std::fs::remove_dir_all(temp_dir);
    }
}
