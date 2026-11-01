use crate::win32_dialogs;
use crate::workspace_bridge::WorkspaceSession;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::Arc;
use tauri::{AppHandle, Manager};
use windows::Win32::Foundation::HWND;

pub type SharedWorkspace = Arc<parking_lot::Mutex<WorkspaceSession>>;

fn get_parent_hwnd_isize(app: &AppHandle) -> isize {
    app.get_webview_window("main")
        .and_then(|w| w.hwnd().ok())
        .map(|h| h.0 as isize)
        .unwrap_or(0)
}

#[tauri::command]
pub async fn workspace_open_folder(app: AppHandle, path: Option<String>) -> Result<Value, String> {
    let target_path = match path {
        Some(p) => {
            let pb = PathBuf::from(&p);
            if !pb.exists() {
                return Err(format!("Specified folder does not exist: {}", p));
            }
            pb.canonicalize().unwrap_or(pb)
        }
        None => {
            let hwnd_raw = get_parent_hwnd_isize(&app);
            let picked: Option<PathBuf> = tauri::async_runtime::spawn_blocking(move || {
                let hwnd = HWND(hwnd_raw as *mut std::ffi::c_void);
                win32_dialogs::pick_folder(hwnd, Some("Select Workspace Folder"), None)
            })
            .await
            .map_err(|e| e.to_string())?
            .map_err(|e| e.to_string())?;

            match picked {
                Some(p) => p,
                None => return Ok(json!({ "cancelled": true })),
            }
        }
    };

    let session_arc = app.state::<SharedWorkspace>();
    let summary = {
        let session = session_arc.lock();
        session.set_single_root(target_path.clone())?
    };

    Ok(json!({
        "success": true,
        "path": target_path.to_string_lossy().to_string(),
        "summary": summary
    }))
}

#[tauri::command]
pub async fn workspace_add_folder(app: AppHandle, path: Option<String>) -> Result<Value, String> {
    let target_path = match path {
        Some(p) => {
            let pb = PathBuf::from(&p);
            if !pb.exists() {
                return Err(format!("Specified folder does not exist: {}", p));
            }
            pb.canonicalize().unwrap_or(pb)
        }
        None => {
            let hwnd_raw = get_parent_hwnd_isize(&app);
            let picked: Option<PathBuf> = tauri::async_runtime::spawn_blocking(move || {
                let hwnd = HWND(hwnd_raw as *mut std::ffi::c_void);
                win32_dialogs::pick_folder(hwnd, Some("Add Folder to Workspace"), None)
            })
            .await
            .map_err(|e| e.to_string())?
            .map_err(|e| e.to_string())?;

            match picked {
                Some(p) => p,
                None => return Ok(json!({ "cancelled": true })),
            }
        }
    };

    let session_arc = app.state::<SharedWorkspace>();
    let summary = {
        let session = session_arc.lock();
        session.add_root(target_path.clone())?
    };

    Ok(json!({
        "success": true,
        "path": target_path.to_string_lossy().to_string(),
        "summary": summary
    }))
}

#[tauri::command]
pub async fn workspace_get_tree(app: AppHandle) -> Result<Value, String> {
    let session_arc = app.state::<SharedWorkspace>();
    let session = session_arc.lock();
    session.get_workspace_summary()
}

#[tauri::command]
pub async fn workspace_open_file(app: AppHandle, path: Option<String>) -> Result<Value, String> {
    let target_path = match path {
        Some(p) => p,
        None => {
            let hwnd_raw = get_parent_hwnd_isize(&app);
            let picked: Option<PathBuf> = tauri::async_runtime::spawn_blocking(move || {
                let hwnd = HWND(hwnd_raw as *mut std::ffi::c_void);
                win32_dialogs::pick_file(
                    hwnd,
                    Some("Open File"),
                    None,
                    Some("All Files"),
                    Some("*.*"),
                )
            })
            .await
            .map_err(|e| e.to_string())?
            .map_err(|e| e.to_string())?;

            match picked {
                Some(p) => p.to_string_lossy().to_string(),
                None => return Ok(json!({ "cancelled": true })),
            }
        }
    };

    let session_arc = app.state::<SharedWorkspace>();
    let session = session_arc.lock();
    let content = session.read_file(&target_path)?;

    Ok(json!({
        "success": true,
        "path": target_path,
        "content": content
    }))
}

#[tauri::command]
pub async fn workspace_save_file(
    app: AppHandle,
    path: String,
    content: String,
) -> Result<Value, String> {
    let session_arc = app.state::<SharedWorkspace>();
    let session = session_arc.lock();
    let res = session.write_file(&path, &content)?;
    Ok(res)
}

#[tauri::command]
pub async fn workspace_save_file_as(
    app: AppHandle,
    content: String,
    default_name: Option<String>,
) -> Result<Value, String> {
    let hwnd_raw = get_parent_hwnd_isize(&app);
    let name_clone = default_name.clone();
    let picked: Option<PathBuf> = tauri::async_runtime::spawn_blocking(move || {
        let hwnd = HWND(hwnd_raw as *mut std::ffi::c_void);
        win32_dialogs::save_file(
            hwnd,
            Some("Save File As"),
            None,
            name_clone.as_deref().or(Some("untitled.txt")),
        )
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())?;

    match picked {
        Some(target_path) => {
            let path_str = target_path.to_string_lossy().to_string();
            let session_arc = app.state::<SharedWorkspace>();
            let session = session_arc.lock();
            let res = session.write_file(&path_str, &content)?;
            Ok(res)
        }
        None => Ok(json!({ "cancelled": true })),
    }
}

#[tauri::command]
pub async fn workspace_create_file(app: AppHandle, path: String) -> Result<Value, String> {
    let session_arc = app.state::<SharedWorkspace>();
    let session = session_arc.lock();
    session.create_file(&path)
}

#[tauri::command]
pub async fn workspace_create_folder(app: AppHandle, path: String) -> Result<Value, String> {
    let session_arc = app.state::<SharedWorkspace>();
    let session = session_arc.lock();
    session.create_directory(&path)
}

#[tauri::command]
pub async fn workspace_rename_entry(
    app: AppHandle,
    old_path: String,
    new_path: String,
) -> Result<Value, String> {
    let session_arc = app.state::<SharedWorkspace>();
    let session = session_arc.lock();
    session.rename_entry(&old_path, &new_path)
}

#[tauri::command]
pub async fn workspace_delete_entry(app: AppHandle, path: String) -> Result<Value, String> {
    let session_arc = app.state::<SharedWorkspace>();
    let session = session_arc.lock();
    session.delete_entry(&path)
}

#[tauri::command]
pub async fn workspace_reveal_in_explorer(app: AppHandle, path: String) -> Result<Value, String> {
    let session_arc = app.state::<SharedWorkspace>();
    let session = session_arc.lock();
    session.reveal_in_explorer(&path)
}

#[tauri::command]
pub async fn workspace_set_trust(
    app: AppHandle,
    path: String,
    trusted: bool,
) -> Result<Value, String> {
    let session_arc = app.state::<SharedWorkspace>();
    let session = session_arc.lock();
    session.set_trust(&path, trusted)
}
