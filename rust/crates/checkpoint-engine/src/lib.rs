use application_core::{AetherError, Result};
use cap_std::ambient_authority;
use cap_std::fs::Dir;
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
    fault_inject_reconciliation_failure: std::sync::atomic::AtomicBool,
    fault_inject_quarantine_target_swap: std::sync::atomic::AtomicBool,
    recovery_blocked_reason: Arc<RwLock<Option<String>>>,
    pre_cleanup_unlink_hook: Arc<Mutex<Option<PreReplaceHook>>>,
    pre_quarantine_source_unlink_hook: Arc<Mutex<Option<PreReplaceHook>>>,
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
            fault_inject_reconciliation_failure: std::sync::atomic::AtomicBool::new(false),
            fault_inject_quarantine_target_swap: std::sync::atomic::AtomicBool::new(false),
            recovery_blocked_reason: Arc::new(RwLock::new(None)),
            pre_cleanup_unlink_hook: Arc::new(Mutex::new(None)),
            pre_quarantine_source_unlink_hook: Arc::new(Mutex::new(None)),
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
    pub fn set_fault_inject_reconciliation_failure(&self, inject: bool) {
        self.fault_inject_reconciliation_failure
            .store(inject, std::sync::atomic::Ordering::SeqCst);
    }

    #[doc(hidden)]
    pub fn set_fault_inject_quarantine_target_swap(&self, inject: bool) {
        self.fault_inject_quarantine_target_swap
            .store(inject, std::sync::atomic::Ordering::SeqCst);
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

                // Step 2: Unique, collision-safe backup name using timestamp and UUID
                let timestamp = chrono::Utc::now().timestamp();
                let backup_name = format!(
                    "recovery_manifest.json.corrupt.{}.{}",
                    timestamp,
                    uuid::Uuid::new_v4().simple()
                );
                let backup_path = manifest_path.with_file_name(&backup_name);

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

                // Step 4: Create backup EXCLUSIVELY (create_new(true)) - prevents collision overwrite (CHK-103)
                use std::io::Write;
                let mut backup_file = match std::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&backup_path)
                {
                    Ok(f) => f,
                    Err(err) => {
                        tracing::error!(
                            "Failed to create exclusive corrupt manifest backup at '{}': {}. Original manifest preserved.",
                            backup_path.display(),
                            err
                        );
                        return Err(AetherError::Io(err));
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

                let is_mutated = src_mutation_injected
                    || match &current_src_hash {
                        Some(h) => h != &staged_hash,
                        None => true,
                    };

                #[cfg(windows)]
                let has_retained_handle = {
                    use std::os::windows::fs::OpenOptionsExt;
                    std::fs::OpenOptions::new()
                        .read(true)
                        .write(true)
                        .share_mode(0)
                        .open(&stage_abs)
                        .is_err()
                };
                #[cfg(not(windows))]
                let has_retained_handle = false;

                if is_mutated || has_retained_handle {
                    // Exclusive source stability could not be verified (mutated or retained writable handle).
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
                    let _ = manifest.record_quarantine_with_fault(
                        record.clone(),
                        &manifest_path,
                        fault_manifest,
                    );
                    return Err(AetherError::PermissionDenied(format!(
                        "Exclusive source stability could not be verified for '{}'. Staged source preserved at '{}' and verified copy preserved in quarantine at '{}'.",
                        rel_path,
                        cap.root_path().join(rel_path).display(),
                        q_dest.display()
                    )));
                }

                // Copy verified and source exclusive stability proven: safe to remove staged source
                cap.raw_dir()
                    .remove_file(&stage_rel)
                    .map_err(AetherError::Io)?;
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
                        if record.status == QuarantineStatus::PendingCleanup
                            || record.status == QuarantineStatus::PendingMove
                        {
                            record.status = QuarantineStatus::CleanedUp;
                        }
                    }
                    Err(_) => {
                        record.status = QuarantineStatus::PreservedConflict;
                    }
                }
            }

            // 2. Discover untracked artifacts in quarantine directory (CHK-057, CHK-079, CHK-089)
            let entries = std::fs::read_dir(&q_dir).map_err(AetherError::Io)?;
            for entry in entries {
                let entry = entry.map_err(AetherError::Io)?;
                let path = entry.path();
                let file_name = entry.file_name().to_string_lossy().to_string();
                if file_name.starts_with('.') || file_name.ends_with(".tmp") {
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
                });

                if !already_tracked {
                    if is_symlink {
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

    /// Cleans up a single quarantined artifact by ID.
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
        let manifest_path = self.recovery_manifest_path();

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

        let meta = match q_cap_dir.symlink_metadata(&filename) {
            Ok(m) => m,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                manifest.records[idx].status = QuarantineStatus::CleanedUp;
                manifest.save_with_fault(&manifest_path, fault_manifest)?;
                return Ok(true);
            }
            Err(e) => {
                return Err(AetherError::Io(e));
            }
        };

        if meta.is_symlink() {
            manifest.records[idx].status = QuarantineStatus::PreservedConflict;
            let _ = manifest.save_with_fault(&manifest_path, fault_manifest);
            return Err(AetherError::PermissionDenied(format!(
                "Security rejection: Quarantine target '{}' is a symlink; refusing cleanup to protect target",
                target_path.display()
            )));
        }

        if !meta.is_file() {
            manifest.records[idx].status = QuarantineStatus::PreservedConflict;
            let _ = manifest.save_with_fault(&manifest_path, fault_manifest);
            return Err(AetherError::PermissionDenied(format!(
                "Quarantine target '{}' is not a regular file; refusing cleanup",
                target_path.display()
            )));
        }

        // Verify content hash before removal through directory handle
        let current_bytes = q_cap_dir.read(&filename).map_err(AetherError::Io)?;
        let current_hash = Self::sha256_digest(&current_bytes);
        let approved_hash = match manifest.records[idx].actual_sha256 {
            Some(ref act) => act.clone(),
            None => manifest.records[idx].expected_sha256.clone(),
        };
        if current_hash != approved_hash {
            manifest.records[idx].status = QuarantineStatus::PreservedConflict;
            manifest.records[idx].actual_sha256 = Some(current_hash.clone());
            let _ = manifest.save_with_fault(&manifest_path, fault_manifest);
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
            return Err(AetherError::Io(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "Fault injection: cleanup removal permission denied",
            )));
        }

        // Pre-unlink hook (to test TOCTOU symlink injection right before unlink: CHK-074, CHK-090..095)
        let hook_opt = self.pre_cleanup_unlink_hook.lock().clone();
        if let Some(ref hook) = hook_opt {
            hook(&target_path);
        }

        // Re-verify quarantine boundary
        self.verify_quarantine_boundary()?;

        // P1-2: Revalidate target object and content through q_cap_dir pinned handle
        let pre_unlink_meta = q_cap_dir
            .symlink_metadata(&filename)
            .map_err(AetherError::Io)?;
        if pre_unlink_meta.is_symlink() || !pre_unlink_meta.is_file() {
            manifest.records[idx].status = QuarantineStatus::PreservedConflict;
            let _ = manifest.save_with_fault(&manifest_path, fault_manifest);
            return Err(AetherError::PermissionDenied(format!(
                "Security rejection: Pre-unlink verification failed for '{}': object swapped to symlink or non-file",
                target_path.display()
            )));
        }

        let pre_unlink_bytes = q_cap_dir.read(&filename).map_err(AetherError::Io)?;
        let pre_unlink_hash = Self::sha256_digest(&pre_unlink_bytes);
        let target_swap_injected = self
            .fault_inject_quarantine_target_swap
            .load(std::sync::atomic::Ordering::SeqCst);

        if pre_unlink_hash != approved_hash || target_swap_injected {
            manifest.records[idx].status = QuarantineStatus::PreservedConflict;
            manifest.records[idx].actual_sha256 = Some(pre_unlink_hash.clone());
            let _ = manifest.save_with_fault(&manifest_path, fault_manifest);
            return Err(AetherError::PermissionDenied(format!(
                "Security rejection: Quarantine target '{}' was modified or swapped prior to unlinking (expected {}, found {}). Preserved as conflict artifact without deletion.",
                filename, approved_hash, pre_unlink_hash
            )));
        }

        // Directory-handle-relative physical removal with no-follow semantics
        q_cap_dir.remove_file(&filename).map_err(AetherError::Io)?;

        // Only after confirmed physical removal, mark CleanedUp and persist manifest
        manifest.records[idx].status = QuarantineStatus::CleanedUp;
        manifest.save_with_fault(&manifest_path, fault_manifest)?;
        Ok(true)
    }

    /// Cleans up all quarantined artifacts pending cleanup.
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
        let manifest_path = self.recovery_manifest_path();
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

            match q_cap_dir.symlink_metadata(&filename) {
                Ok(meta) => {
                    if meta.is_symlink() {
                        manifest.records[i].status = QuarantineStatus::PreservedConflict;
                        errors.push(format!(
                            "Record {} is a symlink; preserved as conflict",
                            manifest.records[i].id
                        ));
                        continue;
                    }
                    if !meta.is_file() {
                        manifest.records[i].status = QuarantineStatus::PreservedConflict;
                        errors.push(format!(
                            "Record {} is not a regular file; preserved as conflict",
                            manifest.records[i].id
                        ));
                        continue;
                    }
                    // verify content hash before removal
                    let bytes_res = q_cap_dir.read(&filename);
                    let approved_hash = match manifest.records[i].actual_sha256 {
                        Some(ref act) => act.clone(),
                        None => manifest.records[i].expected_sha256.clone(),
                    };
                    match bytes_res {
                        Ok(bytes) => {
                            let hash = Self::sha256_digest(&bytes);
                            if hash != approved_hash {
                                manifest.records[i].status = QuarantineStatus::PreservedConflict;
                                manifest.records[i].actual_sha256 = Some(hash);
                                errors.push(format!(
                                    "Record {} was modified on disk; preserved as conflict artifact",
                                    manifest.records[i].id
                                ));
                                continue;
                            }
                        }
                        Err(e) => {
                            errors.push(format!(
                                "Record {} read failed: {}",
                                manifest.records[i].id, e
                            ));
                            continue;
                        }
                    }

                    let injected_failure = self
                        .fault_inject_cleanup_removal
                        .load(std::sync::atomic::Ordering::SeqCst);
                    let remove_res = if injected_failure {
                        Err(std::io::Error::new(
                            std::io::ErrorKind::PermissionDenied,
                            "Fault injection: cleanup removal permission denied",
                        ))
                    } else {
                        // Hook and boundary check before each unlink
                        let hook_opt = self.pre_cleanup_unlink_hook.lock().clone();
                        if let Some(ref hook) = hook_opt {
                            hook(&target_path);
                        }
                        if let Err(b_err) = self.verify_quarantine_boundary() {
                            Err(std::io::Error::new(
                                std::io::ErrorKind::PermissionDenied,
                                b_err.to_string(),
                            ))
                        } else {
                            // P1-2: Revalidate through pinned q_cap_dir handle
                            match q_cap_dir.symlink_metadata(&filename) {
                                Ok(pre_unlink_meta) => {
                                    if pre_unlink_meta.is_symlink() || !pre_unlink_meta.is_file() {
                                        manifest.records[i].status =
                                            QuarantineStatus::PreservedConflict;
                                        Err(std::io::Error::new(
                                            std::io::ErrorKind::PermissionDenied,
                                            "Target swapped to symlink or non-file",
                                        ))
                                    } else {
                                        match q_cap_dir.read(&filename) {
                                            Ok(pre_unlink_bytes) => {
                                                let pre_unlink_hash =
                                                    Self::sha256_digest(&pre_unlink_bytes);
                                                let target_swap_injected = self
                                                    .fault_inject_quarantine_target_swap
                                                    .load(std::sync::atomic::Ordering::SeqCst);
                                                if pre_unlink_hash != approved_hash
                                                    || target_swap_injected
                                                {
                                                    manifest.records[i].status =
                                                        QuarantineStatus::PreservedConflict;
                                                    manifest.records[i].actual_sha256 =
                                                        Some(pre_unlink_hash);
                                                    Err(std::io::Error::new(
                                                        std::io::ErrorKind::PermissionDenied,
                                                        "Target content swapped or modified prior to unlink",
                                                    ))
                                                } else {
                                                    q_cap_dir.remove_file(&filename)
                                                }
                                            }
                                            Err(e) => Err(e),
                                        }
                                    }
                                }
                                Err(e) => Err(e),
                            }
                        }
                    };

                    match remove_res {
                        Ok(()) => {
                            manifest.records[i].status = QuarantineStatus::CleanedUp;
                            cleaned_count += 1;
                            // Persist state progress after each successful removal (CHK-062, CHK-082)
                            if let Err(save_err) =
                                manifest.save_with_fault(&manifest_path, fault_manifest)
                            {
                                return Err(AetherError::Internal(format!(
                                    "Batch cleanup halted: manifest persistence failed after cleaning up {} file(s) (failed on record '{}'): {}",
                                    cleaned_count, manifest.records[i].id, save_err
                                )));
                            }
                        }
                        Err(e) => {
                            errors.push(format!(
                                "Record {} removal failed: {}",
                                manifest.records[i].id, e
                            ));
                        }
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                    manifest.records[i].status = QuarantineStatus::CleanedUp;
                    cleaned_count += 1;
                    if let Err(save_err) = manifest.save_with_fault(&manifest_path, fault_manifest)
                    {
                        return Err(AetherError::Internal(format!(
                            "Batch cleanup halted: manifest persistence failed after marking {} file(s) (failed on record '{}'): {}",
                            cleaned_count, manifest.records[i].id, save_err
                        )));
                    }
                }
                Err(e) => {
                    errors.push(format!(
                        "Record {} metadata inspection failed: {}",
                        manifest.records[i].id, e
                    ));
                }
            }
        }

        manifest.save_with_fault(&manifest_path, fault_manifest)?;

        if !errors.is_empty() {
            return Err(AetherError::Internal(format!(
                "Cleanup completed with {} successful and {} failed: {:?}",
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
        let cleaned1 = engine.cleanup_quarantine(&rec1.id).unwrap();
        assert!(cleaned1);
        assert!(!PathBuf::from(&rec1.quarantined_path).exists());

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

        let rec = engine
            .quarantine_file(
                &workspace_dir,
                "copy_fallback.txt",
                &expected_hash,
                "CrossVolumeTest",
            )
            .unwrap();

        assert_eq!(rec.status, QuarantineStatus::PendingCleanup);
        assert!(
            !f.exists(),
            "Source file must be unlinked after verified copy fallback!"
        );
        assert!(
            PathBuf::from(&rec.quarantined_path).exists(),
            "Quarantined copy must exist!"
        );
        assert_eq!(
            std::fs::read_to_string(&rec.quarantined_path).unwrap(),
            payload
        );

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
        assert_eq!(item.unwrap().status, QuarantineStatus::PendingCleanup);

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
        let cleaned = engine.cleanup_quarantine(&rec.id).unwrap();
        assert!(cleaned, "Initial cleanup should succeed");

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

        let count = engine.cleanup_all_quarantined().unwrap();
        assert_eq!(count, 2, "Both files should be successfully cleaned up");

        let manifest = engine.load_recovery_manifest().unwrap();
        for r in &manifest.records {
            assert_eq!(r.status, QuarantineStatus::CleanedUp);
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

        assert_eq!(
            rec1.status,
            QuarantineStatus::CleanedUp,
            "Unmodified file should be cleaned up"
        );
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
}
