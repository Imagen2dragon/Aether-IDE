use crate::commands::workspace::SharedWorkspace;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager};
use terminal_manager::TerminalManager;

pub type SharedTerminalManager = Arc<parking_lot::Mutex<TerminalManager>>;
pub type ActiveTerminalId = Arc<parking_lot::Mutex<Option<u32>>>;

#[tauri::command]
pub async fn terminal_spawn(
    app: AppHandle,
    cwd: Option<String>,
    cols: Option<u16>,
    rows: Option<u16>,
) -> Result<Value, String> {
    let workspace_cwd = {
        let session_arc = app.state::<SharedWorkspace>();
        let session = session_arc.lock();
        session.primary_root().map(|p| p.to_path_buf())
    };

    let target_cwd = cwd.map(PathBuf::from).or(workspace_cwd);

    let tm_arc = app.state::<SharedTerminalManager>();
    let active_id_arc = app.state::<ActiveTerminalId>();

    let session_info = {
        let tm = tm_arc.lock();
        tm.spawn_interactive(
            "powershell",
            target_cwd,
            cols.unwrap_or(120),
            rows.unwrap_or(30),
        )
        .map_err(|e| format!("Failed to spawn terminal: {}", e))?
    };

    let id = session_info.id;
    *active_id_arc.lock() = Some(id);

    // Subscribe to ConPTY output stream
    let mut rx = {
        let tm = tm_arc.lock();
        tm.subscribe_output(id)
            .map_err(|e| format!("Failed to subscribe to terminal output: {}", e))?
    };

    let app_emitter = app.clone();
    tauri::async_runtime::spawn(async move {
        while let Ok(bytes) = rx.recv().await {
            let data_str = String::from_utf8_lossy(&bytes).to_string();
            let _ = app_emitter.emit("terminal-output", json!({ "id": id, "data": data_str }));
        }
    });

    Ok(json!({
        "success": true,
        "id": id,
        "shell": session_info.shell,
        "cwd": session_info.cwd.map(|p| p.to_string_lossy().to_string())
    }))
}

#[tauri::command]
pub async fn terminal_write(app: AppHandle, data: String) -> Result<Value, String> {
    let active_id_arc = app.state::<ActiveTerminalId>();
    let active_id = active_id_arc.lock().ok_or("No active terminal session")?;

    let tm_arc = app.state::<SharedTerminalManager>();
    let tm = tm_arc.lock();
    tm.write_input(active_id, data.as_bytes())
        .map_err(|e| format!("Failed to write to terminal: {}", e))?;

    Ok(json!({ "success": true }))
}

#[tauri::command]
pub async fn terminal_resize(app: AppHandle, cols: u16, rows: u16) -> Result<Value, String> {
    let active_id_arc = app.state::<ActiveTerminalId>();
    let active_id = active_id_arc.lock().ok_or("No active terminal session")?;

    let tm_arc = app.state::<SharedTerminalManager>();
    let tm = tm_arc.lock();
    tm.resize_terminal(active_id, cols, rows)
        .map_err(|e| format!("Failed to resize terminal: {}", e))?;

    Ok(json!({ "success": true }))
}

#[tauri::command]
pub async fn terminal_kill(app: AppHandle) -> Result<Value, String> {
    let active_id_arc = app.state::<ActiveTerminalId>();
    let mut id_lock = active_id_arc.lock();
    if let Some(id) = id_lock.take() {
        let tm_arc = app.state::<SharedTerminalManager>();
        let tm = tm_arc.lock();
        let _ = tm.close_session(id);
    }
    Ok(json!({ "success": true }))
}
