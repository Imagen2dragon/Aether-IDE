#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

pub mod commands;
#[cfg(windows)]
pub mod win32_dialogs;
pub mod workspace_bridge;

use std::sync::Arc;
use tauri::Manager;
use tracing::info;

pub fn run() {
    info!("Starting AetherIDE Tauri standalone runtime...");

    tauri::Builder::default()
        .setup(|app| {
            let data_dir = app
                .path()
                .app_data_dir()
                .unwrap_or_else(|_| std::path::PathBuf::from("."));
            let _ = std::fs::create_dir_all(&data_dir);

            // 1. Central Workspace Access Manager
            let session: workspace_bridge::SharedWorkspace = Arc::new(parking_lot::Mutex::new(
                workspace_bridge::WorkspaceSession::new(data_dir.clone()),
            ));

            // 2. Terminal Manager
            let term_mgr: commands::terminal::SharedTerminalManager = Arc::new(
                parking_lot::Mutex::new(terminal_manager::TerminalManager::new()),
            );
            let active_term: commands::terminal::ActiveTerminalId =
                Arc::new(parking_lot::Mutex::new(None));

            // 3. Database Engine & Project Brain
            let db_engine = database_engine::DatabaseEngine::open(data_dir.join("aether.db"))
                .expect("Failed to initialize SQLite database engine");
            let db: commands::memory::SharedDatabase =
                Arc::new(parking_lot::Mutex::new(db_engine.clone()));
            let brain: commands::brain::SharedBrain = Arc::new(parking_lot::Mutex::new(
                project_brain::ProjectBrain::new(db_engine.clone()),
            ));

            // 4. Model Gateway & Agent Orchestrator
            let gateway_inst = model_gateway::ModelGateway::new();
            let gateway: commands::ai::SharedGateway = Arc::new(gateway_inst.clone());
            let orchestrator: commands::agents::SharedOrchestrator = Arc::new(
                agent_orchestrator::AgentOrchestrator::new(gateway_inst, db_engine),
            );

            // 5. Streams, Storage Candidates & Build Tracking
            let active_streams: commands::ai::ActiveAiStreams =
                Arc::new(parking_lot::Mutex::new(commands::ai::StreamRegistry::new()));
            let storage_candidates: commands::storage::SharedStorageCandidates =
                Arc::new(parking_lot::Mutex::new(std::collections::HashMap::new()));
            let active_builds: commands::build::SharedActiveBuilds =
                Arc::new(parking_lot::Mutex::new(std::collections::HashMap::new()));

            // 6. Mobile, Checkpoint & LSP
            let metro_pid: commands::mobile::ActiveMetroPid =
                Arc::new(parking_lot::Mutex::new(None));
            let ckpt_engine =
                checkpoint_engine::CheckpointEngine::new(data_dir.join("checkpoints"));
            if let Err(e) = ckpt_engine.initialize_and_reconcile() {
                tracing::error!(
                    "Startup checkpoint quarantine reconciliation failed: {}. Checkpoint engine placed in recovery blocked mode.",
                    e
                );
                ckpt_engine.set_recovery_blocked(Some(e.to_string()));
            }
            let checkpoint_engine: commands::checkpoint::SharedCheckpointEngine =
                Arc::new(parking_lot::Mutex::new(ckpt_engine));
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
            app.manage(active_streams);
            app.manage(storage_candidates);
            app.manage(active_builds);
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
            commands::ai_cancel_stream,
            commands::ai_preview_proposal,
            commands::ai_apply_proposal,
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
            commands::build_cancel_task,
            // Storage Management
            commands::storage_analyze,
            commands::storage_cleanup,
            // Checkpoint Engine
            commands::checkpoint_create,
            commands::checkpoint_list,
            commands::checkpoint_restore_preview,
            commands::checkpoint_restore,
            commands::checkpoint_list_quarantine,
            commands::checkpoint_cleanup_quarantine,
            commands::checkpoint_reconcile_quarantine,
            commands::checkpoint_get_recovery_status,
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
