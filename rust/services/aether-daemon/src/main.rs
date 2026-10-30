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

    let state = Arc::new(DaemonState {
        config: config.clone(),
        session_token: session_token.clone(),
        db,
        model_gateway,
        agent_orchestrator,
        project_brain,
        indexer,
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

        Arc::new(DaemonState {
            config,
            session_token,
            db,
            model_gateway,
            agent_orchestrator,
            project_brain,
            indexer,
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
}
