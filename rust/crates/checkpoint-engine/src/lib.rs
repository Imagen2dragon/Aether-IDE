use application_core::{AetherError, Result};
use filesystem_engine::{
    sanitize_relative_path, ConditionalReplaceHooks, FilesystemEngine, WorkspaceCapability,
};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::{info, warn};

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
    #[serde(default)]
    pub backup_checkpoint_id: Option<String>,
    #[serde(default)]
    pub preserved_backups: Vec<String>,
    #[serde(default)]
    pub compensated_files: Vec<String>,
    #[serde(default)]
    pub recovery_conflicts: Vec<String>,
}

type PreReplaceHook = Arc<dyn Fn(&Path) + Send + Sync>;

struct PathLockGuard {
    set: Arc<Mutex<std::collections::HashSet<PathBuf>>>,
    path: PathBuf,
}

impl PathLockGuard {
    fn acquire(set: &Arc<Mutex<std::collections::HashSet<PathBuf>>>, path: &Path) -> Result<Self> {
        let mut guard = set.lock();
        if guard.contains(path) {
            return Err(AetherError::PermissionDenied(format!(
                "Concurrent restore or modification lock active on path '{:?}'",
                path
            )));
        }
        guard.insert(path.to_path_buf());
        Ok(Self {
            set: Arc::clone(set),
            path: path.to_path_buf(),
        })
    }
}

impl Drop for PathLockGuard {
    fn drop(&mut self) {
        let mut guard = self.set.lock();
        guard.remove(&self.path);
    }
}

struct CommittedFile {
    relative_path: String,
    restored_sha256: String,
    #[allow(dead_code)]
    preserved_backup: Option<PathBuf>,
}

pub struct CheckpointEngine {
    storage_dir: PathBuf,
    active_previews: Arc<Mutex<HashMap<String, ActivePreviewSession>>>,
    simulate_backup_failure: std::sync::atomic::AtomicBool,
    active_restore_paths: Arc<Mutex<std::collections::HashSet<PathBuf>>>,
    pre_replace_hook: Arc<Mutex<Option<PreReplaceHook>>>,
    post_hash_hook: Arc<Mutex<Option<PreReplaceHook>>>,
    post_write_hook: Arc<Mutex<Option<PreReplaceHook>>>,
    fault_inject_post_write_verification: std::sync::atomic::AtomicBool,
    fault_inject_compensation_write: std::sync::atomic::AtomicBool,
    pre_delete_hook: Arc<Mutex<Option<PreReplaceHook>>>,
    post_hash_delete_hook: Arc<Mutex<Option<PreReplaceHook>>>,
    fault_inject_metadata_failure: std::sync::atomic::AtomicBool,
    fault_inject_delete_removal: std::sync::atomic::AtomicBool,
    fault_inject_backup_removal: std::sync::atomic::AtomicBool,
}

impl CheckpointEngine {
    pub fn new<P: AsRef<Path>>(storage_dir: P) -> Self {
        let p = storage_dir.as_ref().to_path_buf();
        let _ = std::fs::create_dir_all(&p);
        Self {
            storage_dir: p,
            active_previews: Arc::new(Mutex::new(HashMap::new())),
            simulate_backup_failure: std::sync::atomic::AtomicBool::new(false),
            active_restore_paths: Arc::new(Mutex::new(std::collections::HashSet::new())),
            pre_replace_hook: Arc::new(Mutex::new(None)),
            post_hash_hook: Arc::new(Mutex::new(None)),
            post_write_hook: Arc::new(Mutex::new(None)),
            fault_inject_post_write_verification: std::sync::atomic::AtomicBool::new(false),
            fault_inject_compensation_write: std::sync::atomic::AtomicBool::new(false),
            pre_delete_hook: Arc::new(Mutex::new(None)),
            post_hash_delete_hook: Arc::new(Mutex::new(None)),
            fault_inject_metadata_failure: std::sync::atomic::AtomicBool::new(false),
            fault_inject_delete_removal: std::sync::atomic::AtomicBool::new(false),
            fault_inject_backup_removal: std::sync::atomic::AtomicBool::new(false),
        }
    }

    #[doc(hidden)]
    pub fn set_simulate_backup_failure(&self, simulate: bool) {
        self.simulate_backup_failure
            .store(simulate, std::sync::atomic::Ordering::SeqCst);
    }

    #[doc(hidden)]
    pub fn set_pre_replace_hook<F>(&self, hook: Option<F>)
    where
        F: Fn(&Path) + Send + Sync + 'static,
    {
        let mut guard = self.pre_replace_hook.lock();
        *guard = hook.map(|h| Arc::new(h) as PreReplaceHook);
    }

    #[doc(hidden)]
    pub fn set_post_hash_hook<F>(&self, hook: Option<F>)
    where
        F: Fn(&Path) + Send + Sync + 'static,
    {
        let mut guard = self.post_hash_hook.lock();
        *guard = hook.map(|h| Arc::new(h) as PreReplaceHook);
    }

    #[doc(hidden)]
    pub fn set_post_write_hook<F>(&self, hook: Option<F>)
    where
        F: Fn(&Path) + Send + Sync + 'static,
    {
        let mut guard = self.post_write_hook.lock();
        *guard = hook.map(|h| Arc::new(h) as PreReplaceHook);
    }

    #[doc(hidden)]
    pub fn set_fault_inject_post_write_verification(&self, inject: bool) {
        self.fault_inject_post_write_verification
            .store(inject, std::sync::atomic::Ordering::SeqCst);
    }

    #[doc(hidden)]
    pub fn set_fault_inject_compensation_write(&self, inject: bool) {
        self.fault_inject_compensation_write
            .store(inject, std::sync::atomic::Ordering::SeqCst);
    }

    #[doc(hidden)]
    pub fn set_pre_delete_hook<F>(&self, hook: Option<F>)
    where
        F: Fn(&Path) + Send + Sync + 'static,
    {
        let mut guard = self.pre_delete_hook.lock();
        *guard = hook.map(|h| Arc::new(h) as PreReplaceHook);
    }

    #[doc(hidden)]
    pub fn set_post_hash_delete_hook<F>(&self, hook: Option<F>)
    where
        F: Fn(&Path) + Send + Sync + 'static,
    {
        let mut guard = self.post_hash_delete_hook.lock();
        *guard = hook.map(|h| Arc::new(h) as PreReplaceHook);
    }

    #[doc(hidden)]
    pub fn set_fault_inject_metadata_failure(&self, inject: bool) {
        self.fault_inject_metadata_failure
            .store(inject, std::sync::atomic::Ordering::SeqCst);
    }

    #[doc(hidden)]
    pub fn set_fault_inject_delete_removal(&self, inject: bool) {
        self.fault_inject_delete_removal
            .store(inject, std::sync::atomic::Ordering::SeqCst);
    }

    #[doc(hidden)]
    pub fn set_fault_inject_backup_removal(&self, inject: bool) {
        self.fault_inject_backup_removal
            .store(inject, std::sync::atomic::Ordering::SeqCst);
    }

    #[doc(hidden)]
    pub fn set_storage_dir(&mut self, path: PathBuf) {
        self.storage_dir = path;
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
        let session = previews.get(token).ok_or_else(|| {
            AetherError::PermissionDenied(format!(
                "Invalid or already consumed preview token '{}'",
                token
            ))
        })?;

        if session.expires_at <= now {
            previews.remove(token);
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

        let session = previews.remove(token).expect("Token must exist");
        Ok(session)
    }

    /// Deprecated: Direct restoration without preview token and safety backup is disabled for security.
    #[deprecated(
        since = "0.1.0-beta.2",
        note = "Direct restoration without preview token is disabled. Use restore_preview_bound and restore_checkpoint_with_token."
    )]
    pub fn restore_checkpoint(&self, _checkpoint: &WorkspaceCheckpoint) -> Result<()> {
        Err(AetherError::PermissionDenied(
            "Security Denial: Direct restore_checkpoint without preview token and mandatory safety backup is disabled. Every restoration must use restore_preview_bound and restore_checkpoint_with_token."
                .to_string(),
        ))
    }

    /// Deprecated: Direct restoration without preview token and safety backup is disabled for security.
    #[deprecated(
        since = "0.1.0-beta.2",
        note = "Direct restoration without preview token is disabled. Use restore_preview_bound and restore_checkpoint_with_token."
    )]
    pub fn restore_checkpoint_into(
        &self,
        _checkpoint_id: &str,
        _root_dir: &Path,
        _force: bool,
    ) -> Result<CheckpointRestoreReport> {
        Err(AetherError::PermissionDenied(
            "Security Denial: Direct restore_checkpoint_into without preview token and mandatory safety backup is disabled. Every restoration must use restore_preview_bound and restore_checkpoint_with_token."
                .to_string(),
        ))
    }

    /// Verifies the recorded disk state against the current filesystem object immediately before write.
    /// Distinguishes NotFound from PermissionDenied and other I/O errors fail-closed.
    pub fn verify_file_disk_state(
        dest_path: &Path,
        rel_path: &str,
        recorded_state: &FileDiskState,
    ) -> Result<()> {
        let current_meta = std::fs::symlink_metadata(dest_path);

        match recorded_state {
            FileDiskState::Present {
                sha256: expected_hash,
                ..
            } => match current_meta {
                Ok(meta) => {
                    if meta.file_type().is_symlink() {
                        return Err(AetherError::PermissionDenied(format!(
                            "Restore race detected: file '{}' was replaced with a symbolic link. Restore aborted.",
                            rel_path
                        )));
                    }
                    if !meta.is_file() {
                        return Err(AetherError::PermissionDenied(format!(
                            "Restore race detected: file '{}' is no longer a regular file. Restore aborted.",
                            rel_path
                        )));
                    }
                    let current_bytes = std::fs::read(dest_path).map_err(|e| {
                        AetherError::PermissionDenied(format!(
                            "Restore race detected: file '{}' could not be read ({}). Restore aborted.",
                            rel_path, e
                        ))
                    })?;
                    let current_hash = Self::sha256_digest(&current_bytes);
                    if &current_hash != expected_hash {
                        return Err(AetherError::PermissionDenied(format!(
                            "Restore race detected: file '{}' was modified on disk after preview was generated (Expected: {}, Current: {}). Restore aborted.",
                            rel_path, expected_hash, current_hash
                        )));
                    }
                    Ok(())
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                    Err(AetherError::PermissionDenied(format!(
                        "Restore race detected: file '{}' was deleted after preview was generated. Restore aborted.",
                        rel_path
                    )))
                }
                Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
                    Err(AetherError::PermissionDenied(format!(
                        "Restore race detected: file '{}' became inaccessible due to permission denied. Restore aborted.",
                        rel_path
                    )))
                }
                Err(e) => {
                    Err(AetherError::PermissionDenied(format!(
                        "Restore race detected: filesystem error inspecting '{}': {}. Restore aborted.",
                        rel_path, e
                    )))
                }
            },
            FileDiskState::Missing => match current_meta {
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
                Ok(meta) => {
                    if meta.file_type().is_symlink() {
                        Err(AetherError::PermissionDenied(format!(
                            "Restore race detected: target file '{}' was newly created as a symbolic link after preview. Restore aborted.",
                            rel_path
                        )))
                    } else if !meta.is_file() {
                        Err(AetherError::PermissionDenied(format!(
                            "Restore race detected: target file '{}' was newly created as a directory or non-file object after preview. Restore aborted.",
                            rel_path
                        )))
                    } else {
                        Err(AetherError::PermissionDenied(format!(
                            "Restore race detected: target file '{}' was newly created on disk after preview was generated. Restore aborted to protect modifications.",
                            rel_path
                        )))
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
                    Err(AetherError::PermissionDenied(format!(
                        "Restore race detected: target file '{}' is inaccessible due to permission denied. Restore aborted.",
                        rel_path
                    )))
                }
                Err(e) => {
                    Err(AetherError::PermissionDenied(format!(
                        "Restore race detected: filesystem error inspecting target file '{}': {}. Restore aborted.",
                        rel_path, e
                    )))
                }
            },
            FileDiskState::Inaccessible { reason } => {
                Err(AetherError::PermissionDenied(format!(
                    "Restore race detected: file '{}' was inaccessible during preview generation ({}). Restore aborted.",
                    rel_path, reason
                )))
            }
        }
    }

    /// Restores a snapshot into target root directory with centralized preview token consumption,
    /// mandatory pre-restore safety snapshot creation, explicit conflict decisions,
    /// immediate per-file TOCTOU race condition protection, byte-level atomic writes,
    /// and post-restore cryptographic integrity verification.
    pub fn restore_checkpoint_with_token(
        &self,
        checkpoint_id: &str,
        workspace_id: &str,
        root_dir: &Path,
        preview_token: &str,
        force: bool,
        explicit_overwrite_conflicts: &[String],
    ) -> Result<CheckpointRestoreReport> {
        let canonical_root = root_dir.canonicalize().map_err(AetherError::Io)?;

        // 1. Atomically validate and consume the preview token (single-use, binds id/workspace/root)
        let session = self.consume_preview_token(
            preview_token,
            checkpoint_id,
            workspace_id,
            &canonical_root,
        )?;

        if !session.preview.integrity_valid {
            return Err(AetherError::Internal(format!(
                "Checkpoint integrity check failed: missing blobs {:?}",
                session.preview.missing_blobs
            )));
        }

        // 2. Enforce conflict authorization: reject unapproved conflicts fail-closed
        let unapproved_conflicts: Vec<_> = if force {
            Vec::new()
        } else {
            session
                .preview
                .conflicts
                .iter()
                .filter(|c| !explicit_overwrite_conflicts.contains(c))
                .cloned()
                .collect()
        };

        if !unapproved_conflicts.is_empty() {
            return Err(AetherError::PermissionDenied(format!(
                "Restore aborted: {} unresolved conflicts detected in {:?}. Explicit user authorization required to overwrite.",
                unapproved_conflicts.len(),
                unapproved_conflicts
            )));
        }

        // 3. Mandatory pre-restore safety snapshot (CHK-013)
        // Must succeed or abort restoration immediately, leaving existing workspace untouched.
        if self
            .simulate_backup_failure
            .load(std::sync::atomic::Ordering::SeqCst)
        {
            return Err(AetherError::Internal(format!(
                "Mandatory pre-restore safety backup failed: Simulated I/O failure for {}. Restoration aborted to protect existing workspace.",
                checkpoint_id
            )));
        }

        let backup_ckpt = self
            .create_checkpoint_with_root(
                &format!("Pre-restore safety backup for {}", checkpoint_id),
                workspace_id,
                &canonical_root,
                None,
            )
            .map_err(|e| {
                AetherError::Internal(format!(
                    "Mandatory pre-restore safety backup failed: {}. Restoration aborted to protect existing workspace.",
                    e
                ))
            })?;

        info!(
            "Mandatory safety snapshot created prior to restore: {}",
            backup_ckpt.checkpoint_id
        );

        // 4. Preflight and read ALL checkpoint blobs into memory before touching disk
        let snapshot_dir = self.storage_dir.join(checkpoint_id);
        let meta_path = snapshot_dir.join("metadata.json");
        let meta_content = FilesystemEngine::read_file_string(&meta_path)?;
        let ckpt: WorkspaceCheckpoint = serde_json::from_str(&meta_content)?;
        let blobs_dir = snapshot_dir.join("blobs");

        let mut prepared_records: Vec<(&CheckpointFileRecord, Vec<u8>)> = Vec::new();
        for record in &ckpt.files {
            sanitize_relative_path(&record.relative_path)?;
            let blob_path = blobs_dir.join(&record.sha256);
            let blob_bytes = std::fs::read(&blob_path).map_err(|e| {
                AetherError::Internal(format!(
                    "Failed reading blob for '{}': {}",
                    record.relative_path, e
                ))
            })?;

            let computed_hash = Self::sha256_digest(&blob_bytes);
            if computed_hash != record.sha256 {
                return Err(AetherError::Internal(format!(
                    "Blob corruption detected for '{}'. Expected sha256 {}, got {}",
                    record.relative_path, record.sha256, computed_hash
                )));
            }
            prepared_records.push((record, blob_bytes));
        }

        // 5. Execute file writes with immediate per-file state revalidation to close TOCTOU window
        let cap = WorkspaceCapability::open(&canonical_root)?;
        let mut committed_files: Vec<CommittedFile> = Vec::new();

        for (record, blob_bytes) in prepared_records {
            let dest_path = canonical_root.join(&record.relative_path);

            // 1. Acquire in-process path synchronization lock
            let _path_lock = PathLockGuard::acquire(&self.active_restore_paths, &dest_path)?;

            // 2. Re-validate disk state immediately before writing this specific file
            let recorded_state = session
                .preview
                .file_states
                .get(&record.relative_path)
                .ok_or_else(|| {
                    AetherError::PermissionDenied(format!(
                        "Restore aborted: unrecorded file state for '{}'",
                        record.relative_path
                    ))
                })?;

            Self::verify_file_disk_state(&dest_path, &record.relative_path, recorded_state)?;

            let expected_hash = match recorded_state {
                FileDiskState::Present { sha256, .. } => Some(sha256.as_str()),
                FileDiskState::Missing => None,
                FileDiskState::Inaccessible { reason } => {
                    return Err(AetherError::PermissionDenied(format!(
                        "Restore race detected: file '{}' was inaccessible during preview generation ({}). Restore aborted.",
                        record.relative_path, reason
                    )));
                }
            };

            let hook_opt = self.pre_replace_hook.lock().clone();
            let hook_ref = hook_opt.as_ref().map(|h| h.as_ref() as &dyn Fn(&Path));

            let post_hook_opt = self.post_hash_hook.lock().clone();
            let post_hook_ref = post_hook_opt.as_ref().map(|h| h.as_ref() as &dyn Fn(&Path));

            let hooks = ConditionalReplaceHooks {
                pre_replace_hook: hook_ref,
                post_hash_hook: post_hook_ref,
                fault_inject_backup_removal: self
                    .fault_inject_backup_removal
                    .load(std::sync::atomic::Ordering::SeqCst),
                ..Default::default()
            };

            // 3. Staged conditional atomic replacement with automatic rollback on detected modifications
            let replace_res = cap.replace_file_bytes_conditional_with_hooks(
                &record.relative_path,
                &blob_bytes,
                expected_hash,
                hooks,
            );

            let outcome = match replace_res {
                Ok(outcome) => outcome,
                Err(e) => {
                    // Replacement failed before or during commit.
                    // Previous committed files must be rolled back via compensating rollback.
                    return self.handle_compensating_rollback(
                        &cap,
                        &canonical_root,
                        &self.storage_dir,
                        &backup_ckpt,
                        &committed_files,
                        e,
                    );
                }
            };

            // Replacement committed to target destination! Track IMMEDIATELY so it cannot be omitted
            // from compensating rollback if post-write verification fails.
            committed_files.push(CommittedFile {
                relative_path: record.relative_path.clone(),
                restored_sha256: record.sha256.clone(),
                preserved_backup: outcome.preserved_backup,
            });

            // 4. Post-write hook (for testing / deterministic injection)
            let post_write_hook_opt = self.post_write_hook.lock().clone();
            if let Some(ref hook) = post_write_hook_opt {
                hook(&dest_path);
            }

            // 5. Immediate post-write verification
            let verification_res: Result<()> = (|| {
                if self
                    .fault_inject_post_write_verification
                    .load(std::sync::atomic::Ordering::SeqCst)
                {
                    return Err(AetherError::Internal(format!(
                        "Fault injection: post-restore verification failure for '{}'",
                        record.relative_path
                    )));
                }

                let written_bytes = std::fs::read(&dest_path).map_err(AetherError::Io)?;
                let written_hash = Self::sha256_digest(&written_bytes);
                if written_hash != record.sha256 {
                    return Err(AetherError::Internal(format!(
                        "Post-restore hash mismatch for '{}'. Expected {}, found on disk {}",
                        record.relative_path, record.sha256, written_hash
                    )));
                }
                Ok(())
            })();

            if let Err(e) = verification_res {
                return self.handle_compensating_rollback(
                    &cap,
                    &canonical_root,
                    &self.storage_dir,
                    &backup_ckpt,
                    &committed_files,
                    e,
                );
            }
        }

        let restored_files: Vec<String> = committed_files
            .iter()
            .map(|c| c.relative_path.clone())
            .collect();
        let preserved_backups: Vec<String> = committed_files
            .into_iter()
            .filter_map(|c| c.preserved_backup.map(|p| p.display().to_string()))
            .collect();

        Ok(CheckpointRestoreReport {
            checkpoint_id: checkpoint_id.to_string(),
            restored_count: restored_files.len(),
            restored_files,
            errors: Vec::new(),
            success: true,
            backup_checkpoint_id: Some(backup_ckpt.checkpoint_id),
            preserved_backups,
            compensated_files: Vec::new(),
            recovery_conflicts: Vec::new(),
        })
    }

    fn handle_compensating_rollback(
        &self,
        cap: &WorkspaceCapability,
        canonical_root: &Path,
        storage_dir: &Path,
        backup_ckpt: &WorkspaceCheckpoint,
        committed_files: &[CommittedFile],
        orig_err: AetherError,
    ) -> Result<CheckpointRestoreReport> {
        if committed_files.is_empty() {
            return Err(orig_err);
        }

        warn!(
            "Restore failed after {} files were committed: {}. Initiating conflict-aware compensating rollback from safety checkpoint '{}'...",
            committed_files.len(),
            orig_err,
            backup_ckpt.checkpoint_id
        );

        let mut rollback_errors: Vec<String> = Vec::new();
        let mut recovery_conflicts: Vec<String> = Vec::new();
        let mut compensated_files: Vec<String> = Vec::new();
        let mut preserved_backups: Vec<String> = committed_files
            .iter()
            .filter_map(|c| c.preserved_backup.as_ref().map(|p| p.display().to_string()))
            .collect();
        let mut rolled_back_count = 0;
        let backup_blobs_dir = storage_dir.join(&backup_ckpt.checkpoint_id).join("blobs");

        for committed in committed_files.iter().rev() {
            let meta_res = if self
                .fault_inject_metadata_failure
                .load(std::sync::atomic::Ordering::SeqCst)
            {
                Err(AetherError::PermissionDenied(
                    "Fault injection: metadata permission denied".into(),
                ))
            } else {
                cap.symlink_metadata(&committed.relative_path)
            };

            let in_backup = backup_ckpt
                .files
                .iter()
                .find(|f| f.relative_path == committed.relative_path);

            match meta_res {
                Err(AetherError::Io(ref e)) if e.kind() == std::io::ErrorKind::NotFound => {
                    // Confirmed absent on disk at inspection time
                    if let Some(backup_rec) = in_backup {
                        let blob_file = backup_blobs_dir.join(&backup_rec.sha256);
                        match std::fs::read(&blob_file) {
                            Ok(orig_bytes) => {
                                let pre_rep_hook_opt = self.pre_replace_hook.lock().clone();
                                let pre_rep_ref = pre_rep_hook_opt
                                    .as_ref()
                                    .map(|h| h.as_ref() as &dyn Fn(&Path));
                                let replace_hooks = ConditionalReplaceHooks {
                                    pre_replace_hook: pre_rep_ref,
                                    ..Default::default()
                                };

                                match cap.replace_file_bytes_conditional_with_hooks(
                                    &committed.relative_path,
                                    &orig_bytes,
                                    None, // Exclusive hard-link creation: never overwrites recreated destination!
                                    replace_hooks,
                                ) {
                                    Ok(outcome) => {
                                        rolled_back_count += 1;
                                        compensated_files.push(committed.relative_path.clone());
                                        if let Some(surviving) = outcome.preserved_backup {
                                            preserved_backups.push(surviving.display().to_string());
                                        }
                                    }
                                    Err(AetherError::PermissionDenied(msg))
                                        if msg.contains("newly created")
                                            || msg.contains("race detected") =>
                                    {
                                        let conflict_msg = format!(
                                            "Missing file '{}' was concurrently recreated before compensation: {}. Concurrently recreated file preserved in workspace without overwrite.",
                                            committed.relative_path, msg
                                        );
                                        warn!("{}", conflict_msg);
                                        recovery_conflicts.push(conflict_msg);
                                    }
                                    Err(err) => {
                                        rollback_errors.push(format!(
                                            "Failed to recreate missing file '{}' exclusively from backup: {}",
                                            committed.relative_path, err
                                        ));
                                    }
                                }
                            }
                            Err(err) => {
                                rollback_errors.push(format!(
                                    "Failed to read backup blob for missing file '{}': {}",
                                    committed.relative_path, err
                                ));
                            }
                        }
                    } else {
                        // Newly created file already absent on disk -> clean rollback
                        rolled_back_count += 1;
                        compensated_files.push(committed.relative_path.clone());
                    }
                }
                Err(err) => {
                    // Inaccessible (PermissionDenied or I/O error) -> state is uncertain!
                    // Do NOT treat as absent. Never increment rolled_back_count.
                    let msg = format!(
                        "File '{}' metadata inspection failed ({}). Recovery state uncertain; manual inspection required.",
                        committed.relative_path, err
                    );
                    warn!("{}", msg);
                    recovery_conflicts.push(msg);
                }
                Ok(meta) => {
                    if meta.is_symlink() {
                        let msg = format!(
                            "File '{}' is a symbolic link during rollback. Refusing to touch symlink; manual inspection required.",
                            committed.relative_path
                        );
                        warn!("{}", msg);
                        recovery_conflicts.push(msg);
                        continue;
                    }
                    if !meta.is_file() {
                        let msg = format!(
                            "File '{}' is not a regular file during rollback. Manual inspection required.",
                            committed.relative_path
                        );
                        warn!("{}", msg);
                        recovery_conflicts.push(msg);
                        continue;
                    }

                    if let Some(backup_rec) = in_backup {
                        // File existed before restore. Inspect content hash.
                        let dest = canonical_root.join(&committed.relative_path);
                        let current_disk_read = std::fs::read(&dest);
                        let current_disk_hash = match current_disk_read {
                            Ok(ref b) => Some(Self::sha256_digest(b)),
                            Err(_) => None,
                        };

                        match current_disk_hash {
                            Some(ref h) if h == &committed.restored_sha256 => {
                                let blob_file = backup_blobs_dir.join(&backup_rec.sha256);
                                match std::fs::read(&blob_file) {
                                    Ok(orig_bytes) => {
                                        let injected_fail = self
                                            .fault_inject_compensation_write
                                            .load(std::sync::atomic::Ordering::SeqCst);
                                        if injected_fail {
                                            rollback_errors.push(format!(
                                                "Injected compensation write failure for '{}'",
                                                committed.relative_path
                                            ));
                                        } else {
                                            match cap.replace_file_bytes_conditional_with_hooks(
                                                &committed.relative_path,
                                                &orig_bytes,
                                                Some(&committed.restored_sha256),
                                                ConditionalReplaceHooks::default(),
                                            ) {
                                                Ok(outcome) => {
                                                    rolled_back_count += 1;
                                                    compensated_files
                                                        .push(committed.relative_path.clone());
                                                    if let Some(surviving) =
                                                        outcome.preserved_backup
                                                    {
                                                        preserved_backups
                                                            .push(surviving.display().to_string());
                                                    }
                                                }
                                                Err(e) => {
                                                    rollback_errors.push(format!(
                                                        "Failed to rollback '{}': {}",
                                                        committed.relative_path, e
                                                    ));
                                                }
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        rollback_errors.push(format!(
                                            "Failed to read backup blob for '{}': {}",
                                            committed.relative_path, e
                                        ));
                                    }
                                }
                            }
                            Some(actual_hash) => {
                                let msg = format!(
                                    "File '{}' modified after restore commit (expected {}, found {}). Concurrent modification preserved in workspace; not rolled back.",
                                    committed.relative_path, committed.restored_sha256, actual_hash
                                );
                                warn!("{}", msg);
                                recovery_conflicts.push(msg);
                            }
                            None => {
                                let msg = format!(
                                    "File '{}' could not be read during rollback verification. File preserved in workspace without overwrite.",
                                    committed.relative_path
                                );
                                warn!("{}", msg);
                                recovery_conflicts.push(msg);
                            }
                        }
                    } else {
                        // File was newly created by this restore.
                        // Capability-anchored conditional deletion!
                        let pre_del_hook_opt = self.pre_delete_hook.lock().clone();
                        let pre_del_ref = pre_del_hook_opt
                            .as_ref()
                            .map(|h| h.as_ref() as &dyn Fn(&Path));
                        let post_hash_del_hook_opt = self.post_hash_delete_hook.lock().clone();
                        let post_hash_del_ref = post_hash_del_hook_opt
                            .as_ref()
                            .map(|h| h.as_ref() as &dyn Fn(&Path));
                        let del_hooks = filesystem_engine::ConditionalDeleteHooks {
                            pre_delete_hook: pre_del_ref,
                            post_hash_hook: post_hash_del_ref,
                            fault_inject_delete_removal: self
                                .fault_inject_delete_removal
                                .load(std::sync::atomic::Ordering::SeqCst),
                            fault_inject_rollback_failure: false,
                        };

                        match cap.delete_file_conditional_with_hooks(
                            &committed.relative_path,
                            &committed.restored_sha256,
                            del_hooks,
                        ) {
                            Ok(outcome) => {
                                if outcome.was_deleted {
                                    rolled_back_count += 1;
                                    compensated_files.push(committed.relative_path.clone());
                                } else if let Some(surviving) = outcome.surviving_path {
                                    let msg = format!(
                                        "Newly created file '{}' modified after restore (or staging preserved at '{}'). File preserved in workspace without deletion.",
                                        committed.relative_path, surviving.display()
                                    );
                                    warn!("{}", msg);
                                    recovery_conflicts.push(msg);
                                    preserved_backups.push(surviving.display().to_string());
                                } else {
                                    let msg = format!(
                                        "Newly created file '{}' modified after restore. File preserved in workspace without deletion.",
                                        committed.relative_path
                                    );
                                    warn!("{}", msg);
                                    recovery_conflicts.push(msg);
                                }
                            }
                            Err(e) => {
                                rollback_errors.push(format!(
                                    "Failed to conditionally delete newly created file '{}': {}",
                                    committed.relative_path, e
                                ));
                            }
                        }
                    }
                }
            }
        }

        if rollback_errors.is_empty() && recovery_conflicts.is_empty() {
            Err(AetherError::Internal(format!(
                "Restore aborted: {}. Compensating rollback succeeded: all {} previously modified files were cleanly rolled back to pre-restore safety checkpoint '{}'. Preserved backups: {:?}.",
                orig_err, rolled_back_count, backup_ckpt.checkpoint_id, preserved_backups
            )))
        } else {
            Err(AetherError::Internal(format!(
                "Restore aborted: {}. Compensating rollback encountered conflicts/errors: {}/{} files cleanly rolled back. Compensated files: {:?}. Files requiring manual recovery (conflicts): {:?}. Rollback errors: {:?}. Preserved backups: {:?}. Mandatory safety checkpoint '{}' is preserved in storage for manual recovery.",
                orig_err,
                rolled_back_count,
                committed_files.len(),
                compensated_files,
                recovery_conflicts,
                rollback_errors,
                preserved_backups,
                backup_ckpt.checkpoint_id
            )))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_legacy_restore_checkpoint_strictly_prohibited() {
        let temp_dir = std::env::temp_dir().join(format!(
            "aether_legacy_prohibit_1_{}",
            uuid::Uuid::new_v4().simple()
        ));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        let engine = CheckpointEngine::new(&storage_dir);
        let dummy_ckpt = WorkspaceCheckpoint {
            checkpoint_id: "ckpt-legacy-1".to_string(),
            workspace_id: "ws-legacy".to_string(),
            description: "legacy".to_string(),
            created_at: "2026-11-05T00:00:00Z".to_string(),
            manifest_version: 1,
            roots: Vec::new(),
            files: Vec::new(),
            file_hashes: HashMap::new(),
        };
        #[allow(deprecated)]
        let res = engine.restore_checkpoint(&dummy_ckpt);
        assert!(res.is_err());
        assert!(matches!(
            res.err().unwrap(),
            AetherError::PermissionDenied(_)
        ));
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_legacy_restore_checkpoint_into_strictly_prohibited() {
        let temp_dir = std::env::temp_dir().join(format!(
            "aether_legacy_prohibit_2_{}",
            uuid::Uuid::new_v4().simple()
        ));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        let workspace_dir = temp_dir.join("workspace");
        std::fs::create_dir_all(&workspace_dir).unwrap();
        let engine = CheckpointEngine::new(&storage_dir);
        #[allow(deprecated)]
        let res = engine.restore_checkpoint_into("ckpt-legacy-2", &workspace_dir, true);
        assert!(res.is_err());
        assert!(matches!(
            res.err().unwrap(),
            AetherError::PermissionDenied(_)
        ));
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

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
            .restore_preview_bound(&ckpt.checkpoint_id, "ws-test", &workspace_dir)
            .expect("Preview failed");
        assert!(preview.integrity_valid);
        assert!(preview.conflicts.is_empty());

        // Modify file1 on disk
        std::fs::write(&file1, "Modified content").unwrap();

        let preview_after = engine
            .restore_preview_bound(&ckpt.checkpoint_id, "ws-test", &workspace_dir)
            .expect("Preview failed");
        assert_eq!(preview_after.conflicts.len(), 1);
        assert_eq!(preview_after.conflicts[0], "hello.txt");

        // Restore snapshot with force=false and no explicit overwrite authorization must abort
        let unapproved_res = engine.restore_checkpoint_with_token(
            &ckpt.checkpoint_id,
            "ws-test",
            &workspace_dir,
            &preview_after.preview_token,
            false,
            &[],
        );
        assert!(
            unapproved_res.is_err(),
            "Default restore must abort on unresolved conflicts"
        );

        // Re-generate fresh preview token for explicit authorization (since prior token was consumed)
        let fresh_preview = engine
            .restore_preview_bound(&ckpt.checkpoint_id, "ws-test", &workspace_dir)
            .expect("Fresh preview failed");

        // Restore snapshot with explicit authorization (force=true)
        let report = engine
            .restore_checkpoint_with_token(
                &ckpt.checkpoint_id,
                "ws-test",
                &workspace_dir,
                &fresh_preview.preview_token,
                true,
                &[],
            )
            .expect("Restore with explicit authorization failed");

        assert!(report.success);
        assert_eq!(report.restored_count, 2);
        assert!(
            report.backup_checkpoint_id.is_some(),
            "Mandatory safety backup must be created and reported"
        );

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

        // Restore with preview and token
        let preview = engine
            .restore_preview_bound(&ckpt.checkpoint_id, "ws-bin", &workspace_dir)
            .expect("Preview failed");

        let report = engine
            .restore_checkpoint_with_token(
                &ckpt.checkpoint_id,
                "ws-bin",
                &workspace_dir,
                &preview.preview_token,
                true,
                &[],
            )
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
            .restore_preview_bound(&ckpt.checkpoint_id, "ws-race", &workspace_dir)
            .expect("Preview failed");
        assert_eq!(preview.conflicts.len(), 1);

        // Concurrent process mutates file on disk AFTER preview was generated!
        std::fs::write(&file, "version 3 (concurrent edit)").unwrap();

        // User attempts to restore with preview token
        let res = engine.restore_checkpoint_with_token(
            &ckpt.checkpoint_id,
            "ws-race",
            &workspace_dir,
            &preview.preview_token,
            true,
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

        let res = engine.restore_preview_bound(&ckpt.checkpoint_id, "ws-sec", &workspace_dir);
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
            "ws-del",
            &workspace_dir,
            &preview.preview_token,
            true,
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
            "ws-new",
            &workspace_dir,
            &preview.preview_token,
            true,
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

    #[test]
    fn test_chk_014_toctou_modification_during_restore_aborted() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_014_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        let workspace_dir = temp_dir.join("workspace");
        std::fs::create_dir_all(&workspace_dir).unwrap();

        let target = workspace_dir.join("toctou_mod.txt");
        std::fs::write(&target, "initial content").unwrap();

        let engine = CheckpointEngine::new(&storage_dir);
        let ckpt = engine
            .create_checkpoint_with_root("TOCTOU mod test", "ws-toctou", &workspace_dir, None)
            .unwrap();

        let preview = engine
            .restore_preview_bound(&ckpt.checkpoint_id, "ws-toctou", &workspace_dir)
            .unwrap();

        // Modify file after preview token was generated
        std::fs::write(&target, "unrecorded modifications").unwrap();

        let res = engine.restore_checkpoint_with_token(
            &ckpt.checkpoint_id,
            "ws-toctou",
            &workspace_dir,
            &preview.preview_token,
            true,
            &[],
        );

        assert!(
            res.is_err(),
            "Modified file must be rejected by TOCTOU check"
        );
        let err = res.err().unwrap().to_string();
        assert!(
            err.contains("was modified on disk after preview was generated"),
            "Unexpected error: {}",
            err
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_015_toctou_deletion_during_restore_aborted() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_015_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        let workspace_dir = temp_dir.join("workspace");
        std::fs::create_dir_all(&workspace_dir).unwrap();

        let target = workspace_dir.join("toctou_del.txt");
        std::fs::write(&target, "content to delete").unwrap();

        let engine = CheckpointEngine::new(&storage_dir);
        let ckpt = engine
            .create_checkpoint_with_root("TOCTOU del test", "ws-del-toctou", &workspace_dir, None)
            .unwrap();

        let preview = engine
            .restore_preview_bound(&ckpt.checkpoint_id, "ws-del-toctou", &workspace_dir)
            .unwrap();

        // Delete file after preview was generated
        std::fs::remove_file(&target).unwrap();

        let res = engine.restore_checkpoint_with_token(
            &ckpt.checkpoint_id,
            "ws-del-toctou",
            &workspace_dir,
            &preview.preview_token,
            true,
            &[],
        );

        assert!(
            res.is_err(),
            "Deleted file must be rejected by TOCTOU check"
        );
        let err = res.err().unwrap().to_string();
        assert!(
            err.contains("was deleted after preview was generated"),
            "Unexpected error: {}",
            err
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_016_toctou_new_file_creation_aborted() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_016_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        let workspace_dir = temp_dir.join("workspace");
        std::fs::create_dir_all(&workspace_dir).unwrap();

        let target = workspace_dir.join("toctou_new.txt");
        std::fs::write(&target, "in snapshot").unwrap();

        let engine = CheckpointEngine::new(&storage_dir);
        let ckpt = engine
            .create_checkpoint_with_root("TOCTOU new test", "ws-new-toctou", &workspace_dir, None)
            .unwrap();

        // Remove it so preview sees FileDiskState::Missing
        std::fs::remove_file(&target).unwrap();

        let preview = engine
            .restore_preview_bound(&ckpt.checkpoint_id, "ws-new-toctou", &workspace_dir)
            .unwrap();

        // Recreate it right before restore
        std::fs::write(&target, "created right before restore").unwrap();

        let res = engine.restore_checkpoint_with_token(
            &ckpt.checkpoint_id,
            "ws-new-toctou",
            &workspace_dir,
            &preview.preview_token,
            true,
            &[],
        );

        assert!(res.is_err(), "Newly created file must be rejected");
        let err = res.err().unwrap().to_string();
        assert!(
            err.contains("newly created on disk after preview was generated"),
            "Unexpected error: {}",
            err
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_017_toctou_symlink_or_directory_replacement_aborted() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_017_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        let workspace_dir = temp_dir.join("workspace");
        std::fs::create_dir_all(&workspace_dir).unwrap();

        let target = workspace_dir.join("toctou_obj.txt");
        std::fs::write(&target, "regular file").unwrap();

        let engine = CheckpointEngine::new(&storage_dir);
        let ckpt = engine
            .create_checkpoint_with_root("TOCTOU dir test", "ws-dir-toctou", &workspace_dir, None)
            .unwrap();

        let preview = engine
            .restore_preview_bound(&ckpt.checkpoint_id, "ws-dir-toctou", &workspace_dir)
            .unwrap();

        // Remove regular file and replace with directory
        std::fs::remove_file(&target).unwrap();
        std::fs::create_dir(&target).unwrap();

        let res = engine.restore_checkpoint_with_token(
            &ckpt.checkpoint_id,
            "ws-dir-toctou",
            &workspace_dir,
            &preview.preview_token,
            true,
            &[],
        );

        assert!(res.is_err(), "Directory replacement must be rejected");
        let err = res.err().unwrap().to_string();
        assert!(
            err.contains("is no longer a regular file")
                || err.contains("was replaced with a directory"),
            "Unexpected error: {}",
            err
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_019_missing_state_rejects_inaccessible_and_non_notfound() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_019_{}", uuid::Uuid::new_v4().simple()));
        std::fs::create_dir_all(&temp_dir).unwrap();

        let existing_file = temp_dir.join("exists.txt");
        std::fs::write(&existing_file, "i exist").unwrap();

        let existing_dir = temp_dir.join("sub_dir");
        std::fs::create_dir(&existing_dir).unwrap();

        // 1. Missing state expected, but an actual regular file exists -> must reject
        let res_file = CheckpointEngine::verify_file_disk_state(
            &existing_file,
            "exists.txt",
            &FileDiskState::Missing,
        );
        assert!(res_file.is_err());
        assert!(res_file
            .err()
            .unwrap()
            .to_string()
            .contains("newly created on disk"));

        // 2. Missing state expected, but a directory exists -> must reject
        let res_dir = CheckpointEngine::verify_file_disk_state(
            &existing_dir,
            "sub_dir",
            &FileDiskState::Missing,
        );
        assert!(res_dir.is_err());
        assert!(res_dir
            .err()
            .unwrap()
            .to_string()
            .contains("directory or non-file object"));

        // 3. Inaccessible state recorded during preview -> must reject immediately fail-closed
        let res_inaccessible = CheckpointEngine::verify_file_disk_state(
            &existing_file,
            "exists.txt",
            &FileDiskState::Inaccessible {
                reason: "Destination was a symlink".into(),
            },
        );
        assert!(res_inaccessible.is_err());
        assert!(res_inaccessible
            .err()
            .unwrap()
            .to_string()
            .contains("was inaccessible during preview generation"));

        // 4. Truly absent file -> must succeed
        let non_existent = temp_dir.join("non_existent.txt");
        let res_ok = CheckpointEngine::verify_file_disk_state(
            &non_existent,
            "non_existent.txt",
            &FileDiskState::Missing,
        );
        assert!(res_ok.is_ok());

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_020_token_binding_mismatches_and_concurrency() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_020_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        let workspace_dir = temp_dir.join("workspace");
        let other_workspace_dir = temp_dir.join("other_workspace");
        std::fs::create_dir_all(&workspace_dir).unwrap();
        std::fs::create_dir_all(&other_workspace_dir).unwrap();

        let file = workspace_dir.join("bound.txt");
        std::fs::write(&file, "bound content").unwrap();

        let engine = std::sync::Arc::new(CheckpointEngine::new(&storage_dir));
        let ckpt = engine
            .create_checkpoint_with_root("Binding test", "ws-correct", &workspace_dir, None)
            .unwrap();

        let preview = engine
            .restore_preview_bound(&ckpt.checkpoint_id, "ws-correct", &workspace_dir)
            .unwrap();
        let token = preview.preview_token.clone();

        // 1. Wrong checkpoint ID rejected
        let res_wrong_ckpt = engine.consume_preview_token(
            &token,
            "wrong-checkpoint-id",
            "ws-correct",
            &workspace_dir,
        );
        assert!(res_wrong_ckpt.is_err());
        assert!(res_wrong_ckpt
            .err()
            .unwrap()
            .to_string()
            .contains("checkpoint mismatch"));

        // 2. Wrong workspace ID rejected
        let res_wrong_ws = engine.consume_preview_token(
            &token,
            &ckpt.checkpoint_id,
            "wrong-workspace-id",
            &workspace_dir,
        );
        assert!(res_wrong_ws.is_err());
        assert!(res_wrong_ws
            .err()
            .unwrap()
            .to_string()
            .contains("workspace mismatch"));

        // 3. Wrong root directory rejected
        let res_wrong_root = engine.consume_preview_token(
            &token,
            &ckpt.checkpoint_id,
            "ws-correct",
            &other_workspace_dir,
        );
        assert!(res_wrong_root.is_err());
        assert!(res_wrong_root
            .err()
            .unwrap()
            .to_string()
            .contains("root directory mismatch"));

        // 4. Concurrent consumption: spawn 10 threads racing to consume the same preview token
        let mut handles = Vec::new();
        let success_count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let error_count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));

        for _ in 0..10 {
            let engine_clone = std::sync::Arc::clone(&engine);
            let token_clone = token.clone();
            let ckpt_id = ckpt.checkpoint_id.clone();
            let ws_path = workspace_dir.clone();
            let succ_clone = std::sync::Arc::clone(&success_count);
            let err_clone = std::sync::Arc::clone(&error_count);

            handles.push(std::thread::spawn(move || {
                let res = engine_clone.consume_preview_token(
                    &token_clone,
                    &ckpt_id,
                    "ws-correct",
                    &ws_path,
                );
                if res.is_ok() {
                    succ_clone.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                } else {
                    err_clone.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                }
            }));
        }

        for h in handles {
            h.join().unwrap();
        }

        // Exactly 1 thread must succeed; all other 9 must be rejected fail-closed
        assert_eq!(
            success_count.load(std::sync::atomic::Ordering::SeqCst),
            1,
            "Exactly one concurrent consumer must succeed"
        );
        assert_eq!(
            error_count.load(std::sync::atomic::Ordering::SeqCst),
            9,
            "All other concurrent consumers must be rejected"
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_021_mandatory_safety_snapshot_failure_aborts_without_mutations() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_021_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        let workspace_dir = temp_dir.join("workspace");
        std::fs::create_dir_all(&workspace_dir).unwrap();

        let file = workspace_dir.join("app.js");
        std::fs::write(&file, "console.log('original');").unwrap();

        let engine = CheckpointEngine::new(&storage_dir);
        let ckpt = engine
            .create_checkpoint_with_root("Safety abort test", "ws-abort", &workspace_dir, None)
            .unwrap();

        // Mutate file on disk so restoration would normally overwrite it
        std::fs::write(&file, "console.log('uncommitted user work');").unwrap();

        let preview = engine
            .restore_preview_bound(&ckpt.checkpoint_id, "ws-abort", &workspace_dir)
            .unwrap();

        // Enable simulated backup failure hook
        engine.set_simulate_backup_failure(true);

        // Attempt restore
        let res = engine.restore_checkpoint_with_token(
            &ckpt.checkpoint_id,
            "ws-abort",
            &workspace_dir,
            &preview.preview_token,
            true,
            &[],
        );

        // Restoration MUST fail because pre-restore safety backup failed
        assert!(res.is_err());
        let err_msg = res.err().unwrap().to_string();
        assert!(
            err_msg.contains("Mandatory pre-restore safety backup failed"),
            "Error must report mandatory backup failure: {}",
            err_msg
        );

        // User's uncommitted work MUST be completely untouched!
        let disk_content = std::fs::read_to_string(&file).unwrap();
        assert_eq!(
            disk_content, "console.log('uncommitted user work');",
            "Target file must not be modified if mandatory safety backup fails"
        );

        // Disable simulation hook
        engine.set_simulate_backup_failure(false);

        // Obtain fresh preview (since previous token was consumed)
        let fresh_preview = engine
            .restore_preview_bound(&ckpt.checkpoint_id, "ws-abort", &workspace_dir)
            .unwrap();

        // Now restore succeeds
        let report = engine
            .restore_checkpoint_with_token(
                &ckpt.checkpoint_id,
                "ws-abort",
                &workspace_dir,
                &fresh_preview.preview_token,
                true,
                &[],
            )
            .expect("Restore must succeed when safety backup succeeds");

        assert!(report.success);
        assert!(report.backup_checkpoint_id.is_some());

        // File is now restored to original
        let restored_content = std::fs::read_to_string(&file).unwrap();
        assert_eq!(restored_content, "console.log('original');");

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_022_actual_filesystem_failure_during_safety_backup_aborts_without_mutations() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_022_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        let workspace_dir = temp_dir.join("workspace");
        std::fs::create_dir_all(&workspace_dir).unwrap();

        let file = workspace_dir.join("actual_io_fail.txt");
        std::fs::write(&file, "initial committed state").unwrap();

        let mut engine = CheckpointEngine::new(&storage_dir);
        let ckpt = engine
            .create_checkpoint_with_root("Real I/O test", "ws-real-io", &workspace_dir, None)
            .unwrap();

        // User makes uncommitted edits on disk
        std::fs::write(&file, "precious uncommitted edits").unwrap();

        let preview = engine
            .restore_preview_bound(&ckpt.checkpoint_id, "ws-real-io", &workspace_dir)
            .unwrap();

        // Create an unwritable storage target: a regular file blocking directory creation
        let blocking_file = temp_dir.join("blocked_storage");
        std::fs::write(&blocking_file, b"this file blocks directory creation").unwrap();
        let unwritable_storage_dir = blocking_file.join("sub_checkpoints");

        // Point engine to unwritable storage location to induce an actual OS-level I/O failure
        engine.set_storage_dir(unwritable_storage_dir);

        let res = engine.restore_checkpoint_with_token(
            &ckpt.checkpoint_id,
            "ws-real-io",
            &workspace_dir,
            &preview.preview_token,
            true,
            &[],
        );

        // Actual OS filesystem failure during safety backup must abort restore
        assert!(
            res.is_err(),
            "Actual filesystem I/O error must abort restore"
        );
        let err_msg = res.err().unwrap().to_string();
        assert!(
            err_msg.contains("Mandatory pre-restore safety backup failed"),
            "Error must identify safety backup failure: {}",
            err_msg
        );

        // Crucial invariant: user uncommitted data MUST be completely untouched!
        let content = std::fs::read_to_string(&file).unwrap();
        assert_eq!(
            content, "precious uncommitted edits",
            "Target file must not be modified when real storage I/O fails"
        );

        // Resolve storage failure: point back to valid storage directory
        engine.set_storage_dir(storage_dir);

        // Generate fresh preview and retry restore
        let fresh_preview = engine
            .restore_preview_bound(&ckpt.checkpoint_id, "ws-real-io", &workspace_dir)
            .unwrap();

        let report = engine
            .restore_checkpoint_with_token(
                &ckpt.checkpoint_id,
                "ws-real-io",
                &workspace_dir,
                &fresh_preview.preview_token,
                true,
                &[],
            )
            .expect("Restore must succeed once storage failure is resolved");

        assert!(report.success);
        assert!(report.backup_checkpoint_id.is_some());

        // File is now restored to initial snapshot version
        let restored_content = std::fs::read_to_string(&file).unwrap();
        assert_eq!(restored_content, "initial committed state");

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_023_deterministic_toctou_mutation_between_validation_and_replacement_aborted() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_023_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        let workspace_dir = temp_dir.join("workspace");
        std::fs::create_dir_all(&workspace_dir).unwrap();

        let target_file = workspace_dir.join("toctou_race_target.txt");
        std::fs::write(&target_file, "original version 1.0").unwrap();

        let engine = CheckpointEngine::new(&storage_dir);
        let ckpt = engine
            .create_checkpoint_with_root("TOCTOU race test", "ws-toctou-race", &workspace_dir, None)
            .unwrap();

        let preview = engine
            .restore_preview_bound(&ckpt.checkpoint_id, "ws-toctou-race", &workspace_dir)
            .unwrap();

        // Inject a deterministic hook that mutates the file in the exact interval between validation and replacement!
        engine.set_pre_replace_hook(Some(|dest: &Path| {
            if dest.file_name() == Some(std::ffi::OsStr::new("toctou_race_target.txt")) {
                // Mutate the destination file right before replacement occurs!
                std::fs::write(dest, "adversarial concurrent write in TOCTOU window").unwrap();
            }
        }));

        let res = engine.restore_checkpoint_with_token(
            &ckpt.checkpoint_id,
            "ws-toctou-race",
            &workspace_dir,
            &preview.preview_token,
            true,
            &[],
        );

        // The staged conditional replacement must detect that the captured file was modified in the TOCTOU window!
        assert!(
            res.is_err(),
            "Restore must abort when target file is modified in TOCTOU window"
        );
        let err_msg = res.err().unwrap().to_string();
        assert!(
            err_msg.contains("Restore race detected") && err_msg.contains("prior to replacement"),
            "Error must identify TOCTOU race detection: {}",
            err_msg
        );

        // Crucial invariant: the rollback MUST restore the concurrent modification intact!
        let surviving_content = std::fs::read_to_string(&target_file).unwrap();
        assert_eq!(
            surviving_content, "adversarial concurrent write in TOCTOU window",
            "Concurrent modification in TOCTOU window must NOT be silently overwritten!"
        );

        // Clear hook
        engine.set_pre_replace_hook::<fn(&Path)>(None);

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_024_multi_file_restore_failure_triggers_compensating_rollback() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_024_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        let workspace_dir = temp_dir.join("workspace");
        std::fs::create_dir_all(&workspace_dir).unwrap();

        // 1. Create 3 files in workspace and commit them to checkpoint
        let f1 = workspace_dir.join("file1.txt");
        let f2 = workspace_dir.join("file2.txt");
        let f3 = workspace_dir.join("file3.txt");

        std::fs::write(&f1, "file1 original base").unwrap();
        std::fs::write(&f2, "file2 original base").unwrap();
        std::fs::write(&f3, "file3 original base").unwrap();

        let engine = CheckpointEngine::new(&storage_dir);
        let ckpt = engine
            .create_checkpoint_with_root("Multi-file test", "ws-multi", &workspace_dir, None)
            .unwrap();

        // 2. User edits all 3 files before restore
        std::fs::write(&f1, "file1 user uncommitted edit").unwrap();
        std::fs::write(&f2, "file2 user uncommitted edit").unwrap();
        std::fs::write(&f3, "file3 user uncommitted edit").unwrap();

        let preview = engine
            .restore_preview_bound(&ckpt.checkpoint_id, "ws-multi", &workspace_dir)
            .unwrap();

        // 3. Inject a failure on the 3rd file using pre_replace_hook:
        // When processing file3.txt, mutate it on disk right before replacement, causing a TOCTOU race detection!
        engine.set_pre_replace_hook(Some(|dest: &Path| {
            if dest.file_name() == Some(std::ffi::OsStr::new("file3.txt")) {
                std::fs::write(dest, "file3 adversarial mutation to cause failure").unwrap();
            }
        }));

        let res = engine.restore_checkpoint_with_token(
            &ckpt.checkpoint_id,
            "ws-multi",
            &workspace_dir,
            &preview.preview_token,
            true,
            &[],
        );

        // 4. Verification:
        // Must abort, and trigger compensating rollback for file1 and file2 back to the pre-restore safety snapshot!
        assert!(
            res.is_err(),
            "Multi-file restore must abort when a file fails"
        );
        let err_msg = res.err().unwrap().to_string();
        assert!(
            err_msg.contains("Compensating rollback succeeded")
                || err_msg.contains("pre-restore safety checkpoint"),
            "Error must detail compensating rollback. Got: {}",
            err_msg
        );

        // Files 1 and 2 must be restored back to the user's uncommitted edit state captured in the safety checkpoint!
        assert_eq!(
            std::fs::read_to_string(&f1).unwrap(),
            "file1 user uncommitted edit",
            "file1 must be rolled back to pre-restore safety state!"
        );
        assert_eq!(
            std::fs::read_to_string(&f2).unwrap(),
            "file2 user uncommitted edit",
            "file2 must be rolled back to pre-restore safety state!"
        );

        engine.set_pre_replace_hook::<fn(&Path)>(None);
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_025_post_hash_modification_via_open_handle_triggers_rollback() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_025_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        let workspace_dir = temp_dir.join("workspace");
        std::fs::create_dir_all(&workspace_dir).unwrap();

        let live_file = workspace_dir.join("live_editor_file.txt");
        std::fs::write(&live_file, "committed base 1.0").unwrap();

        let engine = CheckpointEngine::new(&storage_dir);
        let ckpt = engine
            .create_checkpoint_with_root(
                "Post-hash open handle test",
                "ws-post-hash",
                &workspace_dir,
                None,
            )
            .unwrap();

        // User uncommitted state
        std::fs::write(&live_file, "uncommitted editor text 2.0").unwrap();

        let preview = engine
            .restore_preview_bound(&ckpt.checkpoint_id, "ws-post-hash", &workspace_dir)
            .unwrap();

        // Open handle and keep it open
        let handle = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&live_file)
            .unwrap();
        let handle_cell = std::sync::Mutex::new(Some(handle));

        // Inject post-hash hook: writes into open handle right after initial hash check
        engine.set_post_hash_hook(Some(move |_dest: &Path| {
            let mut lock = handle_cell.lock().unwrap();
            if let Some(ref mut h) = *lock {
                use std::io::{Seek, Write};
                h.seek(std::io::SeekFrom::Start(0)).unwrap();
                h.write_all(b"MUTATION THROUGH OPEN HANDLE POST HASH CHECK!")
                    .unwrap();
                h.set_len(b"MUTATION THROUGH OPEN HANDLE POST HASH CHECK!".len() as u64)
                    .unwrap();
                h.flush().unwrap();
            }
        }));

        let res = engine.restore_checkpoint_with_token(
            &ckpt.checkpoint_id,
            "ws-post-hash",
            &workspace_dir,
            &preview.preview_token,
            true,
            &[],
        );

        assert!(
            res.is_err(),
            "Must abort when open handle mutates file post-hash check"
        );
        let err_msg = res.err().unwrap().to_string();
        assert!(
            err_msg.contains("modified via open handle after hash verification"),
            "Error must detail open handle modification. Got: {}",
            err_msg
        );

        // Crucial invariant: concurrent edit from open handle is preserved at destination!
        let content = std::fs::read_to_string(&live_file).unwrap();
        assert_eq!(
            content, "MUTATION THROUGH OPEN HANDLE POST HASH CHECK!",
            "Open handle concurrent edit must be preserved intact on disk!"
        );

        engine.set_post_hash_hook::<fn(&Path)>(None);
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_028_post_write_verification_failure_includes_current_file_in_compensation() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_028_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        let workspace_dir = temp_dir.join("workspace");
        std::fs::create_dir_all(&workspace_dir).unwrap();

        let file = workspace_dir.join("target_file.txt");
        std::fs::write(&file, "historical base content v1").unwrap();

        let engine = CheckpointEngine::new(&storage_dir);
        let ckpt = engine
            .create_checkpoint_with_root("Chk028 base", "ws-chk028", &workspace_dir, None)
            .unwrap();

        // User uncommitted edit prior to restore
        std::fs::write(&file, "user uncommitted pre-restore edit v2").unwrap();

        let preview = engine
            .restore_preview_bound(&ckpt.checkpoint_id, "ws-chk028", &workspace_dir)
            .unwrap();

        // Inject fault: post-write verification will fail after replacement file commits
        engine.set_fault_inject_post_write_verification(true);

        let res = engine.restore_checkpoint_with_token(
            &ckpt.checkpoint_id,
            "ws-chk028",
            &workspace_dir,
            &preview.preview_token,
            true,
            &[],
        );

        assert!(
            res.is_err(),
            "Restore must abort when post-write verification fails"
        );
        let err_msg = res.err().unwrap().to_string();
        assert!(
            err_msg.contains("Compensating rollback succeeded: all 1 previously modified files were cleanly rolled back"),
            "Current file must be included in compensating rollback! Got: {}",
            err_msg
        );

        // Verify on disk: file was safely rolled back to pre-restore safety checkpoint state!
        let content_on_disk = std::fs::read_to_string(&file).unwrap();
        assert_eq!(
            content_on_disk, "user uncommitted pre-restore edit v2",
            "File must be rolled back to pre-restore state, proving it was included in compensation"
        );

        engine.set_fault_inject_post_write_verification(false);
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_029_new_file_compensation_deleted_only_if_unchanged_and_preserved_if_modified() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_029_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        let workspace_dir = temp_dir.join("workspace");
        std::fs::create_dir_all(&workspace_dir).unwrap();

        let f_existing = workspace_dir.join("existing.txt");
        let f_new = workspace_dir.join("new_file.txt");
        let f_fail = workspace_dir.join("fail_file.txt");

        std::fs::write(&f_existing, "existing v1").unwrap();
        std::fs::write(&f_new, "new file payload v1").unwrap();
        std::fs::write(&f_fail, "fail target v1").unwrap();

        let engine = CheckpointEngine::new(&storage_dir);
        let ckpt = engine
            .create_checkpoint_with_root("Chk029 base", "ws-chk029", &workspace_dir, None)
            .unwrap();

        // 1. Subcase A: new_file is untouched -> deleted upon compensation
        let _ = std::fs::remove_file(&f_new); // remove from workspace so it's a "new file" on restore

        let preview_a = engine
            .restore_preview_bound(&ckpt.checkpoint_id, "ws-chk029", &workspace_dir)
            .unwrap();

        // Trigger failure on f_fail
        engine.set_pre_replace_hook(Some(|dest: &Path| {
            if dest.file_name() == Some(std::ffi::OsStr::new("fail_file.txt")) {
                std::fs::write(dest, "adversarial mutation to trigger failure").unwrap();
            }
        }));

        let res_a = engine.restore_checkpoint_with_token(
            &ckpt.checkpoint_id,
            "ws-chk029",
            &workspace_dir,
            &preview_a.preview_token,
            true,
            &[],
        );
        assert!(res_a.is_err());

        // f_new was untouched -> must be deleted cleanly by compensating rollback
        assert!(
            !f_new.exists(),
            "Untouched restored new file must be deleted during compensation!"
        );

        // 2. Subcase B: new_file is modified after restore -> preserved, NOT deleted!
        let preview_b = engine
            .restore_preview_bound(&ckpt.checkpoint_id, "ws-chk029", &workspace_dir)
            .unwrap();

        // Set post_write_hook to mutate f_new on disk after it is written
        engine.set_post_write_hook(Some(|dest: &Path| {
            if dest.file_name() == Some(std::ffi::OsStr::new("new_file.txt")) {
                std::fs::write(dest, "concurrent edit to newly created file").unwrap();
            }
        }));

        let res_b = engine.restore_checkpoint_with_token(
            &ckpt.checkpoint_id,
            "ws-chk029",
            &workspace_dir,
            &preview_b.preview_token,
            true,
            &[],
        );
        assert!(res_b.is_err());
        let err_b = res_b.err().unwrap().to_string();
        assert!(
            err_b.contains("Files requiring manual recovery"),
            "Must report conflict for modified new file: {}",
            err_b
        );
        assert!(
            err_b.contains("File preserved in workspace without deletion"),
            "Error must state file was preserved without deletion: {}",
            err_b
        );

        // Assert on-disk: f_new must STILL EXIST and retain the concurrent edits!
        assert!(
            f_new.exists(),
            "Modified restored new file must NOT be deleted!"
        );
        assert_eq!(
            std::fs::read_to_string(&f_new).unwrap(),
            "concurrent edit to newly created file"
        );

        engine.set_pre_replace_hook::<fn(&Path)>(None);
        engine.set_post_write_hook::<fn(&Path)>(None);
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_030_external_modification_before_compensation_is_preserved_as_conflict() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_030_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        let workspace_dir = temp_dir.join("workspace");
        std::fs::create_dir_all(&workspace_dir).unwrap();

        let f1 = workspace_dir.join("file1.txt");
        let f2 = workspace_dir.join("file2.txt");
        std::fs::write(&f1, "file1 base v1").unwrap();
        std::fs::write(&f2, "file2 base v1").unwrap();

        let engine = CheckpointEngine::new(&storage_dir);
        let ckpt = engine
            .create_checkpoint_with_root("Chk030 base", "ws-chk030", &workspace_dir, None)
            .unwrap();

        // Workspace files get uncommitted changes
        std::fs::write(&f1, "file1 user uncommitted").unwrap();
        std::fs::write(&f2, "file2 user uncommitted").unwrap();

        let preview = engine
            .restore_preview_bound(&ckpt.checkpoint_id, "ws-chk030", &workspace_dir)
            .unwrap();

        // When f1 is restored, mutate it externally to simulate external process edit
        engine.set_post_write_hook(Some(|dest: &Path| {
            if dest.file_name() == Some(std::ffi::OsStr::new("file1.txt")) {
                std::fs::write(dest, "concurrent external process edit v3").unwrap();
            }
        }));

        // Cause f2 to fail
        engine.set_pre_replace_hook(Some(|dest: &Path| {
            if dest.file_name() == Some(std::ffi::OsStr::new("file2.txt")) {
                std::fs::write(dest, "mutation on f2 to abort restore").unwrap();
            }
        }));

        let res = engine.restore_checkpoint_with_token(
            &ckpt.checkpoint_id,
            "ws-chk030",
            &workspace_dir,
            &preview.preview_token,
            true,
            &[],
        );

        assert!(res.is_err());
        let err_msg = res.err().unwrap().to_string();
        assert!(
            err_msg.contains("Files requiring manual recovery"),
            "Error must report conflict: {}",
            err_msg
        );
        assert!(
            err_msg.contains("Concurrent modification preserved in workspace; not rolled back"),
            "Error must announce concurrent modification preservation: {}",
            err_msg
        );

        // Invariant: file1.txt on disk must NOT be blindly overwritten!
        // It must retain "concurrent external process edit v3"!
        let f1_disk = std::fs::read_to_string(&f1).unwrap();
        assert_eq!(
            f1_disk, "concurrent external process edit v3",
            "Concurrent external edit must be preserved without blind overwrite!"
        );

        engine.set_pre_replace_hook::<fn(&Path)>(None);
        engine.set_post_write_hook::<fn(&Path)>(None);
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_031_compensating_rollback_failure_reports_explicit_recovery_and_preserves_safety_snapshot(
    ) {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_031_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        let workspace_dir = temp_dir.join("workspace");
        std::fs::create_dir_all(&workspace_dir).unwrap();

        let f1 = workspace_dir.join("critical1.txt");
        let f2 = workspace_dir.join("critical2.txt");
        std::fs::write(&f1, "critical1 v1").unwrap();
        std::fs::write(&f2, "critical2 v1").unwrap();

        let engine = CheckpointEngine::new(&storage_dir);
        let ckpt = engine
            .create_checkpoint_with_root("Chk031 base", "ws-chk031", &workspace_dir, None)
            .unwrap();

        let preview = engine
            .restore_preview_bound(&ckpt.checkpoint_id, "ws-chk031", &workspace_dir)
            .unwrap();

        // Cause f2 to fail
        engine.set_pre_replace_hook(Some(|dest: &Path| {
            if dest.file_name() == Some(std::ffi::OsStr::new("critical2.txt")) {
                std::fs::write(dest, "mutation on critical2 to abort").unwrap();
            }
        }));

        // Inject compensation failure on f1
        engine.set_fault_inject_compensation_write(true);

        let res = engine.restore_checkpoint_with_token(
            &ckpt.checkpoint_id,
            "ws-chk031",
            &workspace_dir,
            &preview.preview_token,
            true,
            &[],
        );

        assert!(res.is_err());
        let err_msg = res.err().unwrap().to_string();

        assert!(
            err_msg.contains("Compensating rollback encountered conflicts/errors"),
            "Error must detail rollback failure: {}",
            err_msg
        );
        assert!(
            err_msg.contains("Injected compensation write failure for 'critical1.txt'"),
            "Error must list specific rollback error: {}",
            err_msg
        );
        assert!(
            err_msg.contains("Mandatory safety checkpoint '"),
            "Error must name mandatory safety checkpoint: {}",
            err_msg
        );

        // Extract safety checkpoint ID directly from the error message and assert it exists on disk
        let marker = "Mandatory safety checkpoint '";
        let start = err_msg.find(marker).unwrap() + marker.len();
        let end = err_msg[start..].find('\'').unwrap() + start;
        let safety_id = &err_msg[start..end];

        let safety_dir = storage_dir.join(safety_id);
        assert!(
            safety_dir.exists(),
            "Mandatory safety checkpoint directory '{}' must exist in storage!",
            safety_dir.display()
        );
        let meta_file = safety_dir.join("metadata.json");
        assert!(
            meta_file.exists(),
            "Mandatory safety checkpoint metadata must exist at '{}'!",
            meta_file.display()
        );
        let content = std::fs::read_to_string(&meta_file).unwrap();
        assert!(
            content.contains("Pre-restore safety backup for"),
            "Safety snapshot metadata must record pre-restore safety backup!"
        );

        engine.set_pre_replace_hook::<fn(&Path)>(None);
        engine.set_fault_inject_compensation_write(false);
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_036_external_modification_during_conditional_deletion_is_preserved() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_036_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        let workspace_dir = temp_dir.join("workspace");
        std::fs::create_dir_all(&workspace_dir).unwrap();

        let f_new = workspace_dir.join("newly_created.txt");
        let f_fail = workspace_dir.join("subsequent_fail.txt");

        std::fs::write(&f_new, "initial new file payload").unwrap();
        std::fs::write(&f_fail, "subsequent file base").unwrap();

        let engine = CheckpointEngine::new(&storage_dir);
        let ckpt = engine
            .create_checkpoint_with_root("Chk036 base", "ws-chk036", &workspace_dir, None)
            .unwrap();

        // Remove new_file from workspace so restore will create it as a newly created file
        let _ = std::fs::remove_file(&f_new);

        let preview = engine
            .restore_preview_bound(&ckpt.checkpoint_id, "ws-chk036", &workspace_dir)
            .unwrap();

        // Fail restore on f_fail
        engine.set_pre_replace_hook(Some(|dest: &Path| {
            if dest.file_name() == Some(std::ffi::OsStr::new("subsequent_fail.txt")) {
                std::fs::write(dest, "mutation on subsequent to abort").unwrap();
            }
        }));

        // In pre_delete_hook, mutate the staged file during conditional deletion
        engine.set_pre_delete_hook(Some(|staged_path: &Path| {
            std::fs::write(staged_path, "concurrent edit while in deletion staging!").unwrap();
        }));

        let res = engine.restore_checkpoint_with_token(
            &ckpt.checkpoint_id,
            "ws-chk036",
            &workspace_dir,
            &preview.preview_token,
            true,
            &[],
        );

        assert!(res.is_err());
        let err_msg = res.err().unwrap().to_string();
        assert!(
            err_msg.contains("Files requiring manual recovery"),
            "Error must report conflict for file modified during deletion staging: {}",
            err_msg
        );
        assert!(
            err_msg.contains("File preserved in workspace without deletion"),
            "Error must note file preserved: {}",
            err_msg
        );

        // Crucial invariant: newly_created.txt must exist on disk and preserve the concurrent edit!
        assert!(
            f_new.exists(),
            "File modified during deletion staging must NOT be deleted from workspace!"
        );
        assert_eq!(
            std::fs::read_to_string(&f_new).unwrap(),
            "concurrent edit while in deletion staging!"
        );

        engine.set_pre_replace_hook::<fn(&Path)>(None);
        engine.set_pre_delete_hook::<fn(&Path)>(None);
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_037_metadata_failure_not_counted_as_successful_deletion() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_037_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        let workspace_dir = temp_dir.join("workspace");
        std::fs::create_dir_all(&workspace_dir).unwrap();

        let f_new = workspace_dir.join("newly_created.txt");
        let f_fail = workspace_dir.join("subsequent_fail.txt");

        std::fs::write(&f_new, "payload new").unwrap();
        std::fs::write(&f_fail, "payload fail").unwrap();

        let engine = CheckpointEngine::new(&storage_dir);
        let ckpt = engine
            .create_checkpoint_with_root("Chk037 base", "ws-chk037", &workspace_dir, None)
            .unwrap();

        let _ = std::fs::remove_file(&f_new);

        let preview = engine
            .restore_preview_bound(&ckpt.checkpoint_id, "ws-chk037", &workspace_dir)
            .unwrap();

        // Fail restore on f_fail
        engine.set_pre_replace_hook(Some(|dest: &Path| {
            if dest.file_name() == Some(std::ffi::OsStr::new("subsequent_fail.txt")) {
                std::fs::write(dest, "mutation on fail").unwrap();
            }
        }));

        // Inject metadata failure during compensation
        engine.set_fault_inject_metadata_failure(true);

        let res = engine.restore_checkpoint_with_token(
            &ckpt.checkpoint_id,
            "ws-chk037",
            &workspace_dir,
            &preview.preview_token,
            true,
            &[],
        );

        assert!(res.is_err());
        let err_msg = res.err().unwrap().to_string();

        // Must report 0/1 files cleanly rolled back
        assert!(
            err_msg.contains("0/1 files cleanly rolled back"),
            "Inaccessible file must NEVER be counted as cleanly rolled back! Got: {}",
            err_msg
        );
        assert!(
            err_msg.contains("metadata inspection failed"),
            "Must report metadata inspection failure: {}",
            err_msg
        );

        // The file must still exist on disk untouched
        assert!(f_new.exists());
        assert_eq!(std::fs::read_to_string(&f_new).unwrap(), "payload new");

        engine.set_pre_replace_hook::<fn(&Path)>(None);
        engine.set_fault_inject_metadata_failure(false);
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_038_preserved_backup_path_surfaced_in_restore_report() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_038_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        let workspace_dir = temp_dir.join("workspace");
        std::fs::create_dir_all(&workspace_dir).unwrap();

        let f = workspace_dir.join("existing.txt");
        std::fs::write(&f, "content v1").unwrap();

        let engine = CheckpointEngine::new(&storage_dir);
        let ckpt = engine
            .create_checkpoint_with_root("Chk038 base", "ws-chk038", &workspace_dir, None)
            .unwrap();

        // Modify file in workspace to v2
        std::fs::write(&f, "content v2 modified").unwrap();

        let preview = engine
            .restore_preview_bound(&ckpt.checkpoint_id, "ws-chk038", &workspace_dir)
            .unwrap();

        // Fault-inject backup removal failure so the backup is preserved on disk
        engine.set_fault_inject_backup_removal(true);

        let res = engine.restore_checkpoint_with_token(
            &ckpt.checkpoint_id,
            "ws-chk038",
            &workspace_dir,
            &preview.preview_token,
            true,
            &[],
        );

        assert!(
            res.is_ok(),
            "Restore should succeed even if backup cleanup fails"
        );
        let report = res.unwrap();
        assert!(report.success);
        assert_eq!(report.restored_count, 1);
        assert_eq!(
            report.preserved_backups.len(),
            1,
            "Must surface preserved backup path in report!"
        );

        let preserved_path = PathBuf::from(&report.preserved_backups[0]);
        assert!(
            preserved_path.exists(),
            "Preserved backup file must physically exist at '{}'",
            preserved_path.display()
        );
        assert_eq!(
            std::fs::read_to_string(&preserved_path).unwrap(),
            "content v2 modified",
            "Preserved backup file must contain the pre-restore content"
        );

        engine.set_fault_inject_backup_removal(false);
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_039_recovery_report_accurately_distinguishes_complete_from_partial_compensation() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_039_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        let workspace_dir = temp_dir.join("workspace");
        std::fs::create_dir_all(&workspace_dir).unwrap();

        let f1 = workspace_dir.join("file1.txt");
        let f2 = workspace_dir.join("file2.txt");
        let f3 = workspace_dir.join("file3.txt");

        std::fs::write(&f1, "f1 v1").unwrap();
        std::fs::write(&f2, "f2 v1").unwrap();
        std::fs::write(&f3, "f3 v1").unwrap();

        let engine = CheckpointEngine::new(&storage_dir);
        let ckpt = engine
            .create_checkpoint_with_root("Chk039 base", "ws-chk039", &workspace_dir, None)
            .unwrap();

        // Uncommitted changes
        std::fs::write(&f1, "f1 user edit").unwrap();
        std::fs::write(&f2, "f2 user edit").unwrap();
        std::fs::write(&f3, "f3 user edit").unwrap();

        let preview = engine
            .restore_preview_bound(&ckpt.checkpoint_id, "ws-chk039", &workspace_dir)
            .unwrap();

        // Cause f3 to fail during restore (f1 and f2 are committed)
        // And concurrently mutate f1 after f1 has already passed post-write verification!
        let f1_clone = f1.clone();
        engine.set_pre_replace_hook(Some(move |dest: &Path| {
            if dest.file_name() == Some(std::ffi::OsStr::new("file3.txt")) {
                std::fs::write(&f1_clone, "concurrent edit to f1 after restore").unwrap();
                std::fs::write(dest, "mutation on f3 to abort").unwrap();
            }
        }));

        let res = engine.restore_checkpoint_with_token(
            &ckpt.checkpoint_id,
            "ws-chk039",
            &workspace_dir,
            &preview.preview_token,
            true,
            &[],
        );

        assert!(res.is_err());
        let err_msg = res.err().unwrap().to_string();

        // 1/2 files cleanly rolled back
        assert!(
            err_msg.contains("1/2 files cleanly rolled back"),
            "Report must accurately distinguish complete from partial compensation! Got: {}",
            err_msg
        );
        assert!(
            err_msg.contains("Compensated files: [\"file2.txt\"]"),
            "Report must explicitly list compensated files: {}",
            err_msg
        );
        assert!(
            err_msg.contains("file1.txt"),
            "Report must list conflicted file1.txt: {}",
            err_msg
        );

        // Verify on disk:
        // f2 was cleanly rolled back to "f2 user edit" (pre-restore safety backup state)
        assert_eq!(std::fs::read_to_string(&f2).unwrap(), "f2 user edit");
        // f1 retains concurrent edit
        assert_eq!(
            std::fs::read_to_string(&f1).unwrap(),
            "concurrent edit to f1 after restore"
        );

        engine.set_pre_replace_hook::<fn(&Path)>(None);
        engine.set_post_write_hook::<fn(&Path)>(None);
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_040_open_handle_mutation_after_hash_preserves_staged_artifact() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_040_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        let workspace_dir = temp_dir.join("workspace");
        std::fs::create_dir_all(&workspace_dir).unwrap();

        let f_new = workspace_dir.join("new_file.txt");
        let f_fail = workspace_dir.join("z_fail.txt");

        std::fs::write(&f_new, "clean payload v1").unwrap();
        std::fs::write(&f_fail, "fail base v1").unwrap();

        let engine = CheckpointEngine::new(&storage_dir);
        let ckpt = engine
            .create_checkpoint_with_root("Chk040 base", "ws-chk040", &workspace_dir, None)
            .unwrap();

        // Remove new_file from workspace so restore will create it as a new file
        let _ = std::fs::remove_file(&f_new);

        let preview = engine
            .restore_preview_bound(&ckpt.checkpoint_id, "ws-chk040", &workspace_dir)
            .unwrap();

        // Cause z_fail to abort restore after new_file was committed
        engine.set_pre_replace_hook(Some(|dest: &Path| {
            if dest.file_name() == Some(std::ffi::OsStr::new("z_fail.txt")) {
                std::fs::write(dest, "mutation on fail to abort").unwrap();
            }
        }));

        // Mutate staged file in post_hash_delete_hook simulating an open writable handle
        // writing after initial hash check but before unlink
        engine.set_post_hash_delete_hook(Some(|staged_path: &Path| {
            let mut f = std::fs::OpenOptions::new()
                .write(true)
                .open(staged_path)
                .unwrap();
            use std::io::Write;
            f.write_all(b"open handle mutation after hash verification!")
                .unwrap();
        }));

        let res = engine.restore_checkpoint_with_token(
            &ckpt.checkpoint_id,
            "ws-chk040",
            &workspace_dir,
            &preview.preview_token,
            true,
            &[],
        );

        assert!(res.is_err());
        let err_msg = res.err().unwrap().to_string();
        assert!(
            err_msg.contains("Files requiring manual recovery"),
            "Must report conflict for open-handle mutation: {}",
            err_msg
        );

        // Extract preserved path and assert user's modification remains recoverable on disk
        let marker = "staging preserved at '";
        assert!(
            err_msg.contains(marker),
            "Error must identify surviving staged artifact: {}",
            err_msg
        );
        let start = err_msg.find(marker).unwrap() + marker.len();
        let end = err_msg[start..].find('\'').unwrap() + start;
        let preserved_str = err_msg[start..end].replace(r"\\", r"\");
        let preserved_path = PathBuf::from(&preserved_str);

        assert!(
            preserved_path.exists(),
            "Preserved artifact must physically exist on disk at '{}'",
            preserved_path.display()
        );
        assert_eq!(
            std::fs::read_to_string(&preserved_path).unwrap(),
            "open handle mutation after hash verification!",
            "User modifications made through open handle must remain recoverable!"
        );

        engine.set_pre_replace_hook::<fn(&Path)>(None);
        engine.set_post_hash_delete_hook::<fn(&Path)>(None);
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_041_rollback_rename_does_not_overwrite_recreated_destination() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_041_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        let workspace_dir = temp_dir.join("workspace");
        std::fs::create_dir_all(&workspace_dir).unwrap();

        let f_new = workspace_dir.join("new_file.txt");
        let f_fail = workspace_dir.join("z_fail.txt");

        std::fs::write(&f_new, "initial new file payload").unwrap();
        std::fs::write(&f_fail, "fail base v1").unwrap();

        let engine = CheckpointEngine::new(&storage_dir);
        let ckpt = engine
            .create_checkpoint_with_root("Chk041 base", "ws-chk041", &workspace_dir, None)
            .unwrap();

        let _ = std::fs::remove_file(&f_new);

        let preview = engine
            .restore_preview_bound(&ckpt.checkpoint_id, "ws-chk041", &workspace_dir)
            .unwrap();

        // Cause z_fail to abort
        engine.set_pre_replace_hook(Some(|dest: &Path| {
            if dest.file_name() == Some(std::ffi::OsStr::new("z_fail.txt")) {
                std::fs::write(dest, "mutation on fail to abort").unwrap();
            }
        }));

        // In pre_delete_hook on new_file:
        // 1. Mutate the staged file (triggering rollback)
        // 2. Concurrently recreate the original destination!
        let dest_clone = f_new.clone();
        engine.set_pre_delete_hook(Some(move |staged_path: &Path| {
            std::fs::write(staged_path, "staged file modified").unwrap();
            std::fs::write(&dest_clone, "concurrently recreated destination payload").unwrap();
        }));

        let res = engine.restore_checkpoint_with_token(
            &ckpt.checkpoint_id,
            "ws-chk041",
            &workspace_dir,
            &preview.preview_token,
            true,
            &[],
        );

        assert!(res.is_err());
        let err_msg = res.err().unwrap().to_string();

        // 1. Recreated destination MUST survive on disk with its recreated payload!
        assert!(f_new.exists());
        assert_eq!(
            std::fs::read_to_string(&f_new).unwrap(),
            "concurrently recreated destination payload",
            "Destination MUST NOT be overwritten by rollback rename!"
        );

        // 2. Staged recovery artifact MUST survive on disk under a conflict path!
        let marker = "staging preserved at '";
        assert!(
            err_msg.contains(marker),
            "Must report conflict surviving path: {}",
            err_msg
        );
        let start = err_msg.find(marker).unwrap() + marker.len();
        let end = err_msg[start..].find('\'').unwrap() + start;
        let conflict_str = err_msg[start..end].replace(r"\\", r"\");
        let conflict_path = PathBuf::from(&conflict_str);

        assert!(
            conflict_path.exists(),
            "Conflict artifact must physically exist at '{}'",
            conflict_path.display()
        );
        assert_ne!(
            conflict_path, f_new,
            "Conflict path must not equal destination path!"
        );
        assert_eq!(
            std::fs::read_to_string(&conflict_path).unwrap(),
            "staged file modified",
            "Staged artifact must retain its modified content!"
        );

        engine.set_pre_replace_hook::<fn(&Path)>(None);
        engine.set_pre_delete_hook::<fn(&Path)>(None);
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_042_missing_file_compensation_exclusive_prevents_overwrite_of_recreated_destination(
    ) {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_042_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        let workspace_dir = temp_dir.join("workspace");
        std::fs::create_dir_all(&workspace_dir).unwrap();

        let f_orig = workspace_dir.join("orig.txt");
        let f_fail = workspace_dir.join("z_fail.txt");

        std::fs::write(&f_orig, "orig v1 in checkpoint").unwrap();
        std::fs::write(&f_fail, "fail base v1").unwrap();

        let engine = CheckpointEngine::new(&storage_dir);
        let ckpt = engine
            .create_checkpoint_with_root("Chk042 base", "ws-chk042", &workspace_dir, None)
            .unwrap();

        // User has uncommitted edit in workspace
        std::fs::write(&f_orig, "orig uncommitted user edit").unwrap();

        let preview = engine
            .restore_preview_bound(&ckpt.checkpoint_id, "ws-chk042", &workspace_dir)
            .unwrap();

        // In pre_replace_hook:
        // - When processing z_fail.txt: delete orig.txt so compensation will see it as NotFound!
        // - When processing orig.txt during compensation: recreate orig.txt with concurrent edit!
        let orig_clone = f_orig.clone();
        let fail_triggered = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let fail_flag = fail_triggered.clone();
        engine.set_pre_replace_hook(Some(move |dest: &Path| {
            if dest.file_name() == Some(std::ffi::OsStr::new("z_fail.txt")) {
                fail_flag.store(true, std::sync::atomic::Ordering::SeqCst);
                let _ = std::fs::remove_file(&orig_clone);
                std::fs::write(dest, "mutation on fail to abort").unwrap();
            } else if dest.file_name() == Some(std::ffi::OsStr::new("orig.txt"))
                && fail_flag.load(std::sync::atomic::Ordering::SeqCst)
            {
                std::fs::write(
                    &orig_clone,
                    "concurrently created between NotFound check and compensation publication",
                )
                .unwrap();
            }
        }));

        let res = engine.restore_checkpoint_with_token(
            &ckpt.checkpoint_id,
            "ws-chk042",
            &workspace_dir,
            &preview.preview_token,
            true,
            &[],
        );

        assert!(res.is_err());
        let err_msg = res.err().unwrap().to_string();

        assert!(
            err_msg
                .contains("Missing file 'orig.txt' was concurrently recreated before compensation"),
            "Error must explicitly report exclusive creation conflict: {}",
            err_msg
        );
        assert!(
            err_msg.contains("0/1 files cleanly rolled back"),
            "Conflicted file must not be counted as cleanly rolled back: {}",
            err_msg
        );

        // Invariant: orig.txt must NEVER be overwritten!
        assert!(f_orig.exists());
        assert_eq!(
            std::fs::read_to_string(&f_orig).unwrap(),
            "concurrently created between NotFound check and compensation publication",
            "Concurrently recreated file must be preserved intact!"
        );

        // Safety snapshot blob must remain intact in storage
        let marker = "Mandatory safety checkpoint '";
        let start = err_msg.find(marker).unwrap() + marker.len();
        let end = err_msg[start..].find('\'').unwrap() + start;
        let safety_id = &err_msg[start..end];
        let safety_dir = storage_dir.join(safety_id);
        assert!(safety_dir.exists());

        engine.set_pre_replace_hook::<fn(&Path)>(None);
        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
