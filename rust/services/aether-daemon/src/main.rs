use application_core::{AetherConfig, AutonomyMode};
use database_engine::DatabaseEngine;
use diagnostics::EnvironmentDoctor;
use ipc_protocol::{
    decode_message, encode_message, IpcEnvelope, IpcPayload, IpcRequest, IpcResponse,
    DEFAULT_NAMED_PIPE_PATH,
};
use model_gateway::{CompletionRequest, ModelGateway};
use project_brain::ProjectBrain;
use project_indexer::ProjectIndexer;
use security_core::TokenManager;
use std::path::PathBuf;
use std::sync::Arc;
use symbol_index::SymbolIndex;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tracing::{error, info};

struct DaemonState {
    config: AetherConfig,
    session_token: String,
    db: DatabaseEngine,
    model_gateway: ModelGateway,
    agent_orchestrator: agent_orchestrator::AgentOrchestrator,
    project_brain: parking_lot::Mutex<ProjectBrain>,
    indexer: ProjectIndexer,
    lsp_multiplexer: Arc<lsp_client::LspMultiplexer>,
    terminal_manager: Arc<terminal_manager::TerminalManager>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt().with_env_filter("info").init();

    info!("Starting AetherIDE Native Daemon v0.1.0-alpha");

    let config = AetherConfig::default();
    let session_token = TokenManager::generate_session_token();

    let db_path = config.data_dir.join("aether.db");
    let db = DatabaseEngine::open(&db_path)
        .unwrap_or_else(|_| DatabaseEngine::open_in_memory().unwrap());
    let symbol_index = SymbolIndex::new(db.clone());
    let indexer = ProjectIndexer::new(symbol_index);

    let model_gateway = ModelGateway::new();
    let agent_orchestrator =
        agent_orchestrator::AgentOrchestrator::new(model_gateway.clone(), db.clone());
    let project_brain = parking_lot::Mutex::new(ProjectBrain::new(db.clone()));
    let workspace_root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let lsp_multiplexer = Arc::new(lsp_client::LspMultiplexer::new(workspace_root));
    let terminal_manager = Arc::new(terminal_manager::TerminalManager::new());

    let state = Arc::new(DaemonState {
        config: config.clone(),
        session_token: session_token.clone(),
        db,
        model_gateway,
        agent_orchestrator,
        project_brain,
        indexer,
        lsp_multiplexer,
        terminal_manager,
    });

    // Write token file for UI workbench to authenticate
    let token_file = config.data_dir.join("daemon_session.token");
    let _ = std::fs::create_dir_all(&config.data_dir);
    let _ = std::fs::write(&token_file, &session_token);
    info!("Daemon session token published to {:?}", token_file);

    // Launch TCP loopback listener for cross-platform/workbench transport
    let tcp_addr = "127.0.0.1:44888";
    let listener = TcpListener::bind(tcp_addr).await?;
    info!("Aether daemon IPC listening on {}", tcp_addr);

    // Also support Windows Named Pipe listener in background if on Windows
    #[cfg(windows)]
    {
        let state_clone = state.clone();
        tokio::spawn(async move {
            run_windows_named_pipe_listener(state_clone).await;
        });
    }

    loop {
        let (mut socket, peer) = listener.accept().await?;
        let peer_addr = peer.to_string();
        info!("Accepted IPC connection from {}", peer_addr);
        let state_clone = state.clone();

        tokio::spawn(async move {
            let mut buf = vec![0u8; 65536];
            loop {
                let n = match socket.read(&mut buf).await {
                    Ok(0) => break,
                    Ok(n) => n,
                    Err(e) => {
                        error!("IPC read error: {}", e);
                        break;
                    }
                };

                if let Ok(envelope) = decode_message(&buf[..n]) {
                    let response = handle_ipc_envelope(&state_clone, envelope, &peer_addr).await;
                    if let Ok(bytes) = encode_message(&response) {
                        let _ = socket.write_all(&bytes).await;
                    }
                }
            }
        });
    }
}

#[cfg(windows)]
async fn run_windows_named_pipe_listener(state: Arc<DaemonState>) {
    use tokio::net::windows::named_pipe::ServerOptions;

    let pipe_name = DEFAULT_NAMED_PIPE_PATH;
    info!("Registering Windows Named Pipe at {}", pipe_name);

    loop {
        let server = match ServerOptions::new()
            .first_pipe_instance(true)
            .reject_remote_clients(true)
            .create(pipe_name)
        {
            Ok(s) => s,
            Err(e) => {
                error!("Failed to create named pipe {}: {}", pipe_name, e);
                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                continue;
            }
        };

        if server.connect().await.is_ok() {
            let state_clone = state.clone();
            tokio::spawn(async move {
                let mut server = server;
                let mut buf = vec![0u8; 65536];
                loop {
                    let n = match server.read(&mut buf).await {
                        Ok(0) => break,
                        Ok(n) => n,
                        Err(_) => break,
                    };

                    if let Ok(envelope) = decode_message(&buf[..n]) {
                        let response =
                            handle_ipc_envelope(&state_clone, envelope, "windows-named-pipe").await;
                        if let Ok(bytes) = encode_message(&response) {
                            let _ = server.write_all(&bytes).await;
                        }
                    }
                }
            });
        }
    }
}

async fn handle_ipc_envelope(
    state: &Arc<DaemonState>,
    envelope: IpcEnvelope,
    peer_info: &str,
) -> IpcEnvelope {
    // 1. Verify Authentication Token (SEC-01 remediation)
    if !TokenManager::verify_token(&state.session_token, envelope.session_token.as_deref()) {
        tracing::warn!(
            "SEC-01: Unauthorized IPC request rejected from {} (request id: {})",
            peer_info,
            envelope.id
        );
        return IpcEnvelope {
            id: envelope.id,
            session_token: None,
            payload: IpcPayload::Response(IpcResponse::unauthorized(
                "Access denied: missing or invalid session token",
            )),
        };
    }

    let req = match envelope.payload {
        IpcPayload::Request(r) => r,
        _ => {
            return IpcEnvelope {
                id: envelope.id,
                session_token: Some(state.session_token.clone()),
                payload: IpcPayload::Response(IpcResponse::err("Only Request payloads supported")),
            }
        }
    };

    let response = match req {
        IpcRequest::Ping => {
            IpcResponse::ok(serde_json::json!({ "pong": true, "version": "0.1.0-alpha" }))
        }

        IpcRequest::GetStatus => {
            let models = state.model_gateway.list_all_models();
            let personas = state.agent_orchestrator.list_personas();
            IpcResponse::ok(serde_json::json!({
                "status": "Running",
                "version": "0.1.0-alpha",
                "models_count": models.len(),
                "agent_personas_count": personas.len(),
                "autonomy_mode": format!("{:?}", state.config.autonomy_mode),
            }))
        }

        IpcRequest::RunDoctorCheck => {
            let report = EnvironmentDoctor::diagnose_all().await;
            IpcResponse::ok(serde_json::to_value(report).unwrap())
        }

        IpcRequest::IndexWorkspace { workspace_path } => {
            let path = PathBuf::from(&workspace_path);
            match state
                .indexer
                .index_workspace(path, &state.config.index_excluded_patterns, None)
                .await
            {
                Ok(res) => IpcResponse::ok(serde_json::json!({
                    "files_scanned": res.files_scanned,
                    "total_symbols": res.total_symbols,
                    "status": res.status,
                })),
                Err(e) => IpcResponse::err(e.to_string()),
            }
        }

        IpcRequest::SearchSymbols { query, limit } => {
            let limit = limit.unwrap_or(20);
            match state.db.search_symbols_fts(&query, limit) {
                Ok(symbols) => IpcResponse::ok(serde_json::to_value(symbols).unwrap()),
                Err(e) => IpcResponse::err(e.to_string()),
            }
        }

        IpcRequest::RunCommand {
            command,
            cwd,
            timeout_secs,
        } => {
            let timeout = timeout_secs.map(std::time::Duration::from_secs);
            let dir = cwd.map(PathBuf::from);
            match process_manager::ProcessManager::run_command_streaming(
                &command, dir, timeout, None, None,
            )
            .await
            {
                Ok(out) => IpcResponse::ok(serde_json::json!({
                    "exit_code": out.exit_code,
                    "stdout": out.stdout,
                    "stderr": out.stderr,
                })),
                Err(e) => IpcResponse::err(e.to_string()),
            }
        }

        IpcRequest::CreateAgentTask {
            role,
            objective,
            autonomy_mode,
        } => {
            let mode = match autonomy_mode.to_lowercase().as_str() {
                "observe" => AutonomyMode::Observe,
                "autopilot" => AutonomyMode::Autopilot,
                _ => AutonomyMode::Assist,
            };
            let task_id = state.agent_orchestrator.spawn_task(&role, &objective, mode);
            let _ = state.agent_orchestrator.step_task(&task_id).await;
            let task = state.agent_orchestrator.get_task(&task_id);
            IpcResponse::ok(serde_json::to_value(task).unwrap())
        }

        IpcRequest::GetAgentTask { task_id } => match state.agent_orchestrator.get_task(&task_id) {
            Some(task) => IpcResponse::ok(serde_json::to_value(task).unwrap()),
            None => IpcResponse::err("Task not found"),
        },

        IpcRequest::QueryProjectBrain { query: _ } => {
            let graph = state.project_brain.lock().get_graph().clone();
            IpcResponse::ok(serde_json::to_value(graph).unwrap())
        }

        IpcRequest::GenerateMobileApp {
            prompt: _,
            target_framework: _,
            app_name,
        } => {
            let target_dir = state.config.data_dir.join("mobile_projects");
            let bundle_id = format!("com.aether.{}", app_name.to_lowercase().replace(' ', ""));
            match mobile_build_manager::MobileBuildManager::scaffold_expo_app(
                &target_dir,
                &app_name,
                &bundle_id,
            ) {
                Ok(path) => IpcResponse::ok(serde_json::json!({
                    "status": "Created",
                    "project_path": path.to_string_lossy(),
                    "app_name": app_name,
                    "bundle_id": bundle_id,
                })),
                Err(e) => IpcResponse::err(e.to_string()),
            }
        }

        IpcRequest::TriggerMobileBuild {
            project_path,
            platform,
            build_type,
        } => {
            match mobile_build_manager::MobileBuildManager::trigger_eas_build(
                &project_path,
                &platform,
                &build_type,
            )
            .await
            {
                Ok(job) => IpcResponse::ok(serde_json::to_value(job).unwrap()),
                Err(e) => IpcResponse::err(e.to_string()),
            }
        }

        IpcRequest::ModelComplete {
            prompt,
            model,
            system,
        } => {
            let req = CompletionRequest {
                prompt,
                system,
                model,
                temperature: Some(0.2),
                max_tokens: Some(1024),
            };
            match state.model_gateway.execute_completion(&req).await {
                Ok(resp) => IpcResponse::ok(serde_json::to_value(resp).unwrap()),
                Err(e) => IpcResponse::err(e.to_string()),
            }
        }

        IpcRequest::LspOpenDocument {
            path,
            content,
            language_id,
        } => {
            let p = PathBuf::from(&path);
            match state
                .lsp_multiplexer
                .open_document(&p, &content, language_id.as_deref())
                .await
            {
                Ok(_) => IpcResponse::ok(serde_json::json!({ "opened": true, "path": path })),
                Err(e) => IpcResponse::err(e.to_string()),
            }
        }

        IpcRequest::LspChangeDocument {
            path,
            content,
            version,
        } => {
            let p = PathBuf::from(&path);
            match state
                .lsp_multiplexer
                .change_document(&p, &content, version)
                .await
            {
                Ok(_) => IpcResponse::ok(serde_json::json!({ "changed": true, "path": path })),
                Err(e) => IpcResponse::err(e.to_string()),
            }
        }

        IpcRequest::LspCloseDocument { path } => {
            let p = PathBuf::from(&path);
            match state.lsp_multiplexer.close_document(&p).await {
                Ok(_) => IpcResponse::ok(serde_json::json!({ "closed": true, "path": path })),
                Err(e) => IpcResponse::err(e.to_string()),
            }
        }

        IpcRequest::LspCompletion {
            path,
            line,
            character,
        } => {
            let p = PathBuf::from(&path);
            match state.lsp_multiplexer.completion(&p, line, character).await {
                Ok(items) => IpcResponse::ok(serde_json::to_value(items).unwrap()),
                Err(e) => IpcResponse::err(e.to_string()),
            }
        }

        IpcRequest::LspHover {
            path,
            line,
            character,
        } => {
            let p = PathBuf::from(&path);
            match state.lsp_multiplexer.hover(&p, line, character).await {
                Ok(hover) => IpcResponse::ok(serde_json::to_value(hover).unwrap()),
                Err(e) => IpcResponse::err(e.to_string()),
            }
        }

        IpcRequest::LspDefinition {
            path,
            line,
            character,
        } => {
            let p = PathBuf::from(&path);
            match state.lsp_multiplexer.definition(&p, line, character).await {
                Ok(defs) => IpcResponse::ok(serde_json::to_value(defs).unwrap()),
                Err(e) => IpcResponse::err(e.to_string()),
            }
        }

        IpcRequest::LspDiagnostics { path } => {
            let p = PathBuf::from(&path);
            let diags = state.lsp_multiplexer.get_diagnostics(&p);
            IpcResponse::ok(serde_json::to_value(diags).unwrap())
        }

        IpcRequest::TerminalSpawn {
            shell,
            cwd,
            cols,
            rows,
        } => {
            #[cfg(windows)]
            {
                let shell_name = shell.unwrap_or_else(|| "powershell".to_string());
                let cwd_path = cwd.map(PathBuf::from);
                let cols = cols.unwrap_or(80);
                let rows = rows.unwrap_or(24);
                match state
                    .terminal_manager
                    .spawn_interactive(&shell_name, cwd_path, cols, rows)
                {
                    Ok(info) => IpcResponse::ok(serde_json::to_value(info).unwrap()),
                    Err(e) => IpcResponse::err(e.to_string()),
                }
            }
            #[cfg(not(windows))]
            {
                let info = state.terminal_manager.create_session(
                    "Terminal",
                    "bash",
                    cwd.map(PathBuf::from),
                );
                IpcResponse::ok(serde_json::to_value(info).unwrap())
            }
        }

        IpcRequest::TerminalWrite { session_id, data } => {
            #[cfg(windows)]
            {
                match state
                    .terminal_manager
                    .write_input(session_id, data.as_bytes())
                {
                    Ok(written) => IpcResponse::ok(serde_json::json!({ "written": written })),
                    Err(e) => IpcResponse::err(e.to_string()),
                }
            }
            #[cfg(not(windows))]
            {
                let _ = (session_id, data);
                IpcResponse::ok(serde_json::json!({ "written": 0 }))
            }
        }

        IpcRequest::TerminalResize {
            session_id,
            cols,
            rows,
        } => {
            #[cfg(windows)]
            {
                match state
                    .terminal_manager
                    .resize_terminal(session_id, cols, rows)
                {
                    Ok(_) => IpcResponse::ok(serde_json::json!({ "resized": true })),
                    Err(e) => IpcResponse::err(e.to_string()),
                }
            }
            #[cfg(not(windows))]
            {
                let _ = (session_id, cols, rows);
                IpcResponse::ok(serde_json::json!({ "resized": true }))
            }
        }

        IpcRequest::TerminalClose { session_id } => {
            match state.terminal_manager.close_session(session_id) {
                Ok(_) => IpcResponse::ok(serde_json::json!({ "closed": true })),
                Err(e) => IpcResponse::err(e.to_string()),
            }
        }

        IpcRequest::SyncGeneratePairing {
            device_name,
            host,
            port,
        } => {
            let dev_name = device_name.unwrap_or_else(|| {
                std::env::var("COMPUTERNAME").unwrap_or_else(|_| "AetherIDE-Desktop".to_string())
            });
            let h = host.unwrap_or_else(|| "127.0.0.1".to_string());
            let p = port.unwrap_or(44888);
            match workspace_sync::PairingPayload::new(&h, p, 300) {
                Ok((payload, _psk)) => {
                    let qr_uri = payload.to_pairing_uri();
                    IpcResponse::ok(serde_json::json!({
                        "device_name": dev_name,
                        "pairing_payload": payload,
                        "qr_uri": qr_uri,
                    }))
                }
                Err(e) => IpcResponse::err(format!("Pairing error: {}", e)),
            }
        }

        IpcRequest::SyncGetManifest {
            workspace_path,
            workspace_name,
        } => {
            let p = PathBuf::from(&workspace_path);
            let name = workspace_name.unwrap_or_else(|| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("workspace")
                    .to_string()
            });
            let default_ignores = vec![
                ".git".to_string(),
                "node_modules".to_string(),
                "target".to_string(),
                ".build".to_string(),
            ];
            match workspace_sync::WorkspaceManifest::build(&p, &name, &default_ignores) {
                Ok(manifest) => IpcResponse::ok(serde_json::to_value(manifest).unwrap()),
                Err(e) => IpcResponse::err(e.to_string()),
            }
        }

        IpcRequest::SyncDiffManifests {
            local_manifest,
            remote_manifest,
        } => {
            let local: Result<workspace_sync::WorkspaceManifest, _> =
                serde_json::from_value(local_manifest);
            let remote: Result<workspace_sync::WorkspaceManifest, _> =
                serde_json::from_value(remote_manifest);

            match (local, remote) {
                (Ok(loc), Ok(rem)) => {
                    let diff = workspace_sync::diff_manifests(&loc, &rem);
                    IpcResponse::ok(serde_json::to_value(diff).unwrap())
                }
                (Err(e), _) => IpcResponse::err(format!("Invalid local_manifest: {}", e)),
                (_, Err(e)) => IpcResponse::err(format!("Invalid remote_manifest: {}", e)),
            }
        }

        _ => IpcResponse::err("Action not yet implemented in daemon slice"),
    };

    IpcEnvelope {
        id: envelope.id,
        session_token: Some(state.session_token.clone()),
        payload: IpcPayload::Response(response),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_state() -> Arc<DaemonState> {
        let config = AetherConfig::default();
        let session_token = TokenManager::generate_session_token();
        let db = DatabaseEngine::open_in_memory().unwrap();
        let symbol_index = SymbolIndex::new(db.clone());
        let indexer = ProjectIndexer::new(symbol_index);
        let model_gateway = ModelGateway::new();
        let agent_orchestrator =
            agent_orchestrator::AgentOrchestrator::new(model_gateway.clone(), db.clone());
        let project_brain = parking_lot::Mutex::new(ProjectBrain::new(db.clone()));
        let workspace_root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let lsp_multiplexer = Arc::new(lsp_client::LspMultiplexer::new(workspace_root));
        let terminal_manager = Arc::new(terminal_manager::TerminalManager::new());

        Arc::new(DaemonState {
            config,
            session_token,
            db,
            model_gateway,
            agent_orchestrator,
            project_brain,
            indexer,
            lsp_multiplexer,
            terminal_manager,
        })
    }

    #[tokio::test]
    async fn test_unauthenticated_request_rejected() {
        let state = create_test_state();
        let envelope = IpcEnvelope {
            id: "test-unauth-1".into(),
            session_token: None,
            payload: IpcPayload::Request(IpcRequest::Ping),
        };

        let resp = handle_ipc_envelope(&state, envelope, "127.0.0.1:9999").await;
        match resp.payload {
            IpcPayload::Response(r) => {
                assert!(!r.success, "Unauthenticated request should fail");
                assert_eq!(r.status_code, Some(401));
                assert!(r.error.unwrap().contains("401 Unauthorized"));
            }
            _ => panic!("Expected response payload"),
        }
    }

    #[tokio::test]
    async fn test_invalid_token_request_rejected() {
        let state = create_test_state();
        let envelope = IpcEnvelope {
            id: "test-invalid-2".into(),
            session_token: Some("invalid-token-12345".into()),
            payload: IpcPayload::Request(IpcRequest::Ping),
        };

        let resp = handle_ipc_envelope(&state, envelope, "127.0.0.1:9999").await;
        match resp.payload {
            IpcPayload::Response(r) => {
                assert!(!r.success, "Invalid token request should fail");
                assert_eq!(r.status_code, Some(401));
                assert!(r.error.unwrap().contains("401 Unauthorized"));
            }
            _ => panic!("Expected response payload"),
        }
    }

    #[tokio::test]
    async fn test_valid_token_ping_succeeds() {
        let state = create_test_state();
        let envelope = IpcEnvelope {
            id: "test-valid-3".into(),
            session_token: Some(state.session_token.clone()),
            payload: IpcPayload::Request(IpcRequest::Ping),
        };

        let resp = handle_ipc_envelope(&state, envelope, "127.0.0.1:9999").await;
        match resp.payload {
            IpcPayload::Response(r) => {
                assert!(r.success, "Valid token request should succeed");
                assert_eq!(r.status_code, Some(200));
                let data = r.data.unwrap();
                assert_eq!(data["pong"], true);
            }
            _ => panic!("Expected response payload"),
        }
    }

    #[tokio::test]
    async fn test_unauthenticated_run_command_rejected() {
        let state = create_test_state();
        let envelope = IpcEnvelope {
            id: "test-unauth-cmd-4".into(),
            session_token: None,
            payload: IpcPayload::Request(IpcRequest::RunCommand {
                command: "echo malicious".into(),
                cwd: None,
                timeout_secs: Some(5),
            }),
        };

        let resp = handle_ipc_envelope(&state, envelope, "127.0.0.1:9999").await;
        match resp.payload {
            IpcPayload::Response(r) => {
                assert!(
                    !r.success,
                    "Unauthenticated command execution must be blocked"
                );
                assert_eq!(r.status_code, Some(401));
                assert!(r.error.unwrap().contains("401 Unauthorized"));
            }
            _ => panic!("Expected response payload"),
        }
    }

    #[tokio::test]
    async fn test_authenticated_lsp_diagnostics_query() {
        let state = create_test_state();
        let envelope = IpcEnvelope {
            id: "test-lsp-diag-5".into(),
            session_token: Some(state.session_token.clone()),
            payload: IpcPayload::Request(IpcRequest::LspDiagnostics {
                path: "src/main.rs".into(),
            }),
        };

        let resp = handle_ipc_envelope(&state, envelope, "127.0.0.1:9999").await;
        match resp.payload {
            IpcPayload::Response(r) => {
                assert!(
                    r.success,
                    "Authenticated LSP diagnostics query should succeed"
                );
                assert_eq!(r.status_code, Some(200));
                assert!(r.data.unwrap().is_array());
            }
            _ => panic!("Expected response payload"),
        }
    }

    #[tokio::test]
    async fn test_authenticated_terminal_spawn_and_close() {
        let state = create_test_state();

        // Spawn
        let spawn_envelope = IpcEnvelope {
            id: "test-term-spawn-6".into(),
            session_token: Some(state.session_token.clone()),
            payload: IpcPayload::Request(IpcRequest::TerminalSpawn {
                shell: Some("cmd".into()),
                cwd: None,
                cols: Some(80),
                rows: Some(24),
            }),
        };

        let spawn_resp = handle_ipc_envelope(&state, spawn_envelope, "127.0.0.1:9999").await;
        let session_id = match spawn_resp.payload {
            IpcPayload::Response(r) => {
                assert!(
                    r.success,
                    "Authenticated TerminalSpawn should succeed: {:?}",
                    r.error
                );
                let data = r.data.unwrap();
                data["id"].as_u64().unwrap() as u32
            }
            _ => panic!("Expected response payload"),
        };

        // Close
        let close_envelope = IpcEnvelope {
            id: "test-term-close-7".into(),
            session_token: Some(state.session_token.clone()),
            payload: IpcPayload::Request(IpcRequest::TerminalClose { session_id }),
        };

        let close_resp = handle_ipc_envelope(&state, close_envelope, "127.0.0.1:9999").await;
        match close_resp.payload {
            IpcPayload::Response(r) => {
                assert!(r.success, "TerminalClose should succeed");
            }
            _ => panic!("Expected response payload"),
        }
    }

    #[tokio::test]
    async fn test_authenticated_sync_generate_pairing() {
        let state = create_test_state();
        let envelope = IpcEnvelope {
            id: "test-sync-pair-8".into(),
            session_token: Some(state.session_token.clone()),
            payload: IpcPayload::Request(IpcRequest::SyncGeneratePairing {
                device_name: Some("Test-Device".into()),
                host: Some("192.168.1.100".into()),
                port: Some(44888),
            }),
        };

        let resp = handle_ipc_envelope(&state, envelope, "127.0.0.1:9999").await;
        match resp.payload {
            IpcPayload::Response(r) => {
                assert!(
                    r.success,
                    "SyncGeneratePairing should succeed: {:?}",
                    r.error
                );
                let data = r.data.unwrap();
                assert_eq!(data["device_name"], "Test-Device");
                assert!(data["qr_uri"]
                    .as_str()
                    .unwrap()
                    .starts_with("aether://pair?"));
                assert!(data["pairing_payload"]["psk_hex"].is_string());
            }
            _ => panic!("Expected response payload"),
        }
    }

    #[tokio::test]
    async fn test_authenticated_sync_get_manifest() {
        let state = create_test_state();
        let current_dir = std::env::current_dir().unwrap();
        let envelope = IpcEnvelope {
            id: "test-sync-manifest-9".into(),
            session_token: Some(state.session_token.clone()),
            payload: IpcPayload::Request(IpcRequest::SyncGetManifest {
                workspace_path: current_dir.to_string_lossy().to_string(),
                workspace_name: Some("test-ws".into()),
            }),
        };

        let resp = handle_ipc_envelope(&state, envelope, "127.0.0.1:9999").await;
        match resp.payload {
            IpcPayload::Response(r) => {
                assert!(r.success, "SyncGetManifest should succeed: {:?}", r.error);
                let data = r.data.unwrap();
                assert_eq!(data["workspace_name"], "test-ws");
                assert!(data["root_hash"].is_string());
                assert!(data["entries"].is_object());
            }
            _ => panic!("Expected response payload"),
        }
    }
}
