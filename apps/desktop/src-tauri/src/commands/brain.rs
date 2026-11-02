use crate::workspace_bridge::SharedWorkspace;
use parking_lot::Mutex;
use project_brain::{BrainReconcileReport, ImpactReport, ProjectBrain, ProjectBrainGraph};
use serde_json::json;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager};

pub type SharedBrain = Arc<Mutex<ProjectBrain>>;

#[tauri::command]
pub async fn brain_scan_workspace(
    app: AppHandle,
    path: Option<String>,
) -> Result<BrainReconcileReport, String> {
    let root_path = if let Some(p) = path {
        PathBuf::from(p)
    } else {
        let ws_arc = app.state::<SharedWorkspace>();
        let ws = ws_arc.lock();
        let roots = ws.get_roots();
        roots
            .into_iter()
            .next()
            .ok_or_else(|| "No workspace root currently open to scan".to_string())?
    };

    let report = {
        let brain_arc = app.state::<SharedBrain>();
        let mut brain = brain_arc.lock();
        brain
            .reconcile_workspace(&root_path)
            .map_err(|e| format!("Brain reconciliation failed: {}", e))?
    };

    // Emit live event to frontend
    let _ = app.emit("brain-graph-updated", json!({ "report": &report }));

    Ok(report)
}

#[tauri::command]
pub async fn brain_get_graph(app: AppHandle) -> Result<ProjectBrainGraph, String> {
    let brain_arc = app.state::<SharedBrain>();
    let brain = brain_arc.lock();
    Ok(brain.get_graph().clone())
}

#[tauri::command]
pub async fn brain_calculate_impact(
    app: AppHandle,
    target_file: String,
) -> Result<ImpactReport, String> {
    let brain_arc = app.state::<SharedBrain>();
    let brain = brain_arc.lock();
    Ok(brain.calculate_impact_radar(&target_file))
}
