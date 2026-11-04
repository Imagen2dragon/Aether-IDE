use crate::workspace_bridge::SharedWorkspace;
use checkpoint_engine::{
    CheckpointEngine, CheckpointRestorePreview, CheckpointRestoreReport, WorkspaceCheckpoint,
};
use parking_lot::Mutex;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::{AppHandle, Manager};

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

    // 1. Atomically validate and consume the preview token generated during preview review
    let session = engine
        .consume_preview_token(&preview_token, &checkpoint_id, &ws_id, &root)
        .map_err(|e| format!("Preview token validation failed: {}", e))?;

    // 2. Mandatory safety snapshot: MUST SUCCEED or abort restore completely (CHK-013)
    let backup_ckpt = engine
        .create_checkpoint_with_root(
            &format!("Pre-restore safety backup for {}", checkpoint_id),
            &ws_id,
            &root,
            None,
        )
        .map_err(|e| {
            format!(
                "Mandatory pre-restore safety backup failed: {}. Restoration aborted to protect existing workspace.",
                e
            )
        })?;
    tracing::info!(
        "Mandatory safety snapshot created prior to restore: {}",
        backup_ckpt.checkpoint_id
    );

    let allow_overwrite = if force.unwrap_or(false) {
        session.preview.conflicts.clone()
    } else {
        approved_conflicts.unwrap_or_default()
    };

    engine
        .restore_checkpoint_with_token(&checkpoint_id, &root, &session.preview, &allow_overwrite)
        .map_err(|e| format!("Failed to restore checkpoint: {}", e))
}
