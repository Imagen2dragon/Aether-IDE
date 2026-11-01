//! AetherIDE — Production-Grade AI-Native Software Engineering Studio
//! Tauri 2 Desktop Host and Native Services Integration

pub mod commands;
pub mod win32_dialogs;
pub mod workspace_bridge;

use std::path::PathBuf;
use std::sync::Arc;
use tauri::Manager;

pub fn run() {
    tauri::Builder::default()
        // Single-instance enforcement: if another instance is launched, focus the existing main window
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
        }))
        .setup(|app| {
            let data_dir = std::env::var("LOCALAPPDATA")
                .map(|p| PathBuf::from(p).join("AetherIDE"))
                .unwrap_or_else(|_| PathBuf::from("data"));

            let session = Arc::new(parking_lot::Mutex::new(
                workspace_bridge::WorkspaceSession::new(data_dir),
            ));
            let term_mgr = Arc::new(parking_lot::Mutex::new(
                terminal_manager::TerminalManager::new(),
            ));
            let active_term: commands::terminal::ActiveTerminalId =
                Arc::new(parking_lot::Mutex::new(None));

            app.manage(session);
            app.manage(term_mgr);
            app.manage(active_term);

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::workspace_open_folder,
            commands::workspace_add_folder,
            commands::workspace_get_tree,
            commands::workspace_open_file,
            commands::workspace_save_file,
            commands::workspace_save_file_as,
            commands::workspace_create_file,
            commands::workspace_create_folder,
            commands::workspace_rename_entry,
            commands::workspace_delete_entry,
            commands::workspace_reveal_in_explorer,
            commands::workspace_set_trust,
            commands::terminal_spawn,
            commands::terminal_write,
            commands::terminal_resize,
            commands::terminal_kill,
            commands::app_ping,
            commands::app_version,
            commands::app_doctor,
            commands::ide_dispatch,
        ])
        .run(tauri::generate_context!())
        .expect("AetherIDE failed to launch");
}
