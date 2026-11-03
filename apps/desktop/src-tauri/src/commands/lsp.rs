use crate::workspace_bridge::SharedWorkspace;
use lsp_client::{Diagnostic, LspMultiplexer};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::{AppHandle, Manager};

pub type SharedLspMultiplexer = Arc<LspMultiplexer>;

fn resolve_file_path(app: &AppHandle, user_path: &str) -> PathBuf {
    let ws_arc = app.state::<SharedWorkspace>();
    let ws = ws_arc.lock();
    if let Ok((_, root, rel)) = ws.resolve_in_workspace(user_path) {
        root.join(rel)
    } else {
        PathBuf::from(user_path)
    }
}

#[tauri::command]
pub async fn lsp_get_diagnostics(
    app: AppHandle,
    path: Option<String>,
) -> Result<HashMap<String, Vec<Diagnostic>>, String> {
    let lsp = app.state::<SharedLspMultiplexer>();
    if let Some(p) = path {
        let file_path = resolve_file_path(&app, &p);
        let diags = lsp.get_diagnostics(&file_path);
        let mut map = HashMap::new();
        map.insert(p, diags);
        Ok(map)
    } else {
        Ok(lsp.get_all_diagnostics())
    }
}

#[tauri::command]
pub async fn lsp_open_document(
    app: AppHandle,
    path: String,
    content: String,
    language_id: Option<String>,
) -> Result<bool, String> {
    let lsp = app.state::<SharedLspMultiplexer>();
    let resolved = resolve_file_path(&app, &path);
    lsp.open_document(&resolved, &content, language_id.as_deref())
        .await
        .map_err(|e| format!("LSP open failed: {}", e))?;
    Ok(true)
}

#[tauri::command]
pub async fn lsp_change_document(
    app: AppHandle,
    path: String,
    content: String,
    version: i64,
) -> Result<bool, String> {
    let lsp = app.state::<SharedLspMultiplexer>();
    let resolved = resolve_file_path(&app, &path);
    lsp.change_document(&resolved, &content, version)
        .await
        .map_err(|e| format!("LSP change failed: {}", e))?;
    Ok(true)
}

#[tauri::command]
pub async fn lsp_save_document(app: AppHandle, path: String) -> Result<bool, String> {
    let lsp = app.state::<SharedLspMultiplexer>();
    let resolved = resolve_file_path(&app, &path);
    lsp.save_document(&resolved)
        .await
        .map_err(|e| format!("LSP save failed: {}", e))?;
    Ok(true)
}

#[tauri::command]
pub async fn lsp_close_document(app: AppHandle, path: String) -> Result<bool, String> {
    let lsp = app.state::<SharedLspMultiplexer>();
    let resolved = resolve_file_path(&app, &path);
    lsp.close_document(&resolved)
        .await
        .map_err(|e| format!("LSP close failed: {}", e))?;
    Ok(true)
}
