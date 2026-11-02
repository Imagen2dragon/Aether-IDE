use crate::workspace_bridge::SharedWorkspace;
use checkpoint_engine::{CheckpointEngine, WorkspaceCheckpoint};
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
    let files_to_snapshot = if let Some(flist) = files {
        flist.into_iter().map(PathBuf::from).collect()
    } else {
        // Collect files from active workspace
        let ws_arc = app.state::<SharedWorkspace>();
        let ws = ws_arc.lock();
        let roots = ws.get_roots();
        let mut paths = Vec::new();
        for root in roots {
            if let Ok(entries) = std::fs::read_dir(&root) {
                for entry in entries.flatten() {
                    let p = entry.path();
                    if p.is_file() {
                        paths.push(p);
                    }
                }
            }
        }
        paths
    };

    let engine_arc = app.state::<SharedCheckpointEngine>();
    let engine = engine_arc.lock();

    engine
        .create_checkpoint(&description, &files_to_snapshot)
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
pub async fn checkpoint_restore(app: AppHandle, checkpoint_id: String) -> Result<bool, String> {
    let engine_arc = app.state::<SharedCheckpointEngine>();
    let engine = engine_arc.lock();

    let list = engine
        .list_checkpoints()
        .map_err(|e| format!("Failed to list checkpoints: {}", e))?;

    let ckpt = list
        .into_iter()
        .find(|c| c.checkpoint_id == checkpoint_id)
        .ok_or_else(|| format!("Checkpoint '{}' not found", checkpoint_id))?;

    engine
        .restore_checkpoint(&ckpt)
        .map_err(|e| format!("Failed to restore checkpoint: {}", e))?;

    Ok(true)
}
