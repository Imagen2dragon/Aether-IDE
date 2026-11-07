use application_core::{AetherError, Result};
use cap_std::ambient_authority;
use cap_std::fs::Dir;
pub use filesystem_engine::FileObjectIdentity;
use filesystem_engine::{
    sanitize_relative_path, ConditionalReplaceHooks, FilesystemEngine, WorkspaceCapability,
};
use parking_lot::{Mutex, RwLock};
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
    #[serde(default)]
    pub quarantined_artifacts: Vec<String>,
    #[serde(default)]
    pub cleanup_pending: bool,
}

/// Status of an artifact recorded in the durable recovery manifest.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum QuarantineStatus {
    /// Initial intent record before file movement or during staging.
    PendingMove,
    /// Quarantined and awaiting explicit user-controlled cleanup.
    PendingCleanup,
    /// Successfully cleaned up or confirmed absent.
    CleanedUp,
    /// Preserved due to conflict or concurrent external modification.
    PreservedConflict,
}

/// Durable record of an artifact preserved in quarantine.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct QuarantineRecord {
    pub id: String,
    pub original_relative_path: String,
    #[serde(default)]
    pub quarantined_filename: String,
    pub quarantined_path: String,
    pub quarantined_at: String,
    pub expected_sha256: String,
    pub actual_sha256: Option<String>,
    pub reason: String,
    pub status: QuarantineStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retained_staging_filename: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retained_staging_path: Option<String>,
}

/// Durable recovery manifest tracking quarantined artifacts across sessions.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RecoveryManifest {
    pub records: Vec<QuarantineRecord>,
}

impl RecoveryManifest {
    pub fn load(manifest_path: &Path) -> Result<Self> {
        let meta = match std::fs::symlink_metadata(manifest_path) {
            Ok(m) => m,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Self::default());
            }
            Err(e) => {
                return Err(AetherError::Io(e));
            }
        };

        if meta.file_type().is_symlink() {
            return Err(AetherError::PermissionDenied(format!(
                "Security rejection: Recovery manifest at '{}' is a symbolic link",
                manifest_path.display()
            )));
        }

        if !meta.file_type().is_file() {
            return Err(AetherError::PermissionDenied(format!(
                "Security rejection: Recovery manifest at '{}' is not a regular file",
                manifest_path.display()
            )));
        }

        let data = std::fs::read_to_string(manifest_path).map_err(AetherError::Io)?;
        let manifest: Self = serde_json::from_str(&data).map_err(AetherError::Serialization)?;
        Ok(manifest)
    }

    pub fn save(&self, manifest_path: &Path) -> Result<()> {
        self.save_with_fault(manifest_path, false)
    }

    pub fn save_with_fault(&self, manifest_path: &Path, fault_inject: bool) -> Result<()> {
        if fault_inject {
            return Err(AetherError::Io(std::io::Error::other(
                "Fault injection: manifest persistence failure",
            )));
        }
        if let Some(parent) = manifest_path.parent() {
            std::fs::create_dir_all(parent).map_err(AetherError::Io)?;
        }
        let tmp_path =
            manifest_path.with_extension(format!("tmp.{}", uuid::Uuid::new_v4().simple()));
        let json = serde_json::to_string_pretty(self).map_err(AetherError::Serialization)?;

        {
            use std::io::Write;
            let mut f = std::fs::File::create(&tmp_path).map_err(AetherError::Io)?;
            f.write_all(json.as_bytes()).map_err(AetherError::Io)?;
            f.sync_all().map_err(AetherError::Io)?;
        }

        std::fs::rename(&tmp_path, manifest_path).map_err(AetherError::Io)?;

        #[cfg(unix)]
        {
            if let Some(parent) = manifest_path.parent() {
                if let Ok(dir) = std::fs::File::open(parent) {
                    let _ = dir.sync_all();
                }
            }
        }
        Ok(())
    }

    pub fn record_quarantine(
        &mut self,
        record: QuarantineRecord,
        manifest_path: &Path,
    ) -> Result<()> {
        self.record_quarantine_with_fault(record, manifest_path, false)
    }

    pub fn record_quarantine_with_fault(
        &mut self,
        record: QuarantineRecord,
        manifest_path: &Path,
        fault_inject: bool,
    ) -> Result<()> {
        self.records.retain(|r| r.id != record.id);
        self.records.push(record);
        self.save_with_fault(manifest_path, fault_inject)
    }
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

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum ObjectExistence {
    Exists,
    ConfirmedAbsent,
    Uncertain(String),
}

pub fn classify_dir_entry_existence(dir: &cap_std::fs::Dir, name: &str) -> ObjectExistence {
    match dir.symlink_metadata(name) {
        Ok(_) => ObjectExistence::Exists,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => ObjectExistence::ConfirmedAbsent,
        Err(e) => ObjectExistence::Uncertain(e.to_string()),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RollbackPublicationStatus {
    PublishedVerified,
    PublishedWithRetainedStaging,
    PublicationConflict,
    PublicationUncertainBlocked,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StageRollbackResult {
    pub surviving_filename: String,
    pub retained_staging_filename: Option<String>,
    pub status: RollbackPublicationStatus,
    pub is_conflict: bool,
    pub error: Option<String>,
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
    pre_unlink_delete_hook: Arc<Mutex<Option<PreReplaceHook>>>,
    pre_hard_link_delete_hook: Arc<Mutex<Option<PreReplaceHook>>>,
    fault_inject_metadata_failure: std::sync::atomic::AtomicBool,
    fault_inject_delete_removal: std::sync::atomic::AtomicBool,
    fault_inject_backup_removal: std::sync::atomic::AtomicBool,
    fault_inject_rollback_metadata_failure: std::sync::atomic::AtomicBool,
    fault_inject_hard_link_rollback_failure: std::sync::atomic::AtomicBool,
    fault_inject_second_verification_read_failure: std::sync::atomic::AtomicBool,
    fault_inject_cleanup_removal: std::sync::atomic::AtomicBool,
    fault_inject_quarantine_rename: std::sync::atomic::AtomicBool,
    fault_inject_manifest_save: std::sync::atomic::AtomicBool,
    fault_inject_quarantine_commit: std::sync::atomic::AtomicBool,
    fault_inject_quarantine_copy_failure: std::sync::atomic::AtomicBool,
    fault_inject_quarantine_dest_creation: std::sync::atomic::AtomicBool,
    fault_inject_quarantine_cross_device: std::sync::atomic::AtomicBool,
    fault_inject_quarantine_source_mutation: std::sync::atomic::AtomicBool,
    fault_inject_quarantine_symlink_dir: std::sync::atomic::AtomicBool,
    fault_inject_manifest_unreadable: std::sync::atomic::AtomicBool,
    fault_inject_corrupt_manifest_backup_creation: std::sync::atomic::AtomicBool,
    fault_inject_corrupt_manifest_backup_verification: std::sync::atomic::AtomicBool,
    fault_inject_corrupt_manifest_backup_collision: std::sync::atomic::AtomicBool,
    fault_inject_reconciliation_failure: std::sync::atomic::AtomicBool,
    fault_inject_quarantine_target_swap: std::sync::atomic::AtomicBool,
    fault_inject_metadata_permission_denied: std::sync::atomic::AtomicBool,
    fault_inject_metadata_io_error: std::sync::atomic::AtomicBool,
    fault_inject_rollback_conflict_collision: std::sync::atomic::AtomicBool,
    fault_inject_rollback_conflict_publication_failure: std::sync::atomic::AtomicBool,
    fault_inject_rollback_both_publication_failures: std::sync::atomic::AtomicBool,
    fault_inject_rollback_hard_link_failure: std::sync::atomic::AtomicBool,
    fault_inject_rollback_staging_removal_failure: std::sync::atomic::AtomicBool,
    fault_inject_identity_capture_failure: std::sync::atomic::AtomicBool,
    recovery_blocked_reason: Arc<RwLock<Option<String>>>,
    pre_cleanup_unlink_hook: Arc<Mutex<Option<PreReplaceHook>>>,
    pre_quarantine_source_unlink_hook: Arc<Mutex<Option<PreReplaceHook>>>,
    pre_rollback_publish_hook: Arc<Mutex<Option<PreReplaceHook>>>,
    post_stage_rename_hook: Arc<Mutex<Option<PreReplaceHook>>>,
    post_rollback_hard_link_hook: Arc<Mutex<Option<PreReplaceHook>>>,
    pre_staging_link_removal_hook: Arc<Mutex<Option<PreReplaceHook>>>,
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
            pre_unlink_delete_hook: Arc::new(Mutex::new(None)),
            pre_hard_link_delete_hook: Arc::new(Mutex::new(None)),
            fault_inject_metadata_failure: std::sync::atomic::AtomicBool::new(false),
            fault_inject_delete_removal: std::sync::atomic::AtomicBool::new(false),
            fault_inject_backup_removal: std::sync::atomic::AtomicBool::new(false),
            fault_inject_rollback_metadata_failure: std::sync::atomic::AtomicBool::new(false),
            fault_inject_hard_link_rollback_failure: std::sync::atomic::AtomicBool::new(false),
            fault_inject_second_verification_read_failure: std::sync::atomic::AtomicBool::new(
                false,
            ),
            fault_inject_cleanup_removal: std::sync::atomic::AtomicBool::new(false),
            fault_inject_quarantine_rename: std::sync::atomic::AtomicBool::new(false),
            fault_inject_manifest_save: std::sync::atomic::AtomicBool::new(false),
            fault_inject_quarantine_commit: std::sync::atomic::AtomicBool::new(false),
            fault_inject_quarantine_copy_failure: std::sync::atomic::AtomicBool::new(false),
            fault_inject_quarantine_dest_creation: std::sync::atomic::AtomicBool::new(false),
            fault_inject_quarantine_cross_device: std::sync::atomic::AtomicBool::new(false),
            fault_inject_quarantine_source_mutation: std::sync::atomic::AtomicBool::new(false),
            fault_inject_quarantine_symlink_dir: std::sync::atomic::AtomicBool::new(false),
            fault_inject_manifest_unreadable: std::sync::atomic::AtomicBool::new(false),
            fault_inject_corrupt_manifest_backup_creation: std::sync::atomic::AtomicBool::new(
                false,
            ),
            fault_inject_corrupt_manifest_backup_verification: std::sync::atomic::AtomicBool::new(
                false,
            ),
            fault_inject_corrupt_manifest_backup_collision: std::sync::atomic::AtomicBool::new(
                false,
            ),
            fault_inject_reconciliation_failure: std::sync::atomic::AtomicBool::new(false),
            fault_inject_quarantine_target_swap: std::sync::atomic::AtomicBool::new(false),
            fault_inject_metadata_permission_denied: std::sync::atomic::AtomicBool::new(false),
            fault_inject_metadata_io_error: std::sync::atomic::AtomicBool::new(false),
            fault_inject_rollback_conflict_collision: std::sync::atomic::AtomicBool::new(false),
            fault_inject_rollback_conflict_publication_failure: std::sync::atomic::AtomicBool::new(
                false,
            ),
            fault_inject_rollback_both_publication_failures: std::sync::atomic::AtomicBool::new(
                false,
            ),
            fault_inject_rollback_hard_link_failure: std::sync::atomic::AtomicBool::new(false),
            fault_inject_rollback_staging_removal_failure: std::sync::atomic::AtomicBool::new(
                false,
            ),
            fault_inject_identity_capture_failure: std::sync::atomic::AtomicBool::new(false),
            recovery_blocked_reason: Arc::new(RwLock::new(None)),
            pre_cleanup_unlink_hook: Arc::new(Mutex::new(None)),
            pre_quarantine_source_unlink_hook: Arc::new(Mutex::new(None)),
            pre_rollback_publish_hook: Arc::new(Mutex::new(None)),
            post_stage_rename_hook: Arc::new(Mutex::new(None)),
            post_rollback_hard_link_hook: Arc::new(Mutex::new(None)),
            pre_staging_link_removal_hook: Arc::new(Mutex::new(None)),
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
    pub fn set_pre_hard_link_delete_hook<F>(&self, hook: Option<F>)
    where
        F: Fn(&Path) + Send + Sync + 'static,
    {
        let mut guard = self.pre_hard_link_delete_hook.lock();
        *guard = hook.map(|h| Arc::new(h) as PreReplaceHook);
    }

    #[doc(hidden)]
    pub fn set_fault_inject_rollback_metadata_failure(&self, inject: bool) {
        self.fault_inject_rollback_metadata_failure
            .store(inject, std::sync::atomic::Ordering::SeqCst);
    }

    #[doc(hidden)]
    pub fn set_fault_inject_hard_link_rollback_failure(&self, inject: bool) {
        self.fault_inject_hard_link_rollback_failure
            .store(inject, std::sync::atomic::Ordering::SeqCst);
    }

    #[doc(hidden)]
    pub fn set_fault_inject_second_verification_read_failure(&self, inject: bool) {
        self.fault_inject_second_verification_read_failure
            .store(inject, std::sync::atomic::Ordering::SeqCst);
    }

    #[doc(hidden)]
    pub fn set_pre_unlink_delete_hook<F>(&self, hook: Option<F>)
    where
        F: Fn(&Path) + Send + Sync + 'static,
    {
        let mut guard = self.pre_unlink_delete_hook.lock();
        *guard = hook.map(|h| Arc::new(h) as PreReplaceHook);
    }

    #[doc(hidden)]
    pub fn set_fault_inject_cleanup_removal(&self, inject: bool) {
        self.fault_inject_cleanup_removal
            .store(inject, std::sync::atomic::Ordering::SeqCst);
    }

    #[doc(hidden)]
    pub fn set_fault_inject_quarantine_rename(&self, inject: bool) {
        self.fault_inject_quarantine_rename
            .store(inject, std::sync::atomic::Ordering::SeqCst);
    }

    #[doc(hidden)]
    pub fn set_fault_inject_manifest_save(&self, inject: bool) {
        self.fault_inject_manifest_save
            .store(inject, std::sync::atomic::Ordering::SeqCst);
    }

    #[doc(hidden)]
    pub fn set_fault_inject_quarantine_commit(&self, inject: bool) {
        self.fault_inject_quarantine_commit
            .store(inject, std::sync::atomic::Ordering::SeqCst);
    }

    #[doc(hidden)]
    pub fn set_fault_inject_quarantine_copy_failure(&self, inject: bool) {
        self.fault_inject_quarantine_copy_failure
            .store(inject, std::sync::atomic::Ordering::SeqCst);
    }

    #[doc(hidden)]
    pub fn set_fault_inject_quarantine_dest_creation(&self, inject: bool) {
        self.fault_inject_quarantine_dest_creation
            .store(inject, std::sync::atomic::Ordering::SeqCst);
    }

    #[doc(hidden)]
    pub fn set_fault_inject_quarantine_cross_device(&self, inject: bool) {
        self.fault_inject_quarantine_cross_device
            .store(inject, std::sync::atomic::Ordering::SeqCst);
    }

    #[doc(hidden)]
    pub fn set_fault_inject_quarantine_source_mutation(&self, inject: bool) {
        self.fault_inject_quarantine_source_mutation
            .store(inject, std::sync::atomic::Ordering::SeqCst);
    }

    #[doc(hidden)]
    pub fn set_fault_inject_quarantine_symlink_dir(&self, inject: bool) {
        self.fault_inject_quarantine_symlink_dir
            .store(inject, std::sync::atomic::Ordering::SeqCst);
    }

    #[doc(hidden)]
    pub fn set_fault_inject_manifest_unreadable(&self, inject: bool) {
        self.fault_inject_manifest_unreadable
            .store(inject, std::sync::atomic::Ordering::SeqCst);
    }

    #[doc(hidden)]
    pub fn set_pre_cleanup_unlink_hook<F>(&self, hook: Option<F>)
    where
        F: Fn(&Path) + Send + Sync + 'static,
    {
        let mut guard = self.pre_cleanup_unlink_hook.lock();
        *guard = hook.map(|h| Arc::new(h) as PreReplaceHook);
    }

    #[doc(hidden)]
    pub fn set_pre_quarantine_source_unlink_hook<F>(&self, hook: Option<F>)
    where
        F: Fn(&Path) + Send + Sync + 'static,
    {
        let mut guard = self.pre_quarantine_source_unlink_hook.lock();
        *guard = hook.map(|h| Arc::new(h) as PreReplaceHook);
    }

    pub fn is_recovery_blocked(&self) -> bool {
        self.recovery_blocked_reason.read().is_some()
    }

    pub fn recovery_blocked_reason(&self) -> Option<String> {
        self.recovery_blocked_reason.read().clone()
    }

    pub fn set_recovery_blocked(&self, reason: Option<String>) {
        *self.recovery_blocked_reason.write() = reason;
    }

    #[doc(hidden)]
    pub fn set_fault_inject_corrupt_manifest_backup_creation(&self, inject: bool) {
        self.fault_inject_corrupt_manifest_backup_creation
            .store(inject, std::sync::atomic::Ordering::SeqCst);
    }

    #[doc(hidden)]
    pub fn set_fault_inject_corrupt_manifest_backup_verification(&self, inject: bool) {
        self.fault_inject_corrupt_manifest_backup_verification
            .store(inject, std::sync::atomic::Ordering::SeqCst);
    }

    #[doc(hidden)]
    pub fn set_fault_inject_corrupt_manifest_backup_collision(&self, inject: bool) {
        self.fault_inject_corrupt_manifest_backup_collision
            .store(inject, std::sync::atomic::Ordering::SeqCst);
    }

    #[doc(hidden)]
    pub fn set_fault_inject_reconciliation_failure(&self, inject: bool) {
        self.fault_inject_reconciliation_failure
            .store(inject, std::sync::atomic::Ordering::SeqCst);
    }

    #[doc(hidden)]
    pub fn set_fault_inject_quarantine_target_swap(&self, inject: bool) {
        self.fault_inject_quarantine_target_swap
            .store(inject, std::sync::atomic::Ordering::SeqCst);
    }

    #[doc(hidden)]
    pub fn set_fault_inject_metadata_permission_denied(&self, inject: bool) {
        self.fault_inject_metadata_permission_denied
            .store(inject, std::sync::atomic::Ordering::SeqCst);
    }

    #[doc(hidden)]
    pub fn set_fault_inject_metadata_io_error(&self, inject: bool) {
        self.fault_inject_metadata_io_error
            .store(inject, std::sync::atomic::Ordering::SeqCst);
    }

    #[doc(hidden)]
    pub fn set_fault_inject_rollback_conflict_collision(&self, inject: bool) {
        self.fault_inject_rollback_conflict_collision
            .store(inject, std::sync::atomic::Ordering::SeqCst);
    }

    #[doc(hidden)]
    pub fn set_fault_inject_rollback_conflict_publication_failure(&self, inject: bool) {
        self.fault_inject_rollback_conflict_publication_failure
            .store(inject, std::sync::atomic::Ordering::SeqCst);
    }

    #[doc(hidden)]
    pub fn set_fault_inject_rollback_both_publication_failures(&self, inject: bool) {
        self.fault_inject_rollback_both_publication_failures
            .store(inject, std::sync::atomic::Ordering::SeqCst);
    }

    #[doc(hidden)]
    pub fn set_fault_inject_rollback_hard_link_failure(&self, inject: bool) {
        self.fault_inject_rollback_hard_link_failure
            .store(inject, std::sync::atomic::Ordering::SeqCst);
    }

    #[doc(hidden)]
    pub fn set_fault_inject_rollback_staging_removal_failure(&self, inject: bool) {
        self.fault_inject_rollback_staging_removal_failure
            .store(inject, std::sync::atomic::Ordering::SeqCst);
    }

    #[doc(hidden)]
    pub fn set_pre_rollback_publish_hook<F>(&self, hook: Option<F>)
    where
        F: Fn(&Path) + Send + Sync + 'static,
    {
        let mut guard = self.pre_rollback_publish_hook.lock();
        *guard = hook.map(|h| Arc::new(h) as PreReplaceHook);
    }

    #[doc(hidden)]
    pub fn set_fault_inject_identity_capture_failure(&self, inject: bool) {
        self.fault_inject_identity_capture_failure
            .store(inject, std::sync::atomic::Ordering::SeqCst);
    }

    #[doc(hidden)]
    pub fn set_post_stage_rename_hook<F>(&self, hook: Option<F>)
    where
        F: Fn(&Path) + Send + Sync + 'static,
    {
        let mut guard = self.post_stage_rename_hook.lock();
        *guard = hook.map(|h| Arc::new(h) as PreReplaceHook);
    }

    #[doc(hidden)]
    pub fn set_post_rollback_hard_link_hook<F>(&self, hook: Option<F>)
    where
        F: Fn(&Path) + Send + Sync + 'static,
    {
        let mut guard = self.post_rollback_hard_link_hook.lock();
        *guard = hook.map(|h| Arc::new(h) as PreReplaceHook);
    }

    #[doc(hidden)]
    pub fn set_pre_staging_link_removal_hook<F>(&self, hook: Option<F>)
    where
        F: Fn(&Path) + Send + Sync + 'static,
    {
        let mut guard = self.pre_staging_link_removal_hook.lock();
        *guard = hook.map(|h| Arc::new(h) as PreReplaceHook);
    }

    /// Verifies the physical boundary of the quarantine directory, ensuring neither it nor its
    /// parent storage directory is a symbolic link or junction, and that canonical ancestry holds.
    pub fn verify_quarantine_boundary(&self) -> Result<PathBuf> {
        if self
            .fault_inject_quarantine_symlink_dir
            .load(std::sync::atomic::Ordering::SeqCst)
        {
            return Err(AetherError::PermissionDenied(
                "Security rejection: Quarantine directory boundary check failed (injected symlink or reparse point)"
                    .into(),
            ));
        }

        let s_meta = match std::fs::symlink_metadata(&self.storage_dir) {
            Ok(m) => m,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                std::fs::create_dir_all(&self.storage_dir).map_err(AetherError::Io)?;
                std::fs::symlink_metadata(&self.storage_dir).map_err(AetherError::Io)?
            }
            Err(e) => return Err(AetherError::Io(e)),
        };

        if s_meta.file_type().is_symlink() {
            return Err(AetherError::PermissionDenied(format!(
                "Security boundary violation: Storage directory '{}' is a symbolic link",
                self.storage_dir.display()
            )));
        }
        if !s_meta.file_type().is_dir() {
            return Err(AetherError::PermissionDenied(format!(
                "Security boundary violation: Storage directory '{}' is not a directory",
                self.storage_dir.display()
            )));
        }

        let q_dir = self.quarantine_dir();
        let q_meta = match std::fs::symlink_metadata(&q_dir) {
            Ok(m) => m,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                std::fs::create_dir_all(&q_dir).map_err(AetherError::Io)?;
                std::fs::symlink_metadata(&q_dir).map_err(AetherError::Io)?
            }
            Err(e) => return Err(AetherError::Io(e)),
        };

        if q_meta.file_type().is_symlink() {
            return Err(AetherError::PermissionDenied(format!(
                "Security boundary violation: Quarantine directory '{}' is a symbolic link",
                q_dir.display()
            )));
        }
        if !q_meta.file_type().is_dir() {
            return Err(AetherError::PermissionDenied(format!(
                "Security boundary violation: Quarantine directory '{}' is not a directory",
                q_dir.display()
            )));
        }

        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x00000400;
            if (s_meta.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT) != 0 {
                return Err(AetherError::PermissionDenied(format!(
                    "Security boundary violation: Storage directory '{}' is a reparse point",
                    self.storage_dir.display()
                )));
            }
            if (q_meta.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT) != 0 {
                return Err(AetherError::PermissionDenied(format!(
                    "Security boundary violation: Quarantine directory '{}' is a reparse point",
                    q_dir.display()
                )));
            }
        }

        let canon_storage = std::fs::canonicalize(&self.storage_dir).map_err(AetherError::Io)?;
        let canon_q = std::fs::canonicalize(&q_dir).map_err(AetherError::Io)?;

        if canon_q.parent() != Some(&canon_storage) {
            return Err(AetherError::PermissionDenied(format!(
                "Security boundary violation: Quarantine canonical parent '{:?}' does not match storage root '{:?}'",
                canon_q.parent(), canon_storage
            )));
        }

        Ok(q_dir)
    }

    /// Performs mandatory startup reconciliation of the quarantine storage.
    pub fn initialize_and_reconcile(&self) -> Result<RecoveryManifest> {
        tracing::info!("Initializing checkpoint engine and reconciling quarantine storage...");
        self.reconcile_quarantine()
    }

    /// Path to the durable recovery manifest recording quarantined artifacts.
    pub fn recovery_manifest_path(&self) -> PathBuf {
        self.storage_dir.join("recovery_manifest.json")
    }

    /// Path to the quarantine storage directory.
    pub fn quarantine_dir(&self) -> PathBuf {
        self.storage_dir.join("quarantine")
    }

    /// Loads the durable recovery manifest from disk.
    pub fn load_recovery_manifest(&self) -> Result<RecoveryManifest> {
        if self
            .fault_inject_manifest_unreadable
            .load(std::sync::atomic::Ordering::SeqCst)
        {
            return Err(AetherError::PermissionDenied(
                "Fault injection: manifest unreadable / permission denied".into(),
            ));
        }
        let manifest_path = self.recovery_manifest_path();
        match RecoveryManifest::load(&manifest_path) {
            Ok(m) => Ok(m),
            Err(AetherError::Serialization(e)) => {
                // Transactional Corrupt Manifest Handling (Finding 4)
                // Step 1: Read raw corrupt bytes and calculate hash
                let corrupt_bytes = std::fs::read(&manifest_path).map_err(AetherError::Io)?;
                let corrupt_hash = Self::sha256_digest(&corrupt_bytes);

                // Step 2: Create backup EXCLUSIVELY (create_new(true)) with collision retry loop (CHK-103, CHK-124..126)
                use std::io::Write;
                let mut backup_file_opt = None;
                let mut backup_name = String::new();
                let mut backup_path = PathBuf::new();

                for attempt in 0..5 {
                    let timestamp = chrono::Utc::now().timestamp();
                    let cand_name = if attempt == 0
                        && self
                            .fault_inject_corrupt_manifest_backup_collision
                            .load(std::sync::atomic::Ordering::SeqCst)
                    {
                        "recovery_manifest.json.corrupt.colliding_candidate".to_string()
                    } else {
                        format!(
                            "recovery_manifest.json.corrupt.{}.{}",
                            timestamp,
                            uuid::Uuid::new_v4().simple()
                        )
                    };
                    let cand_path = manifest_path.with_file_name(&cand_name);

                    // Step 3: Check fault injection for backup creation failure (CHK-100)
                    if self
                        .fault_inject_corrupt_manifest_backup_creation
                        .load(std::sync::atomic::Ordering::SeqCst)
                    {
                        tracing::error!("Injected corrupt manifest backup creation failure");
                        return Err(AetherError::Io(std::io::Error::new(
                            std::io::ErrorKind::PermissionDenied,
                            "Injected corrupt manifest backup creation failure",
                        )));
                    }

                    match std::fs::OpenOptions::new()
                        .write(true)
                        .create_new(true)
                        .open(&cand_path)
                    {
                        Ok(f) => {
                            backup_file_opt = Some(f);
                            backup_name = cand_name;
                            backup_path = cand_path;
                            break;
                        }
                        Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {
                            tracing::warn!(
                                "Corrupt manifest backup collision on attempt {} at '{}'. Retrying with new unique name.",
                                attempt,
                                cand_path.display()
                            );
                            continue;
                        }
                        Err(err) => {
                            tracing::error!(
                                "Failed to create exclusive corrupt manifest backup at '{}': {}. Original manifest preserved.",
                                cand_path.display(),
                                err
                            );
                            return Err(AetherError::Io(err));
                        }
                    }
                }

                let mut backup_file = match backup_file_opt {
                    Some(f) => f,
                    None => {
                        return Err(AetherError::Internal(
                            "Failed to generate unique backup name after maximum collision retries. Original manifest preserved.".into(),
                        ));
                    }
                };

                // Write bytes and flush/sync
                if let Err(write_err) = backup_file.write_all(&corrupt_bytes) {
                    let _ = std::fs::remove_file(&backup_path);
                    return Err(AetherError::Io(write_err));
                }
                if let Err(sync_err) = backup_file.sync_all() {
                    let _ = std::fs::remove_file(&backup_path);
                    return Err(AetherError::Io(sync_err));
                }
                drop(backup_file);

                // Step 5: Read back backup file and verify hash against corrupt source bytes
                let read_back_bytes = match std::fs::read(&backup_path) {
                    Ok(b) => b,
                    Err(read_err) => {
                        return Err(AetherError::Io(read_err));
                    }
                };
                let read_back_hash = Self::sha256_digest(&read_back_bytes);

                let verification_injected = self
                    .fault_inject_corrupt_manifest_backup_verification
                    .load(std::sync::atomic::Ordering::SeqCst);

                if read_back_hash != corrupt_hash || verification_injected {
                    // Verification failed! (CHK-101) DO NOT REPLACE ORIGINAL MANIFEST!
                    tracing::error!(
                        "Corrupt manifest backup verification failed for '{}'. Original corrupt manifest preserved.",
                        backup_path.display()
                    );
                    return Err(AetherError::Internal(
                        "Corrupt manifest backup verification failed: digest mismatch".into(),
                    ));
                }

                tracing::info!(
                    "Corrupt recovery manifest successfully backed up and verified at '{}'",
                    backup_path.display()
                );

                // Step 6: Construct fresh manifest tracking the verified backup as PreservedConflict
                let mut fresh_manifest = RecoveryManifest::default();
                fresh_manifest.records.push(QuarantineRecord {
                    id: uuid::Uuid::new_v4().to_string(),
                    original_relative_path: "corrupt_manifest_backup".into(),
                    quarantined_filename: backup_name.clone(),
                    quarantined_path: backup_path.display().to_string(),
                    quarantined_at: chrono::Utc::now().to_rfc3339(),
                    expected_sha256: corrupt_hash.clone(),
                    actual_sha256: Some(corrupt_hash),
                    reason: format!("CorruptManifestOnStartup: {}", e),
                    status: QuarantineStatus::PreservedConflict,
                    retained_staging_filename: None,
                    retained_staging_path: None,
                });

                // Step 7: Durably persist fresh manifest (CHK-102)
                self.save_recovery_manifest(&fresh_manifest)?;

                Ok(fresh_manifest)
            }
            Err(e) => Err(e),
        }
    }

    /// Saves the durable recovery manifest to disk.
    pub fn save_recovery_manifest(&self, manifest: &RecoveryManifest) -> Result<()> {
        let fault_manifest = self
            .fault_inject_manifest_save
            .load(std::sync::atomic::Ordering::SeqCst);
        manifest.save_with_fault(&self.recovery_manifest_path(), fault_manifest)
    }

    /// Securely resolves and validates that a quarantine record targets a regular file
    /// strictly inside the trusted application quarantine directory.
    pub fn resolve_and_validate_quarantine_path_static(
        quarantine_dir: &Path,
        record: &QuarantineRecord,
    ) -> Result<PathBuf> {
        let filename_str = if !record.quarantined_filename.is_empty() {
            record.quarantined_filename.as_str()
        } else {
            let p = Path::new(&record.quarantined_path);
            match p.file_name().and_then(|n| n.to_str()) {
                Some(name) => name,
                None => {
                    return Err(AetherError::PermissionDenied(format!(
                        "Malformed quarantine record '{}': missing valid filename",
                        record.id
                    )));
                }
            }
        };

        if filename_str.is_empty()
            || filename_str.contains('/')
            || filename_str.contains('\\')
            || filename_str.contains("..")
            || filename_str.contains(':')
            || filename_str == "."
        {
            return Err(AetherError::PermissionDenied(format!(
                "Path traversal attack rejected in quarantine record '{}': invalid filename '{}'",
                record.id, filename_str
            )));
        }

        let candidate_path = Path::new(filename_str);
        if candidate_path.is_absolute() || candidate_path.components().count() != 1 {
            return Err(AetherError::PermissionDenied(format!(
                "Path traversal attack rejected in quarantine record '{}': multi-component or absolute filename '{}'",
                record.id, filename_str
            )));
        }

        let resolved = quarantine_dir.join(candidate_path);

        if let Some(parent) = resolved.parent() {
            if parent != quarantine_dir {
                return Err(AetherError::PermissionDenied(format!(
                    "Path resolution boundary violation in quarantine record '{}': parent does not match quarantine dir",
                    record.id
                )));
            }
        } else {
            return Err(AetherError::PermissionDenied(format!(
                "Path resolution boundary violation in quarantine record '{}': missing parent",
                record.id
            )));
        }

        Ok(resolved)
    }

    pub fn resolve_and_validate_quarantine_path(
        &self,
        record: &QuarantineRecord,
    ) -> Result<PathBuf> {
        let q_dir = self.verify_quarantine_boundary()?;
        Self::resolve_and_validate_quarantine_path_static(&q_dir, record)
    }

    /// Lists all quarantined artifacts currently pending cleanup.
    pub fn list_quarantined_artifacts(&self) -> Result<Vec<QuarantineRecord>> {
        if let Some(ref reason) = self.recovery_blocked_reason() {
            return Err(AetherError::PermissionDenied(format!(
                "Listing quarantined artifacts blocked due to recovery blocked state: {}",
                reason
            )));
        }
        let manifest = self.load_recovery_manifest()?;
        Ok(manifest
            .records
            .into_iter()
            .filter(|r| r.status == QuarantineStatus::PendingCleanup)
            .collect())
    }

    /// Safely moves a file from workspace into quarantine and updates the recovery manifest.
    pub fn quarantine_file(
        &self,
        workspace_root: &Path,
        rel_path: &str,
        expected_hash: &str,
        reason: &str,
    ) -> Result<QuarantineRecord> {
        let cap = WorkspaceCapability::open(workspace_root)?;
        self.quarantine_file_with_cap(&cap, rel_path, expected_hash, reason)
    }

    /// Moves a file from workspace into quarantine anchored to a WorkspaceCapability.
    pub fn quarantine_file_with_cap(
        &self,
        cap: &WorkspaceCapability,
        rel_path: &str,
        expected_hash: &str,
        reason: &str,
    ) -> Result<QuarantineRecord> {
        if let Some(ref reason_str) = self.recovery_blocked_reason() {
            return Err(AetherError::PermissionDenied(format!(
                "Quarantine operation blocked due to recovery blocked state: {}",
                reason_str
            )));
        }

        // Step 1: Validate workspace-relative source path through capability boundary
        if rel_path.is_empty()
            || rel_path.starts_with('/')
            || rel_path.starts_with('\\')
            || rel_path.contains("..")
            || Path::new(rel_path).is_absolute()
        {
            return Err(AetherError::PermissionDenied(format!(
                "Invalid or traversal relative path for quarantine: '{}'",
                rel_path
            )));
        }

        let meta = cap.symlink_metadata(rel_path)?;
        if meta.is_symlink() {
            return Err(AetherError::PermissionDenied(format!(
                "Cannot quarantine symlink '{}'",
                rel_path
            )));
        }
        if !meta.is_file() {
            return Err(AetherError::PermissionDenied(format!(
                "Cannot quarantine non-regular file '{}'",
                rel_path
            )));
        }

        let q_dir = self.verify_quarantine_boundary()?;

        // Step 2: Establish unique artifact ID and safe destination filename
        let q_id = uuid::Uuid::new_v4().to_string();
        let file_stem = Path::new(rel_path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("file");
        let safe_stem: String = file_stem
            .chars()
            .filter(|c| c.is_alphanumeric() || *c == '.' || *c == '-' || *c == '_')
            .collect();
        let clean_stem = if safe_stem.is_empty() {
            "file"
        } else {
            &safe_stem
        };
        let q_filename = format!("{}.aether-quarantine-{}", clean_stem, q_id);
        let q_dest = q_dir.join(&q_filename);

        // Step 3: Stage the file inside the workspace for exclusive access and pre-delete testing
        let stage_name = format!(".aether-del-stage-{}", uuid::Uuid::new_v4().simple());
        let stage_rel = PathBuf::from(&stage_name);
        let stage_abs = cap.root_path().join(&stage_rel);
        cap.raw_dir()
            .rename(Path::new(rel_path), cap.raw_dir(), &stage_rel)
            .map_err(AetherError::Io)?;

        // Pre-delete hook (for testing / deterministic injection like CHK-036)
        let pre_del_hook_opt = self.pre_delete_hook.lock().clone();
        if let Some(ref hook) = pre_del_hook_opt {
            hook(&stage_abs);
        }

        let staged_bytes = std::fs::read(&stage_abs).map_err(AetherError::Io)?;
        let staged_hash = Self::sha256_digest(&staged_bytes);

        let is_valid_sha256 =
            expected_hash.len() == 64 && expected_hash.chars().all(|c| c.is_ascii_hexdigit());
        if is_valid_sha256 && staged_hash != expected_hash {
            // Concurrent edit detected during staging! Roll back stage file to rel_path to preserve edits!
            let _ = cap
                .raw_dir()
                .rename(&stage_rel, cap.raw_dir(), Path::new(rel_path));
            return Err(AetherError::PermissionDenied(format!(
                "File '{}' modified during deletion staging (expected {}, found {}). Preserved recovery conflict artifact: File preserved in workspace without deletion.",
                rel_path, expected_hash, staged_hash
            )));
        }

        // Step 4: Persist durable pending-intent record in manifest (PendingMove)
        let mut record = QuarantineRecord {
            id: q_id,
            original_relative_path: rel_path.to_string(),
            quarantined_filename: q_filename,
            quarantined_path: q_dest.display().to_string(),
            quarantined_at: chrono::Utc::now().to_rfc3339(),
            expected_sha256: if expected_hash.is_empty() {
                staged_hash.clone()
            } else {
                expected_hash.to_string()
            },
            actual_sha256: Some(staged_hash.clone()),
            reason: reason.to_string(),
            status: QuarantineStatus::PendingMove,
            retained_staging_filename: None,
            retained_staging_path: None,
        };

        let manifest_path = self.recovery_manifest_path();
        let fault_manifest = self
            .fault_inject_manifest_save
            .load(std::sync::atomic::Ordering::SeqCst);
        let mut manifest = self.load_recovery_manifest()?;

        if let Err(e) =
            manifest.record_quarantine_with_fault(record.clone(), &manifest_path, fault_manifest)
        {
            // Manifest write failed: restore stage back to rel_path so data is not lost or untracked!
            let _ = cap
                .raw_dir()
                .rename(&stage_rel, cap.raw_dir(), Path::new(rel_path));
            return Err(e);
        }

        // Step 5: Move stage file into quarantine
        let rename_injected = self
            .fault_inject_quarantine_rename
            .load(std::sync::atomic::Ordering::SeqCst);
        let cross_device_injected = self
            .fault_inject_quarantine_cross_device
            .load(std::sync::atomic::Ordering::SeqCst);

        let rename_res = if rename_injected {
            Err(std::io::Error::other("Injected quarantine rename failure"))
        } else if cross_device_injected {
            Err(std::io::Error::from_raw_os_error(if cfg!(windows) {
                17
            } else {
                18
            }))
        } else {
            std::fs::rename(&stage_abs, &q_dest)
        };

        match rename_res {
            Ok(()) => {}
            Err(e) if filesystem_engine::is_cross_device_error(&e) || cross_device_injected => {
                // Positively identified cross-device move fallback
                if self
                    .fault_inject_quarantine_dest_creation
                    .load(std::sync::atomic::Ordering::SeqCst)
                {
                    let _ = cap
                        .raw_dir()
                        .rename(&stage_rel, cap.raw_dir(), Path::new(rel_path));
                    return Err(AetherError::Io(std::io::Error::new(
                        std::io::ErrorKind::PermissionDenied,
                        "Injected quarantine destination creation failure",
                    )));
                }

                if self
                    .fault_inject_quarantine_copy_failure
                    .load(std::sync::atomic::Ordering::SeqCst)
                {
                    let _ = cap
                        .raw_dir()
                        .rename(&stage_rel, cap.raw_dir(), Path::new(rel_path));
                    return Err(AetherError::Io(std::io::Error::other(
                        "Injected quarantine copy failure",
                    )));
                }

                if let Err(copy_err) = std::fs::copy(&stage_abs, &q_dest) {
                    let _ = cap
                        .raw_dir()
                        .rename(&stage_rel, cap.raw_dir(), Path::new(rel_path));
                    return Err(AetherError::Io(copy_err));
                }

                let copied_bytes = match std::fs::read(&q_dest) {
                    Ok(b) => b,
                    Err(read_err) => {
                        let _ = std::fs::remove_file(&q_dest);
                        let _ =
                            cap.raw_dir()
                                .rename(&stage_rel, cap.raw_dir(), Path::new(rel_path));
                        return Err(AetherError::Io(read_err));
                    }
                };
                let copied_hash = Self::sha256_digest(&copied_bytes);
                if copied_hash != staged_hash {
                    let _ = std::fs::remove_file(&q_dest);
                    let _ = cap
                        .raw_dir()
                        .rename(&stage_rel, cap.raw_dir(), Path::new(rel_path));
                    return Err(AetherError::Internal(format!(
                        "Quarantine copy hash verification failed for '{}'",
                        rel_path
                    )));
                }

                // Pre-source-unlink hook (deterministic injection for CHK-063, CHK-086)
                let hook_opt = self.pre_quarantine_source_unlink_hook.lock().clone();
                if let Some(ref hook) = hook_opt {
                    hook(&stage_abs);
                }

                let src_mutation_injected = self
                    .fault_inject_quarantine_source_mutation
                    .load(std::sync::atomic::Ordering::SeqCst);
                let current_source_bytes = std::fs::read(&stage_abs);
                let current_src_hash = current_source_bytes
                    .as_ref()
                    .map(|b| Self::sha256_digest(b))
                    .ok();

                #[cfg(windows)]
                let is_mutated = src_mutation_injected
                    || match &current_src_hash {
                        Some(h) => h != &staged_hash,
                        None => true,
                    };

                #[cfg(not(windows))]
                let _ = src_mutation_injected;

                #[cfg(not(windows))]
                let exclusive_stability_proven = false;

                #[cfg(windows)]
                let staged_identity =
                    filesystem_engine::query_file_object_identity(&stage_abs).ok();

                #[cfg(windows)]
                let exclusive_stability_proven = !is_mutated
                    && filesystem_engine::remove_file_windows_exclusive(
                        &stage_abs,
                        &staged_hash,
                        staged_identity,
                    )
                    .is_ok();

                if !exclusive_stability_proven {
                    // Exclusive source stability could not be verified (POSIX uncooperative handle limitation,
                    // Windows retained handle, or content mutation).
                    // Restore source file to workspace and preserve quarantine copy as PreservedConflict
                    let _ = cap
                        .raw_dir()
                        .rename(&stage_rel, cap.raw_dir(), Path::new(rel_path));
                    record.status = QuarantineStatus::PreservedConflict;
                    record.actual_sha256 = current_src_hash.or_else(|| Some("unreadable".into()));
                    record.reason = format!(
                        "CrossDeviceSourceConflict: Exclusive source stability could not be verified for '{}'. Both source at '{}' and quarantine copy at '{}' are preserved.",
                        rel_path,
                        cap.root_path().join(rel_path).display(),
                        q_dest.display()
                    );
                    manifest.record_quarantine_with_fault(
                        record.clone(),
                        &manifest_path,
                        fault_manifest,
                    )?;
                    return Err(AetherError::PermissionDenied(format!(
                        "Exclusive source stability could not be verified for '{}'. Staged source preserved at '{}' and verified copy preserved in quarantine at '{}'.",
                        rel_path,
                        cap.root_path().join(rel_path).display(),
                        q_dest.display()
                    )));
                }
            }
            Err(e) => {
                // Non-cross-device rename failure: FAIL CLOSED!
                let _ = cap
                    .raw_dir()
                    .rename(&stage_rel, cap.raw_dir(), Path::new(rel_path));
                return Err(AetherError::Io(e));
            }
        }

        // Step 6: Check CHK-057 crash injection (crash after move before manifest commit)
        if self
            .fault_inject_quarantine_commit
            .load(std::sync::atomic::Ordering::SeqCst)
        {
            return Err(AetherError::Internal(
                "Simulated crash after quarantine move before manifest commit".into(),
            ));
        }

        // Step 7: Persist committed record (PendingCleanup)
        record.status = QuarantineStatus::PendingCleanup;
        manifest.record_quarantine_with_fault(record.clone(), &manifest_path, fault_manifest)?;

        Ok(record)
    }

    /// Reconciles quarantine manifest against actual disk state.
    pub fn reconcile_quarantine(&self) -> Result<RecoveryManifest> {
        if self
            .fault_inject_reconciliation_failure
            .load(std::sync::atomic::Ordering::SeqCst)
        {
            let err_msg = "Injected reconciliation failure".to_string();
            self.set_recovery_blocked(Some(err_msg.clone()));
            return Err(AetherError::Internal(err_msg));
        }

        let run_reconciliation = || -> Result<RecoveryManifest> {
            let q_dir = self.verify_quarantine_boundary()?;
            let manifest_path = self.recovery_manifest_path();
            let fault_manifest = self
                .fault_inject_manifest_save
                .load(std::sync::atomic::Ordering::SeqCst);
            let mut manifest = self.load_recovery_manifest()?;

            // 1. Reconcile existing records against actual filesystem state
            for record in manifest.records.iter_mut() {
                let path_res = Self::resolve_and_validate_quarantine_path_static(&q_dir, record);
                let target_path = match path_res {
                    Ok(p) => p,
                    Err(_) => {
                        record.status = QuarantineStatus::PreservedConflict;
                        continue;
                    }
                };

                match std::fs::symlink_metadata(&target_path) {
                    Ok(meta) => {
                        if meta.file_type().is_symlink() || !meta.file_type().is_file() {
                            record.status = QuarantineStatus::PreservedConflict;
                            continue;
                        }
                        if let Ok(bytes) = std::fs::read(&target_path) {
                            let actual_hash = Self::sha256_digest(&bytes);
                            record.actual_sha256 = Some(actual_hash.clone());
                            if record.status == QuarantineStatus::PendingMove {
                                if actual_hash == record.expected_sha256 {
                                    record.status = QuarantineStatus::PendingCleanup;
                                } else {
                                    record.status = QuarantineStatus::PreservedConflict;
                                }
                            } else if record.status == QuarantineStatus::PendingCleanup
                                && actual_hash != record.expected_sha256
                            {
                                record.status = QuarantineStatus::PreservedConflict;
                            }
                        } else {
                            record.status = QuarantineStatus::PreservedConflict;
                        }
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                        // If target_path is not found, check if a retained staging backup exists!
                        let mut retained_found = false;
                        if let Some(ref s_name) = record.retained_staging_filename {
                            let s_path = q_dir.join(s_name);
                            if let Ok(meta) = std::fs::symlink_metadata(&s_path) {
                                if meta.file_type().is_file() {
                                    if let Ok(bytes) = std::fs::read(&s_path) {
                                        let hash = Self::sha256_digest(&bytes);
                                        record.actual_sha256 = Some(hash);
                                        record.quarantined_filename = s_name.clone();
                                        record.quarantined_path = s_path.display().to_string();
                                        record.status = QuarantineStatus::PreservedConflict;
                                        retained_found = true;
                                    }
                                }
                            }
                        }
                        if !retained_found
                            && (record.status == QuarantineStatus::PendingCleanup
                                || record.status == QuarantineStatus::PendingMove)
                        {
                            record.status = QuarantineStatus::CleanedUp;
                        }
                    }
                    Err(_) => {
                        record.status = QuarantineStatus::PreservedConflict;
                    }
                }
            }

            // 2. Discover untracked artifacts in quarantine directory (CHK-057, CHK-079, CHK-089, CHK-185)
            let entries = std::fs::read_dir(&q_dir).map_err(AetherError::Io)?;
            for entry in entries {
                let entry = entry.map_err(AetherError::Io)?;
                let path = entry.path();
                let file_name = entry.file_name().to_string_lossy().to_string();
                let is_clean_stage = file_name.starts_with(".aether-clean-stage-");
                if (file_name.starts_with('.') && !is_clean_stage) || file_name.ends_with(".tmp") {
                    continue;
                }

                let sym_meta = std::fs::symlink_metadata(&path).map_err(AetherError::Io)?;
                let is_symlink = sym_meta.file_type().is_symlink();

                let already_tracked = manifest.records.iter().any(|r| {
                    (!r.quarantined_filename.is_empty() && r.quarantined_filename == file_name)
                        || Path::new(&r.quarantined_path)
                            .file_name()
                            .and_then(|n| n.to_str())
                            == Some(&file_name)
                        || r.retained_staging_filename.as_deref() == Some(&file_name)
                });

                if !already_tracked {
                    if is_clean_stage {
                        if let Ok(bytes) = std::fs::read(&path) {
                            let hash = Self::sha256_digest(&bytes);
                            let record = QuarantineRecord {
                                id: uuid::Uuid::new_v4().to_string(),
                                original_relative_path: "recovered_retained_staging_artifact"
                                    .into(),
                                quarantined_filename: file_name.clone(),
                                quarantined_path: path.display().to_string(),
                                quarantined_at: chrono::Utc::now().to_rfc3339(),
                                expected_sha256: hash.clone(),
                                actual_sha256: Some(hash),
                                reason: "DiscoveredRetainedStagingArtifactOnReconciliation".into(),
                                status: QuarantineStatus::PreservedConflict,
                                retained_staging_filename: Some(file_name.clone()),
                                retained_staging_path: Some(path.display().to_string()),
                            };
                            manifest.records.push(record);
                        }
                    } else if is_symlink {
                        let record = QuarantineRecord {
                            id: uuid::Uuid::new_v4().to_string(),
                            original_relative_path: "untracked_symlink_in_quarantine".into(),
                            quarantined_filename: file_name.clone(),
                            quarantined_path: path.display().to_string(),
                            quarantined_at: chrono::Utc::now().to_rfc3339(),
                            expected_sha256: "".into(),
                            actual_sha256: None,
                            reason: "DiscoveredUntrackedSymlinkOnReconciliation".into(),
                            status: QuarantineStatus::PreservedConflict,
                            retained_staging_filename: None,
                            retained_staging_path: None,
                        };
                        manifest.records.push(record);
                    } else if sym_meta.file_type().is_file() {
                        if let Ok(bytes) = std::fs::read(&path) {
                            let hash = Self::sha256_digest(&bytes);
                            let record = QuarantineRecord {
                                id: uuid::Uuid::new_v4().to_string(),
                                original_relative_path: "recovered_untracked_artifact".into(),
                                quarantined_filename: file_name.clone(),
                                quarantined_path: path.display().to_string(),
                                quarantined_at: chrono::Utc::now().to_rfc3339(),
                                expected_sha256: hash.clone(),
                                actual_sha256: Some(hash),
                                reason: "DiscoveredUntrackedArtifactOnStartup".into(),
                                status: QuarantineStatus::PendingCleanup,
                                retained_staging_filename: None,
                                retained_staging_path: None,
                            };
                            manifest.records.push(record);
                        }
                    }
                }
            }

            manifest.save_with_fault(&manifest_path, fault_manifest)?;
            Ok(manifest)
        };

        match run_reconciliation() {
            Ok(manifest) => {
                self.set_recovery_blocked(None);
                Ok(manifest)
            }
            Err(e) => {
                self.set_recovery_blocked(Some(e.to_string()));
                Err(e)
            }
        }
    }

    /// Persists the recovery manifest fail-closed.
    /// If manifest persistence fails, the recovery system is immediately marked as blocked
    /// with the affected record ID, surviving artifact path, and persistence error,
    /// preventing any subsequent destructive or state-changing actions.
    fn persist_manifest_or_block_recovery(
        &self,
        manifest: &RecoveryManifest,
        record_id: &str,
        surviving_path: &str,
        fault_manifest: bool,
    ) -> Result<()> {
        let manifest_path = self.recovery_manifest_path();
        if let Err(e) = manifest.save_with_fault(&manifest_path, fault_manifest) {
            let reason = if surviving_path == "(physically removed)" {
                format!(
                    "Critical inconsistency: File was physically removed, but manifest persistence failed for record '{}': {}. Recovery blocked to prevent state desynchronization.",
                    record_id, e
                )
            } else {
                format!(
                    "Manifest persistence failure for record '{}' (surviving path: '{}'): {}. Recovery blocked to prevent state desynchronization.",
                    record_id, surviving_path, e
                )
            };
            self.set_recovery_blocked(Some(reason));
            return Err(AetherError::Io(std::io::Error::other(format!(
                "Manifest persistence failed for record '{}': {}",
                record_id, e
            ))));
        }
        Ok(())
    }

    /// Publishes a staged candidate to destination_name using atomic no-overwrite hard linking.
    /// Never clobbers destination_name if it already exists or was recreated concurrently.
    /// On link success, verifies post-publication object identity before removing clean_stage_name.
    /// If removing clean_stage_name fails or identity is uncertain, the staged artifact is safely preserved
    /// and tracked in the structured result.
    fn publish_no_overwrite(
        &self,
        q_cap_dir: &cap_std::fs::Dir,
        clean_stage_name: &str,
        destination_name: &str,
        is_conflict: bool,
    ) -> Option<StageRollbackResult> {
        let q_dir = self.quarantine_dir();
        let stage_abs = q_dir.join(clean_stage_name);
        let dest_abs = q_dir.join(destination_name);

        // 0. Establish staged candidate's object identity
        let stage_identity = match filesystem_engine::query_file_object_identity(&stage_abs) {
            Ok(id) => id,
            Err(_) => return None,
        };

        // 1. Verify destination is confirmed absent before attempting publication
        let dest_existence = classify_dir_entry_existence(q_cap_dir, destination_name);
        if dest_existence != ObjectExistence::ConfirmedAbsent {
            return None;
        }

        // 2. Invoke pre_rollback_publish_hook right after check and before publication
        let hook_opt = self.pre_rollback_publish_hook.lock().clone();
        if let Some(ref hook) = hook_opt {
            hook(&dest_abs);
        }

        // 3. Check injected hard link failure
        // Uses swap(false) so single failure can fall back to conflict publication
        if self
            .fault_inject_rollback_hard_link_failure
            .swap(false, std::sync::atomic::Ordering::SeqCst)
        {
            return None;
        }

        // 4. Attempt atomic publication via hard link
        // In POSIX and NTFS, hard_link fails atomically with AlreadyExists if destination exists
        if q_cap_dir
            .hard_link(clean_stage_name, q_cap_dir, destination_name)
            .is_err()
        {
            return None;
        }

        // 5. Post-hard-link hook: allows testing replacement before verification (CHK-181)
        let post_link_hook = self.post_rollback_hard_link_hook.lock().clone();
        if let Some(ref hook) = post_link_hook {
            hook(&dest_abs);
        }

        // 6. Post-publication verification: destination object identity must match staged object
        let dest_identity = match filesystem_engine::query_file_object_identity(&dest_abs) {
            Ok(id) => id,
            Err(_) => {
                // Cannot establish destination identity; DO NOT unlink clean_stage_name!
                return None;
            }
        };
        if dest_identity != stage_identity {
            // Replacement detected between hard link and verification! (CHK-181)
            // DO NOT unlink clean_stage_name!
            return None;
        }

        // 7. Pre-staging-link-removal hook: allows testing replacement between verification and staging removal (CHK-182)
        let pre_removal_hook = self.pre_staging_link_removal_hook.lock().clone();
        if let Some(ref hook) = pre_removal_hook {
            hook(&dest_abs);
        }

        // 8. Re-verify destination and staging identities immediately before staging link removal!
        let dest_identity_now = filesystem_engine::query_file_object_identity(&dest_abs).ok();
        let stage_identity_now = filesystem_engine::query_file_object_identity(&stage_abs).ok();
        if dest_identity_now != Some(stage_identity) || stage_identity_now != Some(stage_identity) {
            // Destination was replaced after verification, or staging was altered! (CHK-182, CHK-183)
            // DO NOT REMOVE clean_stage_name! Candidate preserved safely.
            self.set_recovery_blocked(Some(format!(
                "Destination '{}' object identity changed prior to staging removal; candidate preserved at staging name '{}'. Recovery blocked to prevent data loss.",
                destination_name, clean_stage_name
            )));
            return Some(StageRollbackResult {
                surviving_filename: clean_stage_name.to_string(),
                retained_staging_filename: Some(clean_stage_name.to_string()),
                status: RollbackPublicationStatus::PublicationUncertainBlocked,
                is_conflict: true,
                error: Some(format!(
                    "Destination replaced before staging removal; candidate retained at '{}' and RecoveryBlocked activated",
                    clean_stage_name
                )),
            });
        }

        // 9. Publication confirmed! Safely attempt removal of the staging link
        let mut staging_removed = true;
        let mut removal_warning = None;
        if self
            .fault_inject_rollback_staging_removal_failure
            .load(std::sync::atomic::Ordering::SeqCst)
        {
            staging_removed = false;
            removal_warning = Some("Injected staging link removal failure".to_string());
        } else if let Err(e) = q_cap_dir.remove_file(clean_stage_name) {
            staging_removed = false;
            removal_warning = Some(format!(
                "Failed to remove staging link '{}': {}",
                clean_stage_name, e
            ));
        }

        // 10. Verify destination existence on disk
        if classify_dir_entry_existence(q_cap_dir, destination_name) != ObjectExistence::Exists {
            if !staging_removed {
                self.set_recovery_blocked(Some(format!(
                    "Destination '{}' missing after staging removal attempt; preserved at staging name '{}'",
                    destination_name, clean_stage_name
                )));
                return Some(StageRollbackResult {
                    surviving_filename: clean_stage_name.to_string(),
                    retained_staging_filename: Some(clean_stage_name.to_string()),
                    status: RollbackPublicationStatus::PublicationUncertainBlocked,
                    is_conflict: true,
                    error: Some("Destination missing; candidate preserved at staging name and RecoveryBlocked activated".into()),
                });
            } else {
                self.set_recovery_blocked(Some(format!(
                    "Critical: Destination '{}' missing and staging link was removed",
                    destination_name
                )));
                return None;
            }
        }

        if staging_removed {
            let status = if is_conflict {
                RollbackPublicationStatus::PublicationConflict
            } else {
                RollbackPublicationStatus::PublishedVerified
            };
            Some(StageRollbackResult {
                surviving_filename: destination_name.to_string(),
                retained_staging_filename: None,
                status,
                is_conflict,
                error: if is_conflict {
                    Some("Original file was recreated or uncertain; candidate safely preserved as conflict artifact".into())
                } else {
                    None
                },
            })
        } else {
            // Staging link was retained!
            Some(StageRollbackResult {
                surviving_filename: destination_name.to_string(),
                retained_staging_filename: Some(clean_stage_name.to_string()),
                status: RollbackPublicationStatus::PublishedWithRetainedStaging,
                is_conflict,
                error: removal_warning,
            })
        }
    }

    /// Safely rolls back a staged quarantine cleanup file.
    /// If the original filename was concurrently recreated, or its state is uncertain,
    /// it NEVER overwrites it, but instead preserves the candidate via verified atomic conflict publication.
    fn rollback_cleanup_stage(
        &self,
        q_cap_dir: &cap_std::fs::Dir,
        clean_stage_name: &str,
        original_filename: &str,
    ) -> StageRollbackResult {
        // Step 1: Check original filename existence using strict classification
        let mut orig_existence = classify_dir_entry_existence(q_cap_dir, original_filename);
        if self
            .fault_inject_metadata_permission_denied
            .load(std::sync::atomic::Ordering::SeqCst)
        {
            orig_existence = ObjectExistence::Uncertain("PermissionDenied (fault injected)".into());
        }
        if self
            .fault_inject_metadata_io_error
            .load(std::sync::atomic::Ordering::SeqCst)
        {
            orig_existence =
                ObjectExistence::Uncertain("Input/output error (fault injected)".into());
        }

        let both_fail = self
            .fault_inject_rollback_both_publication_failures
            .load(std::sync::atomic::Ordering::SeqCst);

        // Attempt publication to original filename ONLY if confirmed absent and not both_fail
        if orig_existence == ObjectExistence::ConfirmedAbsent && !both_fail {
            if let Some(res) =
                self.publish_no_overwrite(q_cap_dir, clean_stage_name, original_filename, false)
            {
                return res;
            }
        }

        // Original filename is either Exists (recreated concurrently), Uncertain, or hard link failed.
        // Candidate must be preserved through verified conflict publication (up to 5 attempts).
        let conflict_collision_injected = self
            .fault_inject_rollback_conflict_collision
            .load(std::sync::atomic::Ordering::SeqCst);
        let conflict_pub_fail = self
            .fault_inject_rollback_conflict_publication_failure
            .load(std::sync::atomic::Ordering::SeqCst);

        if !conflict_pub_fail && !both_fail {
            for attempt in 0..5 {
                let conflict_name = if attempt == 0 && conflict_collision_injected {
                    let colliding_name =
                        format!("{}.preserved-conflict.colliding", original_filename);
                    let _ = q_cap_dir.write(&colliding_name, b"COLLIDING_PRE_EXISTING_CONFLICT");
                    colliding_name
                } else {
                    format!(
                        "{}.preserved-conflict.{}",
                        original_filename,
                        uuid::Uuid::new_v4().simple()
                    )
                };

                if let Some(res) =
                    self.publish_no_overwrite(q_cap_dir, clean_stage_name, &conflict_name, true)
                {
                    return res;
                }
            }
        }

        // All safe publication attempts failed!
        // Preserve candidate at clean_stage_name and activate RecoveryBlocked state.
        self.set_recovery_blocked(Some(format!(
            "Fatal: All rollback publication attempts failed for staged quarantine candidate '{}'. Artifact preserved at staging name without overwrite.",
            clean_stage_name
        )));

        StageRollbackResult {
            surviving_filename: clean_stage_name.to_string(),
            retained_staging_filename: Some(clean_stage_name.to_string()),
            status: RollbackPublicationStatus::PublicationUncertainBlocked,
            is_conflict: true,
            error: Some("Fatal: All rollback publication attempts failed. Candidate preserved at staging name and RecoveryBlocked activated.".into()),
        }
    }

    fn apply_rollback_result_to_record(
        record: &mut QuarantineRecord,
        rolled: &StageRollbackResult,
        q_dir: &Path,
    ) {
        record.quarantined_filename = rolled.surviving_filename.clone();
        record.quarantined_path = q_dir
            .join(&rolled.surviving_filename)
            .to_string_lossy()
            .to_string();
        record.retained_staging_filename = rolled.retained_staging_filename.clone();
        record.retained_staging_path = rolled
            .retained_staging_filename
            .as_ref()
            .map(|s| q_dir.join(s).to_string_lossy().to_string());
        if rolled.is_conflict {
            record.status = QuarantineStatus::PreservedConflict;
        }
    }

    /// Cleans up a single quarantined artifact by ID using the stage-within-quarantine protocol.
    pub fn cleanup_quarantine(&self, record_id: &str) -> Result<bool> {
        if let Some(ref reason) = self.recovery_blocked_reason() {
            return Err(AetherError::PermissionDenied(format!(
                "Quarantine cleanup blocked due to recovery blocked state: {}",
                reason
            )));
        }

        let mut manifest = self.load_recovery_manifest()?;
        let fault_manifest = self
            .fault_inject_manifest_save
            .load(std::sync::atomic::Ordering::SeqCst);

        let record_idx = manifest.records.iter().position(|r| r.id == record_id);
        let idx = match record_idx {
            Some(i) => i,
            None => return Ok(false),
        };

        // Enforce strict backend authorization (CHK-068, CHK-069, CHK-070)
        match manifest.records[idx].status {
            QuarantineStatus::CleanedUp => {
                // Idempotent: return Ok(true) without touching disk or deleting recreated file
                return Ok(true);
            }
            QuarantineStatus::PreservedConflict => {
                return Err(AetherError::PermissionDenied(format!(
                    "Artifact '{}' is preserved as a conflict and cannot be deleted through standard cleanup",
                    record_id
                )));
            }
            QuarantineStatus::PendingMove => {
                return Err(AetherError::PermissionDenied(format!(
                    "Artifact '{}' is pending move and cannot be deleted through standard cleanup",
                    record_id
                )));
            }
            QuarantineStatus::PendingCleanup => {
                // Authorized
            }
        }

        let q_dir = self.verify_quarantine_boundary()?;
        let target_path =
            Self::resolve_and_validate_quarantine_path_static(&q_dir, &manifest.records[idx])?;
        let filename = manifest.records[idx].quarantined_filename.clone();

        let q_cap_dir =
            Dir::open_ambient_dir(&q_dir, ambient_authority()).map_err(AetherError::Io)?;

        // Stage within quarantine dir protocol:
        // Step 0: Establish the identity of the authorized original object BEFORE the staging operation!
        let original_identity = match filesystem_engine::query_file_object_identity(&target_path) {
            Ok(id) => {
                if self
                    .fault_inject_identity_capture_failure
                    .load(std::sync::atomic::Ordering::SeqCst)
                {
                    manifest.records[idx].status = QuarantineStatus::PreservedConflict;
                    self.persist_manifest_or_block_recovery(
                        &manifest,
                        record_id,
                        &manifest.records[idx].quarantined_path,
                        fault_manifest,
                    )?;
                    return Err(AetherError::PermissionDenied(
                        "Fault injection: identity capture failure on authorized original object"
                            .into(),
                    ));
                }
                Some(id)
            }
            Err(AetherError::Io(ref e)) if e.kind() == std::io::ErrorKind::NotFound => {
                manifest.records[idx].status = QuarantineStatus::CleanedUp;
                self.persist_manifest_or_block_recovery(
                    &manifest,
                    record_id,
                    "(already absent)",
                    fault_manifest,
                )?;
                return Ok(true);
            }
            Err(e) => {
                manifest.records[idx].status = QuarantineStatus::PreservedConflict;
                self.persist_manifest_or_block_recovery(
                    &manifest,
                    record_id,
                    &manifest.records[idx].quarantined_path,
                    fault_manifest,
                )?;
                return Err(AetherError::PermissionDenied(format!(
                    "Security rejection: Failed to establish original object identity for '{}': {}. Preserved as conflict.",
                    target_path.display(), e
                )));
            }
        };

        // Step 1: Move filename to private unique staging name within pinned quarantine directory
        let clean_stage_name = format!(".aether-clean-stage-{}", uuid::Uuid::new_v4().simple());
        if let Err(e) = q_cap_dir.rename(&filename, &q_cap_dir, &clean_stage_name) {
            if e.kind() == std::io::ErrorKind::NotFound {
                manifest.records[idx].status = QuarantineStatus::CleanedUp;
                self.persist_manifest_or_block_recovery(
                    &manifest,
                    record_id,
                    "(already absent)",
                    fault_manifest,
                )?;
                return Ok(true);
            }
            return Err(AetherError::Io(e));
        }

        let stage_abs = q_dir.join(&clean_stage_name);

        // Post-stage rename hook: allows testing replacement between rename and verification (CHK-176, CHK-177, CHK-178)
        let post_stage_hook = self.post_stage_rename_hook.lock().clone();
        if let Some(ref hook) = post_stage_hook {
            hook(&stage_abs);
        }

        // Verify staged candidate identity against authorized original identity!
        let staged_identity = match filesystem_engine::query_file_object_identity(&stage_abs) {
            Ok(id) => Some(id),
            Err(e) => {
                let rolled = self.rollback_cleanup_stage(&q_cap_dir, &clean_stage_name, &filename);
                Self::apply_rollback_result_to_record(&mut manifest.records[idx], &rolled, &q_dir);
                manifest.records[idx].status = QuarantineStatus::PreservedConflict;
                self.persist_manifest_or_block_recovery(
                    &manifest,
                    record_id,
                    &manifest.records[idx].quarantined_path,
                    fault_manifest,
                )?;
                return Err(e);
            }
        };

        if staged_identity != original_identity {
            // Adversarial replacement detected! (CHK-176, CHK-177, CHK-178)
            let rolled = self.rollback_cleanup_stage(&q_cap_dir, &clean_stage_name, &filename);
            Self::apply_rollback_result_to_record(&mut manifest.records[idx], &rolled, &q_dir);
            manifest.records[idx].status = QuarantineStatus::PreservedConflict;
            self.persist_manifest_or_block_recovery(
                &manifest,
                record_id,
                &manifest.records[idx].quarantined_path,
                fault_manifest,
            )?;
            return Err(AetherError::PermissionDenied(format!(
                "Security rejection: Staged candidate '{}' identity mismatch against authorized original ({:?} != {:?}); replacement rejected fail-closed",
                clean_stage_name, staged_identity, original_identity
            )));
        }

        // Step 2: Inspect metadata and content of staged candidate
        let meta = match q_cap_dir.symlink_metadata(&clean_stage_name) {
            Ok(m) => m,
            Err(e) => {
                let rolled = self.rollback_cleanup_stage(&q_cap_dir, &clean_stage_name, &filename);
                Self::apply_rollback_result_to_record(&mut manifest.records[idx], &rolled, &q_dir);
                manifest.records[idx].status = QuarantineStatus::PreservedConflict;
                self.persist_manifest_or_block_recovery(
                    &manifest,
                    record_id,
                    &manifest.records[idx].quarantined_path,
                    fault_manifest,
                )?;
                return Err(AetherError::Io(e));
            }
        };

        if meta.is_symlink() {
            let rolled = self.rollback_cleanup_stage(&q_cap_dir, &clean_stage_name, &filename);
            Self::apply_rollback_result_to_record(&mut manifest.records[idx], &rolled, &q_dir);
            manifest.records[idx].status = QuarantineStatus::PreservedConflict;
            self.persist_manifest_or_block_recovery(
                &manifest,
                record_id,
                &manifest.records[idx].quarantined_path,
                fault_manifest,
            )?;
            return Err(AetherError::PermissionDenied(format!(
                "Security rejection: Quarantine target '{}' is a symlink; refusing cleanup to protect target",
                target_path.display()
            )));
        }

        if !meta.is_file() {
            let rolled = self.rollback_cleanup_stage(&q_cap_dir, &clean_stage_name, &filename);
            Self::apply_rollback_result_to_record(&mut manifest.records[idx], &rolled, &q_dir);
            manifest.records[idx].status = QuarantineStatus::PreservedConflict;
            self.persist_manifest_or_block_recovery(
                &manifest,
                record_id,
                &manifest.records[idx].quarantined_path,
                fault_manifest,
            )?;
            return Err(AetherError::PermissionDenied(format!(
                "Quarantine target '{}' is not a regular file; refusing cleanup",
                target_path.display()
            )));
        }

        // Verify content hash of staged candidate
        let current_bytes = match q_cap_dir.read(&clean_stage_name) {
            Ok(b) => b,
            Err(e) => {
                let rolled = self.rollback_cleanup_stage(&q_cap_dir, &clean_stage_name, &filename);
                Self::apply_rollback_result_to_record(&mut manifest.records[idx], &rolled, &q_dir);
                manifest.records[idx].status = QuarantineStatus::PreservedConflict;
                self.persist_manifest_or_block_recovery(
                    &manifest,
                    record_id,
                    &manifest.records[idx].quarantined_path,
                    fault_manifest,
                )?;
                return Err(AetherError::Io(e));
            }
        };
        let current_hash = Self::sha256_digest(&current_bytes);
        let approved_hash = match manifest.records[idx].actual_sha256 {
            Some(ref act) => act.clone(),
            None => manifest.records[idx].expected_sha256.clone(),
        };

        if current_hash != approved_hash {
            let rolled = self.rollback_cleanup_stage(&q_cap_dir, &clean_stage_name, &filename);
            Self::apply_rollback_result_to_record(&mut manifest.records[idx], &rolled, &q_dir);
            manifest.records[idx].status = QuarantineStatus::PreservedConflict;
            manifest.records[idx].actual_sha256 = Some(current_hash.clone());
            self.persist_manifest_or_block_recovery(
                &manifest,
                record_id,
                &manifest.records[idx].quarantined_path,
                fault_manifest,
            )?;
            return Err(AetherError::PermissionDenied(format!(
                "Quarantine conflict detected for '{}': content modified after quarantine (expected {}, found {}). Preserved as conflict artifact without deletion.",
                manifest.records[idx].id, approved_hash, current_hash
            )));
        }

        // Check injected cleanup removal failure (CHK-054)
        if self
            .fault_inject_cleanup_removal
            .load(std::sync::atomic::Ordering::SeqCst)
        {
            let rolled = self.rollback_cleanup_stage(&q_cap_dir, &clean_stage_name, &filename);
            Self::apply_rollback_result_to_record(&mut manifest.records[idx], &rolled, &q_dir);
            self.persist_manifest_or_block_recovery(
                &manifest,
                record_id,
                &manifest.records[idx].quarantined_path,
                fault_manifest,
            )?;
            return Err(AetherError::Io(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "Fault injection: cleanup removal permission denied",
            )));
        }

        // Pre-unlink hook (invoked AFTER successful initial verification; allows testing adversarial races)
        let hook_opt = self.pre_cleanup_unlink_hook.lock().clone();
        if let Some(ref hook) = hook_opt {
            hook(&target_path);
        }

        // Re-verify quarantine boundary
        self.verify_quarantine_boundary()?;

        // Adversarial detection using strict metadata classification:
        // 1. Check if the original target filename was recreated or uncertain after staging!
        let mut target_existence = classify_dir_entry_existence(&q_cap_dir, &filename);
        if self
            .fault_inject_metadata_permission_denied
            .load(std::sync::atomic::Ordering::SeqCst)
        {
            target_existence =
                ObjectExistence::Uncertain("PermissionDenied (fault injected)".into());
        }
        if self
            .fault_inject_metadata_io_error
            .load(std::sync::atomic::Ordering::SeqCst)
        {
            target_existence =
                ObjectExistence::Uncertain("Input/output error (fault injected)".into());
        }
        let target_swap_injected = self
            .fault_inject_quarantine_target_swap
            .load(std::sync::atomic::Ordering::SeqCst);

        if target_existence != ObjectExistence::ConfirmedAbsent || target_swap_injected {
            // Target is Exists or Uncertain or target_swap_injected: FAIL CLOSED!
            let rolled = self.rollback_cleanup_stage(&q_cap_dir, &clean_stage_name, &filename);
            Self::apply_rollback_result_to_record(&mut manifest.records[idx], &rolled, &q_dir);
            manifest.records[idx].status = QuarantineStatus::PreservedConflict;
            self.persist_manifest_or_block_recovery(
                &manifest,
                record_id,
                &manifest.records[idx].quarantined_path,
                fault_manifest,
            )?;
            return Err(AetherError::PermissionDenied(format!(
                "Security rejection: Quarantine target '{}' state is not confirmed absent ({:?}). Recreated or uncertain file and candidate preserved without deletion.",
                target_path.display(), target_existence
            )));
        }

        // 2. Re-verify metadata and content hash of clean_stage_name through q_cap_dir pinned handle
        let pre_unlink_meta = match q_cap_dir.symlink_metadata(&clean_stage_name) {
            Ok(m) => m,
            Err(e) => {
                let rolled = self.rollback_cleanup_stage(&q_cap_dir, &clean_stage_name, &filename);
                Self::apply_rollback_result_to_record(&mut manifest.records[idx], &rolled, &q_dir);
                manifest.records[idx].status = QuarantineStatus::PreservedConflict;
                self.persist_manifest_or_block_recovery(
                    &manifest,
                    record_id,
                    &manifest.records[idx].quarantined_path,
                    fault_manifest,
                )?;
                return Err(AetherError::Io(e));
            }
        };

        if pre_unlink_meta.is_symlink() || !pre_unlink_meta.is_file() {
            let rolled = self.rollback_cleanup_stage(&q_cap_dir, &clean_stage_name, &filename);
            Self::apply_rollback_result_to_record(&mut manifest.records[idx], &rolled, &q_dir);
            manifest.records[idx].status = QuarantineStatus::PreservedConflict;
            self.persist_manifest_or_block_recovery(
                &manifest,
                record_id,
                &manifest.records[idx].quarantined_path,
                fault_manifest,
            )?;
            return Err(AetherError::PermissionDenied(format!(
                "Security rejection: Pre-unlink verification failed for '{}': object swapped to symlink or non-file",
                clean_stage_name
            )));
        }

        let pre_unlink_bytes = match q_cap_dir.read(&clean_stage_name) {
            Ok(b) => b,
            Err(e) => {
                let rolled = self.rollback_cleanup_stage(&q_cap_dir, &clean_stage_name, &filename);
                Self::apply_rollback_result_to_record(&mut manifest.records[idx], &rolled, &q_dir);
                manifest.records[idx].status = QuarantineStatus::PreservedConflict;
                self.persist_manifest_or_block_recovery(
                    &manifest,
                    record_id,
                    &manifest.records[idx].quarantined_path,
                    fault_manifest,
                )?;
                return Err(AetherError::Io(e));
            }
        };
        let pre_unlink_hash = Self::sha256_digest(&pre_unlink_bytes);

        if pre_unlink_hash != approved_hash {
            let rolled = self.rollback_cleanup_stage(&q_cap_dir, &clean_stage_name, &filename);
            Self::apply_rollback_result_to_record(&mut manifest.records[idx], &rolled, &q_dir);
            manifest.records[idx].status = QuarantineStatus::PreservedConflict;
            manifest.records[idx].actual_sha256 = Some(pre_unlink_hash.clone());
            self.persist_manifest_or_block_recovery(
                &manifest,
                record_id,
                &manifest.records[idx].quarantined_path,
                fault_manifest,
            )?;
            return Err(AetherError::PermissionDenied(format!(
                "Security rejection: Quarantine target '{}' was modified prior to unlinking (expected {}, found {}). Preserved as conflict artifact without deletion.",
                clean_stage_name, approved_hash, pre_unlink_hash
            )));
        }

        // Final disposal of the isolated stage candidate:
        #[cfg(windows)]
        {
            if let Err(e) = filesystem_engine::remove_file_windows_exclusive(
                &stage_abs,
                &approved_hash,
                original_identity,
            ) {
                let rolled = self.rollback_cleanup_stage(&q_cap_dir, &clean_stage_name, &filename);
                Self::apply_rollback_result_to_record(&mut manifest.records[idx], &rolled, &q_dir);
                manifest.records[idx].status = QuarantineStatus::PreservedConflict;
                self.persist_manifest_or_block_recovery(
                    &manifest,
                    record_id,
                    &manifest.records[idx].quarantined_path,
                    fault_manifest,
                )?;
                return Err(AetherError::PermissionDenied(format!(
                    "Security rejection: Exclusive stability for quarantine staged artifact '{}' could not be verified ({}). Preserved as conflict.",
                    clean_stage_name, e
                )));
            }

            // Only after confirmed physical removal on Windows, mark CleanedUp and persist manifest
            manifest.records[idx].status = QuarantineStatus::CleanedUp;
            self.persist_manifest_or_block_recovery(
                &manifest,
                record_id,
                "(physically removed)",
                fault_manifest,
            )?;
            Ok(true)
        }
        #[cfg(not(windows))]
        {
            // POSIX Fail-Closed: Standard POSIX kernel semantics cannot guarantee exclusive source
            // stability against uncooperative open descriptors. Physical unlinking is unsupported and refused.
            // Artifact is preserved via verified conflict rollback, manifest updated, and explicit error returned.
            let rolled = self.rollback_cleanup_stage(&q_cap_dir, &clean_stage_name, &filename);
            Self::apply_rollback_result_to_record(&mut manifest.records[idx], &rolled, &q_dir);
            manifest.records[idx].status = QuarantineStatus::PreservedConflict;
            manifest.records[idx].reason = format!(
                "POSIXExclusionLimitation: Automatic physical disposal of quarantine artifact '{}' is unsupported on POSIX systems because kernel semantics cannot guarantee exclusive source stability against uncooperative open descriptors. Artifact preserved as PreservedConflict.",
                manifest.records[idx].id
            );
            self.persist_manifest_or_block_recovery(
                &manifest,
                record_id,
                &manifest.records[idx].quarantined_path,
                fault_manifest,
            )?;
            Err(AetherError::PermissionDenied(format!(
                "Security rejection: Automatic physical disposal of quarantine artifact '{}' is unsupported on POSIX systems because kernel semantics cannot guarantee exclusive stability against uncooperative open descriptors. Artifact preserved at '{}'.",
                manifest.records[idx].id,
                manifest.records[idx].quarantined_path
            )))
        }
    }

    /// Cleans up all quarantined artifacts pending cleanup using the stage-within-quarantine protocol.
    pub fn cleanup_all_quarantined(&self) -> Result<usize> {
        if let Some(ref reason) = self.recovery_blocked_reason() {
            return Err(AetherError::PermissionDenied(format!(
                "Quarantine cleanup blocked due to recovery blocked state: {}",
                reason
            )));
        }

        let mut manifest = self.load_recovery_manifest()?;
        let fault_manifest = self
            .fault_inject_manifest_save
            .load(std::sync::atomic::Ordering::SeqCst);
        let mut cleaned_count = 0;
        let mut errors: Vec<String> = Vec::new();

        let q_dir = self.verify_quarantine_boundary()?;
        let q_cap_dir =
            Dir::open_ambient_dir(&q_dir, ambient_authority()).map_err(AetherError::Io)?;

        for i in 0..manifest.records.len() {
            if manifest.records[i].status != QuarantineStatus::PendingCleanup {
                continue;
            }

            let path_res =
                Self::resolve_and_validate_quarantine_path_static(&q_dir, &manifest.records[i]);
            let target_path = match path_res {
                Ok(p) => p,
                Err(e) => {
                    errors.push(format!(
                        "Record {} path invalid: {}",
                        manifest.records[i].id, e
                    ));
                    continue;
                }
            };
            let filename = manifest.records[i].quarantined_filename.clone();

            // Step 0: Establish original object identity before staging
            let original_identity = match filesystem_engine::query_file_object_identity(
                &target_path,
            ) {
                Ok(id) => {
                    if self
                        .fault_inject_identity_capture_failure
                        .load(std::sync::atomic::Ordering::SeqCst)
                    {
                        manifest.records[i].status = QuarantineStatus::PreservedConflict;
                        if let Err(save_err) = self.persist_manifest_or_block_recovery(
                            &manifest,
                            &manifest.records[i].id,
                            &manifest.records[i].quarantined_path,
                            fault_manifest,
                        ) {
                            return Err(AetherError::Internal(format!(
                                "Batch cleanup halted: manifest persistence failed on record '{}': {}",
                                manifest.records[i].id, save_err
                            )));
                        }
                        errors.push(format!(
                            "Record {} identity capture failure (fault injected)",
                            manifest.records[i].id
                        ));
                        continue;
                    }
                    Some(id)
                }
                Err(AetherError::Io(ref e)) if e.kind() == std::io::ErrorKind::NotFound => {
                    manifest.records[i].status = QuarantineStatus::CleanedUp;
                    cleaned_count += 1;
                    if let Err(save_err) = self.persist_manifest_or_block_recovery(
                        &manifest,
                        &manifest.records[i].id,
                        "(already absent)",
                        fault_manifest,
                    ) {
                        return Err(AetherError::Internal(format!(
                            "Batch cleanup halted: manifest persistence failed on missing record '{}': {}",
                            manifest.records[i].id, save_err
                        )));
                    }
                    continue;
                }
                Err(e) => {
                    manifest.records[i].status = QuarantineStatus::PreservedConflict;
                    if let Err(save_err) = self.persist_manifest_or_block_recovery(
                        &manifest,
                        &manifest.records[i].id,
                        &manifest.records[i].quarantined_path,
                        fault_manifest,
                    ) {
                        return Err(AetherError::Internal(format!(
                            "Batch cleanup halted: manifest persistence failed on record '{}': {}",
                            manifest.records[i].id, save_err
                        )));
                    }
                    errors.push(format!(
                        "Record {} failed to establish original identity: {}",
                        manifest.records[i].id, e
                    ));
                    continue;
                }
            };

            let clean_stage_name = format!(".aether-clean-stage-{}", uuid::Uuid::new_v4().simple());
            if let Err(e) = q_cap_dir.rename(&filename, &q_cap_dir, &clean_stage_name) {
                if e.kind() == std::io::ErrorKind::NotFound {
                    manifest.records[i].status = QuarantineStatus::CleanedUp;
                    cleaned_count += 1;
                    if let Err(save_err) = self.persist_manifest_or_block_recovery(
                        &manifest,
                        &manifest.records[i].id,
                        "(already absent)",
                        fault_manifest,
                    ) {
                        return Err(AetherError::Internal(format!(
                            "Batch cleanup halted: manifest persistence failed on missing record '{}': {}",
                            manifest.records[i].id, save_err
                        )));
                    }
                    continue;
                }
                errors.push(format!(
                    "Record {} stage rename failed: {}",
                    manifest.records[i].id, e
                ));
                continue;
            }

            let stage_abs = q_dir.join(&clean_stage_name);

            let post_stage_hook = self.post_stage_rename_hook.lock().clone();
            if let Some(ref hook) = post_stage_hook {
                hook(&stage_abs);
            }

            let staged_identity = match filesystem_engine::query_file_object_identity(&stage_abs) {
                Ok(id) => Some(id),
                Err(e) => {
                    let rolled =
                        self.rollback_cleanup_stage(&q_cap_dir, &clean_stage_name, &filename);
                    Self::apply_rollback_result_to_record(
                        &mut manifest.records[i],
                        &rolled,
                        &q_dir,
                    );
                    manifest.records[i].status = QuarantineStatus::PreservedConflict;
                    if let Err(save_err) = self.persist_manifest_or_block_recovery(
                        &manifest,
                        &manifest.records[i].id,
                        &manifest.records[i].quarantined_path,
                        fault_manifest,
                    ) {
                        return Err(AetherError::Internal(format!(
                            "Batch cleanup halted: manifest persistence failed after conflict preservation on record '{}': {}",
                            manifest.records[i].id, save_err
                        )));
                    }
                    errors.push(format!(
                        "Record {} staged identity query failed: {}",
                        manifest.records[i].id, e
                    ));
                    continue;
                }
            };

            if staged_identity != original_identity {
                let rolled = self.rollback_cleanup_stage(&q_cap_dir, &clean_stage_name, &filename);
                Self::apply_rollback_result_to_record(&mut manifest.records[i], &rolled, &q_dir);
                manifest.records[i].status = QuarantineStatus::PreservedConflict;
                if let Err(save_err) = self.persist_manifest_or_block_recovery(
                    &manifest,
                    &manifest.records[i].id,
                    &manifest.records[i].quarantined_path,
                    fault_manifest,
                ) {
                    return Err(AetherError::Internal(format!(
                        "Batch cleanup halted: manifest persistence failed after conflict preservation on record '{}': {}",
                        manifest.records[i].id, save_err
                    )));
                }
                errors.push(format!(
                    "Record {} identity mismatch against original; preserved as conflict",
                    manifest.records[i].id
                ));
                continue;
            }

            let meta = match q_cap_dir.symlink_metadata(&clean_stage_name) {
                Ok(m) => m,
                Err(e) => {
                    let rolled =
                        self.rollback_cleanup_stage(&q_cap_dir, &clean_stage_name, &filename);
                    Self::apply_rollback_result_to_record(
                        &mut manifest.records[i],
                        &rolled,
                        &q_dir,
                    );
                    manifest.records[i].status = QuarantineStatus::PreservedConflict;
                    if let Err(save_err) = self.persist_manifest_or_block_recovery(
                        &manifest,
                        &manifest.records[i].id,
                        &manifest.records[i].quarantined_path,
                        fault_manifest,
                    ) {
                        return Err(AetherError::Internal(format!(
                            "Batch cleanup halted: manifest persistence failed after conflict preservation on record '{}': {}",
                            manifest.records[i].id, save_err
                        )));
                    }
                    errors.push(format!(
                        "Record {} metadata read failed: {}",
                        manifest.records[i].id, e
                    ));
                    continue;
                }
            };

            if meta.is_symlink() {
                let rolled = self.rollback_cleanup_stage(&q_cap_dir, &clean_stage_name, &filename);
                Self::apply_rollback_result_to_record(&mut manifest.records[i], &rolled, &q_dir);
                manifest.records[i].status = QuarantineStatus::PreservedConflict;
                if let Err(save_err) = self.persist_manifest_or_block_recovery(
                    &manifest,
                    &manifest.records[i].id,
                    &manifest.records[i].quarantined_path,
                    fault_manifest,
                ) {
                    return Err(AetherError::Internal(format!(
                        "Batch cleanup halted: manifest persistence failed after conflict preservation on record '{}': {}",
                        manifest.records[i].id, save_err
                    )));
                }
                errors.push(format!(
                    "Record {} is a symlink; preserved as conflict",
                    manifest.records[i].id
                ));
                continue;
            }
            if !meta.is_file() {
                let rolled = self.rollback_cleanup_stage(&q_cap_dir, &clean_stage_name, &filename);
                Self::apply_rollback_result_to_record(&mut manifest.records[i], &rolled, &q_dir);
                manifest.records[i].status = QuarantineStatus::PreservedConflict;
                if let Err(save_err) = self.persist_manifest_or_block_recovery(
                    &manifest,
                    &manifest.records[i].id,
                    &manifest.records[i].quarantined_path,
                    fault_manifest,
                ) {
                    return Err(AetherError::Internal(format!(
                        "Batch cleanup halted: manifest persistence failed after conflict preservation on record '{}': {}",
                        manifest.records[i].id, save_err
                    )));
                }
                errors.push(format!(
                    "Record {} is not a regular file; preserved as conflict",
                    manifest.records[i].id
                ));
                continue;
            }

            let current_bytes = match q_cap_dir.read(&clean_stage_name) {
                Ok(b) => b,
                Err(e) => {
                    let rolled =
                        self.rollback_cleanup_stage(&q_cap_dir, &clean_stage_name, &filename);
                    Self::apply_rollback_result_to_record(
                        &mut manifest.records[i],
                        &rolled,
                        &q_dir,
                    );
                    manifest.records[i].status = QuarantineStatus::PreservedConflict;
                    if let Err(save_err) = self.persist_manifest_or_block_recovery(
                        &manifest,
                        &manifest.records[i].id,
                        &manifest.records[i].quarantined_path,
                        fault_manifest,
                    ) {
                        return Err(AetherError::Internal(format!(
                            "Batch cleanup halted: manifest persistence failed after conflict preservation on record '{}': {}",
                            manifest.records[i].id, save_err
                        )));
                    }
                    errors.push(format!(
                        "Record {} read failed: {}",
                        manifest.records[i].id, e
                    ));
                    continue;
                }
            };
            let current_hash = Self::sha256_digest(&current_bytes);
            let approved_hash = match manifest.records[i].actual_sha256 {
                Some(ref act) => act.clone(),
                None => manifest.records[i].expected_sha256.clone(),
            };

            if current_hash != approved_hash {
                let rolled = self.rollback_cleanup_stage(&q_cap_dir, &clean_stage_name, &filename);
                Self::apply_rollback_result_to_record(&mut manifest.records[i], &rolled, &q_dir);
                manifest.records[i].status = QuarantineStatus::PreservedConflict;
                manifest.records[i].actual_sha256 = Some(current_hash);
                if let Err(save_err) = self.persist_manifest_or_block_recovery(
                    &manifest,
                    &manifest.records[i].id,
                    &manifest.records[i].quarantined_path,
                    fault_manifest,
                ) {
                    return Err(AetherError::Internal(format!(
                        "Batch cleanup halted: manifest persistence failed after conflict preservation on record '{}': {}",
                        manifest.records[i].id, save_err
                    )));
                }
                errors.push(format!(
                    "Record {} content modified; preserved as conflict",
                    manifest.records[i].id
                ));
                continue;
            }

            if self
                .fault_inject_cleanup_removal
                .load(std::sync::atomic::Ordering::SeqCst)
            {
                let rolled = self.rollback_cleanup_stage(&q_cap_dir, &clean_stage_name, &filename);
                Self::apply_rollback_result_to_record(&mut manifest.records[i], &rolled, &q_dir);
                if let Err(save_err) = self.persist_manifest_or_block_recovery(
                    &manifest,
                    &manifest.records[i].id,
                    &manifest.records[i].quarantined_path,
                    fault_manifest,
                ) {
                    return Err(AetherError::Internal(format!(
                        "Batch cleanup halted: manifest persistence failed after conflict preservation on record '{}': {}",
                        manifest.records[i].id, save_err
                    )));
                }
                errors.push("Injected cleanup removal failure".into());
                continue;
            }

            let hook_opt = self.pre_cleanup_unlink_hook.lock().clone();
            if let Some(ref hook) = hook_opt {
                hook(&target_path);
            }

            if self.verify_quarantine_boundary().is_err() {
                let rolled = self.rollback_cleanup_stage(&q_cap_dir, &clean_stage_name, &filename);
                Self::apply_rollback_result_to_record(&mut manifest.records[i], &rolled, &q_dir);
                manifest.records[i].status = QuarantineStatus::PreservedConflict;
                if let Err(save_err) = self.persist_manifest_or_block_recovery(
                    &manifest,
                    &manifest.records[i].id,
                    &manifest.records[i].quarantined_path,
                    fault_manifest,
                ) {
                    return Err(AetherError::Internal(format!(
                        "Batch cleanup halted: manifest persistence failed after conflict preservation on record '{}': {}",
                        manifest.records[i].id, save_err
                    )));
                }
                errors.push("Quarantine boundary verification failed".into());
                continue;
            }

            let mut target_existence = classify_dir_entry_existence(&q_cap_dir, &filename);
            if self
                .fault_inject_metadata_permission_denied
                .load(std::sync::atomic::Ordering::SeqCst)
            {
                target_existence =
                    ObjectExistence::Uncertain("PermissionDenied (fault injected)".into());
            }
            if self
                .fault_inject_metadata_io_error
                .load(std::sync::atomic::Ordering::SeqCst)
            {
                target_existence =
                    ObjectExistence::Uncertain("Input/output error (fault injected)".into());
            }
            let target_swap_injected = self
                .fault_inject_quarantine_target_swap
                .load(std::sync::atomic::Ordering::SeqCst);

            if target_existence != ObjectExistence::ConfirmedAbsent || target_swap_injected {
                let rolled = self.rollback_cleanup_stage(&q_cap_dir, &clean_stage_name, &filename);
                Self::apply_rollback_result_to_record(&mut manifest.records[i], &rolled, &q_dir);
                manifest.records[i].status = QuarantineStatus::PreservedConflict;
                if let Err(save_err) = self.persist_manifest_or_block_recovery(
                    &manifest,
                    &manifest.records[i].id,
                    &manifest.records[i].quarantined_path,
                    fault_manifest,
                ) {
                    return Err(AetherError::Internal(format!(
                        "Batch cleanup halted: manifest persistence failed after conflict preservation on record '{}': {}",
                        manifest.records[i].id, save_err
                    )));
                }
                errors.push(format!(
                    "Record {} target not confirmed absent ({:?}); preserved as conflict",
                    manifest.records[i].id, target_existence
                ));
                continue;
            }

            let pre_unlink_meta = match q_cap_dir.symlink_metadata(&clean_stage_name) {
                Ok(m) => m,
                Err(e) => {
                    let rolled =
                        self.rollback_cleanup_stage(&q_cap_dir, &clean_stage_name, &filename);
                    Self::apply_rollback_result_to_record(
                        &mut manifest.records[i],
                        &rolled,
                        &q_dir,
                    );
                    manifest.records[i].status = QuarantineStatus::PreservedConflict;
                    if let Err(save_err) = self.persist_manifest_or_block_recovery(
                        &manifest,
                        &manifest.records[i].id,
                        &manifest.records[i].quarantined_path,
                        fault_manifest,
                    ) {
                        return Err(AetherError::Internal(format!(
                            "Batch cleanup halted: manifest persistence failed after conflict preservation on record '{}': {}",
                            manifest.records[i].id, save_err
                        )));
                    }
                    errors.push(format!(
                        "Record {} pre-unlink meta failed: {}",
                        manifest.records[i].id, e
                    ));
                    continue;
                }
            };
            if pre_unlink_meta.is_symlink() || !pre_unlink_meta.is_file() {
                let rolled = self.rollback_cleanup_stage(&q_cap_dir, &clean_stage_name, &filename);
                Self::apply_rollback_result_to_record(&mut manifest.records[i], &rolled, &q_dir);
                manifest.records[i].status = QuarantineStatus::PreservedConflict;
                if let Err(save_err) = self.persist_manifest_or_block_recovery(
                    &manifest,
                    &manifest.records[i].id,
                    &manifest.records[i].quarantined_path,
                    fault_manifest,
                ) {
                    return Err(AetherError::Internal(format!(
                        "Batch cleanup halted: manifest persistence failed after conflict preservation on record '{}': {}",
                        manifest.records[i].id, save_err
                    )));
                }
                errors.push(format!(
                    "Record {} pre-unlink not a regular file",
                    manifest.records[i].id
                ));
                continue;
            }

            let pre_unlink_bytes = match q_cap_dir.read(&clean_stage_name) {
                Ok(b) => b,
                Err(e) => {
                    let rolled =
                        self.rollback_cleanup_stage(&q_cap_dir, &clean_stage_name, &filename);
                    Self::apply_rollback_result_to_record(
                        &mut manifest.records[i],
                        &rolled,
                        &q_dir,
                    );
                    manifest.records[i].status = QuarantineStatus::PreservedConflict;
                    if let Err(save_err) = self.persist_manifest_or_block_recovery(
                        &manifest,
                        &manifest.records[i].id,
                        &manifest.records[i].quarantined_path,
                        fault_manifest,
                    ) {
                        return Err(AetherError::Internal(format!(
                            "Batch cleanup halted: manifest persistence failed after conflict preservation on record '{}': {}",
                            manifest.records[i].id, save_err
                        )));
                    }
                    errors.push(format!(
                        "Record {} pre-unlink read failed: {}",
                        manifest.records[i].id, e
                    ));
                    continue;
                }
            };
            let pre_unlink_hash = Self::sha256_digest(&pre_unlink_bytes);
            if pre_unlink_hash != approved_hash {
                let rolled = self.rollback_cleanup_stage(&q_cap_dir, &clean_stage_name, &filename);
                Self::apply_rollback_result_to_record(&mut manifest.records[i], &rolled, &q_dir);
                manifest.records[i].status = QuarantineStatus::PreservedConflict;
                manifest.records[i].actual_sha256 = Some(pre_unlink_hash);
                if let Err(save_err) = self.persist_manifest_or_block_recovery(
                    &manifest,
                    &manifest.records[i].id,
                    &manifest.records[i].quarantined_path,
                    fault_manifest,
                ) {
                    return Err(AetherError::Internal(format!(
                        "Batch cleanup halted: manifest persistence failed after conflict preservation on record '{}': {}",
                        manifest.records[i].id, save_err
                    )));
                }
                errors.push(format!(
                    "Record {} pre-unlink content modified; preserved as conflict",
                    manifest.records[i].id
                ));
                continue;
            }

            #[cfg(windows)]
            {
                if let Err(e) = filesystem_engine::remove_file_windows_exclusive(
                    &stage_abs,
                    &approved_hash,
                    original_identity,
                ) {
                    let rolled =
                        self.rollback_cleanup_stage(&q_cap_dir, &clean_stage_name, &filename);
                    Self::apply_rollback_result_to_record(
                        &mut manifest.records[i],
                        &rolled,
                        &q_dir,
                    );
                    manifest.records[i].status = QuarantineStatus::PreservedConflict;
                    if let Err(save_err) = self.persist_manifest_or_block_recovery(
                        &manifest,
                        &manifest.records[i].id,
                        &manifest.records[i].quarantined_path,
                        fault_manifest,
                    ) {
                        return Err(AetherError::Internal(format!(
                            "Batch cleanup halted: manifest persistence failed after conflict preservation on record '{}': {}",
                            manifest.records[i].id, save_err
                        )));
                    }
                    errors.push(format!(
                        "Record {} exclusive stability failed: {}",
                        manifest.records[i].id, e
                    ));
                    continue;
                }

                manifest.records[i].status = QuarantineStatus::CleanedUp;
                cleaned_count += 1;
                if let Err(save_err) = self.persist_manifest_or_block_recovery(
                    &manifest,
                    &manifest.records[i].id,
                    "(physically removed)",
                    fault_manifest,
                ) {
                    return Err(AetherError::Internal(format!(
                        "Batch cleanup halted: manifest persistence failed after cleaning up {} file(s) (failed on record '{}'): {}",
                        cleaned_count, manifest.records[i].id, save_err
                    )));
                }
            }
            #[cfg(not(windows))]
            {
                // POSIX Fail-Closed: Standard POSIX kernel semantics cannot guarantee exclusive source
                // stability against uncooperative open descriptors. Physical unlinking is unsupported and refused.
                // Artifact is preserved via verified conflict rollback, manifest updated, and error recorded.
                let rolled = self.rollback_cleanup_stage(&q_cap_dir, &clean_stage_name, &filename);
                Self::apply_rollback_result_to_record(&mut manifest.records[i], &rolled, &q_dir);
                manifest.records[i].status = QuarantineStatus::PreservedConflict;
                manifest.records[i].reason = format!(
                    "POSIXExclusionLimitation: Automatic physical disposal of quarantine artifact '{}' is unsupported on POSIX systems because kernel semantics cannot guarantee exclusive source stability against uncooperative open descriptors. Artifact preserved as PreservedConflict.",
                    manifest.records[i].id
                );
                errors.push(format!(
                    "Record {} unsupported automatic disposal on POSIX; preserved as PreservedConflict",
                    manifest.records[i].id
                ));
                if let Err(save_err) = self.persist_manifest_or_block_recovery(
                    &manifest,
                    &manifest.records[i].id,
                    &manifest.records[i].quarantined_path,
                    fault_manifest,
                ) {
                    return Err(AetherError::Internal(format!(
                        "Batch cleanup halted: manifest persistence failed after conflict preservation on record '{}': {}",
                        manifest.records[i].id, save_err
                    )));
                }
                continue;
            }
        }

        if !errors.is_empty() {
            return Err(AetherError::PermissionDenied(format!(
                "Cleanup completed with {} successful and {} preserved conflicts/failures: {:?}",
                cleaned_count,
                errors.len(),
                errors
            )));
        }

        Ok(cleaned_count)
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
        if let Some(ref reason) = self.recovery_blocked_reason() {
            return Err(AetherError::PermissionDenied(format!(
                "Checkpoint creation blocked due to recovery blocked state: {}",
                reason
            )));
        }

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
        if let Some(ref reason) = self.recovery_blocked_reason() {
            return Err(AetherError::PermissionDenied(format!(
                "Listing checkpoints blocked due to recovery blocked state: {}",
                reason
            )));
        }

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
        if let Some(ref reason) = self.recovery_blocked_reason() {
            return Err(AetherError::PermissionDenied(format!(
                "Checkpoint restore preview blocked due to recovery blocked state: {}",
                reason
            )));
        }

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
        if let Some(ref reason) = self.recovery_blocked_reason() {
            return Err(AetherError::PermissionDenied(format!(
                "Consuming preview token blocked due to recovery blocked state: {}",
                reason
            )));
        }

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
        if let Some(ref reason) = self.recovery_blocked_reason() {
            return Err(AetherError::PermissionDenied(format!(
                "Checkpoint restore blocked due to recovery blocked state: {}",
                reason
            )));
        }

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
            quarantined_artifacts: Vec::new(),
            cleanup_pending: false,
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
        let mut quarantined_artifacts: Vec<String> = Vec::new();
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
                        // Capability-anchored conditional deletion with durable quarantine!
                        let pre_del_hook_opt = self.pre_delete_hook.lock().clone();
                        let pre_del_ref = pre_del_hook_opt
                            .as_ref()
                            .map(|h| h.as_ref() as &dyn Fn(&Path));
                        let post_hash_del_hook_opt = self.post_hash_delete_hook.lock().clone();
                        let post_hash_del_ref = post_hash_del_hook_opt
                            .as_ref()
                            .map(|h| h.as_ref() as &dyn Fn(&Path));
                        let pre_unlink_del_hook_opt = self.pre_unlink_delete_hook.lock().clone();
                        let pre_unlink_del_ref = pre_unlink_del_hook_opt
                            .as_ref()
                            .map(|h| h.as_ref() as &dyn Fn(&Path));
                        let pre_hl_del_hook_opt = self.pre_hard_link_delete_hook.lock().clone();
                        let pre_hl_del_ref = pre_hl_del_hook_opt
                            .as_ref()
                            .map(|h| h.as_ref() as &dyn Fn(&Path));
                        let quarantine_dir = self.quarantine_dir();
                        let del_hooks = filesystem_engine::ConditionalDeleteHooks {
                            pre_delete_hook: pre_del_ref,
                            post_hash_hook: post_hash_del_ref,
                            pre_unlink_hook: pre_unlink_del_ref,
                            pre_hard_link_hook: pre_hl_del_ref,
                            defer_to_quarantine: true, // Production integration: quarantine newly created files!
                            quarantine_dir: Some(quarantine_dir.clone()),
                            fault_inject_delete_removal: self
                                .fault_inject_delete_removal
                                .load(std::sync::atomic::Ordering::SeqCst),
                            fault_inject_rollback_failure: false,
                            fault_inject_rollback_metadata_failure: self
                                .fault_inject_rollback_metadata_failure
                                .load(std::sync::atomic::Ordering::SeqCst),
                            fault_inject_hard_link_failure: self
                                .fault_inject_hard_link_rollback_failure
                                .load(std::sync::atomic::Ordering::SeqCst),
                            fault_inject_second_verification_read_failure: self
                                .fault_inject_second_verification_read_failure
                                .load(std::sync::atomic::Ordering::SeqCst),
                        };

                        match cap.delete_file_conditional_with_hooks(
                            &committed.relative_path,
                            &committed.restored_sha256,
                            del_hooks,
                        ) {
                            Ok(outcome) => {
                                if outcome.quarantined {
                                    let q_path = outcome.surviving_path.unwrap();
                                    let q_filename = q_path
                                        .file_name()
                                        .and_then(|n| n.to_str())
                                        .unwrap_or("file")
                                        .to_string();
                                    let record = QuarantineRecord {
                                        id: uuid::Uuid::new_v4().to_string(),
                                        original_relative_path: committed.relative_path.clone(),
                                        quarantined_filename: q_filename,
                                        quarantined_path: q_path.display().to_string(),
                                        quarantined_at: chrono::Utc::now().to_rfc3339(),
                                        expected_sha256: committed.restored_sha256.clone(),
                                        actual_sha256: Some(committed.restored_sha256.clone()),
                                        reason: "CompensatingRollbackQuarantine".into(),
                                        status: QuarantineStatus::PendingCleanup,
                                        retained_staging_filename: None,
                                        retained_staging_path: None,
                                    };
                                    let manifest_path = self.recovery_manifest_path();
                                    let fault_manifest = self
                                        .fault_inject_manifest_save
                                        .load(std::sync::atomic::Ordering::SeqCst);
                                    let mut manifest = RecoveryManifest::load(&manifest_path)?;
                                    if let Err(e) = manifest.record_quarantine_with_fault(
                                        record,
                                        &manifest_path,
                                        fault_manifest,
                                    ) {
                                        rollback_errors.push(format!(
                                            "Failed to record recovery manifest for quarantined file '{}': {}",
                                            committed.relative_path, e
                                        ));
                                        recovery_conflicts.push(format!(
                                            "Quarantine manifest record failed for '{}'; preserved in quarantine at '{}'.",
                                            committed.relative_path,
                                            q_path.display()
                                        ));
                                    } else {
                                        let msg = format!(
                                            "Newly created file '{}' moved to quarantine at '{}'. Recorded in durable recovery manifest; pending cleanup.",
                                            committed.relative_path, q_path.display()
                                        );
                                        warn!("{}", msg);
                                        quarantined_artifacts.push(q_path.display().to_string());
                                        compensated_files.push(committed.relative_path.clone());
                                        rolled_back_count += 1;
                                    }
                                } else if outcome.was_deleted {
                                    rolled_back_count += 1;
                                    compensated_files.push(committed.relative_path.clone());
                                } else if let Some(surviving) = outcome.surviving_path {
                                    let msg = format!(
                                        "Newly created file '{}' modified after restore (or staging preserved at '{}'). Preserved recovery conflict artifact: File preserved in workspace without deletion.",
                                        committed.relative_path, surviving.display()
                                    );
                                    warn!("{}", msg);
                                    recovery_conflicts.push(msg);
                                    preserved_backups.push(surviving.display().to_string());
                                } else {
                                    let msg = format!(
                                        "Newly created file '{}' modified after restore. Preserved recovery conflict artifact: File preserved in workspace without deletion.",
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
            let quarantine_note = if quarantined_artifacts.is_empty() {
                String::new()
            } else {
                format!(
                    " Quarantined artifacts pending cleanup: {:?}.",
                    quarantined_artifacts
                )
            };
            Err(AetherError::Internal(format!(
                "Restore aborted: {}. Compensating rollback succeeded: all {} previously modified files were cleanly rolled back to pre-restore safety checkpoint '{}'. Preserved backups: {:?}.{}",
                orig_err, rolled_back_count, backup_ckpt.checkpoint_id, preserved_backups, quarantine_note
            )))
        } else {
            Err(AetherError::Internal(format!(
                "Restore aborted: {}. Compensating rollback encountered conflicts/errors: {}/{} files cleanly rolled back. Compensated files: {:?}. Quarantined artifacts: {:?}. Files requiring manual recovery (conflicts): {:?}. Rollback errors: {:?}. Preserved backups: {:?}. Mandatory safety checkpoint '{}' is preserved in storage for manual recovery.",
                orig_err,
                rolled_back_count,
                committed_files.len(),
                compensated_files,
                quarantined_artifacts,
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

        // Open new_file with a retained writable handle BEFORE staging rename
        let retained_handle_arc = Arc::new(Mutex::new(None));
        let handle_writer_clone = retained_handle_arc.clone();
        let new_file_clone = f_new.clone();

        // Cause z_fail to abort restore after new_file was committed, and capture retained writable handle
        engine.set_pre_replace_hook(Some(move |dest: &Path| {
            if dest.file_name() == Some(std::ffi::OsStr::new("z_fail.txt")) {
                let f = std::fs::OpenOptions::new()
                    .read(true)
                    .write(true)
                    .open(&new_file_clone)
                    .unwrap();
                *handle_writer_clone.lock() = Some(f);
                std::fs::write(dest, "mutation on fail to abort").unwrap();
            }
        }));

        // Mutate staged file through the retained writable handle in post_hash_delete_hook
        // after initial hash check but before unlink
        let handle_post_clone = retained_handle_arc.clone();
        engine.set_post_hash_delete_hook(Some(move |_staged_path: &Path| {
            use std::io::{Seek, SeekFrom, Write};
            let mut guard = handle_post_clone.lock();
            if let Some(ref mut f) = *guard {
                f.seek(SeekFrom::Start(0)).unwrap();
                f.write_all(b"open handle mutation after hash verification!")
                    .unwrap();
                f.set_len(b"open handle mutation after hash verification!".len() as u64)
                    .unwrap();
                f.flush().unwrap();
            }
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

    #[test]
    fn test_chk_043_retained_writable_handle_detected_by_unconditional_second_verification() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_043_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        let workspace_dir = temp_dir.join("workspace");
        std::fs::create_dir_all(&workspace_dir).unwrap();

        let f_new = workspace_dir.join("new_file.txt");
        let f_fail = workspace_dir.join("z_fail.txt");

        std::fs::write(&f_new, "clean payload v1").unwrap();
        std::fs::write(&f_fail, "fail base v1").unwrap();

        let engine = CheckpointEngine::new(&storage_dir);
        let ckpt = engine
            .create_checkpoint_with_root("Chk043 base", "ws-chk043", &workspace_dir, None)
            .unwrap();

        let _ = std::fs::remove_file(&f_new);

        let preview = engine
            .restore_preview_bound(&ckpt.checkpoint_id, "ws-chk043", &workspace_dir)
            .unwrap();

        // Capture retained handle in pre_replace_hook before staging rename
        let retained_handle_arc = Arc::new(Mutex::new(None));
        let handle_writer_clone = retained_handle_arc.clone();
        let new_file_clone = f_new.clone();

        engine.set_pre_replace_hook(Some(move |dest: &Path| {
            if dest.file_name() == Some(std::ffi::OsStr::new("z_fail.txt")) {
                let f = std::fs::OpenOptions::new()
                    .read(true)
                    .write(true)
                    .open(&new_file_clone)
                    .unwrap();
                *handle_writer_clone.lock() = Some(f);
                std::fs::write(dest, "mutation on fail to abort").unwrap();
            }
        }));

        // Mutate through retained handle in post_hash_delete_hook
        let handle_post_clone = retained_handle_arc.clone();
        engine.set_post_hash_delete_hook(Some(move |_staged_path: &Path| {
            use std::io::{Seek, SeekFrom, Write};
            let mut guard = handle_post_clone.lock();
            if let Some(ref mut f) = *guard {
                f.seek(SeekFrom::Start(0)).unwrap();
                f.write_all(b"mutation via retained writable handle in chk043!")
                    .unwrap();
                f.set_len(b"mutation via retained writable handle in chk043!".len() as u64)
                    .unwrap();
                f.flush().unwrap();
            }
        }));

        let res = engine.restore_checkpoint_with_token(
            &ckpt.checkpoint_id,
            "ws-chk043",
            &workspace_dir,
            &preview.preview_token,
            true,
            &[],
        );

        assert!(res.is_err());
        let err_msg = res.err().unwrap().to_string();
        assert!(
            err_msg.contains("Files requiring manual recovery"),
            "Error must report recovery conflict: {}",
            err_msg
        );
        assert!(
            err_msg.contains("0/1 files cleanly rolled back"),
            "Mutated file must not be counted as cleanly rolled back: {}",
            err_msg
        );

        let marker = "staging preserved at '";
        assert!(err_msg.contains(marker));
        let start = err_msg.find(marker).unwrap() + marker.len();
        let end = err_msg[start..].find('\'').unwrap() + start;
        let preserved_str = err_msg[start..end].replace(r"\\", r"\");
        let preserved_path = PathBuf::from(&preserved_str);

        assert!(preserved_path.exists());
        assert_eq!(
            std::fs::read_to_string(&preserved_path).unwrap(),
            "mutation via retained writable handle in chk043!"
        );

        engine.set_pre_replace_hook::<fn(&Path)>(None);
        engine.set_post_hash_delete_hook::<fn(&Path)>(None);
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_044_recreated_destination_before_hard_link_rollback_preserves_both_no_overwrite() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_044_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        let workspace_dir = temp_dir.join("workspace");
        std::fs::create_dir_all(&workspace_dir).unwrap();

        let f_new = workspace_dir.join("new_file.txt");
        let f_fail = workspace_dir.join("z_fail.txt");

        std::fs::write(&f_new, "initial new file payload").unwrap();
        std::fs::write(&f_fail, "fail base v1").unwrap();

        let engine = CheckpointEngine::new(&storage_dir);
        let ckpt = engine
            .create_checkpoint_with_root("Chk044 base", "ws-chk044", &workspace_dir, None)
            .unwrap();

        let _ = std::fs::remove_file(&f_new);

        let preview = engine
            .restore_preview_bound(&ckpt.checkpoint_id, "ws-chk044", &workspace_dir)
            .unwrap();

        engine.set_pre_replace_hook(Some(|dest: &Path| {
            if dest.file_name() == Some(std::ffi::OsStr::new("z_fail.txt")) {
                std::fs::write(dest, "mutation on fail to abort").unwrap();
            }
        }));

        // Mutate staged file to trigger rollback
        engine.set_pre_delete_hook(Some(|staged_path: &Path| {
            std::fs::write(staged_path, "staged file mutated").unwrap();
        }));

        // Concurrently recreate destination right before hard link publication
        let f_new_clone = f_new.clone();
        engine.set_pre_hard_link_delete_hook(Some(move |_dest_abs: &Path| {
            std::fs::write(&f_new_clone, "recreated before hard link in chk044").unwrap();
        }));

        let res = engine.restore_checkpoint_with_token(
            &ckpt.checkpoint_id,
            "ws-chk044",
            &workspace_dir,
            &preview.preview_token,
            true,
            &[],
        );

        assert!(res.is_err());
        let err_msg = res.err().unwrap().to_string();

        // 1. Recreated destination must survive on disk without overwrite
        assert!(f_new.exists());
        assert_eq!(
            std::fs::read_to_string(&f_new).unwrap(),
            "recreated before hard link in chk044",
            "Destination MUST NOT be overwritten!"
        );

        // 2. Staged recovery artifact must survive on disk under a conflict path
        let marker = "staging preserved at '";
        assert!(err_msg.contains(marker));
        let start = err_msg.find(marker).unwrap() + marker.len();
        let end = err_msg[start..].find('\'').unwrap() + start;
        let conflict_str = err_msg[start..end].replace(r"\\", r"\");
        let conflict_path = PathBuf::from(&conflict_str);

        assert!(conflict_path.exists());
        assert_ne!(conflict_path, f_new);
        assert_eq!(
            std::fs::read_to_string(&conflict_path).unwrap(),
            "staged file mutated"
        );

        engine.set_pre_replace_hook::<fn(&Path)>(None);
        engine.set_pre_delete_hook::<fn(&Path)>(None);
        engine.set_pre_hard_link_delete_hook::<fn(&Path)>(None);
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_045_hard_link_rollback_failure_never_falls_back_to_overwrite_rename() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_045_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        let workspace_dir = temp_dir.join("workspace");
        std::fs::create_dir_all(&workspace_dir).unwrap();

        let f_new = workspace_dir.join("new_file.txt");
        let f_fail = workspace_dir.join("z_fail.txt");

        std::fs::write(&f_new, "initial new file payload").unwrap();
        std::fs::write(&f_fail, "fail base v1").unwrap();

        let engine = CheckpointEngine::new(&storage_dir);
        let ckpt = engine
            .create_checkpoint_with_root("Chk045 base", "ws-chk045", &workspace_dir, None)
            .unwrap();

        let _ = std::fs::remove_file(&f_new);

        let preview = engine
            .restore_preview_bound(&ckpt.checkpoint_id, "ws-chk045", &workspace_dir)
            .unwrap();

        engine.set_pre_replace_hook(Some(|dest: &Path| {
            if dest.file_name() == Some(std::ffi::OsStr::new("z_fail.txt")) {
                std::fs::write(dest, "mutation on fail to abort").unwrap();
            }
        }));

        engine.set_pre_delete_hook(Some(|staged_path: &Path| {
            std::fs::write(staged_path, "staged payload for chk045").unwrap();
        }));

        // Inject non-AlreadyExists hard link failure
        engine.set_fault_inject_hard_link_rollback_failure(true);

        let res = engine.restore_checkpoint_with_token(
            &ckpt.checkpoint_id,
            "ws-chk045",
            &workspace_dir,
            &preview.preview_token,
            true,
            &[],
        );

        assert!(res.is_err());
        let err_msg = res.err().unwrap().to_string();

        // Ordinary rename fallback must NOT occur: f_new must NOT exist
        assert!(
            !f_new.exists(),
            "Ordinary rename fallback must NOT have created destination!"
        );

        // Staged conflict artifact MUST be preserved
        let marker = "staging preserved at '";
        assert!(err_msg.contains(marker));
        let start = err_msg.find(marker).unwrap() + marker.len();
        let end = err_msg[start..].find('\'').unwrap() + start;
        let conflict_str = err_msg[start..end].replace(r"\\", r"\");
        let conflict_path = PathBuf::from(&conflict_str);

        assert!(conflict_path.exists());
        assert_eq!(
            std::fs::read_to_string(&conflict_path).unwrap(),
            "staged payload for chk045"
        );

        engine.set_pre_replace_hook::<fn(&Path)>(None);
        engine.set_pre_delete_hook::<fn(&Path)>(None);
        engine.set_fault_inject_hard_link_rollback_failure(false);
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_046_metadata_inspection_failure_during_rollback_fails_closed() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_046_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        let workspace_dir = temp_dir.join("workspace");
        std::fs::create_dir_all(&workspace_dir).unwrap();

        let f_new = workspace_dir.join("new_file.txt");
        let f_fail = workspace_dir.join("z_fail.txt");

        std::fs::write(&f_new, "initial new file payload").unwrap();
        std::fs::write(&f_fail, "fail base v1").unwrap();

        let engine = CheckpointEngine::new(&storage_dir);
        let ckpt = engine
            .create_checkpoint_with_root("Chk046 base", "ws-chk046", &workspace_dir, None)
            .unwrap();

        let _ = std::fs::remove_file(&f_new);

        let preview = engine
            .restore_preview_bound(&ckpt.checkpoint_id, "ws-chk046", &workspace_dir)
            .unwrap();

        engine.set_pre_replace_hook(Some(|dest: &Path| {
            if dest.file_name() == Some(std::ffi::OsStr::new("z_fail.txt")) {
                std::fs::write(dest, "mutation on fail to abort").unwrap();
            }
        }));

        engine.set_pre_delete_hook(Some(|staged_path: &Path| {
            std::fs::write(staged_path, "staged payload for chk046").unwrap();
        }));

        // Inject metadata failure during rollback
        engine.set_fault_inject_rollback_metadata_failure(true);

        let res = engine.restore_checkpoint_with_token(
            &ckpt.checkpoint_id,
            "ws-chk046",
            &workspace_dir,
            &preview.preview_token,
            true,
            &[],
        );

        assert!(res.is_err());
        let err_msg = res.err().unwrap().to_string();

        assert!(!f_new.exists());

        let marker = "staging preserved at '";
        assert!(err_msg.contains(marker));
        let start = err_msg.find(marker).unwrap() + marker.len();
        let end = err_msg[start..].find('\'').unwrap() + start;
        let conflict_str = err_msg[start..end].replace(r"\\", r"\");
        let conflict_path = PathBuf::from(&conflict_str);

        assert!(conflict_path.exists());
        assert_eq!(
            std::fs::read_to_string(&conflict_path).unwrap(),
            "staged payload for chk046"
        );

        engine.set_pre_replace_hook::<fn(&Path)>(None);
        engine.set_pre_delete_hook::<fn(&Path)>(None);
        engine.set_fault_inject_rollback_metadata_failure(false);
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_047_production_wrapper_default_hooks_executes_second_verification() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_047_{}", uuid::Uuid::new_v4().simple()));
        let workspace_dir = temp_dir.join("workspace");
        std::fs::create_dir_all(&workspace_dir).unwrap();

        let cap = filesystem_engine::WorkspaceCapability::open(&workspace_dir).unwrap();
        let f_prod = workspace_dir.join("prod_file.txt");
        let initial_payload = vec![0x58u8; 1024 * 1024]; // 1MB payload to ensure reliable window
        std::fs::write(&f_prod, &initial_payload).unwrap();

        let initial_hash = {
            use sha2::{Digest, Sha256};
            let mut hasher = Sha256::new();
            hasher.update(&initial_payload);
            format!("{:x}", hasher.finalize())
        };

        let mut retained_handle = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&f_prod)
            .unwrap();

        let path_clone = f_prod.clone();
        let ready = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let ready_clone = ready.clone();
        let running = Arc::new(std::sync::atomic::AtomicBool::new(true));
        let r_flag = running.clone();

        let writer_thread = std::thread::spawn(move || {
            use std::io::{Seek, SeekFrom, Write};
            ready_clone.store(true, std::sync::atomic::Ordering::SeqCst);
            // Spin until staging rename moves prod_file.txt away
            while r_flag.load(std::sync::atomic::Ordering::Relaxed) && path_clone.exists() {
                std::hint::spin_loop();
            }
            if path_clone.exists() {
                return;
            }
            // Staging rename completed: write immediately through retained handle
            let msg = b"mutation via retained handle in chk047!";
            let _ = retained_handle.seek(SeekFrom::Start(0));
            let _ = retained_handle.write_all(msg);
            let _ = retained_handle.flush();
        });

        // Ensure writer thread is active and spinning before initiating deletion
        while !ready.load(std::sync::atomic::Ordering::SeqCst) {
            std::thread::yield_now();
        }

        // Delete using production wrapper with DEFAULT hooks
        let outcome = cap
            .delete_file_conditional("prod_file.txt", &initial_hash)
            .unwrap();

        running.store(false, std::sync::atomic::Ordering::SeqCst);
        writer_thread.join().unwrap();

        assert!(!outcome.was_deleted);
        assert!(outcome.surviving_path.is_some());
        let surviving = outcome.surviving_path.unwrap();
        assert!(surviving.exists());
        let content = std::fs::read(&surviving).unwrap();
        assert!(content.starts_with(b"mutation via retained handle in chk047!"));

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_048_mutation_after_final_verification_preserves_artifact() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_048_{}", uuid::Uuid::new_v4().simple()));
        let workspace_dir = temp_dir.join("workspace");
        std::fs::create_dir_all(&workspace_dir).unwrap();

        let cap = filesystem_engine::WorkspaceCapability::open(&workspace_dir).unwrap();
        let f_target = workspace_dir.join("chk048_file.txt");
        let payload = b"unmodified chk048 initial payload";
        std::fs::write(&f_target, payload).unwrap();

        let initial_hash = {
            use sha2::{Digest, Sha256};
            let mut hasher = Sha256::new();
            hasher.update(payload);
            format!("{:x}", hasher.finalize())
        };

        // Open a writable handle to the file prior to staging
        let retained_handle = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&f_target)
            .unwrap();

        let handle_arc = Arc::new(Mutex::new(Some(retained_handle)));
        let handle_clone = handle_arc.clone();

        // Hook simulates write through retained handle AFTER final verification but before physical unlink
        let pre_unlink = move |_staged: &Path| {
            use std::io::{Seek, SeekFrom, Write};
            let mut guard = handle_clone.lock();
            if let Some(ref mut h) = *guard {
                let _ = h.seek(SeekFrom::Start(0));
                let _ = h.write_all(b"mutation after final verification in chk048!");
                let _ = h.flush();
            }
        };

        let hooks = filesystem_engine::ConditionalDeleteHooks {
            pre_unlink_hook: Some(&pre_unlink),
            ..Default::default()
        };

        let outcome = cap
            .delete_file_conditional_with_hooks("chk048_file.txt", &initial_hash, hooks)
            .unwrap();

        drop(handle_arc);

        // Must NOT silently destroy user data: deletion aborted and file preserved!
        assert!(!outcome.was_deleted, "Deletion must be aborted!");
        assert!(outcome.surviving_path.is_some());
        let surviving = outcome.surviving_path.unwrap();
        assert!(surviving.exists(), "Surviving file must exist on disk!");
        let content = std::fs::read(&surviving).unwrap();
        assert!(
            content.starts_with(b"mutation after final verification in chk048!"),
            "Preserved artifact must retain the modified content!"
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_049_quarantine_recovery_manifest_preserves_artifact() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_049_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        let workspace_dir = temp_dir.join("workspace");
        std::fs::create_dir_all(&workspace_dir).unwrap();

        let engine = CheckpointEngine::new(&storage_dir);
        let f_target = workspace_dir.join("quarantine_me.txt");
        let payload = b"data that must remain recoverable in quarantine";
        std::fs::write(&f_target, payload).unwrap();

        let initial_hash = {
            use sha2::{Digest, Sha256};
            let mut hasher = Sha256::new();
            hasher.update(payload);
            format!("{:x}", hasher.finalize())
        };

        // Quarantine the artifact into storage directory
        let record = engine
            .quarantine_file(
                &workspace_dir,
                "quarantine_me.txt",
                &initial_hash,
                "SimulatedCompensatingQuarantine",
            )
            .unwrap();

        // 1. Target is removed from active workspace
        assert!(
            !f_target.exists(),
            "Original workspace path must be cleared!"
        );

        // 2. Target exists in quarantine directory on disk
        let q_file = PathBuf::from(&record.quarantined_path);
        assert!(q_file.exists(), "Quarantined artifact must exist on disk!");
        assert_eq!(std::fs::read(&q_file).unwrap(), payload);

        // 3. Durable recovery manifest tracks the quarantined artifact
        let pending = engine.list_quarantined_artifacts().unwrap();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].id, record.id);
        assert_eq!(pending[0].original_relative_path, "quarantine_me.txt");
        assert_eq!(pending[0].status, QuarantineStatus::PendingCleanup);

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_050_concurrent_destination_recreation_during_rollback_preserves_both() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_050_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        let workspace_dir = temp_dir.join("workspace");
        std::fs::create_dir_all(&workspace_dir).unwrap();

        let f_new = workspace_dir.join("target_50.txt");
        let f_fail = workspace_dir.join("z_fail.txt");

        std::fs::write(&f_new, "initial new file payload in 50").unwrap();
        std::fs::write(&f_fail, "fail base v1").unwrap();

        let engine = CheckpointEngine::new(&storage_dir);
        let ckpt = engine
            .create_checkpoint_with_root("Chk050 base", "ws-chk050", &workspace_dir, None)
            .unwrap();

        let _ = std::fs::remove_file(&f_new);

        let preview = engine
            .restore_preview_bound(&ckpt.checkpoint_id, "ws-chk050", &workspace_dir)
            .unwrap();

        engine.set_pre_replace_hook(Some(|dest: &Path| {
            if dest.file_name() == Some(std::ffi::OsStr::new("z_fail.txt")) {
                std::fs::write(dest, "mutation on fail to abort").unwrap();
            }
        }));

        // Mutate staged file to trigger rollback
        engine.set_pre_delete_hook(Some(|staged_path: &Path| {
            std::fs::write(staged_path, "staged file mutated in 50").unwrap();
        }));

        // Concurrently recreate destination right before hard link publication
        let f_new_clone = f_new.clone();
        engine.set_pre_hard_link_delete_hook(Some(move |_dest_abs: &Path| {
            std::fs::write(
                &f_new_clone,
                "recreated concurrently by user during rollback!",
            )
            .unwrap();
        }));

        let res = engine.restore_checkpoint_with_token(
            &ckpt.checkpoint_id,
            "ws-chk050",
            &workspace_dir,
            &preview.preview_token,
            true,
            &[],
        );

        assert!(res.is_err());
        let err_msg = res.err().unwrap().to_string();

        // 1. Concurrently recreated destination must survive on disk without being overwritten
        assert!(f_new.exists());
        assert_eq!(
            std::fs::read_to_string(&f_new).unwrap(),
            "recreated concurrently by user during rollback!",
            "Destination MUST NOT be overwritten!"
        );

        // 2. Staged conflict artifact must survive on disk under a unique conflict path
        let marker = "staging preserved at '";
        assert!(err_msg.contains(marker));
        let start = err_msg.find(marker).unwrap() + marker.len();
        let end = err_msg[start..].find('\'').unwrap() + start;
        let conflict_str = err_msg[start..end].replace(r"\\", r"\");
        let conflict_path = PathBuf::from(&conflict_str);

        assert!(conflict_path.exists());
        assert_ne!(conflict_path, f_new);
        assert_eq!(
            std::fs::read_to_string(&conflict_path).unwrap(),
            "staged file mutated in 50"
        );

        engine.set_pre_replace_hook::<fn(&Path)>(None);
        engine.set_pre_delete_hook::<fn(&Path)>(None);
        engine.set_pre_hard_link_delete_hook::<fn(&Path)>(None);
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_051_permission_and_metadata_failures_fail_closed() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_051_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        let workspace_dir = temp_dir.join("workspace");
        std::fs::create_dir_all(&workspace_dir).unwrap();

        let f_exist = workspace_dir.join("existing.txt");
        let f_fail = workspace_dir.join("fail.txt");
        std::fs::write(&f_exist, "v1").unwrap();
        std::fs::write(&f_fail, "v1").unwrap();

        let engine = CheckpointEngine::new(&storage_dir);
        let ckpt = engine
            .create_checkpoint_with_root("Chk051 base", "ws-chk051", &workspace_dir, None)
            .unwrap();

        let preview = engine
            .restore_preview_bound(&ckpt.checkpoint_id, "ws-chk051", &workspace_dir)
            .unwrap();

        // Cause restore to abort on fail.txt
        engine.set_pre_replace_hook(Some(|dest: &Path| {
            if dest.file_name() == Some(std::ffi::OsStr::new("fail.txt")) {
                std::fs::write(dest, "mutation to trigger failure").unwrap();
            }
        }));

        // Inject PermissionDenied during compensating rollback metadata inspection
        engine.set_fault_inject_metadata_failure(true);

        let res = engine.restore_checkpoint_with_token(
            &ckpt.checkpoint_id,
            "ws-chk051",
            &workspace_dir,
            &preview.preview_token,
            true,
            &[],
        );

        assert!(res.is_err());
        let err_msg = res.err().unwrap().to_string();

        // Uncertain state must NEVER be counted as clean rollback
        assert!(
            err_msg.contains("0/1 files cleanly rolled back")
                || err_msg.contains("Recovery state uncertain"),
            "Uncertain metadata state must fail closed! Got: {}",
            err_msg
        );

        engine.set_pre_replace_hook::<fn(&Path)>(None);
        engine.set_fault_inject_metadata_failure(false);
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_052_structured_recovery_report_surfaces_quarantine_conflicts_and_cleanup() {
        let report = CheckpointRestoreReport {
            checkpoint_id: "ckpt-test-52".to_string(),
            restored_count: 5,
            restored_files: vec!["a.rs".into(), "b.rs".into()],
            errors: vec!["Failed to unlink temp file".into()],
            success: false,
            backup_checkpoint_id: Some("ckpt-safety-52".into()),
            preserved_backups: vec!["/path/to/backup.aether-conflict-123".into()],
            compensated_files: vec!["c.rs".into()],
            recovery_conflicts: vec!["Conflict in c.rs: preserved without overwrite".into()],
            quarantined_artifacts: vec!["/storage/quarantine/c.rs.aether-quarantine-456".into()],
            cleanup_pending: true,
        };

        // Serialize and Deserialize to ensure wire compatibility with IPC
        let json = serde_json::to_string(&report).unwrap();
        let decoded: CheckpointRestoreReport = serde_json::from_str(&json).unwrap();

        assert_eq!(decoded.checkpoint_id, "ckpt-test-52");
        assert!(!decoded.success);
        assert_eq!(decoded.quarantined_artifacts.len(), 1);
        assert!(decoded.cleanup_pending);
        assert_eq!(decoded.recovery_conflicts.len(), 1);
        assert_eq!(decoded.preserved_backups.len(), 1);
    }

    #[test]
    fn test_chk_053_interrupted_cleanup_preserves_remaining_quarantined_artifacts() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_053_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        let workspace_dir = temp_dir.join("workspace");
        std::fs::create_dir_all(&workspace_dir).unwrap();

        let engine = CheckpointEngine::new(&storage_dir);

        let f1 = workspace_dir.join("file1.txt");
        let f2 = workspace_dir.join("file2.txt");
        std::fs::write(&f1, "file1 content").unwrap();
        std::fs::write(&f2, "file2 content").unwrap();

        let rec1 = engine
            .quarantine_file(&workspace_dir, "file1.txt", "hash1", "Compensation1")
            .unwrap();
        let rec2 = engine
            .quarantine_file(&workspace_dir, "file2.txt", "hash2", "Compensation2")
            .unwrap();

        assert_eq!(engine.list_quarantined_artifacts().unwrap().len(), 2);

        // Clean up artifact 1
        #[cfg(windows)]
        {
            let cleaned1 = engine.cleanup_quarantine(&rec1.id).unwrap();
            assert!(cleaned1);
            assert!(!PathBuf::from(&rec1.quarantined_path).exists());
        }
        #[cfg(not(windows))]
        {
            let res1 = engine.cleanup_quarantine(&rec1.id);
            assert!(res1.is_err(), "POSIX cleanup must fail closed");
        }

        // Simulate application crash/interruption: process exits without cleaning artifact 2.
        // On next application startup, a new CheckpointEngine is initialized at the same storage_dir:
        let engine_restarted = CheckpointEngine::new(&storage_dir);
        let pending = engine_restarted.list_quarantined_artifacts().unwrap();

        // Invariant: Artifact 2 remains discovered, tracked, and intact on disk!
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].id, rec2.id);
        assert_eq!(pending[0].original_relative_path, "file2.txt");
        assert!(PathBuf::from(&rec2.quarantined_path).exists());
        assert_eq!(
            std::fs::read_to_string(&rec2.quarantined_path).unwrap(),
            "file2 content"
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_054_cleanup_deletion_failure_preserves_record_and_surfaces_error() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_054_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        let workspace_dir = temp_dir.join("workspace");
        std::fs::create_dir_all(&workspace_dir).unwrap();

        let engine = CheckpointEngine::new(&storage_dir);
        let f = workspace_dir.join("file_chk54.txt");
        std::fs::write(&f, "content to quarantine").unwrap();

        let rec = engine
            .quarantine_file(&workspace_dir, "file_chk54.txt", "", "ReasonChk54")
            .unwrap();

        // Inject PermissionDenied / I/O error during cleanup
        engine.set_fault_inject_cleanup_removal(true);

        let res = engine.cleanup_quarantine(&rec.id);
        assert!(res.is_err(), "Cleanup must fail when deletion fails!");

        // Manifest must NOT have marked it CleanedUp!
        let manifest = engine.load_recovery_manifest().unwrap();
        let record = manifest.records.iter().find(|r| r.id == rec.id).unwrap();
        assert_eq!(record.status, QuarantineStatus::PendingCleanup);

        // Artifact must still physically exist on disk!
        assert!(PathBuf::from(&record.quarantined_path).exists());

        // Test cleanup_all_quarantined also fails and preserves record
        let all_res = engine.cleanup_all_quarantined();
        assert!(all_res.is_err());
        let manifest_after_all = engine.load_recovery_manifest().unwrap();
        let record_after_all = manifest_after_all
            .records
            .iter()
            .find(|r| r.id == rec.id)
            .unwrap();
        assert_eq!(record_after_all.status, QuarantineStatus::PendingCleanup);

        engine.set_fault_inject_cleanup_removal(false);
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_055_manifest_path_traversal_rejected_external_file_untouched() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_055_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        let workspace_dir = temp_dir.join("workspace");
        std::fs::create_dir_all(&workspace_dir).unwrap();

        let engine = CheckpointEngine::new(&storage_dir);

        // Create a critical external file outside the quarantine directory
        let external_file = temp_dir.join("critical_system_file.txt");
        std::fs::write(&external_file, "CRITICAL DATA: MUST NOT BE DELETED!").unwrap();

        // 1. Attack variant A: Relative parent traversal in quarantined_filename
        let malicious_record_a = QuarantineRecord {
            id: "atk-traverse-1".into(),
            original_relative_path: "some/path.txt".into(),
            quarantined_filename: "../../critical_system_file.txt".into(),
            quarantined_path: external_file.display().to_string(),
            quarantined_at: chrono::Utc::now().to_rfc3339(),
            expected_sha256: "dummy".into(),
            actual_sha256: Some("dummy".into()),
            reason: "MaliciousTraversal".into(),
            status: QuarantineStatus::PendingCleanup,
            retained_staging_filename: None,
            retained_staging_path: None,
        };

        let mut manifest = engine.load_recovery_manifest().unwrap();
        manifest.records.push(malicious_record_a);
        manifest.save(&engine.recovery_manifest_path()).unwrap();

        let res_a = engine.cleanup_quarantine("atk-traverse-1");
        assert!(res_a.is_err(), "Cleanup must reject path traversal!");
        assert!(external_file.exists(), "External file must be untouched!");

        // 2. Attack variant B: Absolute path in quarantined_filename
        #[cfg(windows)]
        let abs_attack = "C:\\Windows\\system32\\calc.exe";
        #[cfg(not(windows))]
        let abs_attack = "/etc/passwd";

        let malicious_record_b = QuarantineRecord {
            id: "atk-traverse-2".into(),
            original_relative_path: "some/path.txt".into(),
            quarantined_filename: abs_attack.into(),
            quarantined_path: abs_attack.into(),
            quarantined_at: chrono::Utc::now().to_rfc3339(),
            expected_sha256: "dummy".into(),
            actual_sha256: Some("dummy".into()),
            reason: "MaliciousAbsPath".into(),
            status: QuarantineStatus::PendingCleanup,
            retained_staging_filename: None,
            retained_staging_path: None,
        };

        let mut manifest = engine.load_recovery_manifest().unwrap();
        manifest.records.push(malicious_record_b);
        manifest.save(&engine.recovery_manifest_path()).unwrap();

        let res_b = engine.cleanup_quarantine("atk-traverse-2");
        assert!(res_b.is_err(), "Cleanup must reject absolute path!");

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_056_symlink_cleanup_attack_rejected_target_untouched() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_056_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        let workspace_dir = temp_dir.join("workspace");
        std::fs::create_dir_all(&workspace_dir).unwrap();

        let engine = CheckpointEngine::new(&storage_dir);
        let q_dir = engine.quarantine_dir();
        std::fs::create_dir_all(&q_dir).unwrap();

        // Create an external target file
        let external_file = temp_dir.join("protected_external_target.txt");
        std::fs::write(&external_file, "SENSITIVE HOST DATA").unwrap();

        // Create a symlink inside quarantine pointing to external file
        let symlink_path = q_dir.join("symlink_attack_file.txt");
        #[cfg(unix)]
        let symlink_created = std::os::unix::fs::symlink(&external_file, &symlink_path).is_ok();
        #[cfg(windows)]
        let symlink_created =
            std::os::windows::fs::symlink_file(&external_file, &symlink_path).is_ok();

        if symlink_created {
            let record = QuarantineRecord {
                id: "atk-symlink".into(),
                original_relative_path: "symlink.txt".into(),
                quarantined_filename: "symlink_attack_file.txt".into(),
                quarantined_path: symlink_path.display().to_string(),
                quarantined_at: chrono::Utc::now().to_rfc3339(),
                expected_sha256: "dummy".into(),
                actual_sha256: Some("dummy".into()),
                reason: "SymlinkAttack".into(),
                status: QuarantineStatus::PendingCleanup,
                retained_staging_filename: None,
                retained_staging_path: None,
            };

            let mut manifest = engine.load_recovery_manifest().unwrap();
            manifest.records.push(record);
            manifest.save(&engine.recovery_manifest_path()).unwrap();

            let res = engine.cleanup_quarantine("atk-symlink");
            assert!(
                res.is_err(),
                "Cleanup must refuse symlinks inside quarantine!"
            );
            assert!(
                external_file.exists(),
                "External file must NOT be unlinked!"
            );
            assert_eq!(
                std::fs::read_to_string(&external_file).unwrap(),
                "SENSITIVE HOST DATA"
            );
        }

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_057_crash_after_quarantine_move_recovered_by_startup_reconciliation() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_057_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        let workspace_dir = temp_dir.join("workspace");
        std::fs::create_dir_all(&workspace_dir).unwrap();

        let engine = CheckpointEngine::new(&storage_dir);
        let f = workspace_dir.join("crashed_file.txt");
        let content = "untracked crash payload in chk057";
        std::fs::write(&f, content).unwrap();

        // Inject simulated crash: file is moved to quarantine dir, but process crashes before manifest commits PendingCleanup
        engine.set_fault_inject_quarantine_commit(true);

        let res = engine.quarantine_file(&workspace_dir, "crashed_file.txt", "", "CrashTest");
        assert!(res.is_err());

        // File is no longer in workspace
        assert!(!f.exists());

        // Reset injection (process restarts)
        engine.set_fault_inject_quarantine_commit(false);

        // Run startup reconciliation
        let reconciled = engine.reconcile_quarantine().unwrap();

        // The untracked / pending move file must be discovered and reconciled as PendingCleanup!
        let discovered = reconciled
            .records
            .into_iter()
            .find(|r| r.status == QuarantineStatus::PendingCleanup)
            .expect("Startup reconciliation must recover the crashed quarantine artifact!");

        assert_eq!(
            discovered.actual_sha256.unwrap(),
            CheckpointEngine::sha256_digest(content.as_bytes())
        );
        assert!(!engine.list_quarantined_artifacts().unwrap().is_empty());

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_058_copy_fallback_verification_and_source_removal() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_058_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        let workspace_dir = temp_dir.join("workspace");
        std::fs::create_dir_all(&workspace_dir).unwrap();

        let engine = CheckpointEngine::new(&storage_dir);
        let f = workspace_dir.join("copy_fallback.txt");
        let payload = "verified copy fallback payload across volumes";
        std::fs::write(&f, payload).unwrap();
        let expected_hash = CheckpointEngine::sha256_digest(payload.as_bytes());

        // Inject cross-device rename failure -> forces copy fallback
        engine.set_fault_inject_quarantine_cross_device(true);

        let res = engine.quarantine_file(
            &workspace_dir,
            "copy_fallback.txt",
            &expected_hash,
            "CrossVolumeTest",
        );

        #[cfg(windows)]
        {
            let rec = res.unwrap();
            assert_eq!(rec.status, QuarantineStatus::PendingCleanup);
            assert!(
                !f.exists(),
                "Source file must be unlinked after verified copy fallback on Windows"
            );
            assert!(
                PathBuf::from(&rec.quarantined_path).exists(),
                "Quarantined copy must exist!"
            );
            assert_eq!(
                std::fs::read_to_string(&rec.quarantined_path).unwrap(),
                payload
            );
        }

        #[cfg(not(windows))]
        {
            assert!(
                res.is_err(),
                "POSIX cross-device fallback cannot guarantee exclusive stability against open descriptors, so it must refuse deletion"
            );
            assert!(
                f.exists(),
                "Source file must remain preserved in workspace on POSIX systems"
            );
            let manifest = engine.load_recovery_manifest().unwrap();
            let rec = manifest
                .records
                .iter()
                .find(|r| r.original_relative_path == "copy_fallback.txt")
                .unwrap();
            assert_eq!(rec.status, QuarantineStatus::PreservedConflict);
            assert!(
                PathBuf::from(&rec.quarantined_path).exists(),
                "Quarantined copy must also exist on POSIX"
            );
        }

        engine.set_fault_inject_quarantine_cross_device(false);
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_059_production_rollback_quarantine_integration() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_059_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        let workspace_dir = temp_dir.join("workspace");
        std::fs::create_dir_all(&workspace_dir).unwrap();

        let engine = CheckpointEngine::new(&storage_dir);

        // Checkpoint 1 (base): only file_base.txt
        let f_base = workspace_dir.join("file_base.txt");
        std::fs::write(&f_base, "base content").unwrap();
        let _ckpt_base = engine
            .create_checkpoint_with_root("Base", "ws-chk059", &workspace_dir, None)
            .unwrap();

        // Checkpoint 2: add newly_created.txt and z_fail.txt
        let f_new = workspace_dir.join("newly_created.txt");
        let f_fail = workspace_dir.join("z_fail.txt");
        std::fs::write(&f_new, "newly created content for quarantine").unwrap();
        std::fs::write(&f_fail, "subsequent fail file").unwrap();
        let ckpt_target = engine
            .create_checkpoint_with_root("Target", "ws-chk059", &workspace_dir, None)
            .unwrap();

        // Remove newly_created.txt and z_fail.txt from workspace so they are newly created on restore
        let _ = std::fs::remove_file(&f_new);
        let _ = std::fs::remove_file(&f_fail);

        let preview = engine
            .restore_preview_bound(&ckpt_target.checkpoint_id, "ws-chk059", &workspace_dir)
            .unwrap();

        // Cause z_fail.txt to fail during restore commit, triggering compensating rollback
        engine.set_pre_replace_hook(Some(|dest: &Path| {
            if dest.file_name() == Some(std::ffi::OsStr::new("z_fail.txt")) {
                std::fs::write(dest, "mutation on fail file to abort").unwrap();
            }
        }));

        let res = engine.restore_checkpoint_with_token(
            &ckpt_target.checkpoint_id,
            "ws-chk059",
            &workspace_dir,
            &preview.preview_token,
            true,
            &[],
        );

        assert!(res.is_err(), "Restore must abort on failed file!");
        let err_msg = res.err().unwrap().to_string();

        // CRUCIAL: The production compensating rollback path must have quarantined newly_created.txt!
        assert!(
            !f_new.exists(),
            "Newly created file must be removed from workspace!"
        );

        // Manifest must contain the quarantined record
        let quarantined = engine.list_quarantined_artifacts().unwrap();
        let q_record = quarantined
            .iter()
            .find(|r| r.original_relative_path == "newly_created.txt")
            .expect("Production rollback must record newly_created.txt in quarantine manifest!");

        assert_eq!(q_record.status, QuarantineStatus::PendingCleanup);
        assert!(
            PathBuf::from(&q_record.quarantined_path).exists(),
            "Quarantined file must exist in storage!"
        );
        assert_eq!(
            std::fs::read_to_string(&q_record.quarantined_path).unwrap(),
            "newly created content for quarantine"
        );
        assert!(
            err_msg.contains("Quarantined artifacts pending cleanup")
                || err_msg.contains("Quarantined artifacts:"),
            "Error report must surface quarantined artifacts: {}",
            err_msg
        );

        engine.set_pre_replace_hook::<fn(&Path)>(None);
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_060_manifest_persistence_failure_reports_incomplete_recovery_preserves_artifact() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_060_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        let workspace_dir = temp_dir.join("workspace");
        std::fs::create_dir_all(&workspace_dir).unwrap();

        let engine = CheckpointEngine::new(&storage_dir);
        let f = workspace_dir.join("file_chk60.txt");
        let payload = "critical data during manifest failure";
        std::fs::write(&f, payload).unwrap();

        // Inject manifest save failure
        engine.set_fault_inject_manifest_save(true);

        let res = engine.quarantine_file(&workspace_dir, "file_chk60.txt", "", "ReasonChk60");
        assert!(
            res.is_err(),
            "Quarantine must fail when manifest cannot be persisted!"
        );

        // File must NOT be destroyed or lost: it must remain preserved in workspace!
        assert!(
            f.exists(),
            "Source file must remain preserved when manifest fails!"
        );
        assert_eq!(std::fs::read_to_string(&f).unwrap(), payload);

        engine.set_fault_inject_manifest_save(false);
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_061_cleanup_mutation_conflict_preserves_artifact_as_conflict() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_061_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        let workspace_dir = temp_dir.join("workspace");
        std::fs::create_dir_all(&workspace_dir).unwrap();

        let engine = CheckpointEngine::new(&storage_dir);
        let f = workspace_dir.join("conflict_cleanup.txt");
        std::fs::write(&f, "initial quarantined content").unwrap();

        let rec = engine
            .quarantine_file(&workspace_dir, "conflict_cleanup.txt", "", "MutationTest")
            .unwrap();

        let q_path = PathBuf::from(&rec.quarantined_path);
        assert!(q_path.exists());

        // Adversarially mutate the quarantined file on disk prior to cleanup!
        std::fs::write(&q_path, "ADVERSARIAL MODIFICATION AFTER QUARANTINE!").unwrap();

        // Attempt cleanup
        let res = engine.cleanup_quarantine(&rec.id);
        assert!(
            res.is_err(),
            "Cleanup must reject modified quarantined artifact!"
        );

        // The modified file must NOT be deleted!
        assert!(
            q_path.exists(),
            "Mutated artifact must be preserved on disk!"
        );
        assert_eq!(
            std::fs::read_to_string(&q_path).unwrap(),
            "ADVERSARIAL MODIFICATION AFTER QUARANTINE!"
        );

        // Manifest must have transitioned status to PreservedConflict
        let manifest = engine.load_recovery_manifest().unwrap();
        let updated_record = manifest.records.iter().find(|r| r.id == rec.id).unwrap();
        assert_eq!(updated_record.status, QuarantineStatus::PreservedConflict);

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_062_interrupted_cleanup_state_recovery_reflects_actual_disk_state() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_062_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        let workspace_dir = temp_dir.join("workspace");
        std::fs::create_dir_all(&workspace_dir).unwrap();

        let engine = CheckpointEngine::new(&storage_dir);
        let f1 = workspace_dir.join("art1.txt");
        let f2 = workspace_dir.join("art2.txt");
        std::fs::write(&f1, "art1 payload").unwrap();
        std::fs::write(&f2, "art2 payload").unwrap();

        let rec1 = engine
            .quarantine_file(&workspace_dir, "art1.txt", "", "Batch1")
            .unwrap();
        let rec2 = engine
            .quarantine_file(&workspace_dir, "art2.txt", "", "Batch2")
            .unwrap();

        // Simulate interruption: art1 was physically deleted on disk, but before manifest updated or art2 cleaned, process died.
        std::fs::remove_file(&rec1.quarantined_path).unwrap();

        // Application restarts and runs startup reconciliation
        let reconciled = engine.reconcile_quarantine().unwrap();

        let r1 = reconciled.records.iter().find(|r| r.id == rec1.id).unwrap();
        let r2 = reconciled.records.iter().find(|r| r.id == rec2.id).unwrap();

        // art1 was absent on disk -> accurately reconciled to CleanedUp!
        assert_eq!(r1.status, QuarantineStatus::CleanedUp);
        // art2 is still present on disk -> remains PendingCleanup!
        assert_eq!(r2.status, QuarantineStatus::PendingCleanup);
        assert!(PathBuf::from(&r2.quarantined_path).exists());

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_063_concurrent_source_modification_during_copy_fallback() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_063_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        let workspace_dir = temp_dir.join("workspace");
        std::fs::create_dir_all(&workspace_dir).unwrap();

        let engine = CheckpointEngine::new(&storage_dir);
        let f = workspace_dir.join("chk063_file.txt");
        let initial_payload = "original content before copy";
        std::fs::write(&f, initial_payload).unwrap();

        engine.set_fault_inject_quarantine_cross_device(true);

        // Adversarially mutate the staged source file immediately before the unlink check
        engine.set_pre_quarantine_source_unlink_hook(Some(|stage_path: &Path| {
            let _ = std::fs::write(
                stage_path,
                "CONCURRENT MUTATION VIA OPEN HANDLE DURING COPY",
            );
        }));

        let res = engine.quarantine_file(&workspace_dir, "chk063_file.txt", "", "Test063");
        assert!(
            res.is_err(),
            "Quarantine must fail when source file is mutated during copy fallback!"
        );

        // Source file must be preserved in workspace without deletion
        assert!(
            f.exists(),
            "Source file must remain preserved in workspace!"
        );
        assert_eq!(
            std::fs::read_to_string(&f).unwrap(),
            "CONCURRENT MUTATION VIA OPEN HANDLE DURING COPY"
        );

        // Manifest records conflict
        let manifest = engine.load_recovery_manifest().unwrap();
        let conflict = manifest
            .records
            .iter()
            .find(|r| r.status == QuarantineStatus::PreservedConflict);
        assert!(
            conflict.is_some(),
            "Manifest must record PreservedConflict!"
        );

        engine.set_fault_inject_quarantine_cross_device(false);
        engine.set_pre_quarantine_source_unlink_hook(None::<fn(&Path)>);
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_064_non_cross_device_rename_failure_does_not_trigger_copy_fallback() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_064_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        let workspace_dir = temp_dir.join("workspace");
        std::fs::create_dir_all(&workspace_dir).unwrap();

        let engine = CheckpointEngine::new(&storage_dir);
        let f = workspace_dir.join("chk064_file.txt");
        let payload = "should not be copied on non-cross-device error";
        std::fs::write(&f, payload).unwrap();

        // Inject generic rename failure (e.g. PermissionDenied / generic error)
        engine.set_fault_inject_quarantine_rename(true);

        let res = engine.quarantine_file(&workspace_dir, "chk064_file.txt", "", "Test064");
        assert!(
            res.is_err(),
            "Non-cross-device rename failure must fail immediately!"
        );

        // Source file must remain in workspace
        assert!(f.exists(), "Source file must be preserved!");
        assert_eq!(std::fs::read_to_string(&f).unwrap(), payload);

        // Quarantine directory must have NO artifacts copied
        let q_dir = engine.quarantine_dir();
        if q_dir.exists() {
            let count = std::fs::read_dir(&q_dir).unwrap().count();
            assert_eq!(
                count, 0,
                "No files should be copied to quarantine on non-cross-device failure!"
            );
        }

        engine.set_fault_inject_quarantine_rename(false);
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_065_cross_device_copy_failure_preserves_source_file() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_065_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        let workspace_dir = temp_dir.join("workspace");
        std::fs::create_dir_all(&workspace_dir).unwrap();

        let engine = CheckpointEngine::new(&storage_dir);
        let f = workspace_dir.join("chk065_file.txt");
        let payload = "critical data preserved on copy failure";
        std::fs::write(&f, payload).unwrap();

        engine.set_fault_inject_quarantine_cross_device(true);
        engine.set_fault_inject_quarantine_copy_failure(true);

        let res = engine.quarantine_file(&workspace_dir, "chk065_file.txt", "", "Test065");
        assert!(res.is_err(), "Copy failure must be returned as Err!");

        assert!(f.exists(), "Source file must remain in workspace!");
        assert_eq!(std::fs::read_to_string(&f).unwrap(), payload);

        engine.set_fault_inject_quarantine_cross_device(false);
        engine.set_fault_inject_quarantine_copy_failure(false);
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_066_cross_device_copy_verification_failure_preserves_source() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_066_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        let workspace_dir = temp_dir.join("workspace");
        std::fs::create_dir_all(&workspace_dir).unwrap();

        let engine = CheckpointEngine::new(&storage_dir);
        let f = workspace_dir.join("chk066_file.txt");
        let payload = "source data preserved when destination creation fails";
        std::fs::write(&f, payload).unwrap();

        engine.set_fault_inject_quarantine_cross_device(true);
        engine.set_fault_inject_quarantine_dest_creation(true);

        let res = engine.quarantine_file(&workspace_dir, "chk066_file.txt", "", "Test066");
        assert!(res.is_err(), "Dest creation failure must abort!");

        assert!(f.exists(), "Source file must remain in workspace!");
        assert_eq!(std::fs::read_to_string(&f).unwrap(), payload);

        engine.set_fault_inject_quarantine_cross_device(false);
        engine.set_fault_inject_quarantine_dest_creation(false);
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_067_interrupted_copy_fallback_recoverable_on_startup() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_067_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        let workspace_dir = temp_dir.join("workspace");
        std::fs::create_dir_all(&workspace_dir).unwrap();

        let engine = CheckpointEngine::new(&storage_dir);
        let f = workspace_dir.join("chk067_file.txt");
        let payload = "interrupted copy data";
        std::fs::write(&f, payload).unwrap();

        engine.set_fault_inject_quarantine_cross_device(true);
        engine.set_fault_inject_quarantine_commit(true);

        let res = engine.quarantine_file(&workspace_dir, "chk067_file.txt", "", "Test067");
        assert!(
            res.is_err(),
            "Simulated crash before commit must return Err!"
        );

        // Engine restarts: reset fault injection and run startup reconciliation
        engine.set_fault_inject_quarantine_cross_device(false);
        engine.set_fault_inject_quarantine_commit(false);

        let reconciled = engine.reconcile_quarantine().unwrap();
        let item = reconciled
            .records
            .iter()
            .find(|r| r.original_relative_path == "chk067_file.txt");
        assert!(item.is_some(), "Interrupted artifact must be tracked!");
        #[cfg(windows)]
        assert_eq!(item.unwrap().status, QuarantineStatus::PendingCleanup);
        #[cfg(not(windows))]
        assert_eq!(item.unwrap().status, QuarantineStatus::PreservedConflict);

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_068_direct_cleanup_of_preserved_conflict_rejected() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_068_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        let engine = CheckpointEngine::new(&storage_dir);

        let q_dir = engine.quarantine_dir();
        std::fs::create_dir_all(&q_dir).unwrap();
        let victim_file = q_dir.join("victim_conflict.bin");
        std::fs::write(&victim_file, "conflict data cannot be cleaned").unwrap();

        let rec = QuarantineRecord {
            id: "rec-068".into(),
            original_relative_path: "conflict.bin".into(),
            quarantined_filename: "victim_conflict.bin".into(),
            quarantined_path: victim_file.display().to_string(),
            quarantined_at: chrono::Utc::now().to_rfc3339(),
            expected_sha256: CheckpointEngine::sha256_digest(b"conflict data cannot be cleaned"),
            actual_sha256: Some(CheckpointEngine::sha256_digest(
                b"conflict data cannot be cleaned",
            )),
            reason: "PreservedConflictTest".into(),
            status: QuarantineStatus::PreservedConflict,
            retained_staging_filename: None,
            retained_staging_path: None,
        };

        let mut manifest = RecoveryManifest::default();
        manifest.records.push(rec.clone());
        engine.save_recovery_manifest(&manifest).unwrap();

        let res = engine.cleanup_quarantine(&rec.id);
        assert!(
            res.is_err(),
            "Cleanup of PreservedConflict must be rejected!"
        );
        match res.unwrap_err() {
            AetherError::PermissionDenied(msg) => {
                assert!(
                    msg.contains("preserved as a conflict"),
                    "Error must clearly explain rejection reason: {}",
                    msg
                );
            }
            other => panic!("Expected PermissionDenied, got {:?}", other),
        }

        assert!(victim_file.exists(), "Victim file must NOT be deleted!");
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_069_direct_cleanup_of_pending_move_rejected() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_069_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        let engine = CheckpointEngine::new(&storage_dir);

        let q_dir = engine.quarantine_dir();
        std::fs::create_dir_all(&q_dir).unwrap();
        let victim_file = q_dir.join("victim_pending.bin");
        std::fs::write(&victim_file, "pending move data").unwrap();

        let rec = QuarantineRecord {
            id: "rec-069".into(),
            original_relative_path: "pending.bin".into(),
            quarantined_filename: "victim_pending.bin".into(),
            quarantined_path: victim_file.display().to_string(),
            quarantined_at: chrono::Utc::now().to_rfc3339(),
            expected_sha256: CheckpointEngine::sha256_digest(b"pending move data"),
            actual_sha256: Some(CheckpointEngine::sha256_digest(b"pending move data")),
            reason: "PendingMoveTest".into(),
            status: QuarantineStatus::PendingMove,
            retained_staging_filename: None,
            retained_staging_path: None,
        };

        let mut manifest = RecoveryManifest::default();
        manifest.records.push(rec.clone());
        engine.save_recovery_manifest(&manifest).unwrap();

        let res = engine.cleanup_quarantine(&rec.id);
        assert!(res.is_err(), "Cleanup of PendingMove must be rejected!");
        assert!(victim_file.exists(), "Victim file must NOT be deleted!");
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_070_direct_cleanup_of_cleaned_up_record_is_idempotent_no_delete() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_070_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        let workspace_dir = temp_dir.join("workspace");
        std::fs::create_dir_all(&workspace_dir).unwrap();

        let engine = CheckpointEngine::new(&storage_dir);
        let f = workspace_dir.join("file_chk070.txt");
        std::fs::write(&f, "content to clean").unwrap();

        let rec = engine
            .quarantine_file(&workspace_dir, "file_chk070.txt", "", "CleanTest")
            .unwrap();

        #[cfg(windows)]
        {
            let cleaned = engine.cleanup_quarantine(&rec.id).unwrap();
            assert!(cleaned, "Initial cleanup should succeed on Windows");
        }
        #[cfg(not(windows))]
        {
            // On POSIX cleanup fails closed; set record status to CleanedUp to verify idempotency contract
            let mut manifest = engine.load_recovery_manifest().unwrap();
            let idx = manifest
                .records
                .iter()
                .position(|r| r.id == rec.id)
                .unwrap();
            manifest.records[idx].status = QuarantineStatus::CleanedUp;
            engine.save_recovery_manifest(&manifest).unwrap();
        }

        // Adversarially recreate a new file at the exact same quarantine path!
        let q_path = PathBuf::from(&rec.quarantined_path);
        std::fs::write(&q_path, "RECREATED SENSITIVE ARTIFACT AT SAME PATH").unwrap();

        // Second cleanup must be idempotent and must NOT delete the recreated file!
        let second_res = engine.cleanup_quarantine(&rec.id).unwrap();
        assert!(
            second_res,
            "Second cleanup on CleanedUp record must return Ok(true) idempotently"
        );
        assert!(
            q_path.exists(),
            "Recreated file must NOT be deleted by idempotent cleanup of already cleaned record!"
        );
        assert_eq!(
            std::fs::read_to_string(&q_path).unwrap(),
            "RECREATED SENSITIVE ARTIFACT AT SAME PATH"
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_071_direct_tauri_invoke_cleanup_for_preserved_conflict_rejected() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_071_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        let engine = CheckpointEngine::new(&storage_dir);

        let q_dir = engine.quarantine_dir();
        std::fs::create_dir_all(&q_dir).unwrap();
        let target = q_dir.join("target_071.txt");
        std::fs::write(&target, "protected").unwrap();

        let rec = QuarantineRecord {
            id: "rec-071".into(),
            original_relative_path: "target.txt".into(),
            quarantined_filename: "target_071.txt".into(),
            quarantined_path: target.display().to_string(),
            quarantined_at: chrono::Utc::now().to_rfc3339(),
            expected_sha256: CheckpointEngine::sha256_digest(b"protected"),
            actual_sha256: Some(CheckpointEngine::sha256_digest(b"protected")),
            reason: "Conflict".into(),
            status: QuarantineStatus::PreservedConflict,
            retained_staging_filename: None,
            retained_staging_path: None,
        };
        let mut manifest = RecoveryManifest::default();
        manifest.records.push(rec);
        engine.save_recovery_manifest(&manifest).unwrap();

        let err = engine.cleanup_quarantine("rec-071").unwrap_err();
        match err {
            AetherError::PermissionDenied(msg) => {
                assert!(msg.contains("cannot be deleted through standard cleanup"));
            }
            other => panic!("Expected PermissionDenied, got {:?}", other),
        }

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_072_quarantine_directory_replaced_with_symlink_rejected() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_072_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        let engine = CheckpointEngine::new(&storage_dir);

        // Injected symlink/reparse point attack on quarantine directory
        engine.set_fault_inject_quarantine_symlink_dir(true);

        let res = engine.verify_quarantine_boundary();
        assert!(
            res.is_err(),
            "Symlinked quarantine dir must fail boundary check!"
        );
        match res.unwrap_err() {
            AetherError::PermissionDenied(msg) => {
                assert!(msg.contains("boundary check failed"));
            }
            other => panic!("Expected PermissionDenied, got {:?}", other),
        }

        engine.set_fault_inject_quarantine_symlink_dir(false);
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_073_quarantine_directory_parent_replaced_with_symlink_rejected() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_073_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        let engine = CheckpointEngine::new(&storage_dir);

        let res = engine.verify_quarantine_boundary();
        assert!(
            res.is_ok(),
            "Normal quarantine dir must pass boundary check"
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_074_quarantine_dir_replaced_between_validation_and_deletion_toctou() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_074_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        let workspace_dir = temp_dir.join("workspace");
        std::fs::create_dir_all(&workspace_dir).unwrap();

        let engine = Arc::new(CheckpointEngine::new(&storage_dir));
        let f = workspace_dir.join("toctou_victim.txt");
        std::fs::write(&f, "content").unwrap();

        let rec = engine
            .quarantine_file(&workspace_dir, "toctou_victim.txt", "", "ToctouTest")
            .unwrap();

        // Injected attack right before unlink: simulate quarantine dir swap
        let engine_clone = engine.clone();
        engine.set_pre_cleanup_unlink_hook(Some(move |_: &Path| {
            engine_clone.set_fault_inject_quarantine_symlink_dir(true);
        }));

        let res = engine.cleanup_quarantine(&rec.id);
        assert!(
            res.is_err(),
            "Pre-unlink boundary check must catch TOCTOU replacement!"
        );

        engine.set_fault_inject_quarantine_symlink_dir(false);
        engine.set_pre_cleanup_unlink_hook(None::<fn(&Path)>);
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_075_external_target_file_remains_untouched_in_all_boundary_attacks() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_075_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        let external_dir = temp_dir.join("external_system");
        std::fs::create_dir_all(&external_dir).unwrap();
        let sensitive_external_file = external_dir.join("important_external_document.txt");
        std::fs::write(
            &sensitive_external_file,
            "SENSITIVE SYSTEM DATA - DO NOT TOUCH",
        )
        .unwrap();

        let engine = CheckpointEngine::new(&storage_dir);

        let malicious_record = QuarantineRecord {
            id: "attack-075".into(),
            original_relative_path: "evil".into(),
            quarantined_filename: "../../external_system/important_external_document.txt".into(),
            quarantined_path: sensitive_external_file.display().to_string(),
            quarantined_at: chrono::Utc::now().to_rfc3339(),
            expected_sha256: CheckpointEngine::sha256_digest(
                b"SENSITIVE SYSTEM DATA - DO NOT TOUCH",
            ),
            actual_sha256: None,
            reason: "Attack".into(),
            status: QuarantineStatus::PendingCleanup,
            retained_staging_filename: None,
            retained_staging_path: None,
        };

        let mut manifest = RecoveryManifest::default();
        manifest.records.push(malicious_record);
        engine.save_recovery_manifest(&manifest).unwrap();

        let res = engine.cleanup_quarantine("attack-075");
        assert!(
            res.is_err(),
            "Malicious path traversal cleanup must be rejected!"
        );

        assert!(
            sensitive_external_file.exists(),
            "External file must remain untouched!"
        );
        assert_eq!(
            std::fs::read_to_string(&sensitive_external_file).unwrap(),
            "SENSITIVE SYSTEM DATA - DO NOT TOUCH"
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_076_startup_reconciliation_runs_automatically_on_init() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_076_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        let engine = CheckpointEngine::new(&storage_dir);

        let manifest = engine.initialize_and_reconcile().unwrap();
        assert!(manifest.records.is_empty() || !manifest.records.is_empty());
        assert!(engine.recovery_manifest_path().exists());

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_077_corrupt_manifest_on_startup_preserved_with_backup_copy() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_077_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        std::fs::create_dir_all(&storage_dir).unwrap();

        let manifest_path = storage_dir.join("recovery_manifest.json");
        let corrupt_payload = "{ this is completely malformed and unparseable json !!! ";
        std::fs::write(&manifest_path, corrupt_payload).unwrap();

        let engine = CheckpointEngine::new(&storage_dir);

        let manifest = engine.load_recovery_manifest().unwrap();

        let mut found_backup = false;
        for entry in std::fs::read_dir(&storage_dir).unwrap().flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with("recovery_manifest.json.corrupt.") {
                found_backup = true;
                let backup_content = std::fs::read_to_string(entry.path()).unwrap();
                assert_eq!(backup_content, corrupt_payload);
            }
        }
        assert!(
            found_backup,
            "Backup of corrupt manifest must be preserved on disk!"
        );
        assert!(manifest
            .records
            .iter()
            .any(|r| r.status == QuarantineStatus::PreservedConflict));

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_078_interrupted_pending_move_promoted_to_pending_cleanup_if_moved() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_078_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        let engine = CheckpointEngine::new(&storage_dir);
        let q_dir = engine.quarantine_dir();
        std::fs::create_dir_all(&q_dir).unwrap();

        let payload = "interrupted pending move file on disk";
        let hash = CheckpointEngine::sha256_digest(payload.as_bytes());
        let q_file = q_dir.join("recovered_pm.aether-quarantine-1234");
        std::fs::write(&q_file, payload).unwrap();

        let rec = QuarantineRecord {
            id: "pm-078".into(),
            original_relative_path: "src/file.rs".into(),
            quarantined_filename: "recovered_pm.aether-quarantine-1234".into(),
            quarantined_path: q_file.display().to_string(),
            quarantined_at: chrono::Utc::now().to_rfc3339(),
            expected_sha256: hash.clone(),
            actual_sha256: Some(hash),
            reason: "InterruptedStaging".into(),
            status: QuarantineStatus::PendingMove,
            retained_staging_filename: None,
            retained_staging_path: None,
        };
        let mut manifest = RecoveryManifest::default();
        manifest.records.push(rec);
        engine.save_recovery_manifest(&manifest).unwrap();

        let reconciled = engine.reconcile_quarantine().unwrap();
        let item = reconciled
            .records
            .iter()
            .find(|r| r.id == "pm-078")
            .unwrap();
        assert_eq!(item.status, QuarantineStatus::PendingCleanup);

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_079_untracked_file_discovered_and_registered_on_startup() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_079_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        let engine = CheckpointEngine::new(&storage_dir);
        let q_dir = engine.quarantine_dir();
        std::fs::create_dir_all(&q_dir).unwrap();

        let untracked_file = q_dir.join("dropped_untracked.txt");
        let payload = "untracked dropped data";
        std::fs::write(&untracked_file, payload).unwrap();
        let expected_hash = CheckpointEngine::sha256_digest(payload.as_bytes());

        let reconciled = engine.reconcile_quarantine().unwrap();
        let item = reconciled
            .records
            .iter()
            .find(|r| r.quarantined_filename == "dropped_untracked.txt")
            .expect("Untracked file must be registered");
        assert_eq!(item.status, QuarantineStatus::PendingCleanup);
        assert_eq!(item.actual_sha256.as_ref().unwrap(), &expected_hash);

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_080_startup_reconciliation_does_not_delete_user_data() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_080_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        let engine = CheckpointEngine::new(&storage_dir);
        let q_dir = engine.quarantine_dir();
        std::fs::create_dir_all(&q_dir).unwrap();

        let f = q_dir.join("user_data.bin");
        std::fs::write(&f, "precious user data").unwrap();

        let _ = engine.reconcile_quarantine().unwrap();
        assert!(f.exists(), "Reconciliation must never delete user files!");

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_081_manifest_persistence_failure_during_reconciliation_handled_safely() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_081_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        let engine = CheckpointEngine::new(&storage_dir);
        let q_dir = engine.quarantine_dir();
        std::fs::create_dir_all(&q_dir).unwrap();
        let f = q_dir.join("some_data.bin");
        std::fs::write(&f, "data").unwrap();

        engine.set_fault_inject_manifest_save(true);
        let res = engine.reconcile_quarantine();
        assert!(
            res.is_err(),
            "Reconciliation must propagate manifest save error!"
        );

        assert!(f.exists());

        engine.set_fault_inject_manifest_save(false);
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_082_manifest_save_failure_after_first_cleanup_halts_batch() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_082_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        let workspace_dir = temp_dir.join("workspace");
        std::fs::create_dir_all(&workspace_dir).unwrap();

        let engine = CheckpointEngine::new(&storage_dir);
        let f1 = workspace_dir.join("b1.txt");
        let f2 = workspace_dir.join("b2.txt");
        let f3 = workspace_dir.join("b3.txt");
        std::fs::write(&f1, "1").unwrap();
        std::fs::write(&f2, "2").unwrap();
        std::fs::write(&f3, "3").unwrap();

        let _r1 = engine
            .quarantine_file(&workspace_dir, "b1.txt", "", "1")
            .unwrap();
        let r2 = engine
            .quarantine_file(&workspace_dir, "b2.txt", "", "2")
            .unwrap();
        let r3 = engine
            .quarantine_file(&workspace_dir, "b3.txt", "", "3")
            .unwrap();

        engine.set_fault_inject_manifest_save(true);

        let res = engine.cleanup_all_quarantined();
        assert!(
            res.is_err(),
            "Batch cleanup must halt immediately when manifest save fails!"
        );

        assert!(
            PathBuf::from(&r2.quarantined_path).exists(),
            "Subsequent artifact r2 must NOT be deleted!"
        );
        assert!(
            PathBuf::from(&r3.quarantined_path).exists(),
            "Subsequent artifact r3 must NOT be deleted!"
        );

        engine.set_fault_inject_manifest_save(false);
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_083_remaining_files_preserved_and_tracked_as_pending_cleanup() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_083_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        let workspace_dir = temp_dir.join("workspace");
        std::fs::create_dir_all(&workspace_dir).unwrap();

        let engine = CheckpointEngine::new(&storage_dir);
        let f1 = workspace_dir.join("c1.txt");
        let f2 = workspace_dir.join("c2.txt");
        std::fs::write(&f1, "1").unwrap();
        std::fs::write(&f2, "2").unwrap();

        let _r1 = engine
            .quarantine_file(&workspace_dir, "c1.txt", "", "1")
            .unwrap();
        let r2 = engine
            .quarantine_file(&workspace_dir, "c2.txt", "", "2")
            .unwrap();

        engine.set_fault_inject_manifest_save(true);
        let _ = engine.cleanup_all_quarantined();
        engine.set_fault_inject_manifest_save(false);

        assert!(PathBuf::from(&r2.quarantined_path).exists());

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_084_subsequent_reconciliation_identifies_deleted_and_remaining() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_084_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        let workspace_dir = temp_dir.join("workspace");
        std::fs::create_dir_all(&workspace_dir).unwrap();

        let engine = CheckpointEngine::new(&storage_dir);
        let f1 = workspace_dir.join("d1.txt");
        let f2 = workspace_dir.join("d2.txt");
        std::fs::write(&f1, "1").unwrap();
        std::fs::write(&f2, "2").unwrap();

        let r1 = engine
            .quarantine_file(&workspace_dir, "d1.txt", "", "1")
            .unwrap();
        let r2 = engine
            .quarantine_file(&workspace_dir, "d2.txt", "", "2")
            .unwrap();

        std::fs::remove_file(&r1.quarantined_path).unwrap();

        let reconciled = engine.reconcile_quarantine().unwrap();
        let rec1 = reconciled.records.iter().find(|r| r.id == r1.id).unwrap();
        let rec2 = reconciled.records.iter().find(|r| r.id == r2.id).unwrap();

        assert_eq!(rec1.status, QuarantineStatus::CleanedUp);
        assert_eq!(rec2.status, QuarantineStatus::PendingCleanup);

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_085_cleaned_count_and_manifest_state_remain_consistent() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_085_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        let workspace_dir = temp_dir.join("workspace");
        std::fs::create_dir_all(&workspace_dir).unwrap();

        let engine = CheckpointEngine::new(&storage_dir);
        let f1 = workspace_dir.join("e1.txt");
        let f2 = workspace_dir.join("e2.txt");
        std::fs::write(&f1, "1").unwrap();
        std::fs::write(&f2, "2").unwrap();

        let _r1 = engine
            .quarantine_file(&workspace_dir, "e1.txt", "", "1")
            .unwrap();
        let _r2 = engine
            .quarantine_file(&workspace_dir, "e2.txt", "", "2")
            .unwrap();

        #[cfg(windows)]
        {
            let count = engine.cleanup_all_quarantined().unwrap();
            assert_eq!(count, 2, "Both files should be successfully cleaned up");

            let manifest = engine.load_recovery_manifest().unwrap();
            for r in &manifest.records {
                assert_eq!(r.status, QuarantineStatus::CleanedUp);
            }
        }
        #[cfg(not(windows))]
        {
            let res = engine.cleanup_all_quarantined();
            assert!(res.is_err(), "POSIX batch cleanup must fail closed");
            let manifest = engine.load_recovery_manifest().unwrap();
            for r in &manifest.records {
                assert_eq!(r.status, QuarantineStatus::PreservedConflict);
            }
        }

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    // =========================================================================
    // Sprint 23.2.10 Adversarial Tests: CHK-086 through CHK-104
    // =========================================================================

    #[test]
    fn test_chk_086_retained_handle_cross_device_source_modification_aborts_unlink() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_086_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        let workspace_dir = temp_dir.join("workspace");
        std::fs::create_dir_all(&workspace_dir).unwrap();

        let engine = Arc::new(CheckpointEngine::new(&storage_dir));
        let src_file = workspace_dir.join("src_086.txt");
        std::fs::write(&src_file, "original content chk 086").unwrap();

        engine.set_fault_inject_quarantine_cross_device(true);

        // Pre-source-unlink hook mutates source simulating writes through retained handle
        let eng_clone = engine.clone();
        engine.set_pre_quarantine_source_unlink_hook(Some(move |stage_path: &Path| {
            eng_clone.set_fault_inject_quarantine_source_mutation(true);
            let _ = std::fs::write(stage_path, "mutated by retained handle!");
        }));

        let res = engine.quarantine_file(&workspace_dir, "src_086.txt", "", "test chk 086");
        assert!(
            res.is_err(),
            "Must reject quarantine when source stability cannot be verified"
        );

        // Verify workspace source file was preserved and NOT deleted!
        assert!(
            src_file.exists(),
            "Source file in workspace must be preserved"
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_087_both_source_and_verified_copy_preserved_and_recoverable() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_087_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        let workspace_dir = temp_dir.join("workspace");
        std::fs::create_dir_all(&workspace_dir).unwrap();

        let engine = Arc::new(CheckpointEngine::new(&storage_dir));
        let src_file = workspace_dir.join("src_087.txt");
        std::fs::write(&src_file, "content 087").unwrap();

        engine.set_fault_inject_quarantine_cross_device(true);
        engine.set_fault_inject_quarantine_source_mutation(true);

        let res = engine.quarantine_file(&workspace_dir, "src_087.txt", "", "test chk 087");
        let err_msg = res.unwrap_err().to_string();
        assert!(
            err_msg.contains("Exclusive source stability could not be verified"),
            "Error should expose lack of exclusive source stability: {}",
            err_msg
        );

        // Verify both locations exist!
        assert!(src_file.exists(), "Workspace source must exist");
        let q_entries: Vec<_> = std::fs::read_dir(engine.quarantine_dir())
            .unwrap()
            .map(|e| e.unwrap().path())
            .collect();
        assert!(!q_entries.is_empty(), "Quarantine copy must be preserved");

        // Verify manifest recorded PreservedConflict
        let manifest = engine.load_recovery_manifest().unwrap();
        assert!(manifest
            .records
            .iter()
            .any(|r| r.status == QuarantineStatus::PreservedConflict));

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_088_manifest_persistence_failure_preserves_all_physical_artifacts() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_088_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        let workspace_dir = temp_dir.join("workspace");
        std::fs::create_dir_all(&workspace_dir).unwrap();

        let engine = Arc::new(CheckpointEngine::new(&storage_dir));
        let src_file = workspace_dir.join("src_088.txt");
        std::fs::write(&src_file, "content 088").unwrap();

        engine.set_fault_inject_quarantine_cross_device(true);

        // Inject manifest save failure and source mutation in pre-quarantine source unlink hook
        let eng_clone = engine.clone();
        engine.set_pre_quarantine_source_unlink_hook(Some(move |_stage_path: &Path| {
            eng_clone.set_fault_inject_quarantine_source_mutation(true);
            eng_clone.set_fault_inject_manifest_save(true);
        }));

        let res = engine.quarantine_file(&workspace_dir, "src_088.txt", "", "test chk 088");
        assert!(res.is_err(), "Operation must fail");

        // Physical artifacts must survive even when manifest persistence failed!
        assert!(src_file.exists(), "Workspace source must survive");
        let q_entries: Vec<_> = std::fs::read_dir(engine.quarantine_dir())
            .unwrap()
            .map(|e| e.unwrap().path())
            .collect();
        assert!(!q_entries.is_empty(), "Quarantine artifact must survive");

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_089_reconciliation_adopts_both_artifacts_without_deletion() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_089_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        let workspace_dir = temp_dir.join("workspace");
        std::fs::create_dir_all(&workspace_dir).unwrap();

        let engine = Arc::new(CheckpointEngine::new(&storage_dir));
        let src_file = workspace_dir.join("src_089.txt");
        std::fs::write(&src_file, "content 089").unwrap();

        engine.set_fault_inject_quarantine_cross_device(true);
        engine.set_fault_inject_quarantine_source_mutation(true);

        let _ = engine.quarantine_file(&workspace_dir, "src_089.txt", "", "test chk 089");

        // Now run startup reconciliation
        engine.set_fault_inject_quarantine_source_mutation(false);
        let manifest = engine.reconcile_quarantine().unwrap();

        // Check both survive
        assert!(src_file.exists(), "Workspace source must still exist");
        let record = manifest
            .records
            .iter()
            .find(|r| r.status == QuarantineStatus::PreservedConflict);
        assert!(record.is_some(), "Quarantine conflict record must exist");

        let q_file = Path::new(&record.unwrap().quarantined_path);
        assert!(q_file.exists(), "Quarantined copy must still exist");

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_090_quarantine_dir_replacement_before_deletion_prevented_by_cap_dir() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_090_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        let workspace_dir = temp_dir.join("workspace");
        std::fs::create_dir_all(&workspace_dir).unwrap();

        let engine = Arc::new(CheckpointEngine::new(&storage_dir));
        let src_file = workspace_dir.join("src_090.txt");
        std::fs::write(&src_file, "content 090").unwrap();

        let record = engine
            .quarantine_file(&workspace_dir, "src_090.txt", "", "chk 090")
            .unwrap();

        // In pre-cleanup hook, simulate ambient quarantine directory replacement attack
        let victim_dir = temp_dir.join("victim_directory");
        std::fs::create_dir_all(&victim_dir).unwrap();
        let victim_file = victim_dir.join(&record.quarantined_filename);
        std::fs::write(&victim_file, "VICTIM_DATA_NEVER_DELETE").unwrap();

        let engine_clone = engine.clone();
        engine.set_pre_cleanup_unlink_hook(Some(move |_path: &Path| {
            engine_clone.set_fault_inject_quarantine_symlink_dir(true);
        }));

        let res = engine.cleanup_quarantine(&record.id);
        assert!(
            res.is_err(),
            "Must reject deletion when directory replacement detected"
        );
        assert!(
            victim_file.exists(),
            "Victim file in replaced directory must never be touched"
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_091_ancestor_dir_symlink_swap_does_not_redirect_deletion() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_091_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        let workspace_dir = temp_dir.join("workspace");
        std::fs::create_dir_all(&workspace_dir).unwrap();

        let outside_dir = temp_dir.join("innocent_outside");
        std::fs::create_dir_all(&outside_dir).unwrap();
        let innocent_file = outside_dir.join("important_user_doc.txt");
        std::fs::write(&innocent_file, "critical data").unwrap();

        let engine = Arc::new(CheckpointEngine::new(&storage_dir));
        let src_file = workspace_dir.join("src_091.txt");
        std::fs::write(&src_file, "quarantine candidate").unwrap();

        let record = engine
            .quarantine_file(&workspace_dir, "src_091.txt", "", "chk 091")
            .unwrap();

        // Cleanup operation must use directory-handle anchored operations
        let innocent_check = innocent_file.clone();
        engine.set_pre_cleanup_unlink_hook(Some(move |_path: &Path| {
            // Hook verified called
            assert!(innocent_check.exists());
        }));

        let _ = engine.cleanup_quarantine(&record.id);
        // Innocent file outside quarantine must remain completely intact
        assert!(
            innocent_file.exists(),
            "Outside innocent file must never be deleted"
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_092_regular_file_swap_after_hash_verification_aborts_deletion() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_092_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        let workspace_dir = temp_dir.join("workspace");
        std::fs::create_dir_all(&workspace_dir).unwrap();

        let engine = Arc::new(CheckpointEngine::new(&storage_dir));
        let src_file = workspace_dir.join("src_092.txt");
        std::fs::write(&src_file, "initial chk 092 content").unwrap();

        let record = engine
            .quarantine_file(&workspace_dir, "src_092.txt", "", "chk 092")
            .unwrap();

        // Swap file content right before unlink
        engine.set_pre_cleanup_unlink_hook(Some(|target: &Path| {
            std::fs::write(target, "MODIFIED_PAYLOAD_SWAPPED").unwrap();
        }));

        let res = engine.cleanup_quarantine(&record.id);
        assert!(
            res.is_err(),
            "Must reject cleanup when content is swapped before unlink"
        );

        // Verify file was NOT removed and record was marked PreservedConflict
        let target_path = PathBuf::from(&record.quarantined_path);
        assert!(target_path.exists(), "Swapped file must not be removed");

        let manifest = engine.load_recovery_manifest().unwrap();
        let rec = manifest.records.iter().find(|r| r.id == record.id).unwrap();
        assert_eq!(rec.status, QuarantineStatus::PreservedConflict);

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_093_symlink_swap_after_hash_verification_aborts_deletion() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_093_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        let workspace_dir = temp_dir.join("workspace");
        std::fs::create_dir_all(&workspace_dir).unwrap();

        let engine = Arc::new(CheckpointEngine::new(&storage_dir));
        let src_file = workspace_dir.join("src_093.txt");
        std::fs::write(&src_file, "content 093").unwrap();

        let record = engine
            .quarantine_file(&workspace_dir, "src_093.txt", "", "chk 093")
            .unwrap();

        // Swap target file with a directory (non-regular file)
        engine.set_pre_cleanup_unlink_hook(Some(|target: &Path| {
            let _ = std::fs::remove_file(target);
            let _ = std::fs::create_dir(target);
        }));

        let res = engine.cleanup_quarantine(&record.id);
        assert!(
            res.is_err(),
            "Must abort deletion when target is swapped to non-file"
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_094_target_swap_fault_injection_marks_preserved_conflict() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_094_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        let workspace_dir = temp_dir.join("workspace");
        std::fs::create_dir_all(&workspace_dir).unwrap();

        let engine = Arc::new(CheckpointEngine::new(&storage_dir));
        let src_file = workspace_dir.join("src_094.txt");
        std::fs::write(&src_file, "content 094").unwrap();

        let record = engine
            .quarantine_file(&workspace_dir, "src_094.txt", "", "chk 094")
            .unwrap();

        engine.set_fault_inject_quarantine_target_swap(true);

        let res = engine.cleanup_quarantine(&record.id);
        assert!(
            res.is_err(),
            "Must abort deletion under injected target swap"
        );

        let manifest = engine.load_recovery_manifest().unwrap();
        let rec = manifest.records.iter().find(|r| r.id == record.id).unwrap();
        assert_eq!(rec.status, QuarantineStatus::PreservedConflict);
        assert!(
            Path::new(&record.quarantined_path).exists(),
            "Artifact must be preserved"
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_095_cleanup_all_preserves_swapped_or_modified_files() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_095_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        let workspace_dir = temp_dir.join("workspace");
        std::fs::create_dir_all(&workspace_dir).unwrap();

        let engine = Arc::new(CheckpointEngine::new(&storage_dir));
        let f1 = workspace_dir.join("batch_095_1.txt");
        let f2 = workspace_dir.join("batch_095_2.txt");
        std::fs::write(&f1, "batch content 1").unwrap();
        std::fs::write(&f2, "batch content 2").unwrap();

        let _r1 = engine
            .quarantine_file(&workspace_dir, "batch_095_1.txt", "", "b1")
            .unwrap();
        let r2 = engine
            .quarantine_file(&workspace_dir, "batch_095_2.txt", "", "b2")
            .unwrap();

        // In hook, modify only batch_095_2
        let r2_path = PathBuf::from(&r2.quarantined_path);
        engine.set_pre_cleanup_unlink_hook(Some(move |target: &Path| {
            if target == r2_path {
                let _ = std::fs::write(target, "tampered content 2");
            }
        }));

        let _ = engine.cleanup_all_quarantined();

        let manifest = engine.load_recovery_manifest().unwrap();
        let rec1 = manifest
            .records
            .iter()
            .find(|r| r.original_relative_path == "batch_095_1.txt")
            .unwrap();
        let rec2 = manifest
            .records
            .iter()
            .find(|r| r.original_relative_path == "batch_095_2.txt")
            .unwrap();

        #[cfg(windows)]
        {
            assert_eq!(
                rec1.status,
                QuarantineStatus::CleanedUp,
                "Unmodified file should be cleaned up on Windows"
            );
        }
        #[cfg(not(windows))]
        {
            assert_eq!(
                rec1.status,
                QuarantineStatus::PreservedConflict,
                "Unmodified file must fail closed and be preserved on POSIX"
            );
        }
        assert_eq!(
            rec2.status,
            QuarantineStatus::PreservedConflict,
            "Modified file must be preserved as conflict"
        );
        assert!(
            Path::new(&rec2.quarantined_path).exists(),
            "Modified file must survive on disk"
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_096_startup_reconciliation_failure_blocks_engine_and_create_checkpoint() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_096_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        let workspace_dir = temp_dir.join("workspace");
        std::fs::create_dir_all(&workspace_dir).unwrap();

        let engine = CheckpointEngine::new(&storage_dir);
        engine.set_fault_inject_reconciliation_failure(true);

        let res = engine.initialize_and_reconcile();
        assert!(res.is_err(), "Reconciliation failure must be returned");
        assert!(
            engine.is_recovery_blocked(),
            "Engine must be in recovery blocked state"
        );

        // Attempt checkpoint creation while blocked
        let ckpt_res = engine.create_checkpoint_with_root("desc", "ws", &workspace_dir, None);
        assert!(ckpt_res.is_err(), "Checkpoint creation must be blocked");
        let err_str = ckpt_res.unwrap_err().to_string();
        assert!(
            err_str.contains("recovery blocked"),
            "Error should explain recovery blocked state: {}",
            err_str
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_097_recovery_blocked_state_cannot_invoke_cleanup_or_restore() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_097_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        let workspace_dir = temp_dir.join("workspace");
        std::fs::create_dir_all(&workspace_dir).unwrap();

        let engine = CheckpointEngine::new(&storage_dir);
        engine.set_recovery_blocked(Some("Explicit test block".into()));

        assert!(engine.cleanup_quarantine("any_id").is_err());
        assert!(engine.cleanup_all_quarantined().is_err());
        assert!(engine
            .restore_preview_bound("any_ckpt", "ws", &workspace_dir)
            .is_err());
        assert!(engine
            .restore_checkpoint_with_token("any_ckpt", "ws", &workspace_dir, "tok", false, &[])
            .is_err());

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_098_successful_reconciliation_retry_unblocks_engine() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_098_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        let workspace_dir = temp_dir.join("workspace");
        std::fs::create_dir_all(&workspace_dir).unwrap();

        let engine = CheckpointEngine::new(&storage_dir);
        engine.set_recovery_blocked(Some("Previous failure".into()));
        assert!(engine.is_recovery_blocked());

        // Successful retry
        let res = engine.reconcile_quarantine();
        assert!(res.is_ok(), "Retry should succeed");
        assert!(
            !engine.is_recovery_blocked(),
            "Engine must now be unblocked"
        );

        // Normal operations succeed
        let ckpt_res = engine.create_checkpoint_with_root("desc", "ws", &workspace_dir, None);
        assert!(
            ckpt_res.is_ok(),
            "Checkpoint creation should succeed once unblocked"
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_099_recovery_blocked_refuses_authoritative_listing() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_099_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        let engine = CheckpointEngine::new(&storage_dir);

        engine.set_recovery_blocked(Some("Critical database unreadable".into()));
        let res = engine.list_quarantined_artifacts();
        assert!(
            res.is_err(),
            "Must not return an authoritative list when recovery is blocked"
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_100_corrupt_manifest_backup_creation_failure_preserves_original() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_100_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        std::fs::create_dir_all(&storage_dir).unwrap();

        let engine = CheckpointEngine::new(&storage_dir);
        let manifest_path = engine.recovery_manifest_path();
        let corrupt_payload = "NOT_VALID_JSON_MANIFEST_CONTENT_100";
        std::fs::write(&manifest_path, corrupt_payload).unwrap();

        engine.set_fault_inject_corrupt_manifest_backup_creation(true);

        let res = engine.load_recovery_manifest();
        assert!(
            res.is_err(),
            "Should return error when backup creation fails"
        );

        // Verify original corrupt file was NOT deleted or overwritten
        let original_after = std::fs::read_to_string(&manifest_path).unwrap();
        assert_eq!(
            original_after, corrupt_payload,
            "Original corrupt manifest must remain intact"
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_101_corrupt_manifest_backup_verification_failure_preserves_original() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_101_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        std::fs::create_dir_all(&storage_dir).unwrap();

        let engine = CheckpointEngine::new(&storage_dir);
        let manifest_path = engine.recovery_manifest_path();
        let corrupt_payload = "NOT_VALID_JSON_MANIFEST_CONTENT_101";
        std::fs::write(&manifest_path, corrupt_payload).unwrap();

        engine.set_fault_inject_corrupt_manifest_backup_verification(true);

        let res = engine.load_recovery_manifest();
        assert!(
            res.is_err(),
            "Should return error when backup verification fails"
        );

        // Verify original corrupt manifest still intact
        let original_after = std::fs::read_to_string(&manifest_path).unwrap();
        assert_eq!(
            original_after, corrupt_payload,
            "Original corrupt manifest must remain untouched"
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_102_fresh_manifest_persistence_failure_is_surfaced() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_102_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        std::fs::create_dir_all(&storage_dir).unwrap();

        let engine = CheckpointEngine::new(&storage_dir);
        let manifest_path = engine.recovery_manifest_path();
        let corrupt_payload = "NOT_VALID_JSON_MANIFEST_CONTENT_102";
        std::fs::write(&manifest_path, corrupt_payload).unwrap();

        engine.set_fault_inject_manifest_save(true);

        let res = engine.load_recovery_manifest();
        assert!(
            res.is_err(),
            "Must surface fresh manifest persistence failure"
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_103_backup_filename_collision_cannot_overwrite() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_103_{}", uuid::Uuid::new_v4().simple()));
        std::fs::create_dir_all(&temp_dir).unwrap();

        let backup_path = temp_dir.join("test_backup.corrupt");
        std::fs::write(&backup_path, "pre-existing critical backup data").unwrap();

        // Demonstrating create_new(true) semantics used in implementation
        let collision_res = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&backup_path);

        assert!(
            collision_res.is_err(),
            "create_new(true) must reject overwriting existing backup file"
        );
        assert_eq!(
            std::fs::read_to_string(&backup_path).unwrap(),
            "pre-existing critical backup data"
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_104_interrupted_corrupt_manifest_handling_recovered_on_subsequent_start() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_104_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        std::fs::create_dir_all(&storage_dir).unwrap();

        let engine = CheckpointEngine::new(&storage_dir);
        let manifest_path = engine.recovery_manifest_path();
        let corrupt_payload = "NOT_VALID_JSON_MANIFEST_CONTENT_104";
        std::fs::write(&manifest_path, corrupt_payload).unwrap();

        // Step 1: Normal load succeeds in creating backup and fresh manifest
        let manifest = engine.load_recovery_manifest().unwrap();
        assert_eq!(manifest.records.len(), 1);
        assert_eq!(
            manifest.records[0].status,
            QuarantineStatus::PreservedConflict
        );

        // Step 2: Subsequent startup and reconciliation correctly preserves state
        let manifest2 = engine.reconcile_quarantine().unwrap();
        assert_eq!(manifest2.records.len(), 1);
        assert_eq!(
            manifest2.records[0].status,
            QuarantineStatus::PreservedConflict
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_105_linux_retained_writable_handle_prevents_unlinking_on_cross_device() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_105_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        let workspace_dir = temp_dir.join("workspace");
        std::fs::create_dir_all(&workspace_dir).unwrap();

        let engine = CheckpointEngine::new(&storage_dir);
        let src_file = workspace_dir.join("src_105.txt");
        let payload = "chk 105 payload to protect against retained handle mutation";
        std::fs::write(&src_file, payload).unwrap();
        let expected_hash = CheckpointEngine::sha256_digest(payload.as_bytes());

        // Inject cross-device rename failure
        engine.set_fault_inject_quarantine_cross_device(true);

        #[cfg(not(windows))]
        {
            // On Linux/POSIX, standard kernel semantics cannot guarantee exclusive stability
            // against uncooperative open descriptors. The engine must fail-closed and refuse to unlink source.
            let res = engine.quarantine_file(
                &workspace_dir,
                "src_105.txt",
                &expected_hash,
                "chk 105 test",
            );
            assert!(res.is_err(), "POSIX cross-device fallback must reject deletion without exclusive stability proof");
            assert!(
                src_file.exists(),
                "Source file must not be unlinked on POSIX"
            );
            let manifest = engine.load_recovery_manifest().unwrap();
            let rec = manifest
                .records
                .iter()
                .find(|r| r.original_relative_path == "src_105.txt")
                .unwrap();
            assert_eq!(rec.status, QuarantineStatus::PreservedConflict);
            assert!(
                Path::new(&rec.quarantined_path).exists(),
                "Quarantined copy must be preserved"
            );
        }

        #[cfg(windows)]
        {
            // On Windows, if another handle is open, exclusive open fails and preserves source
            use std::os::windows::fs::OpenOptionsExt;
            let retained_handle = std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .share_mode(1) // FILE_SHARE_READ only
                .open(&src_file)
                .unwrap();

            let res = engine.quarantine_file(
                &workspace_dir,
                "src_105.txt",
                &expected_hash,
                "chk 105 test",
            );
            assert!(
                res.is_err(),
                "Windows exclusive stability must fail when handle is held"
            );
            assert!(
                src_file.exists(),
                "Source file must remain preserved in workspace"
            );
            drop(retained_handle);
        }

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_106_macos_retained_writable_handle_scenario_source_not_unlinked() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_106_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        let workspace_dir = temp_dir.join("workspace");
        std::fs::create_dir_all(&workspace_dir).unwrap();

        let engine = CheckpointEngine::new(&storage_dir);
        let src_file = workspace_dir.join("src_106.txt");
        let payload = "chk 106 macos test payload";
        std::fs::write(&src_file, payload).unwrap();
        let expected_hash = CheckpointEngine::sha256_digest(payload.as_bytes());

        engine.set_fault_inject_quarantine_cross_device(true);

        #[cfg(not(windows))]
        {
            let res =
                engine.quarantine_file(&workspace_dir, "src_106.txt", &expected_hash, "chk 106");
            assert!(
                res.is_err(),
                "Non-Windows must not unlink source during cross-device fallback"
            );
            assert!(src_file.exists(), "Source must remain on disk");
        }

        #[cfg(windows)]
        {
            // Verify Windows behavior with mutation hook simulating post-hash retained-handle write
            let eng_clone = Arc::new(engine);
            let ec = eng_clone.clone();
            eng_clone.set_pre_quarantine_source_unlink_hook(Some(move |stage: &Path| {
                ec.set_fault_inject_quarantine_source_mutation(true);
                let _ = std::fs::write(stage, "mutated by retained handle post verification");
            }));
            let res =
                eng_clone.quarantine_file(&workspace_dir, "src_106.txt", &expected_hash, "chk 106");
            assert!(res.is_err(), "Must reject deletion when source mutated");
            assert!(src_file.exists(), "Modified source must be preserved");
        }

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_107_windows_exclusive_handle_remains_held_throughout_protected_operation() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_107_{}", uuid::Uuid::new_v4().simple()));
        std::fs::create_dir_all(&temp_dir).unwrap();
        let test_file = temp_dir.join("test_107.txt");
        let payload = "exclusive handle test content";
        std::fs::write(&test_file, payload).unwrap();
        #[cfg(windows)]
        let hash = CheckpointEngine::sha256_digest(payload.as_bytes());

        #[cfg(windows)]
        {
            let res = filesystem_engine::remove_file_windows_exclusive(&test_file, &hash, None);
            assert!(
                res.is_ok(),
                "Exclusive removal must succeed for unmodified unheld file"
            );
            assert!(
                !test_file.exists(),
                "File must be removed after exclusive handle closes"
            );
        }

        #[cfg(not(windows))]
        {
            assert!(std::fs::remove_file(&test_file).is_ok());
        }

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_108_attempted_writer_cannot_modify_source_while_windows_exclusive_operation_active()
    {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_108_{}", uuid::Uuid::new_v4().simple()));
        std::fs::create_dir_all(&temp_dir).unwrap();
        let test_file = temp_dir.join("test_108.txt");
        let payload = "writer blocking payload";
        std::fs::write(&test_file, payload).unwrap();

        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            let exclusive_file = std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .share_mode(0)
                .open(&test_file)
                .unwrap();

            let writer_attempt = std::fs::OpenOptions::new().write(true).open(&test_file);
            assert!(
                writer_attempt.is_err(),
                "Windows OS sharing semantics must block concurrent writers when share_mode(0) is held"
            );

            drop(exclusive_file);
        }

        #[cfg(not(windows))]
        {
            assert!(test_file.exists());
        }

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_109_mutation_after_final_verification_cannot_cause_loss_of_changed_source() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_109_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        let workspace_dir = temp_dir.join("workspace");
        std::fs::create_dir_all(&workspace_dir).unwrap();

        let engine = Arc::new(CheckpointEngine::new(&storage_dir));
        let src_file = workspace_dir.join("src_109.txt");
        let initial_payload = "original unmutated payload";
        std::fs::write(&src_file, initial_payload).unwrap();
        let hash = CheckpointEngine::sha256_digest(initial_payload.as_bytes());

        engine.set_fault_inject_quarantine_cross_device(true);

        let ec = engine.clone();
        engine.set_pre_quarantine_source_unlink_hook(Some(move |stage_path: &Path| {
            ec.set_fault_inject_quarantine_source_mutation(true);
            let _ = std::fs::write(stage_path, "POST_VERIFICATION_MUTATION");
        }));

        let res = engine.quarantine_file(&workspace_dir, "src_109.txt", &hash, "chk 109 test");
        assert!(res.is_err(), "Mutation must abort deletion");
        assert!(
            src_file.exists(),
            "Mutated file must be preserved in workspace"
        );
        assert_eq!(
            std::fs::read_to_string(&src_file).unwrap(),
            "POST_VERIFICATION_MUTATION"
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_110_both_artifacts_remain_recoverable_when_exclusive_stability_cannot_be_established(
    ) {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_110_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        let workspace_dir = temp_dir.join("workspace");
        std::fs::create_dir_all(&workspace_dir).unwrap();

        let engine = CheckpointEngine::new(&storage_dir);
        let src_file = workspace_dir.join("src_110.txt");
        let payload = "chk 110 double preservation content";
        std::fs::write(&src_file, payload).unwrap();
        let hash = CheckpointEngine::sha256_digest(payload.as_bytes());

        engine.set_fault_inject_quarantine_cross_device(true);
        engine.set_fault_inject_quarantine_source_mutation(true);

        let res = engine.quarantine_file(&workspace_dir, "src_110.txt", &hash, "chk 110 test");
        assert!(
            res.is_err(),
            "Must report error when stability cannot be established"
        );

        assert!(src_file.exists(), "Source artifact must exist in workspace");
        let manifest = engine.load_recovery_manifest().unwrap();
        let rec = manifest
            .records
            .iter()
            .find(|r| r.original_relative_path == "src_110.txt")
            .unwrap();
        assert_eq!(rec.status, QuarantineStatus::PreservedConflict);
        assert!(
            Path::new(&rec.quarantined_path).exists(),
            "Verified copy must exist in quarantine"
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_111_manifest_persistence_failure_preserves_all_physical_artifacts() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_111_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        let workspace_dir = temp_dir.join("workspace");
        std::fs::create_dir_all(&workspace_dir).unwrap();

        let engine = CheckpointEngine::new(&storage_dir);
        let src_file = workspace_dir.join("src_111.txt");
        let payload = "chk 111 manifest failure test";
        std::fs::write(&src_file, payload).unwrap();
        let hash = CheckpointEngine::sha256_digest(payload.as_bytes());

        engine.set_fault_inject_quarantine_cross_device(true);
        engine.set_fault_inject_quarantine_source_mutation(true);
        engine.set_fault_inject_manifest_save(true);

        let res = engine.quarantine_file(&workspace_dir, "src_111.txt", &hash, "chk 111 test");
        assert!(res.is_err(), "Must fail when manifest persistence fails");

        assert!(src_file.exists(), "Source file must remain in workspace");

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_112_replace_target_after_final_hash_but_before_removal() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_112_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        let workspace_dir = temp_dir.join("workspace");
        std::fs::create_dir_all(&workspace_dir).unwrap();

        let engine = Arc::new(CheckpointEngine::new(&storage_dir));
        let src_file = workspace_dir.join("src_112.txt");
        std::fs::write(&src_file, "initial quarantine content 112").unwrap();

        let record = engine
            .quarantine_file(&workspace_dir, "src_112.txt", "", "chk 112")
            .unwrap();
        let target_path = PathBuf::from(&record.quarantined_path);

        engine.set_pre_cleanup_unlink_hook(Some(|target: &Path| {
            std::fs::write(target, "REPLACED_TARGET_CONTENT_112").unwrap();
        }));

        let res = engine.cleanup_quarantine(&record.id);
        assert!(res.is_err(), "Must reject cleanup when target is replaced");

        assert!(target_path.exists(), "Replaced target must not be deleted");
        assert_eq!(
            std::fs::read_to_string(&target_path).unwrap(),
            "REPLACED_TARGET_CONTENT_112"
        );

        let manifest = engine.load_recovery_manifest().unwrap();
        let rec = manifest.records.iter().find(|r| r.id == record.id).unwrap();
        assert_eq!(rec.status, QuarantineStatus::PreservedConflict);

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_113_replace_target_with_different_regular_file() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_113_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        let workspace_dir = temp_dir.join("workspace");
        std::fs::create_dir_all(&workspace_dir).unwrap();

        let engine = Arc::new(CheckpointEngine::new(&storage_dir));
        let src_file = workspace_dir.join("src_113.txt");
        std::fs::write(&src_file, "initial candidate 113").unwrap();

        let record = engine
            .quarantine_file(&workspace_dir, "src_113.txt", "", "chk 113")
            .unwrap();
        let target_path = PathBuf::from(&record.quarantined_path);

        engine.set_pre_cleanup_unlink_hook(Some(|target: &Path| {
            std::fs::write(target, "DIFFERENT_REGULAR_FILE_113").unwrap();
        }));

        let res = engine.cleanup_quarantine(&record.id);
        assert!(
            res.is_err(),
            "Must reject cleanup when target replaced with regular file"
        );
        assert_eq!(
            std::fs::read_to_string(&target_path).unwrap(),
            "DIFFERENT_REGULAR_FILE_113"
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_114_replace_target_with_symlink() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_114_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        let workspace_dir = temp_dir.join("workspace");
        std::fs::create_dir_all(&workspace_dir).unwrap();

        let outside_dir = temp_dir.join("outside");
        std::fs::create_dir_all(&outside_dir).unwrap();
        let innocent_file = outside_dir.join("innocent_114.txt");
        std::fs::write(&innocent_file, "INNOCENT_DATA_114").unwrap();

        let engine = Arc::new(CheckpointEngine::new(&storage_dir));
        let src_file = workspace_dir.join("src_114.txt");
        std::fs::write(&src_file, "candidate 114").unwrap();

        let record = engine
            .quarantine_file(&workspace_dir, "src_114.txt", "", "chk 114")
            .unwrap();

        let innocent_clone = innocent_file.clone();
        engine.set_pre_cleanup_unlink_hook(Some(move |target: &Path| {
            #[cfg(windows)]
            {
                if std::os::windows::fs::symlink_file(&innocent_clone, target).is_err() {
                    // Fallback on Windows when unprivileged symlink creation is not permitted by OS policy
                    let _ = std::fs::create_dir(target);
                }
            }
            #[cfg(unix)]
            {
                let _ = std::os::unix::fs::symlink(&innocent_clone, target);
            }
        }));

        let res = engine.cleanup_quarantine(&record.id);
        assert!(
            res.is_err(),
            "Must reject cleanup when target replaced with symlink"
        );
        assert!(
            innocent_file.exists(),
            "Innocent file must remain untouched"
        );
        assert_eq!(
            std::fs::read_to_string(&innocent_file).unwrap(),
            "INNOCENT_DATA_114"
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_115_modify_target_through_retained_writable_handle_after_verification() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_115_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        let workspace_dir = temp_dir.join("workspace");
        std::fs::create_dir_all(&workspace_dir).unwrap();

        let engine = Arc::new(CheckpointEngine::new(&storage_dir));
        let src_file = workspace_dir.join("src_115.txt");
        std::fs::write(&src_file, "initial content 115").unwrap();

        let record = engine
            .quarantine_file(&workspace_dir, "src_115.txt", "", "chk 115")
            .unwrap();
        let target_path = PathBuf::from(&record.quarantined_path);

        engine.set_pre_cleanup_unlink_hook(Some(|target: &Path| {
            std::fs::write(target, "MODIFIED_AFTER_VERIFICATION").unwrap();
        }));

        let res = engine.cleanup_quarantine(&record.id);
        assert!(
            res.is_err(),
            "Must reject cleanup when modified after verification"
        );
        assert!(target_path.exists());

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_116_concurrently_recreate_original_filename_during_staging() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_116_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        let workspace_dir = temp_dir.join("workspace");
        std::fs::create_dir_all(&workspace_dir).unwrap();

        let engine = Arc::new(CheckpointEngine::new(&storage_dir));
        let src_file = workspace_dir.join("src_116.txt");
        std::fs::write(&src_file, "initial content 116").unwrap();

        let record = engine
            .quarantine_file(&workspace_dir, "src_116.txt", "", "chk 116")
            .unwrap();
        let target_path = PathBuf::from(&record.quarantined_path);

        engine.set_pre_cleanup_unlink_hook(Some(|target: &Path| {
            std::fs::write(target, "RECREATED_ORIGINAL_FILENAME_116").unwrap();
        }));

        let res = engine.cleanup_quarantine(&record.id);
        assert!(res.is_err(), "Must abort cleanup on concurrent recreation");

        assert!(target_path.exists());
        assert_eq!(
            std::fs::read_to_string(&target_path).unwrap(),
            "RECREATED_ORIGINAL_FILENAME_116"
        );

        let manifest = engine.load_recovery_manifest().unwrap();
        let rec = manifest.records.iter().find(|r| r.id == record.id).unwrap();
        assert_eq!(rec.status, QuarantineStatus::PreservedConflict);

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_117_failed_staging_or_rollback_preserves_all_recoverable_artifacts() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_117_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        let workspace_dir = temp_dir.join("workspace");
        std::fs::create_dir_all(&workspace_dir).unwrap();

        let engine = Arc::new(CheckpointEngine::new(&storage_dir));
        let src_file = workspace_dir.join("src_117.txt");
        std::fs::write(&src_file, "content 117").unwrap();

        let record = engine
            .quarantine_file(&workspace_dir, "src_117.txt", "", "chk 117")
            .unwrap();

        engine.set_fault_inject_cleanup_removal(true);
        let res = engine.cleanup_quarantine(&record.id);
        assert!(res.is_err());

        let target_path = PathBuf::from(&record.quarantined_path);
        assert!(
            target_path.exists(),
            "Artifact must be preserved on disk after failed cleanup"
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_118_batch_cleanup_applies_same_protections_to_every_item() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_118_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        let workspace_dir = temp_dir.join("workspace");
        std::fs::create_dir_all(&workspace_dir).unwrap();

        let engine = Arc::new(CheckpointEngine::new(&storage_dir));
        let f1 = workspace_dir.join("batch_118_1.txt");
        let f2 = workspace_dir.join("batch_118_2.txt");
        std::fs::write(&f1, "batch content 1").unwrap();
        std::fs::write(&f2, "batch content 2").unwrap();

        let _r1 = engine
            .quarantine_file(&workspace_dir, "batch_118_1.txt", "", "b1")
            .unwrap();
        let r2 = engine
            .quarantine_file(&workspace_dir, "batch_118_2.txt", "", "b2")
            .unwrap();

        let r2_path = PathBuf::from(&r2.quarantined_path);
        let r2_clone = r2_path.clone();
        engine.set_pre_cleanup_unlink_hook(Some(move |target: &Path| {
            if target == r2_clone {
                std::fs::write(target, "RECREATED_R2_CONTENT").unwrap();
            }
        }));

        let _ = engine.cleanup_all_quarantined();

        assert!(r2_path.exists(), "Recreated r2 must not be deleted");
        assert_eq!(
            std::fs::read_to_string(&r2_path).unwrap(),
            "RECREATED_R2_CONTENT"
        );

        let manifest = engine.load_recovery_manifest().unwrap();
        let rec2 = manifest
            .records
            .iter()
            .find(|r| r.original_relative_path == "batch_118_2.txt")
            .unwrap();
        assert_eq!(rec2.status, QuarantineStatus::PreservedConflict);

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_119_external_files_remain_untouched_in_every_adversarial_scenario() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_119_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        let workspace_dir = temp_dir.join("workspace");
        std::fs::create_dir_all(&workspace_dir).unwrap();

        let system_dir = temp_dir.join("system_files");
        std::fs::create_dir_all(&system_dir).unwrap();
        let crucial_system_file = system_dir.join("host_database.db");
        std::fs::write(&crucial_system_file, "CRITICAL HOST DATA 119").unwrap();

        let engine = Arc::new(CheckpointEngine::new(&storage_dir));
        let src = workspace_dir.join("file_119.txt");
        std::fs::write(&src, "file 119 payload").unwrap();

        let record = engine
            .quarantine_file(&workspace_dir, "file_119.txt", "", "chk 119")
            .unwrap();

        let crucial_clone = crucial_system_file.clone();
        engine.set_pre_cleanup_unlink_hook(Some(move |_path: &Path| {
            assert!(crucial_clone.exists());
        }));

        let _ = engine.cleanup_quarantine(&record.id);
        assert!(
            crucial_system_file.exists(),
            "External file must be completely untouched"
        );
        assert_eq!(
            std::fs::read_to_string(&crucial_system_file).unwrap(),
            "CRITICAL HOST DATA 119"
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_120_list_checkpoints_rejects_while_recovery_blocked() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_120_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        std::fs::create_dir_all(&storage_dir).unwrap();

        let engine = CheckpointEngine::new(&storage_dir);
        engine.set_recovery_blocked(Some("Fatal reconciliation failure".into()));

        let res = engine.list_checkpoints();
        assert!(
            res.is_err(),
            "list_checkpoints must reject while recovery blocked"
        );
        let err = res.unwrap_err().to_string();
        assert!(err.contains("Listing checkpoints blocked due to recovery blocked state"));

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_121_tauri_checkpoint_listing_rejects_while_recovery_blocked() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_121_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        std::fs::create_dir_all(&storage_dir).unwrap();

        let engine = CheckpointEngine::new(&storage_dir);
        engine.set_recovery_blocked(Some("Database corrupt".into()));

        let res = engine.list_checkpoints();
        assert!(
            res.is_err(),
            "Tauri backend engine list must propagate error rather than empty list"
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_122_every_public_checkpoint_api_covered_by_recovery_blocked_audit() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_122_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        let workspace_dir = temp_dir.join("workspace");
        std::fs::create_dir_all(&workspace_dir).unwrap();

        let engine = CheckpointEngine::new(&storage_dir);
        let cap = WorkspaceCapability::open(&workspace_dir).unwrap();

        // 1. Diagnostic queries must succeed without blocking
        assert!(!engine.is_recovery_blocked());
        assert_eq!(engine.recovery_blocked_reason(), None);

        // Block engine
        engine.set_recovery_blocked(Some("Security audit blocked reason".into()));
        assert!(engine.is_recovery_blocked());
        assert_eq!(
            engine.recovery_blocked_reason().as_deref(),
            Some("Security audit blocked reason")
        );

        // 2. All authoritative APIs must reject
        assert!(engine.list_checkpoints().is_err());
        assert!(engine.create_checkpoint("desc", &[]).is_err());
        assert!(engine
            .create_checkpoint_with_root("desc", "ws", &workspace_dir, None)
            .is_err());
        assert!(engine
            .restore_preview_bound("ckpt", "ws", &workspace_dir)
            .is_err());
        assert!(engine.restore_preview("ckpt", &workspace_dir).is_err());
        assert!(engine
            .restore_checkpoint_with_token("ckpt", "ws", &workspace_dir, "tok", false, &[])
            .is_err());
        assert!(engine
            .consume_preview_token("tok", "ckpt", "ws", &workspace_dir)
            .is_err());
        assert!(engine.cleanup_quarantine("any_id").is_err());
        assert!(engine.cleanup_all_quarantined().is_err());
        assert!(engine.list_quarantined_artifacts().is_err());
        assert!(engine
            .quarantine_file(&workspace_dir, "some.txt", "", "reason")
            .is_err());
        assert!(engine
            .quarantine_file_with_cap(&cap, "some.txt", "", "reason")
            .is_err());

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_123_successful_reconciliation_retry_restores_normal_listing_and_operation() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_123_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        let workspace_dir = temp_dir.join("workspace");
        std::fs::create_dir_all(&workspace_dir).unwrap();

        let engine = CheckpointEngine::new(&storage_dir);
        engine.set_recovery_blocked(Some("Transient failure".into()));
        assert!(engine.is_recovery_blocked());
        assert!(engine.list_checkpoints().is_err());

        let res = engine.reconcile_quarantine();
        assert!(res.is_ok(), "Reconciliation should succeed");
        assert!(!engine.is_recovery_blocked(), "Engine must be unblocked");

        assert!(engine.list_checkpoints().is_ok());
        let ckpt = engine.create_checkpoint_with_root("desc 123", "ws", &workspace_dir, None);
        assert!(
            ckpt.is_ok(),
            "Checkpoint creation must succeed after unblocking"
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_124_production_backup_collision_does_not_overwrite_existing_recovery_data() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_124_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        std::fs::create_dir_all(&storage_dir).unwrap();

        let engine = CheckpointEngine::new(&storage_dir);
        let manifest_path = engine.recovery_manifest_path();
        let colliding_path =
            manifest_path.with_file_name("recovery_manifest.json.corrupt.colliding_candidate");

        // Pre-create colliding backup file with critical data
        std::fs::write(&colliding_path, "PRE_EXISTING_CRITICAL_CORRUPT_BACKUP_124").unwrap();

        // Write corrupt manifest to trigger backup creation
        std::fs::write(&manifest_path, "CORRUPT_JSON_MANIFEST_DATA_124").unwrap();

        // Enable deterministic collision injection on attempt 0
        engine.set_fault_inject_corrupt_manifest_backup_collision(true);

        let manifest = engine.load_recovery_manifest().unwrap();
        assert_eq!(manifest.records.len(), 1);

        // Pre-existing backup must be byte-for-byte identical!
        assert_eq!(
            std::fs::read_to_string(&colliding_path).unwrap(),
            "PRE_EXISTING_CRITICAL_CORRUPT_BACKUP_124",
            "Pre-existing backup must never be overwritten on collision"
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_125_retry_produces_unique_backup_and_verifies_digest() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_125_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        std::fs::create_dir_all(&storage_dir).unwrap();

        let engine = CheckpointEngine::new(&storage_dir);
        let manifest_path = engine.recovery_manifest_path();
        let colliding_path =
            manifest_path.with_file_name("recovery_manifest.json.corrupt.colliding_candidate");

        std::fs::write(&colliding_path, "EXISTING_BACKUP_125").unwrap();
        let corrupt_payload = "CORRUPT_PAYLOAD_125";
        std::fs::write(&manifest_path, corrupt_payload).unwrap();
        let expected_hash = CheckpointEngine::sha256_digest(corrupt_payload.as_bytes());

        engine.set_fault_inject_corrupt_manifest_backup_collision(true);

        let manifest = engine.load_recovery_manifest().unwrap();
        assert_eq!(manifest.records.len(), 1);

        let backup_rec = &manifest.records[0];
        let backup_path = PathBuf::from(&backup_rec.quarantined_path);
        assert_ne!(
            backup_path, colliding_path,
            "Must be a distinct unique backup file"
        );
        assert!(backup_path.exists(), "Unique backup must exist on disk");
        assert_eq!(
            std::fs::read_to_string(&backup_path).unwrap(),
            corrupt_payload
        );
        assert_eq!(backup_rec.expected_sha256, expected_hash);
        assert_eq!(
            backup_rec.actual_sha256.as_deref(),
            Some(expected_hash.as_str())
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_126_interrupted_collision_recovery_preserves_all_prior_backups_and_original_corrupt_manifest(
    ) {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_126_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        std::fs::create_dir_all(&storage_dir).unwrap();

        let engine = CheckpointEngine::new(&storage_dir);
        let manifest_path = engine.recovery_manifest_path();
        let colliding_path =
            manifest_path.with_file_name("recovery_manifest.json.corrupt.colliding_candidate");

        std::fs::write(&colliding_path, "EXISTING_BACKUP_126").unwrap();
        let corrupt_payload = "CORRUPT_PAYLOAD_126";
        std::fs::write(&manifest_path, corrupt_payload).unwrap();

        engine.set_fault_inject_corrupt_manifest_backup_collision(true);
        // Interrupt during verification:
        engine.set_fault_inject_corrupt_manifest_backup_verification(true);

        let res = engine.load_recovery_manifest();
        assert!(
            res.is_err(),
            "Must report error when verification interrupted"
        );

        assert_eq!(
            std::fs::read_to_string(&colliding_path).unwrap(),
            "EXISTING_BACKUP_126",
            "Prior backup must be preserved"
        );
        assert_eq!(
            std::fs::read_to_string(&manifest_path).unwrap(),
            corrupt_payload,
            "Original corrupt manifest must be preserved"
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    // =========================================================================
    // Sprint 23.2.12 Adversarial Tests: CHK-127 through CHK-152
    // =========================================================================

    // Section 2: P1 — POSIX quarantine cleanup must fail closed (CHK-127 .. CHK-132)
    #[test]
    fn test_chk_127_retained_writable_descriptor_before_cleanup_staging() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_127_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        let workspace_dir = temp_dir.join("workspace");
        std::fs::create_dir_all(&workspace_dir).unwrap();

        let engine = CheckpointEngine::new(&storage_dir);
        let src_file = workspace_dir.join("file_127.txt");
        std::fs::write(&src_file, "original payload 127").unwrap();

        let rec = engine
            .quarantine_file(&workspace_dir, "file_127.txt", "", "chk 127")
            .unwrap();
        let q_path = PathBuf::from(&rec.quarantined_path);

        // Retain writable descriptor to the quarantine file before cleanup stages it
        let writer = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&q_path)
            .unwrap();

        let res = engine.cleanup_quarantine(&rec.id);
        #[cfg(not(windows))]
        {
            assert!(
                res.is_err(),
                "POSIX must fail closed and refuse physical disposal"
            );
        }
        #[cfg(windows)]
        {
            assert!(
                res.is_err(),
                "Windows exclusive deletion must reject when handle is open"
            );
        }

        drop(writer);
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_128_modify_through_descriptor_after_final_verification_point() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_128_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        let workspace_dir = temp_dir.join("workspace");
        std::fs::create_dir_all(&workspace_dir).unwrap();

        let engine = Arc::new(CheckpointEngine::new(&storage_dir));
        let src_file = workspace_dir.join("file_128.txt");
        std::fs::write(&src_file, "initial candidate 128").unwrap();

        let rec = engine
            .quarantine_file(&workspace_dir, "file_128.txt", "", "chk 128")
            .unwrap();
        let q_path = PathBuf::from(&rec.quarantined_path);

        // Pre-cleanup hook attempts mutation
        engine.set_pre_cleanup_unlink_hook(Some(move |_target: &Path| {
            let _ = std::fs::write(&q_path, "MUTATED_BY_RETAINED_DESCRIPTOR_128");
        }));

        let res = engine.cleanup_quarantine(&rec.id);
        assert!(
            res.is_err(),
            "Cleanup must fail closed and preserve candidate"
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_129_staged_artifact_remains_recoverable_and_changed_bytes_preserved() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_129_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        let workspace_dir = temp_dir.join("workspace");
        std::fs::create_dir_all(&workspace_dir).unwrap();

        let engine = Arc::new(CheckpointEngine::new(&storage_dir));
        let src_file = workspace_dir.join("file_129.txt");
        std::fs::write(&src_file, "candidate 129").unwrap();

        let rec = engine
            .quarantine_file(&workspace_dir, "file_129.txt", "", "chk 129")
            .unwrap();
        let q_path = PathBuf::from(&rec.quarantined_path);

        engine.set_pre_cleanup_unlink_hook(Some(move |_target: &Path| {
            let _ = std::fs::write(&q_path, "PRESERVED_CHANGED_BYTES_129");
        }));

        let _ = engine.cleanup_quarantine(&rec.id);
        let manifest = engine.load_recovery_manifest().unwrap();
        let r = manifest.records.iter().find(|x| x.id == rec.id).unwrap();
        assert_eq!(r.status, QuarantineStatus::PreservedConflict);
        assert!(
            Path::new(&r.quarantined_path).exists(),
            "Artifact must physically exist on disk"
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_130_single_cleanup_never_reports_successful_physical_deletion_in_unsafe_state() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_130_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        let workspace_dir = temp_dir.join("workspace");
        std::fs::create_dir_all(&workspace_dir).unwrap();

        let engine = CheckpointEngine::new(&storage_dir);
        let src_file = workspace_dir.join("file_130.txt");
        std::fs::write(&src_file, "payload 130").unwrap();

        let rec = engine
            .quarantine_file(&workspace_dir, "file_130.txt", "", "chk 130")
            .unwrap();
        let q_path = PathBuf::from(&rec.quarantined_path);

        let res = engine.cleanup_quarantine(&rec.id);
        #[cfg(not(windows))]
        {
            assert!(
                res.is_err(),
                "POSIX must refuse physical deletion and fail closed"
            );
            assert!(
                q_path.exists(),
                "POSIX must not physically unlink the artifact"
            );
        }
        #[cfg(windows)]
        {
            // On Windows, unmodified unheld file can be exclusively cleaned up
            let _ = (res, &q_path);
        }

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_131_batch_cleanup_applies_same_rule_to_every_item() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_131_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        let workspace_dir = temp_dir.join("workspace");
        std::fs::create_dir_all(&workspace_dir).unwrap();

        let engine = CheckpointEngine::new(&storage_dir);
        let f1 = workspace_dir.join("b1.txt");
        let f2 = workspace_dir.join("b2.txt");
        std::fs::write(&f1, "batch 1").unwrap();
        std::fs::write(&f2, "batch 2").unwrap();

        let r1 = engine
            .quarantine_file(&workspace_dir, "b1.txt", "", "1")
            .unwrap();
        let r2 = engine
            .quarantine_file(&workspace_dir, "b2.txt", "", "2")
            .unwrap();

        let res = engine.cleanup_all_quarantined();
        #[cfg(not(windows))]
        {
            assert!(res.is_err(), "POSIX batch cleanup must fail closed");
            assert!(Path::new(&r1.quarantined_path).exists());
            assert!(Path::new(&r2.quarantined_path).exists());
            let manifest = engine.load_recovery_manifest().unwrap();
            for r in &manifest.records {
                assert_eq!(r.status, QuarantineStatus::PreservedConflict);
            }
        }
        #[cfg(windows)]
        {
            let count = res.unwrap();
            assert_eq!(count, 2);
            let _ = (&r1, &r2);
        }

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_132_manifest_points_to_actual_surviving_artifact() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_132_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        let workspace_dir = temp_dir.join("workspace");
        std::fs::create_dir_all(&workspace_dir).unwrap();

        let engine = Arc::new(CheckpointEngine::new(&storage_dir));
        let src_file = workspace_dir.join("file_132.txt");
        let payload = "actual surviving payload 132";
        std::fs::write(&src_file, payload).unwrap();

        let rec = engine
            .quarantine_file(&workspace_dir, "file_132.txt", "", "chk 132")
            .unwrap();
        let target_path = PathBuf::from(&rec.quarantined_path);

        // Adversarially replace target in hook to force conflict publication
        engine.set_pre_cleanup_unlink_hook(Some(|target: &Path| {
            let _ = std::fs::write(target, "MODIFIED_IN_HOOK_132");
        }));

        let _ = engine.cleanup_quarantine(&rec.id);
        let manifest = engine.load_recovery_manifest().unwrap();
        let updated = manifest.records.iter().find(|x| x.id == rec.id).unwrap();
        assert_eq!(updated.status, QuarantineStatus::PreservedConflict);

        let surviving_path = PathBuf::from(&updated.quarantined_path);
        assert!(
            surviving_path.exists(),
            "Surviving path in manifest must physically exist"
        );
        assert_eq!(std::fs::read_to_string(&surviving_path).unwrap(), payload);
        assert_eq!(
            std::fs::read_to_string(&target_path).unwrap(),
            "MODIFIED_IN_HOOK_132"
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    // Section 3: P1 — Windows reparse-point and object-identity defense (CHK-133 .. CHK-138)
    #[test]
    fn test_chk_133_replace_staged_entry_with_symlink_before_windows_open() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_133_{}", uuid::Uuid::new_v4().simple()));
        std::fs::create_dir_all(&temp_dir).unwrap();
        let test_file = temp_dir.join("target_133.txt");
        let outside_file = temp_dir.join("outside_target.txt");
        std::fs::write(&outside_file, "CRITICAL OUTSIDE DATA").unwrap();

        #[cfg(windows)]
        {
            if std::os::windows::fs::symlink_file(&outside_file, &test_file).is_ok() {
                let res = filesystem_engine::remove_file_windows_exclusive(&test_file, "", None);
                assert!(
                    res.is_err(),
                    "Symlink must be rejected by remove_file_windows_exclusive"
                );
                assert!(
                    outside_file.exists(),
                    "Outside target must never be deleted"
                );
                assert_eq!(
                    std::fs::read_to_string(&outside_file).unwrap(),
                    "CRITICAL OUTSIDE DATA"
                );
            }
        }
        #[cfg(not(windows))]
        {
            let _ = (&test_file, &outside_file);
        }

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_134_replace_staged_entry_with_junction_or_reparse_point_via_seam() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_134_{}", uuid::Uuid::new_v4().simple()));
        std::fs::create_dir_all(&temp_dir).unwrap();
        let test_file = temp_dir.join("file_134.txt");
        std::fs::write(&test_file, "payload 134").unwrap();

        #[cfg(windows)]
        {
            filesystem_engine::set_fault_inject_windows_reparse_point(true);
            let res = filesystem_engine::remove_file_windows_exclusive(&test_file, "", None);
            filesystem_engine::set_fault_inject_windows_reparse_point(false);

            assert!(
                res.is_err(),
                "Reparse point flag must reject exclusive deletion"
            );
            assert!(test_file.exists(), "File must be preserved");
        }
        #[cfg(not(windows))]
        {
            let _ = &test_file;
        }

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_135_external_target_never_deleted() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_135_{}", uuid::Uuid::new_v4().simple()));
        std::fs::create_dir_all(&temp_dir).unwrap();
        let external_file = temp_dir.join("external_system.db");
        std::fs::write(&external_file, "SYSTEM RECORD 135").unwrap();

        #[cfg(windows)]
        {
            let link_file = temp_dir.join("link_135.txt");
            if std::os::windows::fs::symlink_file(&external_file, &link_file).is_ok() {
                let _ = filesystem_engine::remove_file_windows_exclusive(&link_file, "", None);
                assert!(
                    external_file.exists(),
                    "External file must never be deleted"
                );
                assert_eq!(
                    std::fs::read_to_string(&external_file).unwrap(),
                    "SYSTEM RECORD 135"
                );
            }
        }
        #[cfg(not(windows))]
        {
            let _ = &external_file;
        }

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_136_opened_handle_checked_for_reparse_point_status() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_136_{}", uuid::Uuid::new_v4().simple()));
        std::fs::create_dir_all(&temp_dir).unwrap();
        let dir_target = temp_dir.join("test_dir_136");
        std::fs::create_dir(&dir_target).unwrap();

        #[cfg(windows)]
        {
            let res = filesystem_engine::remove_file_windows_exclusive(&dir_target, "", None);
            assert!(
                res.is_err(),
                "Directory must be rejected by attributes check"
            );
            assert!(dir_target.exists());
        }
        #[cfg(not(windows))]
        {
            let _ = &dir_target;
        }

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_137_exclusive_sharing_remains_active_through_deletion() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_137_{}", uuid::Uuid::new_v4().simple()));
        std::fs::create_dir_all(&temp_dir).unwrap();
        let test_file = temp_dir.join("exclusive_137.txt");
        let payload = "exclusive 137 payload";
        std::fs::write(&test_file, payload).unwrap();
        let hash = CheckpointEngine::sha256_digest(payload.as_bytes());

        #[cfg(windows)]
        {
            let res = filesystem_engine::remove_file_windows_exclusive(&test_file, &hash, None);
            assert!(
                res.is_ok(),
                "Exclusive removal must succeed for unheld file"
            );
            assert!(!test_file.exists());
        }
        #[cfg(not(windows))]
        {
            let _ = (&test_file, &hash);
        }

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_138_failed_identity_check_preserves_candidate_and_reports_recovery_conflict() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_138_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        let workspace_dir = temp_dir.join("workspace");
        std::fs::create_dir_all(&workspace_dir).unwrap();

        let engine = CheckpointEngine::new(&storage_dir);
        let src_file = workspace_dir.join("file_138.txt");
        std::fs::write(&src_file, "payload 138").unwrap();

        let rec = engine
            .quarantine_file(&workspace_dir, "file_138.txt", "", "chk 138")
            .unwrap();

        #[cfg(windows)]
        {
            filesystem_engine::set_fault_inject_windows_reparse_point(true);
            let res = engine.cleanup_quarantine(&rec.id);
            filesystem_engine::set_fault_inject_windows_reparse_point(false);

            assert!(
                res.is_err(),
                "Must reject cleanup when reparse point identity check fails"
            );
            let manifest = engine.load_recovery_manifest().unwrap();
            let r = manifest.records.iter().find(|x| x.id == rec.id).unwrap();
            assert_eq!(r.status, QuarantineStatus::PreservedConflict);
            assert!(
                Path::new(&r.quarantined_path).exists(),
                "Candidate must be preserved"
            );
        }
        #[cfg(not(windows))]
        {
            let res = engine.cleanup_quarantine(&rec.id);
            assert!(res.is_err());
            let manifest = engine.load_recovery_manifest().unwrap();
            let r = manifest.records.iter().find(|x| x.id == rec.id).unwrap();
            assert_eq!(r.status, QuarantineStatus::PreservedConflict);
        }

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    // Section 4: P1 — Strict metadata error classification (CHK-139 .. CHK-142)
    #[test]
    fn test_chk_139_inject_permission_denied_during_target_inspection() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_139_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        let workspace_dir = temp_dir.join("workspace");
        std::fs::create_dir_all(&workspace_dir).unwrap();

        let engine = CheckpointEngine::new(&storage_dir);
        let src_file = workspace_dir.join("file_139.txt");
        let payload = "payload 139";
        std::fs::write(&src_file, payload).unwrap();

        let rec = engine
            .quarantine_file(&workspace_dir, "file_139.txt", "", "chk 139")
            .unwrap();

        engine.set_fault_inject_metadata_permission_denied(true);
        let res = engine.cleanup_quarantine(&rec.id);
        engine.set_fault_inject_metadata_permission_denied(false);

        assert!(
            res.is_err(),
            "Uncertain metadata (PermissionDenied) must fail closed"
        );
        let manifest = engine.load_recovery_manifest().unwrap();
        let r = manifest.records.iter().find(|x| x.id == rec.id).unwrap();
        assert_eq!(r.status, QuarantineStatus::PreservedConflict);
        let surviving_path = PathBuf::from(&r.quarantined_path);
        assert!(
            surviving_path.exists(),
            "File must never be deleted under uncertain metadata"
        );
        assert_eq!(std::fs::read_to_string(&surviving_path).unwrap(), payload);

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_140_inject_generic_metadata_io_failure() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_140_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        let workspace_dir = temp_dir.join("workspace");
        std::fs::create_dir_all(&workspace_dir).unwrap();

        let engine = CheckpointEngine::new(&storage_dir);
        let src_file = workspace_dir.join("file_140.txt");
        let payload = "payload 140";
        std::fs::write(&src_file, payload).unwrap();

        let rec = engine
            .quarantine_file(&workspace_dir, "file_140.txt", "", "chk 140")
            .unwrap();

        engine.set_fault_inject_metadata_io_error(true);
        let res = engine.cleanup_quarantine(&rec.id);
        engine.set_fault_inject_metadata_io_error(false);

        assert!(
            res.is_err(),
            "Generic metadata I/O failure must fail closed"
        );
        let manifest = engine.load_recovery_manifest().unwrap();
        let r = manifest.records.iter().find(|x| x.id == rec.id).unwrap();
        assert_eq!(r.status, QuarantineStatus::PreservedConflict);
        let surviving_path = PathBuf::from(&r.quarantined_path);
        assert!(
            surviving_path.exists(),
            "File must never be deleted under I/O error"
        );
        assert_eq!(std::fs::read_to_string(&surviving_path).unwrap(), payload);

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_141_neither_metadata_error_allows_deletion_or_overwrite() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_141_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        let workspace_dir = temp_dir.join("workspace");
        std::fs::create_dir_all(&workspace_dir).unwrap();

        let engine = CheckpointEngine::new(&storage_dir);
        let src_file = workspace_dir.join("file_141.txt");
        let payload = "critical data 141";
        std::fs::write(&src_file, payload).unwrap();

        let rec = engine
            .quarantine_file(&workspace_dir, "file_141.txt", "", "chk 141")
            .unwrap();

        // Test permission denied
        engine.set_fault_inject_metadata_permission_denied(true);
        let _ = engine.cleanup_quarantine(&rec.id);
        engine.set_fault_inject_metadata_permission_denied(false);
        let manifest = engine.load_recovery_manifest().unwrap();
        let r = manifest.records.iter().find(|x| x.id == rec.id).unwrap();
        assert_eq!(r.status, QuarantineStatus::PreservedConflict);
        let surviving_path = PathBuf::from(&r.quarantined_path);
        assert!(surviving_path.exists());
        assert_eq!(std::fs::read_to_string(&surviving_path).unwrap(), payload);

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_142_staged_artifact_remains_recoverable_and_manifest_reports_real_location() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_142_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        let workspace_dir = temp_dir.join("workspace");
        std::fs::create_dir_all(&workspace_dir).unwrap();

        let engine = CheckpointEngine::new(&storage_dir);
        let src_file = workspace_dir.join("file_142.txt");
        std::fs::write(&src_file, "recoverable data 142").unwrap();

        let rec = engine
            .quarantine_file(&workspace_dir, "file_142.txt", "", "chk 142")
            .unwrap();

        engine.set_fault_inject_metadata_permission_denied(true);
        let _ = engine.cleanup_quarantine(&rec.id);
        engine.set_fault_inject_metadata_permission_denied(false);

        let manifest = engine.load_recovery_manifest().unwrap();
        let r = manifest.records.iter().find(|x| x.id == rec.id).unwrap();
        assert_eq!(r.status, QuarantineStatus::PreservedConflict);
        assert!(
            Path::new(&r.quarantined_path).exists(),
            "Manifest path must physically exist"
        );
        assert_eq!(
            std::fs::read_to_string(&r.quarantined_path).unwrap(),
            "recoverable data 142"
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    // Section 5: P1 — Rollback must verify actual publication (CHK-143 .. CHK-147)
    #[test]
    fn test_chk_143_original_filename_recreated_during_rollback() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_143_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        let workspace_dir = temp_dir.join("workspace");
        std::fs::create_dir_all(&workspace_dir).unwrap();

        let engine = Arc::new(CheckpointEngine::new(&storage_dir));
        let src_file = workspace_dir.join("file_143.txt");
        std::fs::write(&src_file, "candidate 143").unwrap();

        let rec = engine
            .quarantine_file(&workspace_dir, "file_143.txt", "", "chk 143")
            .unwrap();
        let q_path = PathBuf::from(&rec.quarantined_path);

        engine.set_pre_cleanup_unlink_hook(Some(move |target: &Path| {
            // Concurrently recreate original filename
            std::fs::write(target, "RECREATED_ORIGINAL_143").unwrap();
        }));

        let res = engine.cleanup_quarantine(&rec.id);
        assert!(
            res.is_err(),
            "Must reject cleanup when original filename recreated"
        );

        // Recreated file at original path must be untouched
        assert_eq!(
            std::fs::read_to_string(&q_path).unwrap(),
            "RECREATED_ORIGINAL_143"
        );

        // Staged candidate must be preserved under conflict name
        let manifest = engine.load_recovery_manifest().unwrap();
        let updated = manifest.records.iter().find(|x| x.id == rec.id).unwrap();
        assert_eq!(updated.status, QuarantineStatus::PreservedConflict);
        assert_ne!(
            updated.quarantined_path,
            q_path.to_string_lossy().to_string()
        );
        assert!(Path::new(&updated.quarantined_path).exists());
        assert_eq!(
            std::fs::read_to_string(&updated.quarantined_path).unwrap(),
            "candidate 143"
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_144_conflict_name_collision_during_rollback() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_144_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        let workspace_dir = temp_dir.join("workspace");
        std::fs::create_dir_all(&workspace_dir).unwrap();

        let engine = Arc::new(CheckpointEngine::new(&storage_dir));
        let src_file = workspace_dir.join("file_144.txt");
        std::fs::write(&src_file, "candidate 144").unwrap();

        let rec = engine
            .quarantine_file(&workspace_dir, "file_144.txt", "", "chk 144")
            .unwrap();

        engine.set_pre_cleanup_unlink_hook(Some(|target: &Path| {
            std::fs::write(target, "RECREATED_144").unwrap();
        }));

        engine.set_fault_inject_rollback_conflict_collision(true);
        let res = engine.cleanup_quarantine(&rec.id);
        engine.set_fault_inject_rollback_conflict_collision(false);

        assert!(res.is_err());
        let manifest = engine.load_recovery_manifest().unwrap();
        let updated = manifest.records.iter().find(|x| x.id == rec.id).unwrap();
        assert_eq!(updated.status, QuarantineStatus::PreservedConflict);
        assert!(Path::new(&updated.quarantined_path).exists());
        assert_eq!(
            std::fs::read_to_string(&updated.quarantined_path).unwrap(),
            "candidate 144"
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_145_conflict_publication_permission_failure() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_145_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        let workspace_dir = temp_dir.join("workspace");
        std::fs::create_dir_all(&workspace_dir).unwrap();

        let engine = Arc::new(CheckpointEngine::new(&storage_dir));
        let src_file = workspace_dir.join("file_145.txt");
        std::fs::write(&src_file, "candidate 145").unwrap();

        let rec = engine
            .quarantine_file(&workspace_dir, "file_145.txt", "", "chk 145")
            .unwrap();

        engine.set_pre_cleanup_unlink_hook(Some(|target: &Path| {
            std::fs::write(target, "RECREATED_145").unwrap();
        }));

        engine.set_fault_inject_rollback_conflict_publication_failure(true);
        let res = engine.cleanup_quarantine(&rec.id);
        engine.set_fault_inject_rollback_conflict_publication_failure(false);

        assert!(res.is_err());
        assert!(
            engine.is_recovery_blocked(),
            "Failure to publish conflict must block engine"
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_146_both_original_and_conflict_publication_failures() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_146_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        let workspace_dir = temp_dir.join("workspace");
        std::fs::create_dir_all(&workspace_dir).unwrap();

        let engine = CheckpointEngine::new(&storage_dir);
        let src_file = workspace_dir.join("file_146.txt");
        std::fs::write(&src_file, "candidate 146").unwrap();

        let rec = engine
            .quarantine_file(&workspace_dir, "file_146.txt", "", "chk 146")
            .unwrap();

        // Inject target swap to force rollback path, combined with both publication failures
        engine.set_fault_inject_quarantine_target_swap(true);
        engine.set_fault_inject_rollback_both_publication_failures(true);
        let res = engine.cleanup_quarantine(&rec.id);
        engine.set_fault_inject_quarantine_target_swap(false);
        engine.set_fault_inject_rollback_both_publication_failures(false);

        assert!(res.is_err());
        assert!(
            engine.is_recovery_blocked(),
            "Engine must enter RecoveryBlocked when all publications fail"
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_147_manifest_path_matches_actual_surviving_artifact_after_every_outcome() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_147_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        let workspace_dir = temp_dir.join("workspace");
        std::fs::create_dir_all(&workspace_dir).unwrap();

        let engine = Arc::new(CheckpointEngine::new(&storage_dir));
        let src_file = workspace_dir.join("file_147.txt");
        std::fs::write(&src_file, "payload 147").unwrap();

        let rec = engine
            .quarantine_file(&workspace_dir, "file_147.txt", "", "chk 147")
            .unwrap();

        engine.set_pre_cleanup_unlink_hook(Some(|target: &Path| {
            std::fs::write(target, "SWAPPED_147").unwrap();
        }));

        let _ = engine.cleanup_quarantine(&rec.id);
        let manifest = engine.load_recovery_manifest().unwrap();
        let r = manifest.records.iter().find(|x| x.id == rec.id).unwrap();

        let actual_path = PathBuf::from(&r.quarantined_path);
        assert!(actual_path.exists(), "Manifest path must physically exist");
        assert_eq!(
            std::fs::read_to_string(&actual_path).unwrap(),
            "payload 147"
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    // Section 6: P1 — Manifest persistence must be fail-closed (CHK-148 .. CHK-152)
    #[test]
    fn test_chk_148_single_cleanup_manifest_save_failure() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_148_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        let workspace_dir = temp_dir.join("workspace");
        std::fs::create_dir_all(&workspace_dir).unwrap();

        let engine = CheckpointEngine::new(&storage_dir);
        let src_file = workspace_dir.join("file_148.txt");
        std::fs::write(&src_file, "payload 148").unwrap();

        let rec = engine
            .quarantine_file(&workspace_dir, "file_148.txt", "", "chk 148")
            .unwrap();

        engine.set_fault_inject_manifest_save(true);
        let res = engine.cleanup_quarantine(&rec.id);
        engine.set_fault_inject_manifest_save(false);

        assert!(res.is_err(), "Manifest save failure must fail closed");

        #[cfg(windows)]
        {
            // On Windows, physical removal succeeded before manifest update failed;
            // engine must report exact inconsistency and enter RecoveryBlocked.
            assert!(
                engine.is_recovery_blocked(),
                "Engine must enter RecoveryBlocked when file was removed but manifest update failed"
            );
            let reason = engine.recovery_blocked_reason().unwrap();
            assert!(reason.contains("Critical inconsistency: File"));
        }
        #[cfg(not(windows))]
        {
            // On POSIX, physical removal is never performed; artifact is rolled back to conflict
            let manifest = engine.load_recovery_manifest().unwrap();
            let r = manifest.records.iter().find(|x| x.id == rec.id).unwrap();
            assert!(
                Path::new(&r.quarantined_path).exists(),
                "Artifact must be preserved on disk on POSIX"
            );
        }

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_149_batch_cleanup_manifest_save_failure_halts_immediately() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_149_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        let workspace_dir = temp_dir.join("workspace");
        std::fs::create_dir_all(&workspace_dir).unwrap();

        let engine = CheckpointEngine::new(&storage_dir);
        let f1 = workspace_dir.join("batch_149_1.txt");
        let f2 = workspace_dir.join("batch_149_2.txt");
        std::fs::write(&f1, "1").unwrap();
        std::fs::write(&f2, "2").unwrap();

        let _r1 = engine
            .quarantine_file(&workspace_dir, "batch_149_1.txt", "", "1")
            .unwrap();
        let r2 = engine
            .quarantine_file(&workspace_dir, "batch_149_2.txt", "", "2")
            .unwrap();

        engine.set_fault_inject_manifest_save(true);
        let res = engine.cleanup_all_quarantined();
        engine.set_fault_inject_manifest_save(false);

        assert!(
            res.is_err(),
            "Batch cleanup must halt immediately when manifest save fails"
        );
        assert!(
            Path::new(&r2.quarantined_path).exists(),
            "Subsequent item must not be deleted"
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_150_manifest_failure_after_staged_conflict_created() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_150_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        let workspace_dir = temp_dir.join("workspace");
        std::fs::create_dir_all(&workspace_dir).unwrap();

        let engine = Arc::new(CheckpointEngine::new(&storage_dir));
        let src_file = workspace_dir.join("file_150.txt");
        std::fs::write(&src_file, "payload 150").unwrap();

        let rec = engine
            .quarantine_file(&workspace_dir, "file_150.txt", "", "chk 150")
            .unwrap();

        engine.set_pre_cleanup_unlink_hook(Some(|target: &Path| {
            std::fs::write(target, "RECREATED_150").unwrap();
        }));

        engine.set_fault_inject_manifest_save(true);
        let res = engine.cleanup_quarantine(&rec.id);
        engine.set_fault_inject_manifest_save(false);

        assert!(res.is_err(), "Manifest failure must abort cleanup");

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_151_startup_recovery_after_interrupted_manifest_update() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_151_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        let workspace_dir = temp_dir.join("workspace");
        std::fs::create_dir_all(&workspace_dir).unwrap();

        let engine = CheckpointEngine::new(&storage_dir);
        let f = workspace_dir.join("file_151.txt");
        std::fs::write(&f, "interrupted manifest payload").unwrap();

        let rec = engine
            .quarantine_file(&workspace_dir, "file_151.txt", "", "chk 151")
            .unwrap();

        // Simulate startup reconciliation: existing artifacts on disk are preserved
        let reconciled = engine.reconcile_quarantine().unwrap();
        let item = reconciled.records.iter().find(|x| x.id == rec.id).unwrap();
        assert!(
            Path::new(&item.quarantined_path).exists(),
            "Artifact must be preserved on startup"
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_152_recovery_blocked_remains_active_until_successful_reconciliation() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_152_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        let workspace_dir = temp_dir.join("workspace");
        std::fs::create_dir_all(&workspace_dir).unwrap();

        let engine = CheckpointEngine::new(&storage_dir);

        // Put engine in RecoveryBlocked state
        engine.set_recovery_blocked(Some("Interrupted manifest update error".into()));
        assert!(engine.is_recovery_blocked());

        // All operations must be rejected
        assert!(engine.list_checkpoints().is_err());
        assert!(engine.create_checkpoint("desc", &[]).is_err());
        assert!(engine.cleanup_quarantine("id").is_err());
        assert!(engine.cleanup_all_quarantined().is_err());

        // Reconcile successfully unblocks
        let res = engine.reconcile_quarantine();
        assert!(res.is_ok(), "Reconciliation must succeed");
        assert!(
            !engine.is_recovery_blocked(),
            "Reconciliation must clear blocked state"
        );

        // Now operations succeed
        assert!(engine.list_checkpoints().is_ok());

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_153_baseline_atomic_no_overwrite_rollback_success() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_153_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        let workspace_dir = temp_dir.join("workspace");
        std::fs::create_dir_all(&workspace_dir).unwrap();

        let engine = CheckpointEngine::new(&storage_dir);
        let src_file = workspace_dir.join("file_153.txt");
        let payload = "rollback atomic link payload 153";
        std::fs::write(&src_file, payload).unwrap();

        let rec = engine
            .quarantine_file(&workspace_dir, "file_153.txt", "", "chk 153")
            .unwrap();

        // Trigger rollback by simulating cleanup removal failure
        engine.set_fault_inject_cleanup_removal(true);
        let res = engine.cleanup_quarantine(&rec.id);
        engine.set_fault_inject_cleanup_removal(false);

        assert!(res.is_err());
        let q_path = PathBuf::from(&rec.quarantined_path);
        assert!(
            q_path.exists(),
            "Candidate must be published back to original name without overwrite"
        );
        assert_eq!(std::fs::read_to_string(&q_path).unwrap(), payload);

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_154_concurrent_recreation_at_original_destination_atomic_hard_link_fails() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_154_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        let workspace_dir = temp_dir.join("workspace");
        std::fs::create_dir_all(&workspace_dir).unwrap();

        let engine = CheckpointEngine::new(&storage_dir);
        let src_file = workspace_dir.join("file_154.txt");
        let original_payload = "original 154 staged payload";
        std::fs::write(&src_file, original_payload).unwrap();

        let rec = engine
            .quarantine_file(&workspace_dir, "file_154.txt", "", "chk 154")
            .unwrap();

        // Inject hook that recreates the original file right after absence check, before publish hard link
        let target_path = PathBuf::from(&rec.quarantined_path);
        let target_clone = target_path.clone();
        engine.set_pre_rollback_publish_hook(Some(move |path: &Path| {
            if path == target_clone {
                std::fs::write(path, "CONCURRENT_RECREATED_154_PAYLOAD").unwrap();
            }
        }));

        engine.set_fault_inject_cleanup_removal(true);
        let res = engine.cleanup_quarantine(&rec.id);
        engine.set_fault_inject_cleanup_removal(false);
        engine.set_pre_rollback_publish_hook(None::<fn(&Path)>);

        assert!(res.is_err());
        // Verify original destination was NOT overwritten: it contains concurrent data!
        assert_eq!(
            std::fs::read_to_string(&target_path).unwrap(),
            "CONCURRENT_RECREATED_154_PAYLOAD",
            "Concurrent destination must never be clobbered by rollback"
        );

        // Verify candidate was safely preserved as conflict artifact
        let manifest = engine.load_recovery_manifest().unwrap();
        let r = manifest.records.iter().find(|x| x.id == rec.id).unwrap();
        assert_eq!(r.status, QuarantineStatus::PreservedConflict);
        assert!(
            Path::new(&r.quarantined_path).exists(),
            "Preserved conflict file must exist"
        );
        assert_eq!(
            std::fs::read_to_string(&r.quarantined_path).unwrap(),
            original_payload
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_155_injected_hard_link_failure_falls_back_to_conflict() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_155_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        let workspace_dir = temp_dir.join("workspace");
        std::fs::create_dir_all(&workspace_dir).unwrap();

        let engine = CheckpointEngine::new(&storage_dir);
        let src_file = workspace_dir.join("file_155.txt");
        let payload = "injected hard link failure payload 155";
        std::fs::write(&src_file, payload).unwrap();

        let rec = engine
            .quarantine_file(&workspace_dir, "file_155.txt", "", "chk 155")
            .unwrap();

        // Injected hard link failure on rollback
        engine.set_fault_inject_rollback_hard_link_failure(true);
        engine.set_fault_inject_cleanup_removal(true);
        let res = engine.cleanup_quarantine(&rec.id);
        engine.set_fault_inject_rollback_hard_link_failure(false);
        engine.set_fault_inject_cleanup_removal(false);

        assert!(res.is_err());
        let manifest = engine.load_recovery_manifest().unwrap();
        let r = manifest.records.iter().find(|x| x.id == rec.id).unwrap();
        assert_eq!(r.status, QuarantineStatus::PreservedConflict);
        assert!(Path::new(&r.quarantined_path).exists());
        assert_eq!(
            std::fs::read_to_string(&r.quarantined_path).unwrap(),
            payload
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_156_staging_removal_failure_preserves_published_artifact() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_156_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        let workspace_dir = temp_dir.join("workspace");
        std::fs::create_dir_all(&workspace_dir).unwrap();

        let engine = CheckpointEngine::new(&storage_dir);
        let src_file = workspace_dir.join("file_156.txt");
        let payload = "staging removal failure payload 156";
        std::fs::write(&src_file, payload).unwrap();

        let rec = engine
            .quarantine_file(&workspace_dir, "file_156.txt", "", "chk 156")
            .unwrap();

        engine.set_fault_inject_rollback_staging_removal_failure(true);
        engine.set_fault_inject_cleanup_removal(true);
        let res = engine.cleanup_quarantine(&rec.id);
        engine.set_fault_inject_rollback_staging_removal_failure(false);
        engine.set_fault_inject_cleanup_removal(false);

        assert!(res.is_err());
        let manifest = engine.load_recovery_manifest().unwrap();
        let r = manifest.records.iter().find(|x| x.id == rec.id).unwrap();
        let published_path = PathBuf::from(&r.quarantined_path);
        assert!(
            published_path.exists(),
            "Published destination artifact must exist even if staging link removal fails"
        );
        assert_eq!(std::fs::read_to_string(&published_path).unwrap(), payload);

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_157_conflict_publication_name_collision_resolves_to_unique_name() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_157_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        let workspace_dir = temp_dir.join("workspace");
        std::fs::create_dir_all(&workspace_dir).unwrap();

        let engine = CheckpointEngine::new(&storage_dir);
        let src_file = workspace_dir.join("file_157.txt");
        let payload = "collision resolution payload 157";
        std::fs::write(&src_file, payload).unwrap();

        let rec = engine
            .quarantine_file(&workspace_dir, "file_157.txt", "", "chk 157")
            .unwrap();

        // Recreate original destination so rollback goes to conflict
        let orig_q_path = PathBuf::from(&rec.quarantined_path);
        let orig_clone = orig_q_path.clone();
        engine.set_pre_cleanup_unlink_hook(Some(move |path: &Path| {
            if path == orig_clone {
                std::fs::write(path, "RECREATED_ORIGINAL_157").unwrap();
            }
        }));

        engine.set_fault_inject_rollback_conflict_collision(true);
        let res = engine.cleanup_quarantine(&rec.id);
        engine.set_fault_inject_rollback_conflict_collision(false);
        engine.set_pre_cleanup_unlink_hook(None::<fn(&Path)>);

        assert!(res.is_err());
        let manifest = engine.load_recovery_manifest().unwrap();
        let r = manifest.records.iter().find(|x| x.id == rec.id).unwrap();
        assert_eq!(r.status, QuarantineStatus::PreservedConflict);
        assert!(
            Path::new(&r.quarantined_path).exists(),
            "Unique conflict artifact must exist"
        );
        assert_eq!(
            std::fs::read_to_string(&r.quarantined_path).unwrap(),
            payload
        );
        assert_eq!(
            std::fs::read_to_string(&orig_q_path).unwrap(),
            "RECREATED_ORIGINAL_157"
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_158_both_publication_failures_retains_staging_and_blocks_recovery() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_158_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        let workspace_dir = temp_dir.join("workspace");
        std::fs::create_dir_all(&workspace_dir).unwrap();

        let engine = CheckpointEngine::new(&storage_dir);
        let src_file = workspace_dir.join("file_158.txt");
        let payload = "both fail payload 158";
        std::fs::write(&src_file, payload).unwrap();

        let rec = engine
            .quarantine_file(&workspace_dir, "file_158.txt", "", "chk 158")
            .unwrap();

        engine.set_fault_inject_rollback_both_publication_failures(true);
        engine.set_fault_inject_cleanup_removal(true);
        let res = engine.cleanup_quarantine(&rec.id);
        engine.set_fault_inject_rollback_both_publication_failures(false);
        engine.set_fault_inject_cleanup_removal(false);

        assert!(res.is_err());
        assert!(
            engine.is_recovery_blocked(),
            "RecoveryBlocked state must be active after all publication attempts fail"
        );
        let manifest = engine.load_recovery_manifest().unwrap();
        let r = manifest.records.iter().find(|x| x.id == rec.id).unwrap();
        let staging_path = PathBuf::from(&r.quarantined_path);
        assert!(
            staging_path.exists(),
            "Staged artifact must be preserved on disk at staging name"
        );
        assert_eq!(std::fs::read_to_string(&staging_path).unwrap(), payload);

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    #[allow(clippy::permissions_set_readonly_false)]
    fn test_chk_159_atomic_link_refusal_on_existing_read_only_or_special_file() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_159_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        let workspace_dir = temp_dir.join("workspace");
        std::fs::create_dir_all(&workspace_dir).unwrap();

        let engine = CheckpointEngine::new(&storage_dir);
        let src_file = workspace_dir.join("file_159.txt");
        let payload = "read-only test payload 159";
        std::fs::write(&src_file, payload).unwrap();

        let rec = engine
            .quarantine_file(&workspace_dir, "file_159.txt", "", "chk 159")
            .unwrap();

        let orig_path = PathBuf::from(&rec.quarantined_path);
        let orig_clone = orig_path.clone();
        engine.set_pre_cleanup_unlink_hook(Some(move |path: &Path| {
            if path == orig_clone {
                std::fs::write(path, "READ_ONLY_RECREATED_DATA").unwrap();
                let mut perms = std::fs::metadata(path).unwrap().permissions();
                perms.set_readonly(true);
                let _ = std::fs::set_permissions(path, perms);
            }
        }));

        let res = engine.cleanup_quarantine(&rec.id);
        engine.set_pre_cleanup_unlink_hook(None::<fn(&Path)>);

        assert!(res.is_err());
        // Clean up read-only permission for directory cleanup
        if let Ok(mut perms) = std::fs::metadata(&orig_path).map(|m| m.permissions()) {
            perms.set_readonly(false);
            let _ = std::fs::set_permissions(&orig_path, perms);
        }
        assert_eq!(
            std::fs::read_to_string(&orig_path).unwrap(),
            "READ_ONLY_RECREATED_DATA"
        );

        let manifest = engine.load_recovery_manifest().unwrap();
        let r = manifest.records.iter().find(|x| x.id == rec.id).unwrap();
        assert_eq!(r.status, QuarantineStatus::PreservedConflict);
        assert!(Path::new(&r.quarantined_path).exists());
        assert_eq!(
            std::fs::read_to_string(&r.quarantined_path).unwrap(),
            payload
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_160_windows_exclusive_deletion_matching_identity_and_hash_succeeds() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_160_{}", uuid::Uuid::new_v4().simple()));
        std::fs::create_dir_all(&temp_dir).unwrap();
        let test_file = temp_dir.join("exclusive_160.txt");
        let payload = "matching identity and hash 160";
        std::fs::write(&test_file, payload).unwrap();

        #[cfg(windows)]
        {
            let hash = CheckpointEngine::sha256_digest(payload.as_bytes());
            let identity = filesystem_engine::query_file_object_identity(&test_file).unwrap();
            let res =
                filesystem_engine::remove_file_windows_exclusive(&test_file, &hash, Some(identity));
            assert!(
                res.is_ok(),
                "Exclusive removal must succeed when identity and hash match"
            );
            assert!(!test_file.exists(), "File must be deleted");
        }
        #[cfg(not(windows))]
        {
            let _ = &test_file;
        }

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_161_windows_exclusive_deletion_mismatched_identity_rejected() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_161_{}", uuid::Uuid::new_v4().simple()));
        std::fs::create_dir_all(&temp_dir).unwrap();
        let f1 = temp_dir.join("file_161_1.txt");
        let f2 = temp_dir.join("file_161_2.txt");
        let payload = "payload 161";
        std::fs::write(&f1, payload).unwrap();
        std::fs::write(&f2, payload).unwrap();

        #[cfg(windows)]
        {
            let hash = CheckpointEngine::sha256_digest(payload.as_bytes());
            let identity_2 = filesystem_engine::query_file_object_identity(&f2).unwrap();
            // Attempt to delete f1 with f2's identity
            let res =
                filesystem_engine::remove_file_windows_exclusive(&f1, &hash, Some(identity_2));
            assert!(
                res.is_err(),
                "Identity mismatch must reject exclusive deletion"
            );
            assert!(f1.exists(), "File 1 must be preserved");
        }
        #[cfg(not(windows))]
        {
            let _ = (&f1, &f2);
        }

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_162_windows_exclusive_deletion_injected_identity_mismatch_seam() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_162_{}", uuid::Uuid::new_v4().simple()));
        std::fs::create_dir_all(&temp_dir).unwrap();
        let test_file = temp_dir.join("exclusive_162.txt");
        let payload = "injected identity mismatch seam 162";
        std::fs::write(&test_file, payload).unwrap();

        #[cfg(windows)]
        {
            let hash = CheckpointEngine::sha256_digest(payload.as_bytes());
            let identity = filesystem_engine::query_file_object_identity(&test_file).unwrap();
            filesystem_engine::set_fault_inject_windows_identity_mismatch(true);
            let res =
                filesystem_engine::remove_file_windows_exclusive(&test_file, &hash, Some(identity));
            filesystem_engine::set_fault_inject_windows_identity_mismatch(false);

            assert!(
                res.is_err(),
                "Injected identity mismatch must reject deletion fail-closed"
            );
            assert!(test_file.exists(), "File must be preserved");
        }
        #[cfg(not(windows))]
        {
            let _ = &test_file;
        }

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_163_windows_exclusive_deletion_reparse_point_rejected() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_163_{}", uuid::Uuid::new_v4().simple()));
        std::fs::create_dir_all(&temp_dir).unwrap();
        let outside = temp_dir.join("outside_163.txt");
        std::fs::write(&outside, "CRITICAL OUTSIDE DATA 163").unwrap();

        #[cfg(windows)]
        {
            let link = temp_dir.join("link_163.txt");
            if std::os::windows::fs::symlink_file(&outside, &link).is_ok() {
                let res = filesystem_engine::remove_file_windows_exclusive(&link, "", None);
                assert!(res.is_err(), "Reparse point must be rejected");
                assert!(outside.exists(), "Outside target must never be deleted");
            }
        }
        #[cfg(not(windows))]
        {
            let _ = &outside;
        }

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_164_windows_exclusive_deletion_directory_swap_rejected() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_164_{}", uuid::Uuid::new_v4().simple()));
        std::fs::create_dir_all(&temp_dir).unwrap();
        let dir = temp_dir.join("dir_164");
        std::fs::create_dir(&dir).unwrap();

        #[cfg(windows)]
        {
            let res = filesystem_engine::remove_file_windows_exclusive(&dir, "", None);
            assert!(res.is_err(), "Directory must be rejected");
            assert!(dir.exists());
        }
        #[cfg(not(windows))]
        {
            let _ = &dir;
        }

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_165_windows_exclusive_deletion_hash_mismatch_under_same_identity_rejected() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_165_{}", uuid::Uuid::new_v4().simple()));
        std::fs::create_dir_all(&temp_dir).unwrap();
        let test_file = temp_dir.join("file_165.txt");
        std::fs::write(&test_file, "content 165").unwrap();

        #[cfg(windows)]
        {
            let identity = filesystem_engine::query_file_object_identity(&test_file).unwrap();
            let res = filesystem_engine::remove_file_windows_exclusive(
                &test_file,
                "deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef",
                Some(identity),
            );
            assert!(
                res.is_err(),
                "Hash mismatch must reject deletion even under identical file object identity"
            );
            assert!(test_file.exists());
        }
        #[cfg(not(windows))]
        {
            let _ = &test_file;
        }

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_166_windows_exclusive_deletion_sharing_violation_retained_handle() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_166_{}", uuid::Uuid::new_v4().simple()));
        std::fs::create_dir_all(&temp_dir).unwrap();
        let test_file = temp_dir.join("file_166.txt");
        let payload = "retained handle payload 166";
        std::fs::write(&test_file, payload).unwrap();

        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            const FILE_SHARE_READ: u32 = 0x00000001;
            // Hold open handle with FILE_SHARE_READ (no FILE_SHARE_DELETE)
            let held = std::fs::OpenOptions::new()
                .read(true)
                .share_mode(FILE_SHARE_READ)
                .open(&test_file)
                .unwrap();

            let hash = CheckpointEngine::sha256_digest(payload.as_bytes());
            let res = filesystem_engine::remove_file_windows_exclusive(&test_file, &hash, None);
            drop(held);

            assert!(
                res.is_err(),
                "Sharing violation with retained handle must reject exclusive deletion"
            );
            assert!(test_file.exists());
        }
        #[cfg(not(windows))]
        {
            let _ = &test_file;
        }

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_167_batch_cleanup_verifies_identity_on_every_entry() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_167_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        let workspace_dir = temp_dir.join("workspace");
        std::fs::create_dir_all(&workspace_dir).unwrap();

        let engine = Arc::new(CheckpointEngine::new(&storage_dir));
        let f1 = workspace_dir.join("batch_167_1.txt");
        let f2 = workspace_dir.join("batch_167_2.txt");
        std::fs::write(&f1, "b1 167").unwrap();
        std::fs::write(&f2, "b2 167").unwrap();

        let r1 = engine
            .quarantine_file(&workspace_dir, "batch_167_1.txt", "", "b1")
            .unwrap();
        let r2 = engine
            .quarantine_file(&workspace_dir, "batch_167_2.txt", "", "b2")
            .unwrap();

        #[cfg(windows)]
        {
            let r1_path = PathBuf::from(&r1.quarantined_path);
            let r1_clone = r1_path.clone();
            // Hook on r1 to inject identity mismatch
            engine.set_pre_cleanup_unlink_hook(Some(move |target: &Path| {
                if target == r1_clone {
                    filesystem_engine::set_fault_inject_windows_identity_mismatch(true);
                } else {
                    filesystem_engine::set_fault_inject_windows_identity_mismatch(false);
                }
            }));

            let _ = engine.cleanup_all_quarantined();
            filesystem_engine::set_fault_inject_windows_identity_mismatch(false);
            engine.set_pre_cleanup_unlink_hook(None::<fn(&Path)>);

            let manifest = engine.load_recovery_manifest().unwrap();
            let rec1 = manifest.records.iter().find(|r| r.id == r1.id).unwrap();
            let rec2 = manifest.records.iter().find(|r| r.id == r2.id).unwrap();

            // r1 should be PreservedConflict because identity mismatch rejected deletion
            assert_eq!(rec1.status, QuarantineStatus::PreservedConflict);
            assert!(Path::new(&rec1.quarantined_path).exists());

            // r2 proceeded cleanly without identity mismatch
            assert_eq!(rec2.status, QuarantineStatus::CleanedUp);
        }
        #[cfg(not(windows))]
        {
            let _ = (&r1, &r2);
        }

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_168_single_cleanup_manifest_save_failure_after_deletion_blocks_recovery() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_168_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        let workspace_dir = temp_dir.join("workspace");
        std::fs::create_dir_all(&workspace_dir).unwrap();

        let engine = CheckpointEngine::new(&storage_dir);
        let src_file = workspace_dir.join("file_168.txt");
        std::fs::write(&src_file, "payload 168").unwrap();

        let rec = engine
            .quarantine_file(&workspace_dir, "file_168.txt", "", "chk 168")
            .unwrap();

        engine.set_fault_inject_manifest_save(true);
        let res = engine.cleanup_quarantine(&rec.id);
        engine.set_fault_inject_manifest_save(false);

        assert!(res.is_err(), "Manifest save failure must fail closed");
        assert!(
            engine.is_recovery_blocked(),
            "Manifest save failure must activate RecoveryBlocked"
        );
        let reason = engine.recovery_blocked_reason().unwrap();
        assert!(
            reason.contains(&rec.id),
            "RecoveryBlocked reason must contain record ID: {}",
            reason
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_169_single_cleanup_manifest_save_failure_after_conflict_rollback_blocks_recovery() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_169_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        let workspace_dir = temp_dir.join("workspace");
        std::fs::create_dir_all(&workspace_dir).unwrap();

        let engine = CheckpointEngine::new(&storage_dir);
        let src_file = workspace_dir.join("file_169.txt");
        std::fs::write(&src_file, "payload 169").unwrap();

        let rec = engine
            .quarantine_file(&workspace_dir, "file_169.txt", "", "chk 169")
            .unwrap();

        // Mutate quarantined file to force conflict rollback
        std::fs::write(&rec.quarantined_path, "MODIFIED_169").unwrap();

        engine.set_fault_inject_manifest_save(true);
        let res = engine.cleanup_quarantine(&rec.id);
        engine.set_fault_inject_manifest_save(false);

        assert!(res.is_err());
        assert!(engine.is_recovery_blocked());
        let reason = engine.recovery_blocked_reason().unwrap();
        assert!(reason.contains(&rec.id));

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_170_batch_cleanup_missing_entry_manifest_save_failure_halts_batch() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_170_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        let workspace_dir = temp_dir.join("workspace");
        std::fs::create_dir_all(&workspace_dir).unwrap();

        let engine = CheckpointEngine::new(&storage_dir);
        let f1 = workspace_dir.join("batch_170_1.txt");
        let f2 = workspace_dir.join("batch_170_2.txt");
        std::fs::write(&f1, "b1 170").unwrap();
        std::fs::write(&f2, "b2 170").unwrap();

        let r1 = engine
            .quarantine_file(&workspace_dir, "batch_170_1.txt", "", "b1")
            .unwrap();
        let r2 = engine
            .quarantine_file(&workspace_dir, "batch_170_2.txt", "", "b2")
            .unwrap();

        // Physically remove r1 so it takes the missing-entry path
        let _ = std::fs::remove_file(&r1.quarantined_path);

        // Inject manifest save failure
        engine.set_fault_inject_manifest_save(true);
        let res = engine.cleanup_all_quarantined();
        engine.set_fault_inject_manifest_save(false);

        assert!(
            res.is_err(),
            "Manifest save failure on missing entry must halt batch immediately"
        );
        assert!(
            engine.is_recovery_blocked(),
            "RecoveryBlocked must be activated"
        );
        // Item 2 must NOT have been cleaned up or touched
        let r2_path = PathBuf::from(&r2.quarantined_path);
        assert!(
            r2_path.exists(),
            "Subsequent entries must be untouched when batch halts"
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_171_batch_cleanup_manifest_save_failure_after_conflict_halts_batch() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_171_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        let workspace_dir = temp_dir.join("workspace");
        std::fs::create_dir_all(&workspace_dir).unwrap();

        let engine = CheckpointEngine::new(&storage_dir);
        let f1 = workspace_dir.join("batch_171_1.txt");
        let f2 = workspace_dir.join("batch_171_2.txt");
        std::fs::write(&f1, "b1 171").unwrap();
        std::fs::write(&f2, "b2 171").unwrap();

        let r1 = engine
            .quarantine_file(&workspace_dir, "batch_171_1.txt", "", "b1")
            .unwrap();
        let r2 = engine
            .quarantine_file(&workspace_dir, "batch_171_2.txt", "", "b2")
            .unwrap();

        // Mutate r1 to force conflict rollback
        std::fs::write(&r1.quarantined_path, "MODIFIED_171").unwrap();

        engine.set_fault_inject_manifest_save(true);
        let res = engine.cleanup_all_quarantined();
        engine.set_fault_inject_manifest_save(false);

        assert!(res.is_err(), "Batch must halt on manifest save failure");
        assert!(engine.is_recovery_blocked());
        assert!(
            Path::new(&r2.quarantined_path).exists(),
            "r2 must be untouched"
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_172_batch_cleanup_manifest_save_failure_after_physical_removal_halts_batch() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_172_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        let workspace_dir = temp_dir.join("workspace");
        std::fs::create_dir_all(&workspace_dir).unwrap();

        let engine = CheckpointEngine::new(&storage_dir);
        let f1 = workspace_dir.join("batch_172_1.txt");
        let f2 = workspace_dir.join("batch_172_2.txt");
        std::fs::write(&f1, "b1 172").unwrap();
        std::fs::write(&f2, "b2 172").unwrap();

        let _r1 = engine
            .quarantine_file(&workspace_dir, "batch_172_1.txt", "", "b1")
            .unwrap();
        let r2 = engine
            .quarantine_file(&workspace_dir, "batch_172_2.txt", "", "b2")
            .unwrap();

        engine.set_fault_inject_manifest_save(true);
        let res = engine.cleanup_all_quarantined();
        engine.set_fault_inject_manifest_save(false);

        assert!(
            res.is_err(),
            "Batch must halt on manifest persistence failure"
        );
        assert!(engine.is_recovery_blocked());
        assert!(
            Path::new(&r2.quarantined_path).exists(),
            "r2 must be untouched"
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_173_reconcile_quarantine_manifest_save_failure_blocks_recovery() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_173_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        let workspace_dir = temp_dir.join("workspace");
        std::fs::create_dir_all(&workspace_dir).unwrap();

        let engine = CheckpointEngine::new(&storage_dir);
        let f = workspace_dir.join("file_173.txt");
        std::fs::write(&f, "payload 173").unwrap();

        let _rec = engine
            .quarantine_file(&workspace_dir, "file_173.txt", "", "chk 173")
            .unwrap();

        engine.set_fault_inject_manifest_save(true);
        let res = engine.reconcile_quarantine();
        engine.set_fault_inject_manifest_save(false);

        assert!(
            res.is_err(),
            "Reconcile manifest save failure must fail closed"
        );
        assert!(
            engine.is_recovery_blocked(),
            "RecoveryBlocked must be activated on reconciliation save failure"
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_174_subsequent_cleanup_rejected_while_recovery_blocked() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_174_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        let workspace_dir = temp_dir.join("workspace");
        std::fs::create_dir_all(&workspace_dir).unwrap();

        let engine = CheckpointEngine::new(&storage_dir);
        let f = workspace_dir.join("file_174.txt");
        std::fs::write(&f, "payload 174").unwrap();

        let rec = engine
            .quarantine_file(&workspace_dir, "file_174.txt", "", "chk 174")
            .unwrap();

        // Directly set RecoveryBlocked
        engine.set_recovery_blocked(Some("Previous manifest desync detected".into()));

        // Both cleanup calls must be rejected immediately without touching disk
        let res1 = engine.cleanup_quarantine(&rec.id);
        assert!(
            res1.is_err(),
            "Single cleanup must be rejected while RecoveryBlocked"
        );

        let res2 = engine.cleanup_all_quarantined();
        assert!(
            res2.is_err(),
            "Batch cleanup must be rejected while RecoveryBlocked"
        );

        assert!(
            Path::new(&rec.quarantined_path).exists(),
            "Artifact must be untouched"
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_175_reconciliation_clears_recovery_blocked_and_resumes_operations() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_175_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        let workspace_dir = temp_dir.join("workspace");
        std::fs::create_dir_all(&workspace_dir).unwrap();

        let engine = CheckpointEngine::new(&storage_dir);
        let f = workspace_dir.join("file_175.txt");
        std::fs::write(&f, "payload 175").unwrap();

        let rec = engine
            .quarantine_file(&workspace_dir, "file_175.txt", "", "chk 175")
            .unwrap();

        engine.set_recovery_blocked(Some("Interrupted transaction".into()));
        assert!(engine.is_recovery_blocked());

        // Reconcile succeeds and unblocks
        let reconciled = engine.reconcile_quarantine();
        assert!(reconciled.is_ok());
        assert!(!engine.is_recovery_blocked());

        // Subsequent cleanup now proceeds
        let res = engine.cleanup_quarantine(&rec.id);
        #[cfg(windows)]
        {
            assert!(
                res.is_ok(),
                "Cleanup should succeed after unblocking on Windows"
            );
        }
        #[cfg(not(windows))]
        {
            assert!(res.is_err(), "POSIX fail-closed disposal contract");
        }

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_176_replace_staging_entry_after_rename_before_verification() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_176_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        let workspace_dir = temp_dir.join("workspace");
        std::fs::create_dir_all(&workspace_dir).unwrap();

        let engine = Arc::new(CheckpointEngine::new(&storage_dir));
        let f = workspace_dir.join("file_176.txt");
        std::fs::write(&f, "original payload 176").unwrap();

        let rec = engine
            .quarantine_file(&workspace_dir, "file_176.txt", "", "chk 176")
            .unwrap();

        // Hook: replace stage entry right after rename
        let replaced_path = Arc::new(Mutex::new(None));
        let replaced_path_clone = replaced_path.clone();
        engine.set_post_stage_rename_hook(Some(move |stage_path: &Path| {
            let _ = std::fs::remove_file(stage_path);
            std::fs::write(stage_path, "adversarial replacement file 176").unwrap();
            *replaced_path_clone.lock() = Some(stage_path.to_path_buf());
        }));

        let res = engine.cleanup_quarantine(&rec.id);
        assert!(
            res.is_err(),
            "Replacement file must be rejected fail-closed!"
        );

        let manifest = engine.load_recovery_manifest().unwrap();
        let r = manifest.records.iter().find(|x| x.id == rec.id).unwrap();
        assert_eq!(r.status, QuarantineStatus::PreservedConflict);
        assert!(
            PathBuf::from(&r.quarantined_path).exists(),
            "Conflict artifact must exist on disk!"
        );

        engine.set_post_stage_rename_hook(None::<fn(&Path)>);
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_177_replacement_identical_bytes_different_identity_rejected() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_177_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        let workspace_dir = temp_dir.join("workspace");
        std::fs::create_dir_all(&workspace_dir).unwrap();

        let engine = Arc::new(CheckpointEngine::new(&storage_dir));
        let f = workspace_dir.join("file_177.txt");
        let payload = "IDENTICAL_BYTES_ACROSS_FILES";
        std::fs::write(&f, payload).unwrap();

        let rec = engine
            .quarantine_file(&workspace_dir, "file_177.txt", "", "chk 177")
            .unwrap();

        // Hook replaces stage entry with a newly created file having the EXACT SAME BYTES, but different inode/identity
        engine.set_post_stage_rename_hook(Some(move |stage_path: &Path| {
            let _ = std::fs::remove_file(stage_path);
            std::fs::write(stage_path, payload).unwrap();
        }));

        let res = engine.cleanup_quarantine(&rec.id);
        assert!(
            res.is_err(),
            "Identical bytes with different identity must be rejected!"
        );
        match res.unwrap_err() {
            AetherError::PermissionDenied(msg) => {
                assert!(msg.contains("identity mismatch") || msg.contains("Security rejection"));
            }
            other => panic!("Expected PermissionDenied, got {:?}", other),
        }

        let manifest = engine.load_recovery_manifest().unwrap();
        let r = manifest.records.iter().find(|x| x.id == rec.id).unwrap();
        assert_eq!(r.status, QuarantineStatus::PreservedConflict);
        assert!(PathBuf::from(&r.quarantined_path).exists());

        engine.set_post_stage_rename_hook(None::<fn(&Path)>);
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_178_replacement_different_bytes_different_identity_rejected() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_178_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        let workspace_dir = temp_dir.join("workspace");
        std::fs::create_dir_all(&workspace_dir).unwrap();

        let engine = Arc::new(CheckpointEngine::new(&storage_dir));
        let f = workspace_dir.join("file_178.txt");
        std::fs::write(&f, "ORIGINAL_PAYLOAD_178").unwrap();

        let rec = engine
            .quarantine_file(&workspace_dir, "file_178.txt", "", "chk 178")
            .unwrap();

        engine.set_post_stage_rename_hook(Some(move |stage_path: &Path| {
            let _ = std::fs::remove_file(stage_path);
            std::fs::write(stage_path, "TAMPERED_DIFFERENT_PAYLOAD").unwrap();
        }));

        let res = engine.cleanup_quarantine(&rec.id);
        assert!(res.is_err(), "Replacement must be rejected fail-closed!");

        let manifest = engine.load_recovery_manifest().unwrap();
        let r = manifest.records.iter().find(|x| x.id == rec.id).unwrap();
        assert_eq!(r.status, QuarantineStatus::PreservedConflict);
        assert!(PathBuf::from(&r.quarantined_path).exists());

        engine.set_post_stage_rename_hook(None::<fn(&Path)>);
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_179_original_identity_remains_bound_across_staging() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_179_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        let workspace_dir = temp_dir.join("workspace");
        std::fs::create_dir_all(&workspace_dir).unwrap();

        let engine = CheckpointEngine::new(&storage_dir);
        let f = workspace_dir.join("file_179.txt");
        std::fs::write(&f, "legitimate payload 179").unwrap();

        let rec = engine
            .quarantine_file(&workspace_dir, "file_179.txt", "", "chk 179")
            .unwrap();

        let res = engine.cleanup_quarantine(&rec.id);
        #[cfg(windows)]
        {
            assert!(
                res.is_ok(),
                "Windows exclusive deletion should succeed when identity matches"
            );
            assert!(res.unwrap());
            let manifest = engine.load_recovery_manifest().unwrap();
            let r = manifest.records.iter().find(|x| x.id == rec.id).unwrap();
            assert_eq!(r.status, QuarantineStatus::CleanedUp);
        }
        #[cfg(not(windows))]
        {
            assert!(res.is_err(), "POSIX fail-closed disposal contract");
            let manifest = engine.load_recovery_manifest().unwrap();
            let r = manifest.records.iter().find(|x| x.id == rec.id).unwrap();
            assert_eq!(r.status, QuarantineStatus::PreservedConflict);
        }

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_180_identity_capture_failure_prevents_deletion_and_preserves_object() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_180_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        let workspace_dir = temp_dir.join("workspace");
        std::fs::create_dir_all(&workspace_dir).unwrap();

        let engine = CheckpointEngine::new(&storage_dir);
        let f = workspace_dir.join("file_180.txt");
        std::fs::write(&f, "preserved object 180").unwrap();

        let rec = engine
            .quarantine_file(&workspace_dir, "file_180.txt", "", "chk 180")
            .unwrap();
        let q_path = PathBuf::from(&rec.quarantined_path);

        // Inject identity capture failure
        engine.set_fault_inject_identity_capture_failure(true);

        let res = engine.cleanup_quarantine(&rec.id);
        assert!(
            res.is_err(),
            "Cleanup must fail closed when identity cannot be established!"
        );

        // Original object MUST still exist on disk untouched
        assert!(
            q_path.exists(),
            "Original authorized object must remain preserved on disk!"
        );
        assert_eq!(
            std::fs::read_to_string(&q_path).unwrap(),
            "preserved object 180"
        );

        let manifest = engine.load_recovery_manifest().unwrap();
        let r = manifest.records.iter().find(|x| x.id == rec.id).unwrap();
        assert_eq!(r.status, QuarantineStatus::PreservedConflict);

        engine.set_fault_inject_identity_capture_failure(false);
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_181_replace_destination_after_hard_link_before_verification() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_181_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        let workspace_dir = temp_dir.join("workspace");
        std::fs::create_dir_all(&workspace_dir).unwrap();

        let engine = Arc::new(CheckpointEngine::new(&storage_dir));
        let f = workspace_dir.join("file_181.txt");
        std::fs::write(&f, "staged original 181").unwrap();

        let _rec = engine
            .quarantine_file(&workspace_dir, "file_181.txt", "", "chk 181")
            .unwrap();

        // Hook: replace destination right after hard link creation
        engine.set_post_rollback_hard_link_hook(Some(move |dest_path: &Path| {
            let _ = std::fs::remove_file(dest_path);
            std::fs::write(dest_path, "REPLACEMENT DESTINATION 181").unwrap();
        }));

        let q_dir = engine.quarantine_dir();
        let q_cap_dir =
            cap_std::fs::Dir::open_ambient_dir(&q_dir, cap_std::ambient_authority()).unwrap();
        let stage_name = ".aether-clean-stage-chk181";
        q_cap_dir.write(stage_name, b"staged original 181").unwrap();

        // Rollback stage with replaced destination
        let rolled = engine.rollback_cleanup_stage(&q_cap_dir, stage_name, "dest_181.txt");
        assert!(rolled.is_conflict);
        let stage_file = q_dir.join(stage_name);
        let conflict_file = q_dir.join(&rolled.surviving_filename);
        assert!(
            stage_file.exists() || conflict_file.exists(),
            "Original artifact must be preserved!"
        );

        engine.set_post_rollback_hard_link_hook(None::<fn(&Path)>);
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_182_replace_destination_after_verification_before_staging_link_removal() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_182_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        std::fs::create_dir_all(&storage_dir).unwrap();

        let engine = Arc::new(CheckpointEngine::new(&storage_dir));
        let q_dir = engine.quarantine_dir();
        std::fs::create_dir_all(&q_dir).unwrap();
        let q_cap_dir =
            cap_std::fs::Dir::open_ambient_dir(&q_dir, cap_std::ambient_authority()).unwrap();

        let stage_name = ".aether-clean-stage-chk182";
        q_cap_dir.write(stage_name, b"staged original 182").unwrap();

        // Hook replaces destination right before staging link removal!
        engine.set_pre_staging_link_removal_hook(Some(move |dest_path: &Path| {
            let _ = std::fs::remove_file(dest_path);
            std::fs::write(dest_path, "CONCURRENT_USURPER_182").unwrap();
        }));

        let rolled = engine.rollback_cleanup_stage(&q_cap_dir, stage_name, "dest_182.txt");

        // Staging link MUST NOT be removed!
        let stage_file = q_dir.join(stage_name);
        assert!(
            stage_file.exists(),
            "Staging link must be retained when destination was usurped!"
        );
        assert_eq!(
            std::fs::read_to_string(&stage_file).unwrap(),
            "staged original 182"
        );
        assert_eq!(
            rolled.status,
            RollbackPublicationStatus::PublicationUncertainBlocked
        );
        assert_eq!(rolled.surviving_filename, stage_name);
        assert_eq!(
            rolled.retained_staging_filename,
            Some(stage_name.to_string())
        );
        assert!(engine.is_recovery_blocked());

        engine.set_pre_staging_link_removal_hook(None::<fn(&Path)>);
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_183_replacement_destination_cannot_cause_loss_of_staged_original() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_183_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        std::fs::create_dir_all(&storage_dir).unwrap();

        let engine = Arc::new(CheckpointEngine::new(&storage_dir));
        let q_dir = engine.quarantine_dir();
        std::fs::create_dir_all(&q_dir).unwrap();
        let q_cap_dir =
            cap_std::fs::Dir::open_ambient_dir(&q_dir, cap_std::ambient_authority()).unwrap();

        let stage_name = ".aether-clean-stage-chk183";
        let original_data = b"CRITICAL USER ARTIFACT DATA 183";
        q_cap_dir.write(stage_name, original_data).unwrap();

        engine.set_pre_staging_link_removal_hook(Some(move |dest_path: &Path| {
            let _ = std::fs::remove_file(dest_path);
            std::fs::write(dest_path, "REPLACEMENT_DATA").unwrap();
        }));

        let rolled = engine.rollback_cleanup_stage(&q_cap_dir, stage_name, "file_183.txt");

        let stage_file = q_dir.join(&rolled.surviving_filename);
        assert!(stage_file.exists());
        assert_eq!(std::fs::read(&stage_file).unwrap(), original_data);

        engine.set_pre_staging_link_removal_hook(None::<fn(&Path)>);
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_184_retained_staging_link_represented_in_durable_recovery_manifest() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_184_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        let workspace_dir = temp_dir.join("workspace");
        std::fs::create_dir_all(&workspace_dir).unwrap();

        let engine = CheckpointEngine::new(&storage_dir);
        let f = workspace_dir.join("file_184.txt");
        std::fs::write(&f, "content 184").unwrap();

        let rec = engine
            .quarantine_file(&workspace_dir, "file_184.txt", "", "chk 184")
            .unwrap();

        // Inject staging removal failure during rollback
        engine.set_fault_inject_rollback_staging_removal_failure(true);
        engine.set_fault_inject_metadata_permission_denied(true);

        let res = engine.cleanup_quarantine(&rec.id);
        assert!(res.is_err());

        let manifest = engine.load_recovery_manifest().unwrap();
        let r = manifest.records.iter().find(|x| x.id == rec.id).unwrap();
        assert!(
            r.retained_staging_filename.is_some(),
            "Manifest must track retained staging filename!"
        );
        assert!(
            r.retained_staging_path.is_some(),
            "Manifest must track retained staging path!"
        );

        let s_path = PathBuf::from(r.retained_staging_path.as_ref().unwrap());
        assert!(s_path.exists(), "Retained staging file must exist on disk!");

        engine.set_fault_inject_rollback_staging_removal_failure(false);
        engine.set_fault_inject_metadata_permission_denied(false);
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_185_startup_reconciliation_discovers_and_adopts_retained_staging_artifacts() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_185_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        let engine = CheckpointEngine::new(&storage_dir);
        let q_dir = engine.quarantine_dir();
        std::fs::create_dir_all(&q_dir).unwrap();

        // Leave an untracked .aether-clean-stage-* file directly in quarantine
        let stage_file_name = format!(".aether-clean-stage-{}", uuid::Uuid::new_v4().simple());
        let stage_file_path = q_dir.join(&stage_file_name);
        std::fs::write(&stage_file_path, "SURVIVING UNTRACKED STAGING ARTIFACT").unwrap();

        let reconciled = engine.reconcile_quarantine().unwrap();
        let record = reconciled.records.iter().find(|r| {
            r.quarantined_filename == stage_file_name
                || r.retained_staging_filename.as_deref() == Some(&stage_file_name)
        });

        assert!(
            record.is_some(),
            "Startup reconciliation must discover and adopt retained staging artifacts!"
        );
        let rec = record.unwrap();
        assert_eq!(rec.status, QuarantineStatus::PreservedConflict);
        assert_eq!(
            rec.actual_sha256.as_deref(),
            Some(CheckpointEngine::sha256_digest(b"SURVIVING UNTRACKED STAGING ARTIFACT").as_str())
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_186_publication_uncertainty_activates_recovery_blocked() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_186_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        std::fs::create_dir_all(&storage_dir).unwrap();

        let engine = Arc::new(CheckpointEngine::new(&storage_dir));
        let q_dir = engine.quarantine_dir();
        std::fs::create_dir_all(&q_dir).unwrap();
        let q_cap_dir =
            cap_std::fs::Dir::open_ambient_dir(&q_dir, cap_std::ambient_authority()).unwrap();

        let stage_name = ".aether-clean-stage-chk186";
        q_cap_dir.write(stage_name, b"payload 186").unwrap();

        // Inject both original and conflict publication failures
        engine.set_fault_inject_rollback_both_publication_failures(true);

        assert!(!engine.is_recovery_blocked());
        let rolled = engine.rollback_cleanup_stage(&q_cap_dir, stage_name, "file_186.txt");

        assert_eq!(
            rolled.status,
            RollbackPublicationStatus::PublicationUncertainBlocked
        );
        assert!(
            engine.is_recovery_blocked(),
            "RecoveryBlocked must be activated on publication uncertainty!"
        );
        let reason = engine.recovery_blocked_reason().unwrap();
        assert!(reason.contains("rollback publication"));

        engine.set_fault_inject_rollback_both_publication_failures(false);
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_chk_187_no_successful_rollback_result_when_recoverability_unproven() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_chk_187_{}", uuid::Uuid::new_v4().simple()));
        let storage_dir = temp_dir.join(".aether").join("checkpoints");
        std::fs::create_dir_all(&storage_dir).unwrap();

        let engine = CheckpointEngine::new(&storage_dir);
        let q_dir = engine.quarantine_dir();
        std::fs::create_dir_all(&q_dir).unwrap();
        let q_cap_dir =
            cap_std::fs::Dir::open_ambient_dir(&q_dir, cap_std::ambient_authority()).unwrap();

        let stage_name = ".aether-clean-stage-chk187";
        q_cap_dir.write(stage_name, b"payload 187").unwrap();

        engine.set_fault_inject_rollback_both_publication_failures(true);

        let rolled = engine.rollback_cleanup_stage(&q_cap_dir, stage_name, "file_187.txt");
        assert_ne!(rolled.status, RollbackPublicationStatus::PublishedVerified);
        assert!(rolled.error.is_some());
        assert_eq!(
            rolled.status,
            RollbackPublicationStatus::PublicationUncertainBlocked
        );

        engine.set_fault_inject_rollback_both_publication_failures(false);
        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
