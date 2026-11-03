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
        .create_checkpoint_with_root(
            &description,
            &ws_id,
            &root,
            explicit_paths.as_deref(),
        )
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
    drop(ws);

    let engine_arc = app.state::<SharedCheckpointEngine>();
    let engine = engine_arc.lock();

    engine
        .restore_preview(&checkpoint_id, &root)
        .map_err(|e| format!("Failed to preview restore: {}", e))
}

#[tauri::command]
pub async fn checkpoint_restore(
    app: AppHandle,
    checkpoint_id: String,
) -> Result<CheckpointRestoreReport, String> {
    let ws_arc = app.state::<SharedWorkspace>();
    let ws = ws_arc.lock();
    let root = ws
        .primary_root()
        .ok_or_else(|| "No active workspace root open to restore".to_string())?;
    drop(ws);

    let engine_arc = app.state::<SharedCheckpointEngine>();
    let engine = engine_arc.lock();

    engine
        .restore_checkpoint_into(&checkpoint_id, &root, false)
        .map_err(|e| format!("Failed to restore checkpoint: {}", e))
}
