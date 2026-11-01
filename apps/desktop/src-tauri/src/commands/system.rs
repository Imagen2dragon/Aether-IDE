use crate::commands::terminal::*;
use crate::commands::workspace::*;
use serde_json::{json, Value};
use tauri::AppHandle;

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
pub async fn app_doctor() -> Result<Value, String> {
    let report = diagnostics::EnvironmentDoctor::diagnose_all().await;
    Ok(json!({
        "all_healthy": report.all_healthy,
        "diagnostics": report.diagnostics,
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
            let path = p.get("path").and_then(|v| v.as_str()).map(|s| s.to_string());
            workspace_open_folder(app, path).await
        }
        "ShowAddFolderDialog" => {
            let path = p.get("path").and_then(|v| v.as_str()).map(|s| s.to_string());
            workspace_add_folder(app, path).await
        }
        "ShowOpenFileDialog" => {
            let path = p.get("path").and_then(|v| v.as_str()).map(|s| s.to_string());
            workspace_open_file(app, path).await
        }
        "ShowSaveFileDialog" => {
            let content = p.get("content").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let default_name = p.get("defaultName").and_then(|v| v.as_str()).map(|s| s.to_string());
            workspace_save_file_as(app, content, default_name).await
        }
        "WorkspaceGetTree" => workspace_get_tree(app).await,
        "WorkspaceOpenFile" => {
            let path = p.get("path").and_then(|v| v.as_str()).ok_or("Missing path")?.to_string();
            workspace_open_file(app, Some(path)).await
        }
        "WorkspaceSaveFile" => {
            let path = p.get("path").and_then(|v| v.as_str()).ok_or("Missing path")?.to_string();
            let content = p.get("content").and_then(|v| v.as_str()).unwrap_or("").to_string();
            workspace_save_file(app, path, content).await
        }
        "WorkspaceCreateFile" => {
            let path = p.get("path").and_then(|v| v.as_str()).ok_or("Missing path")?.to_string();
            workspace_create_file(app, path).await
        }
        "WorkspaceCreateFolder" => {
            let path = p.get("path").and_then(|v| v.as_str()).ok_or("Missing path")?.to_string();
            workspace_create_folder(app, path).await
        }
        "WorkspaceRenameEntry" => {
            let old_path = p.get("oldPath").and_then(|v| v.as_str()).ok_or("Missing oldPath")?.to_string();
            let new_path = p.get("newPath").and_then(|v| v.as_str()).ok_or("Missing newPath")?.to_string();
            workspace_rename_entry(app, old_path, new_path).await
        }
        "WorkspaceDeleteEntry" => {
            let path = p.get("path").and_then(|v| v.as_str()).ok_or("Missing path")?.to_string();
            workspace_delete_entry(app, path).await
        }
        "WorkspaceRevealInExplorer" => {
            let path = p.get("path").and_then(|v| v.as_str()).ok_or("Missing path")?.to_string();
            workspace_reveal_in_explorer(app, path).await
        }
        "WorkspaceSetTrust" => {
            let path = p.get("path").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let trusted = p.get("trusted").and_then(|v| v.as_bool()).unwrap_or(true);
            workspace_set_trust(app, path, trusted).await
        }
        "TerminalSpawn" => {
            let cwd = p.get("cwd").and_then(|v| v.as_str()).map(|s| s.to_string());
            let cols = p.get("cols").and_then(|v| v.as_u64()).map(|n| n as u16);
            let rows = p.get("rows").and_then(|v| v.as_u64()).map(|n| n as u16);
            terminal_spawn(app, cwd, cols, rows).await
        }
        "TerminalWrite" => {
            let data = p.get("data").and_then(|v| v.as_str()).unwrap_or("").to_string();
            terminal_write(app, data).await
        }
        "TerminalResize" => {
            let cols = p.get("cols").and_then(|v| v.as_u64()).unwrap_or(120) as u16;
            let rows = p.get("rows").and_then(|v| v.as_u64()).unwrap_or(30) as u16;
            terminal_resize(app, cols, rows).await
        }
        "TerminalKill" => terminal_kill(app).await,
        "Ping" => app_ping().await.map(|s| json!({ "reply": s })),
        "Doctor" => app_doctor().await,
        other => Err(format!("Unknown or unhandled action: {}", other)),
    }
}
