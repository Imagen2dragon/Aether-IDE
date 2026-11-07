use crate::workspace_bridge::SharedWorkspace;
use checkpoint_engine::{
    CheckpointEngine, CheckpointRestorePreview, CheckpointRestoreReport, WorkspaceCheckpoint,
};
use parking_lot::Mutex;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::{AppHandle, Manager};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RecoveryStatus {
    pub is_blocked: bool,
    pub blocked_reason: Option<String>,
}

pub type SharedCheckpointEngine = Arc<Mutex<CheckpointEngine>>;

#[tauri::command]
pub async fn checkpoint_create(
    app: AppHandle,
    description: String,
    files: Option<Vec<String>>,
) -> Result<WorkspaceCheckpoint, String> {
    let ws_arc = app.state::<SharedWorkspace>();
    let ws = ws_arc.lock();
    let root = ws
        .primary_root()
        .ok_or_else(|| "No active workspace root open to snapshot".to_string())?;
    let ws_id = ws.workspace_id();
    drop(ws);

    let explicit_paths: Option<Vec<PathBuf>> = files.map(|flist| {
        flist
            .into_iter()
            .map(|f| {
                let p = PathBuf::from(&f);
                if p.is_absolute() {
                    p
                } else {
                    root.join(p)
                }
            })
            .collect()
    });

    let engine_arc = app.state::<SharedCheckpointEngine>();
    let engine = engine_arc.lock();

    engine
        .create_checkpoint_with_root(&description, &ws_id, &root, explicit_paths.as_deref())
        .map_err(|e| format!("Failed to create checkpoint: {}", e))
}

#[tauri::command]
pub async fn checkpoint_list(app: AppHandle) -> Result<Vec<WorkspaceCheckpoint>, String> {
    let engine_arc = app.state::<SharedCheckpointEngine>();
    let engine = engine_arc.lock();

    engine
        .list_checkpoints()
        .map_err(|e| format!("Failed to list checkpoints: {}", e))
}

#[tauri::command]
pub async fn checkpoint_restore_preview(
    app: AppHandle,
    checkpoint_id: String,
) -> Result<CheckpointRestorePreview, String> {
    let ws_arc = app.state::<SharedWorkspace>();
    let ws = ws_arc.lock();
    let root = ws
        .primary_root()
        .ok_or_else(|| "No active workspace root open to preview restore".to_string())?;
    let ws_id = ws.workspace_id();
    drop(ws);

    let engine_arc = app.state::<SharedCheckpointEngine>();
    let engine = engine_arc.lock();

    engine
        .restore_preview_bound(&checkpoint_id, &ws_id, &root)
        .map_err(|e| format!("Failed to preview restore: {}", e))
}

#[tauri::command]
pub async fn checkpoint_restore(
    app: AppHandle,
    checkpoint_id: String,
    preview_token: String,
    force: Option<bool>,
    approved_conflicts: Option<Vec<String>>,
) -> Result<CheckpointRestoreReport, String> {
    let ws_arc = app.state::<SharedWorkspace>();
    let ws = ws_arc.lock();
    let root = ws
        .primary_root()
        .ok_or_else(|| "No active workspace root open to restore".to_string())?;
    let ws_id = ws.workspace_id();
    drop(ws);

    let engine_arc = app.state::<SharedCheckpointEngine>();
    let engine = engine_arc.lock();

    let allow_overwrite = approved_conflicts.unwrap_or_default();
    let force_flag = force.unwrap_or(false);

    engine
        .restore_checkpoint_with_token(
            &checkpoint_id,
            &ws_id,
            &root,
            &preview_token,
            force_flag,
            &allow_overwrite,
        )
        .map_err(|e| format!("Failed to restore checkpoint: {}", e))
}

#[tauri::command]
pub async fn checkpoint_list_quarantine(
    app: AppHandle,
) -> Result<Vec<checkpoint_engine::QuarantineRecord>, String> {
    let engine_arc = app.state::<SharedCheckpointEngine>();
    let engine = engine_arc.lock();
    if let Some(reason) = engine.recovery_blocked_reason() {
        return Err(format!(
            "Quarantine operations blocked due to reconciliation failure: {}",
            reason
        ));
    }
    let manifest = engine
        .load_recovery_manifest()
        .map_err(|e| format!("Failed to load recovery manifest: {}", e))?;
    Ok(manifest.records)
}

#[tauri::command]
pub async fn checkpoint_cleanup_quarantine(app: AppHandle, id: String) -> Result<bool, String> {
    let engine_arc = app.state::<SharedCheckpointEngine>();
    let engine = engine_arc.lock();
    engine
        .cleanup_quarantine(&id)
        .map_err(|e| format!("Failed to cleanup quarantine record: {}", e))
}

#[tauri::command]
pub async fn checkpoint_reconcile_quarantine(
    app: AppHandle,
) -> Result<checkpoint_engine::RecoveryManifest, String> {
    let engine_arc = app.state::<SharedCheckpointEngine>();
    let engine = engine_arc.lock();
    engine
        .reconcile_quarantine()
        .map_err(|e| format!("Failed to reconcile quarantine: {}", e))
}

#[tauri::command]
pub async fn checkpoint_get_recovery_status(app: AppHandle) -> Result<RecoveryStatus, String> {
    let engine_arc = app.state::<SharedCheckpointEngine>();
    let engine = engine_arc.lock();
    Ok(RecoveryStatus {
        is_blocked: engine.is_recovery_blocked(),
        blocked_reason: engine.recovery_blocked_reason(),
    })
}
