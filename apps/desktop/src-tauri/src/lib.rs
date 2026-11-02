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

            let _ = std::fs::create_dir_all(&data_dir);

            // 1. Workspace Session & Terminal
            let session: commands::workspace::SharedWorkspace = Arc::new(parking_lot::Mutex::new(
                workspace_bridge::WorkspaceSession::new(data_dir.clone()),
            ));
            let term_mgr = Arc::new(parking_lot::Mutex::new(
                terminal_manager::TerminalManager::new(),
            ));
            let active_term: commands::terminal::ActiveTerminalId =
                Arc::new(parking_lot::Mutex::new(None));

            // 2. Database Engine (SQLite WAL)
            let db_engine = database_engine::DatabaseEngine::open(data_dir.join("aether.db"))
                .unwrap_or_else(|_| database_engine::DatabaseEngine::open_in_memory().unwrap());
            let db: commands::memory::SharedDatabase =
                Arc::new(parking_lot::Mutex::new(db_engine.clone()));

            // 3. Project Brain
            let brain: commands::brain::SharedBrain = Arc::new(parking_lot::Mutex::new(
                project_brain::ProjectBrain::new(db_engine.clone()),
            ));

            // 4. Model Gateway & Agent Orchestrator
            let gateway_inst = model_gateway::ModelGateway::new();
            let gateway: commands::ai::SharedGateway =
                Arc::new(gateway_inst.clone());
            let orchestrator: commands::agents::SharedOrchestrator = Arc::new(
                agent_orchestrator::AgentOrchestrator::new(
                    gateway_inst,
                    db_engine,
                ),
            );

            // 5. Mobile & Checkpoint & LSP
            let metro_pid: commands::mobile::ActiveMetroPid =
                Arc::new(parking_lot::Mutex::new(None));
            let checkpoint_engine: commands::checkpoint::SharedCheckpointEngine = Arc::new(
                parking_lot::Mutex::new(checkpoint_engine::CheckpointEngine::new(
                    data_dir.join("checkpoints"),
                )),
            );
            let lsp_mux: commands::lsp::SharedLspMultiplexer =
                Arc::new(lsp_client::LspMultiplexer::new(data_dir));

            // Manage All States
            app.manage(session);
            app.manage(term_mgr);
            app.manage(active_term);
            app.manage(db);
            app.manage(brain);
            app.manage(gateway);
            app.manage(orchestrator);
            app.manage(metro_pid);
            app.manage(checkpoint_engine);
            app.manage(lsp_mux);

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // Workspace
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
            // Terminal
            commands::terminal_spawn,
            commands::terminal_write,
            commands::terminal_resize,
            commands::terminal_kill,
            // System
            commands::app_ping,
            commands::app_version,
            commands::app_doctor,
            commands::ide_dispatch,
            // Project Brain
            commands::brain_scan_workspace,
            commands::brain_get_graph,
            commands::brain_calculate_impact,
            // Project Memory
            commands::memory_save_note,
            commands::memory_get_notes,
            commands::memory_delete_note,
            // AI Assistant & Model Gateway
            commands::ai_list_models,
            commands::ai_get_providers,
            commands::ai_complete,
            commands::ai_stream_prompt,
            // AI Agents & Swarm Orchestration
            commands::agent_list_personas,
            commands::agent_spawn_task,
            commands::agent_get_task,
            commands::agent_execute_step,
            commands::agent_cancel_task,
            // Mobile Studio
            commands::mobile_discover_devices,
            commands::mobile_check_eas,
            commands::mobile_get_dev_server,
            commands::mobile_start_metro,
            commands::mobile_stop_metro,
            commands::mobile_scaffold_app,
            // Build Management
            commands::build_detect_systems,
            commands::build_run_task,
            // Storage Management
            commands::storage_analyze,
            commands::storage_cleanup,
            // Checkpoint Engine
            commands::checkpoint_create,
            commands::checkpoint_list,
            commands::checkpoint_restore,
            // Git Engine
            commands::git_get_status,
            commands::git_stage_file,
            commands::git_unstage_file,
            commands::git_commit,
            commands::git_get_diff,
            // LSP Client
            commands::lsp_get_diagnostics,
            commands::lsp_open_document,
            commands::lsp_change_document,
            commands::lsp_save_document,
            commands::lsp_close_document,
        ])
        .run(tauri::generate_context!())
        .expect("AetherIDE failed to launch");
}
