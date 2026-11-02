use lsp_client::{Diagnostic, LspMultiplexer};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tauri::{AppHandle, Manager};

pub type SharedLspMultiplexer = Arc<LspMultiplexer>;

#[tauri::command]
pub async fn lsp_get_diagnostics(
    app: AppHandle,
    path: Option<String>,
) -> Result<HashMap<String, Vec<Diagnostic>>, String> {
    let lsp = app.state::<SharedLspMultiplexer>();
    if let Some(p) = path {
        let file_path = PathBuf::from(&p);
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
    let p = Path::new(&path);
    lsp.open_document(p, &content, language_id.as_deref())
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
    let p = Path::new(&path);
    lsp.change_document(p, &content, version)
        .await
        .map_err(|e| format!("LSP change failed: {}", e))?;
    Ok(true)
}

#[tauri::command]
pub async fn lsp_save_document(app: AppHandle, path: String) -> Result<bool, String> {
    let lsp = app.state::<SharedLspMultiplexer>();
    let p = Path::new(&path);
    lsp.save_document(p)
        .await
        .map_err(|e| format!("LSP save failed: {}", e))?;
    Ok(true)
}

#[tauri::command]
pub async fn lsp_close_document(app: AppHandle, path: String) -> Result<bool, String> {
    let lsp = app.state::<SharedLspMultiplexer>();
    let p = Path::new(&path);
    lsp.close_document(p)
        .await
        .map_err(|e| format!("LSP close failed: {}", e))?;
    Ok(true)
}
