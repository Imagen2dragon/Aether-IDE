use crate::commands::terminal::*;
use crate::commands::workspace::*;
use serde_json::{json, Value};
use tauri::{AppHandle, Manager};

#[tauri::command]
pub async fn app_ping() -> Result<String, String> {
    Ok("pong".to_string())
}

#[tauri::command]
pub async fn app_version() -> Result<Value, String> {
    Ok(json!({
        "version": application_core::AETHER_VERSION,
        "name": "AetherIDE",
        "platform": std::env::consts::OS,
        "arch": std::env::consts::ARCH,
        "framework": "Tauri 2"
    }))
}

#[tauri::command]
pub async fn app_doctor(app: AppHandle) -> Result<Value, String> {
    let report = diagnostics::EnvironmentDoctor::diagnose_all().await;

    // Active workspace status
    let ws_arc = app.state::<crate::workspace_bridge::SharedWorkspace>();
    let ws = ws_arc.lock();
    let ws_summary = ws.get_workspace_summary().unwrap_or(json!({}));
    drop(ws);

    // Provider status without exposing secrets
    let gw = app.state::<crate::commands::ai::SharedGateway>();
    let provider_statuses = gw.diagnose_all_providers();

    // Active Metro & Build processes
    let metro_pid_arc = app.state::<crate::commands::mobile::ActiveMetroPid>();
    let metro_pid = *metro_pid_arc.lock();

    let builds_arc = app.state::<crate::commands::build::SharedActiveBuilds>();
    let active_builds = builds_arc.lock().clone();

    Ok(json!({
        "all_healthy": report.all_healthy,
        "diagnostics": report.diagnostics,
        "workspace": ws_summary,
        "ai_providers": provider_statuses,
        "active_processes": {
            "metro_pid": metro_pid,
            "active_build_tasks": active_builds
        },
        "timestamp": chrono::Utc::now().to_rfc3339()
    }))
}

#[tauri::command]
pub async fn ide_dispatch(
    app: AppHandle,
    action: String,
    payload: Option<Value>,
) -> Result<Value, String> {
    let p = payload.unwrap_or(json!({}));
    match action.as_str() {
        "ShowOpenFolderDialog" => {
            let path = p
                .get("path")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            workspace_open_folder(app, path).await
        }
        "ShowAddFolderDialog" => {
            let path = p
                .get("path")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            workspace_add_folder(app, path).await
        }
        "ShowOpenFileDialog" => {
            let path = p
                .get("path")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            workspace_open_file(app, path).await
        }
        "ShowSaveFileDialog" => {
            let content = p
                .get("content")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let default_name = p
                .get("defaultName")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            workspace_save_file_as(app, content, default_name).await
        }
        "WorkspaceGetTree" => workspace_get_tree(app).await,
        "WorkspaceOpenFile" => {
            let path = p
                .get("path")
                .and_then(|v| v.as_str())
                .ok_or("Missing path")?
                .to_string();
            workspace_open_file(app, Some(path)).await
        }
        "WorkspaceSaveFile" => {
            let path = p
                .get("path")
                .and_then(|v| v.as_str())
                .ok_or("Missing path")?
                .to_string();
            let content = p
                .get("content")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            workspace_save_file(app, path, content).await
        }
        "WorkspaceCreateFile" => {
            let path = p
                .get("path")
                .and_then(|v| v.as_str())
                .ok_or("Missing path")?
                .to_string();
            workspace_create_file(app, path).await
        }
        "WorkspaceCreateDirectory" => {
            let path = p
                .get("path")
                .and_then(|v| v.as_str())
                .ok_or("Missing path")?
                .to_string();
            workspace_create_folder(app, path).await
        }
        "WorkspaceRenameEntry" => {
            let old_path = p
                .get("oldPath")
                .and_then(|v| v.as_str())
                .ok_or("Missing oldPath")?
                .to_string();
            let new_path = p
                .get("newPath")
                .and_then(|v| v.as_str())
                .ok_or("Missing newPath")?
                .to_string();
            workspace_rename_entry(app, old_path, new_path).await
        }
        "WorkspaceDeleteEntry" => {
            let path = p
                .get("path")
                .and_then(|v| v.as_str())
                .ok_or("Missing path")?
                .to_string();
            workspace_delete_entry(app, path).await
        }
        "WorkspaceRevealInExplorer" => {
            let path = p
                .get("path")
                .and_then(|v| v.as_str())
                .ok_or("Missing path")?
                .to_string();
            workspace_reveal_in_explorer(app, path).await
        }
        "WorkspaceSetTrust" => {
            let root_path = p
                .get("rootPath")
                .and_then(|v| v.as_str())
                .ok_or("Missing rootPath")?
                .to_string();
            let is_trusted = p
                .get("isTrusted")
                .and_then(|v| v.as_bool())
                .unwrap_or(true);
            workspace_set_trust(app, root_path, is_trusted).await
        }
        "TerminalSpawn" => {
            let cols = p.get("cols").and_then(|v| v.as_u64()).map(|v| v as u16);
            let rows = p.get("rows").and_then(|v| v.as_u64()).map(|v| v as u16);
            terminal_spawn(app, None, cols, rows).await
        }
        "TerminalWrite" => {
            let data = p.get("data").and_then(|v| v.as_str()).unwrap_or("");
            terminal_write(app, data.to_string()).await
        }
        "TerminalResize" => {
            let cols = p.get("cols").and_then(|v| v.as_u64()).unwrap_or(80) as u16;
            let rows = p.get("rows").and_then(|v| v.as_u64()).unwrap_or(24) as u16;
            terminal_resize(app, cols, rows).await
        }
        "TerminalClose" => {
            terminal_kill(app).await
        }
        _ => Err(format!("Unknown ide_dispatch action: {}", action)),
    }
}
