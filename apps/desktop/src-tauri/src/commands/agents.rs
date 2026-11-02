use agent_orchestrator::{AgentOrchestrator, AgentPersona};
use agent_runtime::AgentTask;
use application_core::AutonomyMode;
use serde_json::json;
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
) -> Result<String, String> {
    let mode = match autonomy_mode.as_deref() {
        Some("autopilot") | Some("autonomous") => AutonomyMode::Autopilot,
        Some("observe") => AutonomyMode::Observe,
        _ => AutonomyMode::Assist,
    };

    let orch = app.state::<SharedOrchestrator>();
    let task_id = orch.spawn_task(&role, &objective, mode);

    let _ = app.emit(
        "agent-task-updated",
        json!({
            "task_id": &task_id,
            "role": &role,
            "status": "Running",
            "message": "Task queued and initialized"
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
    orch.cancel_task(&task_id)
        .map_err(|e| format!("Failed to cancel task: {}", e))
}
