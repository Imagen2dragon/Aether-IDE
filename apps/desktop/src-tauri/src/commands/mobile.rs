use crate::workspace_bridge::SharedWorkspace;
use mobile_build_manager::{
    EasCliStatus, ExpoDevServerInfo, MobileBuildManager, MobileDeviceInfo,
};
use parking_lot::Mutex;
use serde::Deserialize;
use serde_json::{json, Value};
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tracing::info;

pub type ActiveMetroPid = Arc<Mutex<Option<u32>>>;

#[derive(Debug, Deserialize)]
pub struct ScaffoldMobileParams {
    pub name: String,
    pub target_dir: Option<String>,
    #[serde(alias = "targetDir")]
    pub target_dir_alias: Option<String>,
    pub bundle_id: Option<String>,
    #[serde(alias = "bundleId")]
    pub bundle_id_alias: Option<String>,
    pub template: Option<String>,
}

#[tauri::command]
pub async fn mobile_discover_devices(_app: AppHandle) -> Result<Vec<MobileDeviceInfo>, String> {
    Ok(MobileBuildManager::discover_devices().await)
}

#[tauri::command]
pub async fn mobile_check_eas(
    app: AppHandle,
    project_dir: Option<String>,
) -> Result<EasCliStatus, String> {
    let dir = {
        let ws_arc = app.state::<SharedWorkspace>();
        let ws = ws_arc.lock();
        if let Some(p) = project_dir {
            Some(ws.validate_process_directory(&PathBuf::from(p))?)
        } else {
            ws.primary_root()
        }
    };

    MobileBuildManager::check_eas_status(dir.as_ref())
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn mobile_get_dev_server(
    app: AppHandle,
    project_dir: Option<String>,
    port: Option<u16>,
) -> Result<ExpoDevServerInfo, String> {
    let dir = {
        let ws_arc = app.state::<SharedWorkspace>();
        let ws = ws_arc.lock();
        if let Some(p) = project_dir {
            ws.validate_process_directory(&PathBuf::from(p))?
        } else {
            ws.primary_root()
                .ok_or_else(|| "No active workspace root open for mobile server inspection".to_string())?
        }
    };

    MobileBuildManager::get_expo_dev_server_info(&dir, port)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn mobile_start_metro(
    app: AppHandle,
    project_dir: Option<String>,
    port: Option<u16>,
) -> Result<Value, String> {
    let dir = {
        let ws_arc = app.state::<SharedWorkspace>();
        let ws = ws_arc.lock();
        if let Some(p) = project_dir {
            ws.validate_process_directory(&PathBuf::from(p))?
        } else {
            ws.primary_root()
                .ok_or_else(|| "No active workspace root open for Metro".to_string())?
        }
    };

    let port_num = port.unwrap_or(8081);

    // 1. Check port availability
    if TcpListener::bind(("127.0.0.1", port_num)).is_err() {
        return Err(format!(
            "Port {} is already in use by another process. Please choose a different port or stop existing instance.",
            port_num
        ));
    }

    // 2. Terminate existing Metro process tree if any
    let active_pid_arc = app.state::<ActiveMetroPid>();
    {
        let mut lock = active_pid_arc.lock();
        if let Some(pid) = *lock {
            #[cfg(windows)]
            let _ = std::process::Command::new("taskkill")
                .args(["/F", "/T", "/PID", &pid.to_string()])
                .status();
            #[cfg(not(windows))]
            let _ = std::process::Command::new("kill")
                .args(["-9", &pid.to_string()])
                .status();
            *lock = None;
        }
    }

    // 3. Spawn Metro bundler with direct executable arguments
    #[cfg(windows)]
    let mut cmd = {
        let mut c = Command::new("cmd.exe");
        c.args(["/C", "npx", "expo", "start", "--port", &port_num.to_string()]);
        c
    };

    #[cfg(not(windows))]
    let mut cmd = {
        let mut c = Command::new("npx");
        c.args(["expo", "start", "--port", &port_num.to_string()]);
        c
    };

    cmd.current_dir(&dir);
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());

    let mut child = cmd
        .spawn()
        .map_err(|e| format!("Failed to spawn Metro bundler: {}", e))?;
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
                // Detect actual readiness from output stream
                if line.contains("Waiting on") || line.contains("Metro waiting on") || line.contains("Logs for your project") {
                    let _ = app_handle.emit("mobile-metro-ready", json!({ "port": port_num }));
                }
                let _ = app_handle.emit("mobile-process-output", json!({ "line": line }));
            }
        });
    }

    info!("Started Metro bundler (PID {}) on port {}", pid, port_num);

    Ok(json!({
        "status": "starting",
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
            .args(["/F", "/T", "/PID", &pid.to_string()])
            .status();
        #[cfg(not(windows))]
        let _ = std::process::Command::new("kill")
            .args(["-9", &pid.to_string()])
            .status();
        *lock = None;
        info!("Stopped Metro bundler (PID {})", pid);
        Ok(true)
    } else {
        Ok(false)
    }
}

#[tauri::command]
pub async fn mobile_scaffold_app(
    app: AppHandle,
    params: ScaffoldMobileParams,
) -> Result<String, String> {
    let ws_arc = app.state::<SharedWorkspace>();
    let ws = ws_arc.lock();
    let raw_target = params
        .target_dir
        .or(params.target_dir_alias)
        .unwrap_or_else(|| ".".into());

    let target_dir = if raw_target == "." {
        ws.primary_root()
            .ok_or_else(|| "No active workspace root open for mobile scaffold".to_string())?
    } else {
        ws.validate_process_directory(&PathBuf::from(&raw_target))?
    };
    drop(ws);

    let bundle_id = params
        .bundle_id
        .or(params.bundle_id_alias)
        .unwrap_or_else(|| format!("com.aether.{}", params.name.to_lowercase().replace(' ', "")));

    let path = MobileBuildManager::scaffold_expo_app(
        target_dir.to_string_lossy().to_string(),
        &params.name,
        &bundle_id,
    )
    .map_err(|e| format!("Scaffolding failed: {}", e))?;

    Ok(path.to_string_lossy().to_string())
}
