use crate::workspace_bridge::SharedWorkspace;
use filesystem_engine::sanitize_relative_path;
use git_engine::{GitEngine, GitStatusResult};
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

fn resolve_repo_path(app: &AppHandle, path: Option<String>) -> Result<PathBuf, String> {
    let ws_arc = app.state::<SharedWorkspace>();
    let ws = ws_arc.lock();
    if let Some(p) = path {
        ws.validate_process_directory(&PathBuf::from(p))
    } else {
        ws.primary_root()
            .ok_or_else(|| "No workspace root currently open".to_string())
    }
}

#[tauri::command]
pub async fn git_get_status(
    app: AppHandle,
    path: Option<String>,
) -> Result<GitStatusResult, String> {
    let repo_path = resolve_repo_path(&app, path)?;
    GitEngine::get_status(&repo_path)
        .await
        .map_err(|e| format!("Failed to get git status: {}", e))
}

#[tauri::command]
pub async fn git_stage_file(
    app: AppHandle,
    file: String,
    path: Option<String>,
) -> Result<bool, String> {
    let repo_path = resolve_repo_path(&app, path)?;
    let rel_sanitized = sanitize_relative_path(&file)
        .map_err(|e| format!("Invalid git target file: {}", e))?;
    GitEngine::stage_file(&repo_path, &rel_sanitized.to_string_lossy())
        .await
        .map_err(|e| format!("Failed to stage file: {}", e))?;
    Ok(true)
}

#[tauri::command]
pub async fn git_unstage_file(
    app: AppHandle,
    file: String,
    path: Option<String>,
) -> Result<bool, String> {
    let repo_path = resolve_repo_path(&app, path)?;
    let rel_sanitized = sanitize_relative_path(&file)
        .map_err(|e| format!("Invalid git target file: {}", e))?;
    GitEngine::unstage_file(&repo_path, &rel_sanitized.to_string_lossy())
        .await
        .map_err(|e| format!("Failed to unstage file: {}", e))?;
    Ok(true)
}

#[tauri::command]
pub async fn git_commit(
    app: AppHandle,
    message: String,
    path: Option<String>,
) -> Result<String, String> {
    let repo_path = resolve_repo_path(&app, path)?;
    if message.trim().is_empty() {
        return Err("Commit message cannot be empty".into());
    }
    GitEngine::commit(&repo_path, &message)
        .await
        .map_err(|e| format!("Failed to commit changes: {}", e))
}

#[tauri::command]
pub async fn git_get_diff(
    app: AppHandle,
    staged: Option<bool>,
    path: Option<String>,
) -> Result<String, String> {
    let repo_path = resolve_repo_path(&app, path)?;
    if staged.unwrap_or(false) {
        GitEngine::get_staged_diff(&repo_path)
            .await
            .map_err(|e| format!("Failed to get staged diff: {}", e))
    } else {
        GitEngine::get_diff(&repo_path)
            .await
            .map_err(|e| format!("Failed to get diff: {}", e))
    }
}
