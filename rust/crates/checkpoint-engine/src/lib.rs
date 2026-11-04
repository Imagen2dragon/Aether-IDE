use application_core::{AetherError, Result};
use filesystem_engine::{sanitize_relative_path, FilesystemEngine, WorkspaceCapability};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::info;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckpointFileRecord {
    pub root_id: String,
    pub relative_path: String, // STRICTLY RELATIVE PATH (never absolute)
    pub sha256: String,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceCheckpoint {
    pub checkpoint_id: String,
    pub workspace_id: String,
    pub description: String,
    pub created_at: String,
    pub manifest_version: u32,
    pub roots: Vec<String>,
    pub files: Vec<CheckpointFileRecord>,
    pub file_hashes: HashMap<String, String>, // relative_path -> sha256
}

/// Explicit representation of disk file state captured during preview inspection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FileDiskState {
    Present { sha256: String, size_bytes: u64 },
    Missing,
    Inaccessible { reason: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckpointRestorePreview {
    pub checkpoint_id: String,
    pub preview_token: String,
    pub files_to_create: Vec<String>,
    pub files_to_overwrite: Vec<String>,
    pub conflicts: Vec<String>,
    pub missing_blobs: Vec<String>,
    pub integrity_valid: bool,
    #[serde(default)]
    pub file_states: HashMap<String, FileDiskState>,
    #[serde(default)]
    pub disk_hashes: HashMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct ActivePreviewSession {
    pub preview_token: String,
    pub checkpoint_id: String,
    pub workspace_id: String,
    pub canonical_root: PathBuf,
    pub preview: CheckpointRestorePreview,
    pub created_at: Instant,
    pub expires_at: Instant,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckpointRestoreReport {
    pub checkpoint_id: String,
    pub restored_count: usize,
    pub restored_files: Vec<String>,
    pub errors: Vec<String>,
    pub success: bool,
}

pub struct CheckpointEngine {
    storage_dir: PathBuf,
    active_previews: Arc<Mutex<HashMap<String, ActivePreviewSession>>>,
}

impl CheckpointEngine {
    pub fn new<P: AsRef<Path>>(storage_dir: P) -> Self {
        let p = storage_dir.as_ref().to_path_buf();
        let _ = std::fs::create_dir_all(&p);
        Self {
            storage_dir: p,
            active_previews: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Recursively collects source files from a workspace directory, ignoring generated caches.
    pub fn collect_workspace_files(root: &Path) -> Vec<PathBuf> {
        let mut files = Vec::new();
        let ignored_names = [
            ".git",
            "target",
            "node_modules",
            "dist",
            "build",
            ".aether",
            ".gradle",
            ".turbo",
            ".cache",
            ".pytest_cache",
            "__pycache__",
            ".next",
            "tmp_atomic",
            "target_atomic",
        ];

        let mut queue = vec![root.to_path_buf()];
        while let Some(current_dir) = queue.pop() {
            if let Ok(entries) = std::fs::read_dir(&current_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    let name = entry.file_name().to_string_lossy().to_string();

                    // Skip ignored directory names and hidden files
                    if ignored_names.contains(&name.as_str()) {
                        continue;
                    }

                    if let Ok(ft) = entry.file_type() {
                        if ft.is_symlink() {
                            // Never follow symlinks in checkpoint collection
                            continue;
                        } else if ft.is_dir() {
                            queue.push(path);
                        } else if ft.is_file() {
                            files.push(path);
                        }
                    }
                }
            }
        }

        files.sort();
        files
    }

    /// Computes cryptographic SHA-256 hex string of bytes.
    pub fn sha256_digest(bytes: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(bytes);
        format!("{:x}", hasher.finalize())
    }

    /// Legacy compatibility method for creating a checkpoint from a file slice.
    pub fn create_checkpoint(
        &self,
        description: &str,
        files: &[PathBuf],
    ) -> Result<WorkspaceCheckpoint> {
        let root = if let Some(first) = files.first() {
            first
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .to_path_buf()
        } else {
            PathBuf::from(".")
        };
        self.create_checkpoint_with_root(description, "ws-default", &root, Some(files))
    }

    /// Creates a snapshot with content-addressed blob storage and relative-path manifest.
    pub fn create_checkpoint_with_root(
        &self,
        description: &str,
        workspace_id: &str,
        root_dir: &Path,
        explicit_files: Option<&[PathBuf]>,
    ) -> Result<WorkspaceCheckpoint> {
        let checkpoint_id = format!("ckpt-{}", uuid::Uuid::new_v4().simple());
        let snapshot_dir = self.storage_dir.join(&checkpoint_id);
        let blobs_dir = snapshot_dir.join("blobs");
        std::fs::create_dir_all(&blobs_dir)?;

        let files_to_snapshot = match explicit_files {
            Some(flist) => flist.to_vec(),
            None => Self::collect_workspace_files(root_dir),
        };

        let mut file_records = Vec::new();
        let mut file_hashes = HashMap::new();

        for file_path in &files_to_snapshot {
            // Must be within root_dir
            if !file_path.starts_with(root_dir) {
                continue;
            }

            let rel_path = match file_path.strip_prefix(root_dir) {
                Ok(r) => r.to_string_lossy().replace('\\', "/"),
                Err(_) => continue,
            };

            // Sanitize relative path
            if sanitize_relative_path(&rel_path).is_err() {
                continue;
            }

            // Read file bytes
            if let Ok(bytes) = std::fs::read(file_path) {
                let hash = Self::sha256_digest(&bytes);
                let blob_path = blobs_dir.join(&hash);

                // Content-addressed write: deduplicate if blob already exists
                if !blob_path.exists() {
                    let _ = std::fs::write(&blob_path, &bytes);
                }

                file_records.push(CheckpointFileRecord {
                    root_id: "root-0".into(),
                    relative_path: rel_path.clone(),
                    sha256: hash.clone(),
                    size_bytes: bytes.len() as u64,
                });

                file_hashes.insert(rel_path, hash);
            }
        }

        let checkpoint = WorkspaceCheckpoint {
            checkpoint_id: checkpoint_id.clone(),
            workspace_id: workspace_id.to_string(),
            description: description.to_string(),
            created_at: chrono::Utc::now().to_rfc3339(),
            manifest_version: 2,
            roots: vec![root_dir.to_string_lossy().to_string()],
            files: file_records,
            file_hashes,
        };

        let meta_file = snapshot_dir.join("metadata.json");
        let json = serde_json::to_string_pretty(&checkpoint)?;
        FilesystemEngine::write_file_atomic(&meta_file, &json)?;

        info!(
            "Created workspace checkpoint '{}' with {} files (snapshot: {:?})",
            checkpoint_id,
            checkpoint.files.len(),
            snapshot_dir
        );

        Ok(checkpoint)
    }

    /// Lists all stored checkpoints ordered by creation date descending.
    pub fn list_checkpoints(&self) -> Result<Vec<WorkspaceCheckpoint>> {
        let mut list = Vec::new();
        if !self.storage_dir.exists() {
            return Ok(list);
        }

        for entry in std::fs::read_dir(&self.storage_dir)? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                let meta = entry.path().join("metadata.json");
                if meta.is_file() {
                    if let Ok(content) = FilesystemEngine::read_file_string(&meta) {
                        if let Ok(ckpt) = serde_json::from_str::<WorkspaceCheckpoint>(&content) {
                            list.push(ckpt);
                        }
                    }
                }
            }
        }

        list.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        Ok(list)
    }

    /// Generates a bound restore preview, recording exact file states and storing an active preview session.
    pub fn restore_preview_bound(
        &self,
        checkpoint_id: &str,
        workspace_id: &str,
        root_dir: &Path,
    ) -> Result<CheckpointRestorePreview> {
        let canonical_root = root_dir.canonicalize().map_err(AetherError::Io)?;
        let snapshot_dir = self.storage_dir.join(checkpoint_id);
        let meta_path = snapshot_dir.join("metadata.json");
        if !meta_path.is_file() {
            return Err(AetherError::Internal(format!(
                "Checkpoint metadata for '{}' not found",
                checkpoint_id
            )));
        }

        let meta_content = FilesystemEngine::read_file_string(&meta_path)?;
        let ckpt: WorkspaceCheckpoint = serde_json::from_str(&meta_content)?;
        let blobs_dir = snapshot_dir.join("blobs");

        let mut files_to_create = Vec::new();
        let mut files_to_overwrite = Vec::new();
        let mut conflicts = Vec::new();
        let mut missing_blobs = Vec::new();
        let mut disk_hashes = HashMap::new();
        let mut file_states = HashMap::new();

        for record in &ckpt.files {
            // Verify relative path security
            if sanitize_relative_path(&record.relative_path).is_err() {
                return Err(AetherError::PermissionDenied(format!(
                    "Malicious relative path '{:?}' in checkpoint manifest",
                    record.relative_path
                )));
            }

            // Verify blob presence
            let blob_path = blobs_dir.join(&record.sha256);
            if !blob_path.is_file() {
                missing_blobs.push(record.relative_path.clone());
                continue;
            }

            let dest_path = canonical_root.join(&record.relative_path);

            let state = match std::fs::symlink_metadata(&dest_path) {
                Ok(meta) => {
                    if meta.file_type().is_symlink() {
                        FileDiskState::Inaccessible {
                            reason: "Destination is a symbolic link".into(),
                        }
                    } else if !meta.is_file() {
                        FileDiskState::Inaccessible {
                            reason: "Destination is a directory or non-file object".into(),
                        }
                    } else {
                        match std::fs::read(&dest_path) {
                            Ok(bytes) => {
                                let hash = Self::sha256_digest(&bytes);
                                disk_hashes.insert(record.relative_path.clone(), hash.clone());
                                if hash != record.sha256 {
                                    files_to_overwrite.push(record.relative_path.clone());
                                    conflicts.push(record.relative_path.clone());
                                }
                                FileDiskState::Present {
                                    sha256: hash,
                                    size_bytes: bytes.len() as u64,
                                }
                            }
                            Err(e) => FileDiskState::Inaccessible {
                                reason: format!("Failed to read file: {}", e),
                            },
                        }
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                    files_to_create.push(record.relative_path.clone());
                    FileDiskState::Missing
                }
                Err(e) => FileDiskState::Inaccessible {
                    reason: format!("File inspection error: {}", e),
                },
            };

            file_states.insert(record.relative_path.clone(), state);
        }

        let integrity_valid = missing_blobs.is_empty();

        let token_uuid = uuid::Uuid::new_v4();
        let rand_bytes = token_uuid.as_bytes();
        let preview_token = format!(
            "prev-{}-{}",
            token_uuid.simple(),
            &Self::sha256_digest(rand_bytes)[..16]
        );

        let preview = CheckpointRestorePreview {
            checkpoint_id: checkpoint_id.to_string(),
            preview_token: preview_token.clone(),
            files_to_create,
            files_to_overwrite,
            conflicts,
            missing_blobs,
            integrity_valid,
            file_states,
            disk_hashes,
        };

        let now = Instant::now();
        let expires_at = now + Duration::from_secs(600); // 10 minutes

        let session = ActivePreviewSession {
            preview_token: preview_token.clone(),
            checkpoint_id: checkpoint_id.to_string(),
            workspace_id: workspace_id.to_string(),
            canonical_root,
            preview: preview.clone(),
            created_at: now,
            expires_at,
        };

        {
            let mut previews = self.active_previews.lock();
            previews.retain(|_, s| s.expires_at > now);
            previews.insert(preview_token, session);
        }

        Ok(preview)
    }

    /// Legacy restore preview method without explicit workspace binding.
    pub fn restore_preview(
        &self,
        checkpoint_id: &str,
        root_dir: &Path,
    ) -> Result<CheckpointRestorePreview> {
        let canon = root_dir
            .canonicalize()
            .unwrap_or_else(|_| root_dir.to_path_buf());
        self.restore_preview_bound(checkpoint_id, "ws-default", &canon)
    }

    /// Validates and consumes an active preview token atomically.
    pub fn consume_preview_token(
        &self,
        token: &str,
        checkpoint_id: &str,
        workspace_id: &str,
        root_dir: &Path,
    ) -> Result<ActivePreviewSession> {
        let canonical_root = root_dir.canonicalize().map_err(AetherError::Io)?;
        let now = Instant::now();

        let mut previews = self.active_previews.lock();
        let session = previews.remove(token).ok_or_else(|| {
            AetherError::PermissionDenied(format!(
                "Invalid or already consumed preview token '{}'",
                token
            ))
        })?;

        if session.expires_at <= now {
            return Err(AetherError::PermissionDenied(format!(
                "Preview token '{}' has expired",
                token
            )));
        }

        if session.checkpoint_id != checkpoint_id {
            return Err(AetherError::PermissionDenied(format!(
                "Preview token checkpoint mismatch: expected '{}', got '{}'",
                session.checkpoint_id, checkpoint_id
            )));
        }

        if session.workspace_id != workspace_id {
            return Err(AetherError::PermissionDenied(format!(
                "Preview token workspace mismatch: expected '{}', got '{}'",
                session.workspace_id, workspace_id
            )));
        }

        if session.canonical_root != canonical_root {
            return Err(AetherError::PermissionDenied(format!(
                "Preview token root directory mismatch: expected {:?}, got {:?}",
                session.canonical_root, canonical_root
            )));
        }

        Ok(session)
    }

    /// Legacy compatibility restore method.
    pub fn restore_checkpoint(&self, checkpoint: &WorkspaceCheckpoint) -> Result<()> {
        let root = checkpoint
            .roots
            .first()
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."));
        let report = self.restore_checkpoint_into(&checkpoint.checkpoint_id, &root, true)?;
        if !report.success {
            return Err(AetherError::Internal(format!(
                "Restore failed: {:?}",
                report.errors
            )));
        }
        Ok(())
    }

    /// Restores a snapshot into target root directory with atomic file writes and hash validation.
    pub fn restore_checkpoint_into(
        &self,
        checkpoint_id: &str,
        root_dir: &Path,
        force: bool,
    ) -> Result<CheckpointRestoreReport> {
        let preview = self.restore_preview(checkpoint_id, root_dir)?;
        let overwrite_list: Vec<String> = if force {
            preview.conflicts.clone()
        } else {
            Vec::new()
        };
        self.restore_checkpoint_with_token(checkpoint_id, root_dir, &preview, &overwrite_list)
    }

    /// Restores a snapshot into target root directory with explicit conflict decisions,
    /// race condition protection against post-preview modifications, byte-level atomic writes,
    /// and post-restore cryptographic integrity verification.
    pub fn restore_checkpoint_with_token(
        &self,
        checkpoint_id: &str,
        root_dir: &Path,
        preview: &CheckpointRestorePreview,
        explicit_overwrite_conflicts: &[String],
    ) -> Result<CheckpointRestoreReport> {
        if !preview.integrity_valid {
            return Err(AetherError::Internal(format!(
                "Checkpoint integrity check failed: missing blobs {:?}",
                preview.missing_blobs
            )));
        }

        // 1. Enforce default fail-closed policy: Abort on unresolved conflicts
        let unapproved_conflicts: Vec<_> = preview
            .conflicts
            .iter()
            .filter(|c| !explicit_overwrite_conflicts.contains(c))
            .cloned()
            .collect();

        if !unapproved_conflicts.is_empty() {
            return Err(AetherError::PermissionDenied(format!(
                "Restore aborted: {} unresolved conflicts detected in {:?}. Explicit user authorization required to overwrite.",
                unapproved_conflicts.len(),
                unapproved_conflicts
            )));
        }

        // 2. Protect against restore races: revalidate current file states against preview snapshot
        for (rel_path, recorded_state) in &preview.file_states {
            let dest_path = root_dir.join(rel_path);
            let current_meta = std::fs::symlink_metadata(&dest_path);

            match recorded_state {
                FileDiskState::Present {
                    sha256: expected_hash,
                    ..
                } => match current_meta {
                    Ok(meta) => {
                        if meta.file_type().is_symlink() || !meta.is_file() {
                            return Err(AetherError::PermissionDenied(format!(
                                    "Restore race detected: file '{}' is no longer a regular file or was replaced with another object. Restore aborted to protect modifications.",
                                    rel_path
                                )));
                        }
                        let current_bytes = std::fs::read(&dest_path).map_err(AetherError::Io)?;
                        let current_hash = Self::sha256_digest(&current_bytes);
                        if &current_hash != expected_hash {
                            return Err(AetherError::PermissionDenied(format!(
                                    "Restore race detected: file '{}' was modified on disk after preview was generated (Expected: {}, Current: {}). Restore aborted to protect modifications.",
                                    rel_path, expected_hash, current_hash
                                )));
                        }
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                        return Err(AetherError::PermissionDenied(format!(
                                "Restore race detected: file '{}' was deleted after preview was generated. Restore aborted to protect modifications.",
                                rel_path
                            )));
                    }
                    Err(e) => {
                        return Err(AetherError::PermissionDenied(format!(
                                "Restore race detected: file '{}' became inaccessible: {}. Restore aborted.",
                                rel_path, e
                            )));
                    }
                },
                FileDiskState::Missing => {
                    if dest_path.exists() || current_meta.is_ok() {
                        return Err(AetherError::PermissionDenied(format!(
                            "Restore race detected: target file '{}' was newly created on disk after preview was generated. Restore aborted to protect modifications.",
                            rel_path
                        )));
                    }
                }
                FileDiskState::Inaccessible { reason } => {
                    return Err(AetherError::PermissionDenied(format!(
                        "Restore race detected: file '{}' was inaccessible during preview generation ({}). Restore aborted.",
                        rel_path, reason
                    )));
                }
            }
        }

        let snapshot_dir = self.storage_dir.join(checkpoint_id);
        let meta_path = snapshot_dir.join("metadata.json");
        let meta_content = FilesystemEngine::read_file_string(&meta_path)?;
        let ckpt: WorkspaceCheckpoint = serde_json::from_str(&meta_content)?;
        let blobs_dir = snapshot_dir.join("blobs");

        let cap = WorkspaceCapability::open(root_dir)?;
        let mut restored_files = Vec::new();
        let mut errors = Vec::new();

        for record in &ckpt.files {
            // Verify path strictly
            if let Err(e) = sanitize_relative_path(&record.relative_path) {
                errors.push(format!(
                    "Skipping unsafe path '{}': {}",
                    record.relative_path, e
                ));
                continue;
            }

            let blob_path = blobs_dir.join(&record.sha256);
            let blob_bytes = match std::fs::read(&blob_path) {
                Ok(b) => b,
                Err(e) => {
                    errors.push(format!(
                        "Failed reading blob for '{}': {}",
                        record.relative_path, e
                    ));
                    continue;
                }
            };

            // Verify blob hash matches manifest
            let computed_hash = Self::sha256_digest(&blob_bytes);
            if computed_hash != record.sha256 {
                errors.push(format!(
                    "Blob corruption detected for '{}'. Expected sha256 {}, got {}",
                    record.relative_path, record.sha256, computed_hash
                ));
                continue;
            }

            // Byte-perfect restoration: write raw bytes atomically via capability
            match cap.write_file_bytes_atomic(&record.relative_path, &blob_bytes) {
                Ok(_) => {
                    // Cryptographic post-restore verification of file on disk
                    let dest = root_dir.join(&record.relative_path);
                    if let Ok(written_bytes) = std::fs::read(&dest) {
                        let written_hash = Self::sha256_digest(&written_bytes);
                        if written_hash == record.sha256 {
                            restored_files.push(record.relative_path.clone());
                        } else {
                            errors.push(format!(
                                "Post-restore hash mismatch for '{}'. Expected {}, found on disk {}",
                                record.relative_path, record.sha256, written_hash
                            ));
                        }
                    } else {
                        errors.push(format!(
                            "Failed to verify restored file '{}' on disk",
                            record.relative_path
                        ));
                    }
                }
                Err(e) => {
                    errors.push(format!(
                        "Failed restoring '{}': {}",
                        record.relative_path, e
                    ));
                }
            }
        }

        let success = errors.is_empty();
        info!(
            "Restored checkpoint '{}': {} succeeded, {} errors",
            checkpoint_id,
            restored_files.len(),
            errors.len()
        );

        Ok(CheckpointRestoreReport {
            checkpoint_id: checkpoint_id.to_string(),
            restored_count: restored_files.len(),
            restored_files,
            errors,
            success,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_checkpoint_create_and_restore() {
        let temp_dir = std::env::temp_dir().join(format!(
            "aether_ckpt_test_{}",
            uuid::Uuid::new_v4().simple()
        ));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        let workspace_dir = temp_dir.join("workspace");
        let src_dir = workspace_dir.join("src");
        std::fs::create_dir_all(&src_dir).unwrap();

        let file1 = workspace_dir.join("hello.txt");
        let file2 = src_dir.join("main.rs");
        std::fs::write(&file1, "Hello world").unwrap();
        std::fs::write(&file2, "fn main() { println!(\"Aether\"); }").unwrap();

        let engine = CheckpointEngine::new(&storage_dir);
        let ckpt = engine
            .create_checkpoint_with_root("Initial snapshot", "ws-test", &workspace_dir, None)
            .expect("Failed to create checkpoint");

        assert_eq!(ckpt.files.len(), 2);
        assert!(ckpt.file_hashes.contains_key("hello.txt"));
        assert!(ckpt.file_hashes.contains_key("src/main.rs"));

        // Verify relative paths only
        for rec in &ckpt.files {
            assert!(!Path::new(&rec.relative_path).is_absolute());
        }

        // Preview restore
        let preview = engine
            .restore_preview(&ckpt.checkpoint_id, &workspace_dir)
            .expect("Preview failed");
        assert!(preview.integrity_valid);
        assert!(preview.conflicts.is_empty());

        // Modify file1 on disk
        std::fs::write(&file1, "Modified content").unwrap();

        let preview_after = engine
            .restore_preview(&ckpt.checkpoint_id, &workspace_dir)
            .expect("Preview failed");
        assert_eq!(preview_after.conflicts.len(), 1);
        assert_eq!(preview_after.conflicts[0], "hello.txt");

        // Restore snapshot with force=false must abort due to conflict
        let unapproved_res =
            engine.restore_checkpoint_into(&ckpt.checkpoint_id, &workspace_dir, false);
        assert!(
            unapproved_res.is_err(),
            "Default restore must abort on unresolved conflicts"
        );

        // Restore snapshot with explicit authorization (force=true)
        let report = engine
            .restore_checkpoint_into(&ckpt.checkpoint_id, &workspace_dir, true)
            .expect("Restore with explicit authorization failed");

        assert!(report.success);
        assert_eq!(report.restored_count, 2);

        // Verify content restored
        let restored_content = std::fs::read_to_string(&file1).unwrap();
        assert_eq!(restored_content, "Hello world");

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_checkpoint_binary_and_invalid_utf8_byte_preservation() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_ckpt_bin_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        let workspace_dir = temp_dir.join("workspace");
        std::fs::create_dir_all(&workspace_dir).unwrap();

        // 1. Valid binary image fixture (PNG magic header + binary data)
        let png_bytes: Vec<u8> = vec![
            0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48,
            0x44, 0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00,
            0x00, 0x1F, 0x15, 0xC4, 0x89,
        ];
        let png_file = workspace_dir.join("image.png");
        std::fs::write(&png_file, &png_bytes).unwrap();
        let expected_png_hash = CheckpointEngine::sha256_digest(&png_bytes);

        // 2. Raw binary file with invalid UTF-8 sequences (0xFF, 0xFE, 0x80, etc.)
        let invalid_utf8_bytes: Vec<u8> =
            vec![0xFF, 0xFE, 0xC0, 0xAF, 0x80, 0x81, 0x00, 0x12, 0x34];
        let bin_file = workspace_dir.join("corrupt_or_raw.bin");
        std::fs::write(&bin_file, &invalid_utf8_bytes).unwrap();
        let expected_bin_hash = CheckpointEngine::sha256_digest(&invalid_utf8_bytes);

        let engine = CheckpointEngine::new(&storage_dir);
        let ckpt = engine
            .create_checkpoint_with_root("Binary test snapshot", "ws-bin", &workspace_dir, None)
            .expect("Failed to create checkpoint");

        assert_eq!(
            ckpt.file_hashes.get("image.png").unwrap(),
            &expected_png_hash
        );
        assert_eq!(
            ckpt.file_hashes.get("corrupt_or_raw.bin").unwrap(),
            &expected_bin_hash
        );

        // Corrupt files on disk
        std::fs::write(&png_file, b"corrupted png bytes").unwrap();
        std::fs::write(&bin_file, b"corrupted bin bytes").unwrap();

        // Restore with explicit authorization
        let report = engine
            .restore_checkpoint_into(&ckpt.checkpoint_id, &workspace_dir, true)
            .expect("Restore failed");

        assert!(report.success);
        assert_eq!(report.restored_count, 2);

        // Verify restored bytes on disk are bit-for-bit identical to original
        let restored_png_bytes = std::fs::read(&png_file).unwrap();
        assert_eq!(
            restored_png_bytes, png_bytes,
            "PNG bytes must be restored without UTF-8 loss"
        );
        assert_eq!(
            CheckpointEngine::sha256_digest(&restored_png_bytes),
            expected_png_hash
        );

        let restored_bin_bytes = std::fs::read(&bin_file).unwrap();
        assert_eq!(
            restored_bin_bytes, invalid_utf8_bytes,
            "Invalid UTF-8 bytes must be restored without substitution characters"
        );
        assert_eq!(
            CheckpointEngine::sha256_digest(&restored_bin_bytes),
            expected_bin_hash
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_checkpoint_restore_race_detection() {
        let temp_dir = std::env::temp_dir().join(format!(
            "aether_ckpt_race_{}",
            uuid::Uuid::new_v4().simple()
        ));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        let workspace_dir = temp_dir.join("workspace");
        std::fs::create_dir_all(&workspace_dir).unwrap();

        let file = workspace_dir.join("race_target.txt");
        std::fs::write(&file, "version 1").unwrap();

        let engine = CheckpointEngine::new(&storage_dir);
        let ckpt = engine
            .create_checkpoint_with_root("Race snapshot", "ws-race", &workspace_dir, None)
            .expect("Failed to create checkpoint");

        // User modifies file
        std::fs::write(&file, "version 2 (user changes)").unwrap();

        // Preview is requested
        let preview = engine
            .restore_preview(&ckpt.checkpoint_id, &workspace_dir)
            .expect("Preview failed");
        assert_eq!(preview.conflicts.len(), 1);

        // Concurrent process mutates file on disk AFTER preview was generated!
        std::fs::write(&file, "version 3 (concurrent edit)").unwrap();

        // User attempts to restore with preview token
        let res = engine.restore_checkpoint_with_token(
            &ckpt.checkpoint_id,
            &workspace_dir,
            &preview,
            &["race_target.txt".to_string()],
        );

        // Must abort because file was modified after preview!
        assert!(res.is_err());
        let err_str = format!("{:?}", res.err().unwrap());
        assert!(err_str.contains("Restore race detected"));

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_checkpoint_path_traversal_and_tamper_defense() {
        let temp_dir = std::env::temp_dir().join(format!(
            "aether_ckpt_defense_{}",
            uuid::Uuid::new_v4().simple()
        ));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        let workspace_dir = temp_dir.join("workspace");
        std::fs::create_dir_all(&workspace_dir).unwrap();

        let file = workspace_dir.join("safe.txt");
        std::fs::write(&file, "Safe content").unwrap();

        let engine = CheckpointEngine::new(&storage_dir);
        let ckpt = engine
            .create_checkpoint_with_root("Safety test", "ws-sec", &workspace_dir, None)
            .expect("Failed to create checkpoint");

        let snapshot_dir = storage_dir.join(&ckpt.checkpoint_id);
        let meta_path = snapshot_dir.join("metadata.json");

        // Tamper with metadata to inject path traversal
        let mut tampered_ckpt = ckpt.clone();
        tampered_ckpt.files.push(CheckpointFileRecord {
            root_id: "root-0".to_string(),
            relative_path: "../../escape.txt".to_string(),
            sha256: "deadbeef".to_string(),
            size_bytes: 10,
        });
        std::fs::write(&meta_path, serde_json::to_string(&tampered_ckpt).unwrap()).unwrap();

        let res = engine.restore_checkpoint_into(&ckpt.checkpoint_id, &workspace_dir, false);
        // Must fail with PermissionDenied because of malicious path traversal
        assert!(res.is_err());
        let err_str = format!("{:?}", res.err().unwrap());
        assert!(
            err_str.contains("Malicious relative path") || err_str.contains("PermissionDenied")
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_008_and_009_preview_token_validation_and_replay_rejection() {
        let temp_dir = std::env::temp_dir().join(format!(
            "aether_chk_tokens_{}",
            uuid::Uuid::new_v4().simple()
        ));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        let workspace_dir = temp_dir.join("workspace");
        std::fs::create_dir_all(&workspace_dir).unwrap();

        let file = workspace_dir.join("token_test.txt");
        std::fs::write(&file, "initial").unwrap();

        let engine = CheckpointEngine::new(&storage_dir);
        let ckpt = engine
            .create_checkpoint_with_root("Token test", "ws-token-1", &workspace_dir, None)
            .unwrap();

        let preview = engine
            .restore_preview_bound(&ckpt.checkpoint_id, "ws-token-1", &workspace_dir)
            .unwrap();
        assert!(!preview.preview_token.is_empty());

        // CHK-008: Invalid token rejected
        let invalid_res = engine.consume_preview_token(
            "prev-invalid-fake-token",
            &ckpt.checkpoint_id,
            "ws-token-1",
            &workspace_dir,
        );
        assert!(invalid_res.is_err(), "Invalid token must be rejected");
        assert!(invalid_res
            .err()
            .unwrap()
            .to_string()
            .contains("Invalid or already consumed preview token"));

        // Valid token consumption succeeds
        let consumed = engine.consume_preview_token(
            &preview.preview_token,
            &ckpt.checkpoint_id,
            "ws-token-1",
            &workspace_dir,
        );
        assert!(consumed.is_ok(), "First consumption of token must succeed");

        // CHK-009: Token replay rejected (cannot use token twice)
        let replay_res = engine.consume_preview_token(
            &preview.preview_token,
            &ckpt.checkpoint_id,
            "ws-token-1",
            &workspace_dir,
        );
        assert!(replay_res.is_err(), "Replayed token must be rejected");
        assert!(replay_res
            .err()
            .unwrap()
            .to_string()
            .contains("Invalid or already consumed preview token"));

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_010_expired_preview_token_rejected() {
        let temp_dir = std::env::temp_dir().join(format!(
            "aether_chk_expire_{}",
            uuid::Uuid::new_v4().simple()
        ));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        let workspace_dir = temp_dir.join("workspace");
        std::fs::create_dir_all(&workspace_dir).unwrap();

        let file = workspace_dir.join("expire.txt");
        std::fs::write(&file, "test").unwrap();

        let engine = CheckpointEngine::new(&storage_dir);
        let ckpt = engine
            .create_checkpoint_with_root("Expire test", "ws-exp", &workspace_dir, None)
            .unwrap();

        let preview = engine
            .restore_preview_bound(&ckpt.checkpoint_id, "ws-exp", &workspace_dir)
            .unwrap();

        // Artificially expire the token in the session store
        {
            let mut previews = engine.active_previews.lock();
            if let Some(session) = previews.get_mut(&preview.preview_token) {
                session.expires_at = Instant::now() - Duration::from_secs(1);
            }
        }

        // CHK-010: Expired token rejected
        let exp_res = engine.consume_preview_token(
            &preview.preview_token,
            &ckpt.checkpoint_id,
            "ws-exp",
            &workspace_dir,
        );
        assert!(exp_res.is_err(), "Expired preview token must be rejected");
        assert!(exp_res.err().unwrap().to_string().contains("has expired"));

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_011_deleted_file_race_detection() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_del_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        let workspace_dir = temp_dir.join("workspace");
        std::fs::create_dir_all(&workspace_dir).unwrap();

        let file = workspace_dir.join("to_be_deleted.txt");
        std::fs::write(&file, "present at checkpoint").unwrap();

        let engine = CheckpointEngine::new(&storage_dir);
        let ckpt = engine
            .create_checkpoint_with_root("Del test", "ws-del", &workspace_dir, None)
            .unwrap();

        // Preview generated while file is Present
        let preview = engine
            .restore_preview_bound(&ckpt.checkpoint_id, "ws-del", &workspace_dir)
            .unwrap();

        // CHK-011: File is deleted on disk after preview was generated!
        std::fs::remove_file(&file).unwrap();

        let res = engine.restore_checkpoint_with_token(
            &ckpt.checkpoint_id,
            &workspace_dir,
            &preview,
            &[],
        );

        assert!(
            res.is_err(),
            "Deleted file race must be detected and aborted"
        );
        let err_msg = res.err().unwrap().to_string();
        assert!(
            err_msg.contains("deleted after preview was generated"),
            "Error must identify deleted-file race: {}",
            err_msg
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_012_newly_created_target_file_race_detection() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_new_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        let workspace_dir = temp_dir.join("workspace");
        std::fs::create_dir_all(&workspace_dir).unwrap();

        let target_file = workspace_dir.join("new_file.txt");
        std::fs::write(&target_file, "content in checkpoint").unwrap();

        let engine = CheckpointEngine::new(&storage_dir);
        let ckpt = engine
            .create_checkpoint_with_root("New file test", "ws-new", &workspace_dir, None)
            .unwrap();

        // Delete target file so that at preview time it is Missing on disk
        std::fs::remove_file(&target_file).unwrap();

        // Preview generated: file state is Missing
        let preview = engine
            .restore_preview_bound(&ckpt.checkpoint_id, "ws-new", &workspace_dir)
            .unwrap();

        // CHK-012: Concurrent developer creates file on disk after preview!
        std::fs::write(&target_file, "concurrent new content").unwrap();

        let res = engine.restore_checkpoint_with_token(
            &ckpt.checkpoint_id,
            &workspace_dir,
            &preview,
            &[],
        );

        assert!(
            res.is_err(),
            "Newly created file race must be detected and aborted"
        );
        let err_msg = res.err().unwrap().to_string();
        assert!(
            err_msg.contains("newly created on disk after preview was generated"),
            "Error must identify newly created file race: {}",
            err_msg
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
