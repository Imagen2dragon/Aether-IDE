use mobile_build_manager::{
    EasCliStatus, ExpoDevServerInfo, MobileBuildManager, MobileDeviceInfo,
};
use parking_lot::Mutex;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

pub type ActiveMetroPid = Arc<Mutex<Option<u32>>>;

#[tauri::command]
pub async fn mobile_discover_devices(_app: AppHandle) -> Result<Vec<MobileDeviceInfo>, String> {
    Ok(MobileBuildManager::discover_devices().await)
}

#[tauri::command]
pub async fn mobile_check_eas(
    _app: AppHandle,
    project_dir: Option<String>,
) -> Result<EasCliStatus, String> {
    let dir = project_dir.map(PathBuf::from);
    MobileBuildManager::check_eas_status(dir.as_ref())
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn mobile_get_dev_server(
    _app: AppHandle,
    project_dir: Option<String>,
    port: Option<u16>,
) -> Result<ExpoDevServerInfo, String> {
    let dir = project_dir.map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."));
    MobileBuildManager::get_expo_dev_server_info(&dir, port)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn mobile_start_metro(
    app: AppHandle,
    project_dir: Option<String>,
    port: Option<u16>,
) -> Result<Value, String> {
    let dir = project_dir.map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."));
    let port_num = port.unwrap_or(8081);

    // Stop existing process if any
    let active_pid_arc = app.state::<ActiveMetroPid>();
    {
        let mut lock = active_pid_arc.lock();
        if let Some(pid) = *lock {
            #[cfg(windows)]
            let _ = std::process::Command::new("taskkill")
                .args(["/F", "/PID", &pid.to_string()])
                .status();
            #[cfg(not(windows))]
            let _ = std::process::Command::new("kill")
                .args(["-9", &pid.to_string()])
                .status();
            *lock = None;
        }
    }

    #[cfg(windows)]
    let mut cmd = Command::new("cmd.exe");
    #[cfg(windows)]
    cmd.args(["/C", &format!("npx expo start --port {}", port_num)]);

    #[cfg(not(windows))]
    let mut cmd = Command::new("sh");
    #[cfg(not(windows))]
    cmd.args(["-c", &format!("npx expo start --port {}", port_num)]);

    cmd.current_dir(&dir);
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());

    let mut child = cmd.spawn().map_err(|e| format!("Failed to spawn Metro bundler: {}", e))?;
    let pid = child.id().unwrap_or(0);

    {
        let mut lock = active_pid_arc.lock();
        *lock = Some(pid);
    }

    let stdout = child.stdout.take();
    let app_handle = app.clone();

    if let Some(out) = stdout {
        tokio::spawn(async move {
            let reader = BufReader::new(out);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let _ = app_handle.emit("mobile-process-output", json!({ "line": line }));
            }
        });
    }

    Ok(json!({
        "status": "started",
        "pid": pid,
        "port": port_num,
        "local_url": format!("http://localhost:{}", port_num),
        "qr_uri": format!("exp://127.0.0.1:{}", port_num)
    }))
}

#[tauri::command]
pub async fn mobile_stop_metro(app: AppHandle) -> Result<bool, String> {
    let active_pid_arc = app.state::<ActiveMetroPid>();
    let mut lock = active_pid_arc.lock();
    if let Some(pid) = *lock {
        #[cfg(windows)]
        let _ = std::process::Command::new("taskkill")
            .args(["/F", "/PID", &pid.to_string()])
            .status();
        #[cfg(not(windows))]
        let _ = std::process::Command::new("kill")
            .args(["-9", &pid.to_string()])
            .status();
        *lock = None;
        Ok(true)
    } else {
        Ok(false)
    }
}

#[tauri::command]
pub async fn mobile_scaffold_app(
    _app: AppHandle,
    target_dir: String,
    name: String,
    bundle_id: String,
) -> Result<String, String> {
    let path = MobileBuildManager::scaffold_expo_app(target_dir, &name, &bundle_id)
        .map_err(|e| format!("Scaffolding failed: {}", e))?;
    Ok(path.to_string_lossy().to_string())
}
