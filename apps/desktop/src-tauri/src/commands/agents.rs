use crate::commands::checkpoint::SharedCheckpointEngine;
use crate::workspace_bridge::SharedWorkspace;
use agent_orchestrator::{AgentOrchestrator, AgentPersona};
use agent_runtime::AgentTask;
use application_core::AutonomyMode;
use serde_json::json;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager};

pub type SharedOrchestrator = Arc<AgentOrchestrator>;

#[tauri::command]
pub async fn agent_list_personas(app: AppHandle) -> Result<Vec<AgentPersona>, String> {
    let orch = app.state::<SharedOrchestrator>();
    Ok(orch.list_personas())
}

#[tauri::command]
pub async fn agent_spawn_task(
    app: AppHandle,
    role: String,
    objective: String,
    autonomy_mode: Option<String>,
    workspace_root: Option<String>,
) -> Result<String, String> {
    let ws_arc = app.state::<SharedWorkspace>();
    let ws = ws_arc.lock();
    let root = if let Some(p) = workspace_root {
        ws.validate_process_directory(&PathBuf::from(p))?
    } else {
        ws.primary_root()
            .ok_or_else(|| "No active workspace root open for agent execution".to_string())?
    };
    let ws_id = ws.workspace_id();
    drop(ws);

    // Create safety recovery checkpoint for workspace before task begins
    let ckpt_engine_arc = app.state::<SharedCheckpointEngine>();
    let ckpt_id = {
        let ckpt_engine = ckpt_engine_arc.lock();
        ckpt_engine
            .create_checkpoint_with_root(
                &format!("Pre-agent task snapshot ({})", role),
                &ws_id,
                &root,
                None,
            )
            .ok()
            .map(|c| c.checkpoint_id)
    };

    let mode = match autonomy_mode.as_deref() {
        Some("autopilot") | Some("autonomous") => AutonomyMode::Autopilot,
        Some("observe") => AutonomyMode::Observe,
        _ => AutonomyMode::Assist,
    };

    let orch = app.state::<SharedOrchestrator>();
    let task_id = orch.spawn_task_with_worktree(&role, &objective, mode, Some(root));

    // Emit true initial lifecycle state: Pending
    let _ = app.emit(
        "agent-task-updated",
        json!({
            "task_id": &task_id,
            "role": &role,
            "status": "Pending",
            "checkpoint_id": ckpt_id,
            "message": "Task created with explicit workspace context"
        }),
    );

    Ok(task_id)
}

#[tauri::command]
pub async fn agent_get_task(app: AppHandle, task_id: String) -> Result<AgentTask, String> {
    let orch = app.state::<SharedOrchestrator>();
    orch.get_task(&task_id)
        .ok_or_else(|| format!("Task {} not found", task_id))
}

#[tauri::command]
pub async fn agent_execute_step(app: AppHandle, task_id: String) -> Result<AgentTask, String> {
    let orch = app.state::<SharedOrchestrator>();
    let updated = orch
        .step_task(&task_id)
        .await
        .map_err(|e| format!("Agent execution step failed: {}", e))?;

    let _ = app.emit(
        "agent-task-updated",
        json!({
            "task_id": &task_id,
            "status": format!("{:?}", updated.status),
            "step_count": updated.steps.len(),
            "last_step": updated.steps.last().map(|s| &s.tool)
        }),
    );

    Ok(updated)
}

#[tauri::command]
pub async fn agent_cancel_task(app: AppHandle, task_id: String) -> Result<AgentTask, String> {
    let orch = app.state::<SharedOrchestrator>();
    let cancelled = orch
        .cancel_task(&task_id)
        .map_err(|e| format!("Failed to cancel task: {}", e))?;

    let _ = app.emit(
        "agent-task-updated",
        json!({
            "task_id": &task_id,
            "status": "Cancelled",
            "message": "Task cancelled by user"
        }),
    );

    Ok(cancelled)
}
