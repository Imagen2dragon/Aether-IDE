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
    let ws_arc = app.state::<SharedWorkspace>();
    let ws = ws_arc.lock();
    let roots_to_scan = if let Some(p) = path {
        vec![ws.validate_process_directory(&PathBuf::from(p))?]
    } else {
        let roots = ws.get_roots();
        if roots.is_empty() {
            return Err("No workspace root currently open to scan".to_string());
        }
        roots
    };
    drop(ws);

    let brain_arc = app.state::<SharedBrain>();
    let mut brain = brain_arc.lock();

    let mut combined_report = BrainReconcileReport::default();
    combined_report.is_complete = true;

    for root_path in roots_to_scan {
        match brain.reconcile_workspace(&root_path) {
            Ok(rep) => {
                combined_report.nodes_total = brain.get_graph().nodes.len();
                combined_report.edges_total = brain.get_graph().edges.len();
                combined_report.nodes_added += rep.nodes_added;
                combined_report.nodes_removed += rep.nodes_removed;
                combined_report.errors.extend(rep.errors);
            }
            Err(e) => {
                combined_report
                    .errors
                    .push(format!("Scan failed for {:?}: {}", root_path, e));
                combined_report.is_complete = false;
            }
        }
    }

    // Emit live event to frontend
    let _ = app.emit("brain-graph-updated", json!({ "report": &combined_report }));

    Ok(combined_report)
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
    let ws_arc = app.state::<SharedWorkspace>();
    let ws = ws_arc.lock();
    let (_, _, rel_path) = ws.resolve_in_workspace(&target_file)?;
    drop(ws);

    let brain_arc = app.state::<SharedBrain>();
    let brain = brain_arc.lock();
    Ok(brain.calculate_impact_radar(&rel_path))
}
