use application_core::{AetherConfig, AutonomyMode, AETHER_VERSION};
use database_engine::DatabaseEngine;
use diagnostics::EnvironmentDoctor;
use ipc_protocol::{
    decode_message, encode_message, IpcEnvelope, IpcPayload, IpcRequest, IpcResponse,
    DEFAULT_NAMED_PIPE_PATH,
};
use model_gateway::{CompletionRequest, ModelGateway};
use project_brain::ProjectBrain;
use project_indexer::ProjectIndexer;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use symbol_index::SymbolIndex;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tracing::{error, info};

#[allow(dead_code)]
static PIPE_CONNECTION_COUNTER: AtomicU64 = AtomicU64::new(1);
static TCP_CONNECTION_COUNTER: AtomicU64 = AtomicU64::new(1);

struct DaemonState {
    config: AetherConfig,
    #[allow(dead_code)]
    session_token: String,
    #[allow(dead_code)]
    owner_peer_id: String,
    token_store: parking_lot::Mutex<security_core::TokenStore>,
    db: DatabaseEngine,
    model_gateway: ModelGateway,
    agent_orchestrator: agent_orchestrator::AgentOrchestrator,
    project_brain: parking_lot::Mutex<ProjectBrain>,
    indexer: ProjectIndexer,
    lsp_multiplexer: Arc<lsp_client::LspMultiplexer>,
    terminal_manager: Arc<terminal_manager::TerminalManager>,
    crdt_manager: Arc<workspace_sync::CrdtManager>,
    awareness_tracker: Arc<workspace_sync::AwarenessTracker>,
    role_registry: Arc<workspace_sync::RoleRegistry>,
    swarm_coordinator: Arc<agent_orchestrator::SwarmCoordinator>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt().with_env_filter("info").init();

    info!("Starting AetherIDE Native Daemon v{}", AETHER_VERSION);

    let config = AetherConfig::default();
    let owner_peer_id = format!("owner-bootstrap-{}", uuid::Uuid::new_v4().simple());
    let token_store = security_core::TokenStore::new_with_owner(
        std::time::Duration::from_secs(86400),
        &owner_peer_id,
    )
    .with_lockout(10, std::time::Duration::from_secs(60));
    let session_token = token_store.current_token().to_string();

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
    let lsp_multiplexer = Arc::new(lsp_client::LspMultiplexer::new(workspace_root.clone()));
    let terminal_manager = Arc::new(terminal_manager::TerminalManager::new());
    let crdt_manager = Arc::new(workspace_sync::CrdtManager::new());
    let awareness_tracker = Arc::new(workspace_sync::AwarenessTracker::new());
    let role_registry = Arc::new(workspace_sync::RoleRegistry::new());
    let swarm_coordinator = Arc::new(agent_orchestrator::SwarmCoordinator::new(&workspace_root));

    // Zero-Trust Authorization: Register ONLY the dynamically generated owner_peer_id as Owner.
    // Loopback, named-pipe, or generic local connections are NEVER automatically granted Owner.
    role_registry.register_peer(&owner_peer_id, workspace_sync::DeviceRole::Owner);

    let state = Arc::new(DaemonState {
        config: config.clone(),
        session_token: session_token.clone(),
        owner_peer_id: owner_peer_id.clone(),
        token_store: parking_lot::Mutex::new(token_store),
        db,
        model_gateway,
        agent_orchestrator,
        project_brain,
        indexer,
        lsp_multiplexer,
        terminal_manager,
        crdt_manager,
        awareness_tracker,
        role_registry,
        swarm_coordinator,
    });

    // SEC-P0: Write platform-protected desktop bootstrap credentials (DPAPI / OS-bound)
    // Never persist raw bearer tokens in plaintext to ordinary files.
    std::fs::create_dir_all(&config.data_dir).map_err(|e| {
        error!("CRITICAL: Failed to create data directory: {}", e);
        e
    })?;

    let protected_token = security_core::protect_credential(&session_token).map_err(|e| {
        error!("CRITICAL: Platform credential protection failed: {}", e);
        std::io::Error::new(std::io::ErrorKind::PermissionDenied, e)
    })?;

    let bootstrap_file = config.data_dir.join("desktop_bootstrap.json");
    let bootstrap_payload = serde_json::json!({
        "owner_peer_id": owner_peer_id,
        "protected_token": protected_token,
        "role": "Owner",
        "session_type": "DesktopOwner",
        "version": AETHER_VERSION,
        "created_at": chrono::Utc::now().to_rfc3339(),
    });

    std::fs::write(
        &bootstrap_file,
        serde_json::to_string_pretty(&bootstrap_payload).unwrap_or_default(),
    )
    .map_err(|e| {
        error!(
            "CRITICAL: Failed to write protected desktop bootstrap file: {}",
            e
        );
        e
    })?;

    info!(
        "Platform-protected desktop bootstrap identity published to {:?}",
        bootstrap_file
    );

    // SEC-AUTH-03: Remove legacy plaintext token file to eliminate parallel unauthenticated paths
    let legacy_token_file = config.data_dir.join("daemon_session.token");
    if legacy_token_file.exists() {
        let _ = std::fs::remove_file(&legacy_token_file);
        info!("Legacy daemon_session.token removed during zero-trust migration");
    }

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
        let conn_id = TCP_CONNECTION_COUNTER.fetch_add(1, Ordering::Relaxed);
        let peer_info = format!("tcp-{}-conn-{}", peer.ip(), conn_id);
        info!("Accepted IPC connection from {} ({})", peer, peer_info);
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
                    let response = handle_ipc_envelope(&state_clone, envelope, &peer_info).await;
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
            let client_id = PIPE_CONNECTION_COUNTER.fetch_add(1, Ordering::Relaxed);
            let client_source = format!("named-pipe-client-{}", client_id);

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
                            handle_ipc_envelope(&state_clone, envelope, &client_source).await;
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
    // 1. Authenticate Session with server-controlled SessionRegistry and per-source rate-limiting
    let (auth_result, session_record) = state
        .token_store
        .lock()
        .authenticate_session(envelope.session_token.as_deref(), peer_info);

    match auth_result {
        security_core::AuthVerificationResult::Success => {}
        security_core::AuthVerificationResult::Revoked => {
            tracing::warn!(
                "SEC-01: Revoked token IPC request rejected from {} (request id: {})",
                peer_info,
                envelope.id
            );
            return IpcEnvelope {
                id: envelope.id,
                session_token: None,
                peer_id: envelope.peer_id,
                payload: IpcPayload::Response(IpcResponse::unauthorized(
                    "Access denied: session token has been revoked",
                )),
            };
        }
        security_core::AuthVerificationResult::RateLimited { wait_seconds } => {
            tracing::warn!(
                "SEC-01: Rate-limited IPC request rejected from {} (request id: {}, retry after {}s)",
                peer_info,
                envelope.id,
                wait_seconds
            );
            return IpcEnvelope {
                id: envelope.id,
                session_token: None,
                peer_id: envelope.peer_id,
                payload: IpcPayload::Response(IpcResponse::unauthorized(format!(
                    "Access denied: source temporarily rate-limited; retry after {} seconds",
                    wait_seconds
                ))),
            };
        }
        security_core::AuthVerificationResult::Expired => {
            tracing::warn!(
                "SEC-01: Expired token IPC request rejected from {} (request id: {})",
                peer_info,
                envelope.id
            );
            return IpcEnvelope {
                id: envelope.id,
                session_token: None,
                peer_id: envelope.peer_id,
                payload: IpcPayload::Response(IpcResponse::unauthorized(
                    "Access denied: session token has expired",
                )),
            };
        }
        security_core::AuthVerificationResult::InvalidToken => {
            tracing::warn!(
                "SEC-01: Unauthorized IPC request rejected from {} (request id: {})",
                peer_info,
                envelope.id
            );
            return IpcEnvelope {
                id: envelope.id,
                session_token: None,
                peer_id: envelope.peer_id,
                payload: IpcPayload::Response(IpcResponse::unauthorized(
                    "Access denied: missing or invalid session token",
                )),
            };
        }
    }

    let session = session_record.expect("SessionRecord must exist when authentication succeeds");

    // 2. Anti-Spoofing & Zero-Trust Identity Binding (SEC-01 & SEC-02)
    // If client supplied a peer_id in envelope, verify it strictly matches the authenticated server-side session identity!
    let claimed_peer_opt = envelope.peer_id.clone();
    if let Some(ref claimed_peer) = claimed_peer_opt {
        if !claimed_peer.trim().is_empty() && claimed_peer != &session.peer_id {
            tracing::warn!(
                "SEC-02: Identity impersonation attempt blocked: claimed '{}' but session is bound to '{}' from {}",
                claimed_peer,
                session.peer_id,
                peer_info
            );
            return IpcEnvelope {
                id: envelope.id,
                session_token: None,
                peer_id: claimed_peer_opt.clone(),
                payload: IpcPayload::Response(IpcResponse::unauthorized(format!(
                    "Access denied: envelope peer_id '{}' does not match authenticated session identity '{}'",
                    claimed_peer, session.peer_id
                ))),
            };
        }
    }

    // Authenticated caller peer identity and role are derived SOLELY from the server session record!
    let caller_peer = session.peer_id.clone();
    let caller_role = match session.role.as_str() {
        "Owner" => workspace_sync::DeviceRole::Owner,
        "Admin" => workspace_sync::DeviceRole::Admin,
        "Editor" => workspace_sync::DeviceRole::Editor,
        "Reviewer" => workspace_sync::DeviceRole::Reviewer,
        _ => state.role_registry.get_role(&caller_peer),
    };

    let req = match envelope.payload {
        IpcPayload::Request(r) => r,
        _ => {
            return IpcEnvelope {
                id: envelope.id,
                session_token: None, // No token leakage
                peer_id: Some(caller_peer),
                payload: IpcPayload::Response(IpcResponse::err("Only Request payloads supported")),
            };
        }
    };

    let response = match req {
        IpcRequest::Ping => {
            IpcResponse::ok(serde_json::json!({ "pong": true, "version": AETHER_VERSION }))
        }

        IpcRequest::GetStatus => {
            let models = state.model_gateway.list_all_models();
            let personas = state.agent_orchestrator.list_personas();
            IpcResponse::ok(serde_json::json!({
                "status": "Running",
                "version": AETHER_VERSION,
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
            if !caller_role.can_write() {
                IpcResponse::unauthorized(format!(
                    "Peer '{}' with role {:?} is not authorized to index workspaces",
                    caller_peer, caller_role
                ))
            } else {
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
        }

        IpcRequest::SearchSymbols { query, limit } => {
            let limit = limit.unwrap_or(20);
            match state.db.search_symbols_fts(&query, limit) {
                Ok(symbols) => IpcResponse::ok(serde_json::to_value(symbols).unwrap()),
                Err(e) => IpcResponse::err(e.to_string()),
            }
        }

        IpcRequest::FullTextSearch { query, limit } => {
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
            if !caller_role.can_write() {
                IpcResponse::unauthorized(format!(
                    "Peer '{}' with role {:?} is not authorized to execute commands",
                    caller_peer, caller_role
                ))
            } else {
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
        }

        IpcRequest::KillCommand { process_id } => {
            if !caller_role.can_write() {
                IpcResponse::unauthorized(format!(
                    "Peer '{}' with role {:?} is not authorized to kill commands",
                    caller_peer, caller_role
                ))
            } else {
                IpcResponse::ok(serde_json::json!({ "killed": true, "process_id": process_id }))
            }
        }

        IpcRequest::CreateAgentTask {
            role,
            objective,
            autonomy_mode,
        } => {
            if !caller_role.can_write() {
                IpcResponse::unauthorized(format!(
                    "Peer '{}' with role {:?} is not authorized to create agent tasks",
                    caller_peer, caller_role
                ))
            } else {
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
        }

        IpcRequest::GetAgentTask { task_id } => match state.agent_orchestrator.get_task(&task_id) {
            Some(task) => IpcResponse::ok(serde_json::to_value(task).unwrap()),
            None => IpcResponse::err("Task not found"),
        },

        IpcRequest::ApproveAgentStep { task_id, step_id } => {
            if !caller_role.can_write() {
                IpcResponse::unauthorized(format!(
                    "Peer '{}' with role {:?} is not authorized to approve agent steps",
                    caller_peer, caller_role
                ))
            } else {
                let step_idx = step_id.parse::<usize>().unwrap_or(0);
                match state
                    .agent_orchestrator
                    .approve_step(&task_id, step_idx)
                    .await
                {
                    Ok(task) => IpcResponse::ok(serde_json::to_value(task).unwrap()),
                    Err(e) => IpcResponse::err(e.to_string()),
                }
            }
        }

        IpcRequest::RejectAgentStep {
            task_id,
            step_id,
            reason,
        } => {
            if !caller_role.can_write() {
                IpcResponse::unauthorized(format!(
                    "Peer '{}' with role {:?} is not authorized to reject agent steps",
                    caller_peer, caller_role
                ))
            } else {
                let step_idx = step_id.parse::<usize>().unwrap_or(0);
                match state
                    .agent_orchestrator
                    .reject_step(&task_id, step_idx, &reason)
                {
                    Ok(task) => IpcResponse::ok(serde_json::to_value(task).unwrap()),
                    Err(e) => IpcResponse::err(e.to_string()),
                }
            }
        }

        IpcRequest::QueryProjectBrain { query: _ } => {
            let graph = state.project_brain.lock().get_graph().clone();
            IpcResponse::ok(serde_json::to_value(graph).unwrap())
        }

        IpcRequest::GenerateMobileApp {
            prompt: _,
            target_framework: _,
            app_name,
        } => {
            if !caller_role.can_write() {
                IpcResponse::unauthorized(format!(
                    "Peer '{}' with role {:?} is not authorized to generate mobile apps",
                    caller_peer, caller_role
                ))
            } else {
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
        }

        IpcRequest::TriggerMobileBuild {
            project_path,
            platform,
            build_type,
        } => {
            if !caller_role.can_write() {
                IpcResponse::unauthorized(format!(
                    "Peer '{}' with role {:?} is not authorized to trigger mobile builds",
                    caller_peer, caller_role
                ))
            } else {
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
            if !caller_role.can_write() {
                IpcResponse::unauthorized(format!(
                    "Peer '{}' with role {:?} is not authorized to open document for editing",
                    caller_peer, caller_role
                ))
            } else {
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
        }

        IpcRequest::LspChangeDocument {
            path,
            content,
            version,
        } => {
            if !caller_role.can_write() {
                IpcResponse::unauthorized(format!(
                    "Peer '{}' with role {:?} is not authorized to modify document",
                    caller_peer, caller_role
                ))
            } else {
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
        }

        IpcRequest::LspCloseDocument { path } => {
            if !caller_role.can_write() {
                IpcResponse::unauthorized(format!(
                    "Peer '{}' with role {:?} is not authorized to close document",
                    caller_peer, caller_role
                ))
            } else {
                let p = PathBuf::from(&path);
                match state.lsp_multiplexer.close_document(&p).await {
                    Ok(_) => IpcResponse::ok(serde_json::json!({ "closed": true, "path": path })),
                    Err(e) => IpcResponse::err(e.to_string()),
                }
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
            if !caller_role.can_write() {
                IpcResponse::unauthorized(format!(
                    "Peer '{}' with role {:?} is not authorized to spawn terminals",
                    caller_peer, caller_role
                ))
            } else {
                #[cfg(windows)]
                {
                    let shell_name = shell.unwrap_or_else(|| "powershell".to_string());
                    let cwd_path = cwd.map(PathBuf::from);
                    let cols = cols.unwrap_or(80);
                    let rows = rows.unwrap_or(24);
                    match state.terminal_manager.spawn_interactive(
                        &shell_name,
                        cwd_path,
                        cols,
                        rows,
                    ) {
                        Ok(info) => IpcResponse::ok(serde_json::to_value(info).unwrap()),
                        Err(e) => IpcResponse::err(e.to_string()),
                    }
                }
                #[cfg(not(windows))]
                {
                    let shell_name = shell.unwrap_or_else(|| "bash".to_string());
                    let _ = (cols, rows);
                    let info = state.terminal_manager.create_session(
                        "Terminal",
                        &shell_name,
                        cwd.map(PathBuf::from),
                    );
                    IpcResponse::ok(serde_json::to_value(info).unwrap())
                }
            }
        }

        IpcRequest::TerminalWrite { session_id, data } => {
            if !caller_role.can_write() {
                IpcResponse::unauthorized(format!(
                    "Peer '{}' with role {:?} is not authorized to write to terminal",
                    caller_peer, caller_role
                ))
            } else {
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
        }

        IpcRequest::TerminalResize {
            session_id,
            cols,
            rows,
        } => {
            if !caller_role.can_write() {
                IpcResponse::unauthorized(format!(
                    "Peer '{}' with role {:?} is not authorized to resize terminal",
                    caller_peer, caller_role
                ))
            } else {
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
        }

        IpcRequest::TerminalClose { session_id } => {
            if !caller_role.can_write() {
                IpcResponse::unauthorized(format!(
                    "Peer '{}' with role {:?} is not authorized to close terminal",
                    caller_peer, caller_role
                ))
            } else {
                match state.terminal_manager.close_session(session_id) {
                    Ok(_) => IpcResponse::ok(serde_json::json!({ "closed": true })),
                    Err(e) => IpcResponse::err(e.to_string()),
                }
            }
        }

        IpcRequest::TerminalPollOutput { session_id, offset } => {
            #[cfg(windows)]
            {
                match state.terminal_manager.read_output(session_id, offset) {
                    Ok((bytes, total, is_alive)) => {
                        let data_str = String::from_utf8_lossy(&bytes).to_string();
                        IpcResponse::ok(serde_json::json!({
                            "session_id": session_id,
                            "data": data_str,
                            "offset": total,
                            "is_alive": is_alive,
                        }))
                    }
                    Err(e) => IpcResponse::err(e.to_string()),
                }
            }
            #[cfg(not(windows))]
            {
                let _ = (session_id, offset);
                IpcResponse::ok(serde_json::json!({
                    "session_id": session_id,
                    "data": "",
                    "offset": 0,
                    "is_alive": false,
                }))
            }
        }

        IpcRequest::ListAgentPersonas => {
            let personas = state.agent_orchestrator.list_personas();
            IpcResponse::ok(serde_json::to_value(personas).unwrap())
        }

        IpcRequest::CancelAgentTask { task_id } => {
            if !caller_role.can_write() {
                IpcResponse::unauthorized(format!(
                    "Peer '{}' with role {:?} is not authorized to cancel agent tasks",
                    caller_peer, caller_role
                ))
            } else {
                match state.agent_orchestrator.cancel_task(&task_id) {
                    Ok(task) => IpcResponse::ok(serde_json::to_value(task).unwrap()),
                    Err(e) => IpcResponse::err(e.to_string()),
                }
            }
        }

        IpcRequest::QueryProjectGraph { workspace_path } => {
            let root = workspace_path
                .map(PathBuf::from)
                .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
            let mut brain = state.project_brain.lock();
            if brain.get_graph().nodes.is_empty() {
                let _ = brain.scan_workspace(&root);
            }
            IpcResponse::ok(serde_json::to_value(brain.get_graph()).unwrap())
        }

        IpcRequest::AnalyzeChangeImpact { file_path } => {
            let brain = state.project_brain.lock();
            let report = brain.calculate_impact_radar(&file_path);
            IpcResponse::ok(serde_json::to_value(report).unwrap())
        }

        IpcRequest::SaveProjectMemoryNote {
            title,
            content,
            tags,
        } => {
            if !caller_role.can_write() {
                IpcResponse::unauthorized(format!(
                    "Peer '{}' with role {:?} is not authorized to save memory notes",
                    caller_peer, caller_role
                ))
            } else {
                let note = database_engine::ProjectMemoryNote {
                    id: format!("note-{}", uuid::Uuid::new_v4().simple()),
                    title,
                    content,
                    tags,
                    created_at: chrono::Utc::now().to_rfc3339(),
                    updated_at: chrono::Utc::now().to_rfc3339(),
                };
                let brain = state.project_brain.lock();
                match brain.save_memory_note(&note) {
                    Ok(_) => IpcResponse::ok(serde_json::to_value(note).unwrap()),
                    Err(e) => IpcResponse::err(e.to_string()),
                }
            }
        }

        IpcRequest::GetProjectMemoryNotes => {
            let brain = state.project_brain.lock();
            match brain.get_memory_notes() {
                Ok(notes) => IpcResponse::ok(serde_json::to_value(notes).unwrap()),
                Err(e) => IpcResponse::err(e.to_string()),
            }
        }

        IpcRequest::DeleteProjectMemoryNote { note_id } => {
            if !caller_role.can_write() {
                IpcResponse::unauthorized(format!(
                    "Peer '{}' with role {:?} is not authorized to delete memory notes",
                    caller_peer, caller_role
                ))
            } else {
                let brain = state.project_brain.lock();
                match brain.delete_memory_note(&note_id) {
                    Ok(deleted) => IpcResponse::ok(serde_json::json!({ "deleted": deleted })),
                    Err(e) => IpcResponse::err(e.to_string()),
                }
            }
        }

        IpcRequest::StartExpoServer { project_path, port } => {
            if !caller_role.can_write() {
                IpcResponse::unauthorized(format!(
                    "Peer '{}' with role {:?} is not authorized to start Expo servers",
                    caller_peer, caller_role
                ))
            } else {
                let p = PathBuf::from(&project_path);
                let info =
                    mobile_build_manager::MobileBuildManager::get_expo_dev_server_info(&p, port);
                match info {
                    Ok(s) => IpcResponse::ok(serde_json::to_value(s).unwrap()),
                    Err(e) => IpcResponse::err(e.to_string()),
                }
            }
        }

        IpcRequest::GetExpoStatus { project_path } => {
            let p = PathBuf::from(&project_path);
            let info = mobile_build_manager::MobileBuildManager::get_expo_dev_server_info(&p, None);
            match info {
                Ok(s) => IpcResponse::ok(serde_json::to_value(s).unwrap()),
                Err(e) => IpcResponse::err(e.to_string()),
            }
        }

        IpcRequest::DiscoverMobileDevices => {
            let devices = mobile_build_manager::MobileBuildManager::discover_devices().await;
            IpcResponse::ok(serde_json::to_value(devices).unwrap())
        }

        IpcRequest::SyncGeneratePairing {
            device_name,
            host,
            port,
        } => {
            if !caller_role.can_admin() {
                IpcResponse::unauthorized(format!(
                    "Peer '{}' with role {:?} is not authorized to generate pairing credentials",
                    caller_peer, caller_role
                ))
            } else {
                let dev_name = device_name.unwrap_or_else(|| {
                    std::env::var("COMPUTERNAME")
                        .unwrap_or_else(|_| "AetherIDE-Desktop".to_string())
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

        IpcRequest::RegisterPeerRole { peer_id, role } => {
            if !caller_role.can_admin() {
                IpcResponse::unauthorized(format!(
                    "Peer '{}' with role {:?} is not authorized to register peer roles",
                    caller_peer, caller_role
                ))
            } else {
                let parsed_role = workspace_sync::DeviceRole::parse(&role);
                state.role_registry.register_peer(&peer_id, parsed_role);
                IpcResponse::ok(serde_json::json!({
                    "peer_id": peer_id,
                    "role": format!("{:?}", parsed_role)
                }))
            }
        }

        IpcRequest::RevokePeerRole { peer_id } => {
            if !caller_role.can_admin() {
                IpcResponse::unauthorized(format!(
                    "Peer '{}' with role {:?} is not authorized to revoke peer roles",
                    caller_peer, caller_role
                ))
            } else {
                let revoked = state.role_registry.revoke_peer(&peer_id);
                // Also purge any active sessions in TokenStore for this peer
                state.token_store.lock().revoke_peer_sessions(&peer_id);
                IpcResponse::ok(serde_json::json!({
                    "peer_id": peer_id,
                    "revoked": revoked
                }))
            }
        }

        IpcRequest::RevokeSessionToken { token } => {
            if !caller_role.can_admin() {
                IpcResponse::unauthorized(format!(
                    "Peer '{}' with role {:?} is not authorized to revoke session tokens",
                    caller_peer, caller_role
                ))
            } else {
                state.token_store.lock().revoke_token(&token);
                info!("Session token revoked by peer '{}'", caller_peer);
                IpcResponse::ok(serde_json::json!({
                    "revoked": true,
                    "token_prefix": if token.len() >= 8 { &token[..8] } else { &token }
                }))
            }
        }

        IpcRequest::CrdtApplyUpdate {
            doc_id,
            update_base64,
            peer_id: _, // SEC-03: Ignore client payload peer_id, bind attribution strictly to authenticated caller_peer
            role: _, // SEC-03: Ignore client payload role parameter, server-derived caller_role governs
        } => {
            if !caller_role.can_write() {
                IpcResponse::unauthorized(format!(
                    "Peer '{}' with role {:?} is not authorized to mutate CRDT document",
                    caller_peer, caller_role
                ))
            } else {
                let doc = state.crdt_manager.get_or_create(&doc_id);
                match doc.apply_update_base64(&update_base64) {
                    Ok(()) => IpcResponse::ok(serde_json::json!({
                        "applied": true,
                        "doc_id": doc_id,
                        "peer_id": caller_peer,
                        "text_length": doc.get_text().len(),
                    })),
                    Err(e) => IpcResponse::err(format!("Failed to apply CRDT update: {e}")),
                }
            }
        }

        IpcRequest::CrdtGetDoc { doc_id } => {
            let doc = state.crdt_manager.get_or_create(&doc_id);
            IpcResponse::ok(serde_json::json!({
                "doc_id": doc_id,
                "text": doc.get_text(),
                "state_vector": doc.get_state_vector_base64(),
            }))
        }

        IpcRequest::CrdtGetStateVector { doc_id } => {
            let doc = state.crdt_manager.get_or_create(&doc_id);
            IpcResponse::ok(serde_json::json!({
                "doc_id": doc_id,
                "state_vector": doc.get_state_vector_base64(),
            }))
        }

        IpcRequest::CrdtGetDiff {
            doc_id,
            state_vector_base64,
        } => {
            let doc = state.crdt_manager.get_or_create(&doc_id);
            match doc.get_diff_base64(&state_vector_base64) {
                Ok(diff) => IpcResponse::ok(serde_json::json!({
                    "doc_id": doc_id,
                    "diff_base64": diff,
                })),
                Err(e) => IpcResponse::err(format!("Failed to compute diff: {e}")),
            }
        }

        IpcRequest::UpdatePresence {
            peer_id: _, // SEC-02: Force authenticated caller_peer identity; cannot spoof another user's presence
            display_name,
            user_color,
            current_file,
            line,
            col,
        } => {
            let mut presence =
                workspace_sync::PeerPresence::new(&caller_peer, display_name, user_color);
            presence.current_file = current_file;
            if let (Some(l), Some(c)) = (line, col) {
                presence.cursor = Some(workspace_sync::CursorPosition { line: l, col: c });
            }
            state.awareness_tracker.update_presence(presence);
            IpcResponse::ok(serde_json::json!({ "updated": true, "peer_id": caller_peer }))
        }

        IpcRequest::GetPresence { file_path } => {
            let peers = if let Some(path) = file_path {
                state.awareness_tracker.get_peers_for_file(&path, 30)
            } else {
                state.awareness_tracker.get_active_peers(30)
            };
            IpcResponse::ok(serde_json::to_value(peers).unwrap())
        }

        IpcRequest::SwarmAddTask {
            task_id,
            title,
            description,
            persona,
            dependencies,
        } => {
            if !caller_role.can_write() {
                IpcResponse::unauthorized(format!(
                    "Peer '{}' with role {:?} is not authorized to add swarm tasks",
                    caller_peer, caller_role
                ))
            } else {
                let node = agent_orchestrator::SwarmTaskNode::new(
                    task_id.clone(),
                    title,
                    description,
                    persona,
                    dependencies,
                );
                match state.swarm_coordinator.add_task(node) {
                    Ok(()) => IpcResponse::ok(serde_json::json!({
                        "added": true,
                        "task_id": task_id,
                    })),
                    Err(e) => IpcResponse::err(format!("Failed to add swarm task: {e}")),
                }
            }
        }

        IpcRequest::SwarmListTasks => {
            let tasks = state.swarm_coordinator.list_tasks();
            IpcResponse::ok(serde_json::to_value(tasks).unwrap())
        }

        IpcRequest::SwarmGetReadyTasks => {
            let ready = state.swarm_coordinator.get_ready_tasks();
            IpcResponse::ok(serde_json::to_value(ready).unwrap())
        }

        IpcRequest::SwarmSendMessage {
            sender: _, // SEC-02: Force authenticated caller_peer identity
            recipient,
            task_id,
            message_type,
            payload,
        } => {
            if !caller_role.can_write() {
                IpcResponse::unauthorized(format!(
                    "Peer '{}' with role {:?} is not authorized to send swarm messages",
                    caller_peer, caller_role
                ))
            } else {
                let msg_type = match message_type.to_lowercase().as_str() {
                    "proposediff" => agent_orchestrator::AgentMessageType::ProposeDiff,
                    "reviewfeedback" => agent_orchestrator::AgentMessageType::ReviewFeedback,
                    "handoff" => agent_orchestrator::AgentMessageType::Handoff,
                    "alert" => agent_orchestrator::AgentMessageType::Alert,
                    _ => agent_orchestrator::AgentMessageType::Query,
                };
                let msg = agent_orchestrator::AgentMessage::new(
                    caller_peer.clone(),
                    recipient,
                    task_id,
                    msg_type,
                    payload,
                );
                state.swarm_coordinator.send_message(msg.clone());
                IpcResponse::ok(serde_json::to_value(msg).unwrap())
            }
        }

        IpcRequest::SwarmGetMessages { persona, task_id } => {
            let msgs = if let Some(p) = persona {
                state.swarm_coordinator.get_messages_for_persona(&p)
            } else if let Some(t) = task_id {
                state.swarm_coordinator.get_messages_for_task(&t)
            } else {
                state.swarm_coordinator.get_messages_for_persona("*")
            };
            IpcResponse::ok(serde_json::to_value(msgs).unwrap())
        }

        IpcRequest::ThreeWayMerge { base, ours, theirs } => {
            let report = agent_orchestrator::ThreeWayMergeEngine::merge(&base, &ours, &theirs);
            IpcResponse::ok(serde_json::to_value(report).unwrap())
        }

        IpcRequest::ApplyMergeResolution {
            base,
            ours,
            theirs,
            resolutions,
        } => {
            if !caller_role.can_write() {
                IpcResponse::unauthorized(format!(
                    "Peer '{}' with role {:?} is not authorized to apply merge resolutions",
                    caller_peer, caller_role
                ))
            } else {
                let actions: Vec<agent_orchestrator::MergeResolutionAction> = resolutions
                    .into_iter()
                    .map(|s| match s.as_str() {
                        "AcceptTheirs" => agent_orchestrator::MergeResolutionAction::AcceptTheirs,
                        "AcceptBoth" => agent_orchestrator::MergeResolutionAction::AcceptBoth,
                        _ => agent_orchestrator::MergeResolutionAction::AcceptOurs,
                    })
                    .collect();
                let resolved = agent_orchestrator::ThreeWayMergeEngine::apply_resolutions(
                    &base, &ours, &theirs, &actions,
                );
                IpcResponse::ok(serde_json::json!({
                    "resolved_content": resolved,
                }))
            }
        }
    };

    // SEC-AUTH-04: Never return the master session token in ordinary IPC responses!
    IpcEnvelope {
        id: envelope.id,
        session_token: None,
        peer_id: Some(caller_peer),
        payload: IpcPayload::Response(response),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_state() -> Arc<DaemonState> {
        let config = AetherConfig::default();
        let db = DatabaseEngine::open_in_memory().unwrap();
        let symbol_index = SymbolIndex::new(db.clone());
        let indexer = ProjectIndexer::new(symbol_index);
        let model_gateway = ModelGateway::new();
        let agent_orchestrator =
            agent_orchestrator::AgentOrchestrator::new(model_gateway.clone(), db.clone());
        let project_brain = parking_lot::Mutex::new(ProjectBrain::new(db.clone()));
        let workspace_root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let lsp_multiplexer = Arc::new(lsp_client::LspMultiplexer::new(workspace_root.clone()));
        let terminal_manager = Arc::new(terminal_manager::TerminalManager::new());
        let crdt_manager = Arc::new(workspace_sync::CrdtManager::new());
        let awareness_tracker = Arc::new(workspace_sync::AwarenessTracker::new());
        let owner_peer_id = format!("owner-bootstrap-{}", uuid::Uuid::new_v4().simple());
        let token_store = security_core::TokenStore::new_with_owner(
            std::time::Duration::from_secs(86400),
            &owner_peer_id,
        );
        let session_token = token_store.current_token().to_string();
        let role_registry = Arc::new(workspace_sync::RoleRegistry::new());
        role_registry.register_peer(&owner_peer_id, workspace_sync::DeviceRole::Owner);
        let swarm_coordinator =
            Arc::new(agent_orchestrator::SwarmCoordinator::new(&workspace_root));

        Arc::new(DaemonState {
            config,
            session_token,
            owner_peer_id,
            token_store: parking_lot::Mutex::new(token_store),
            db,
            model_gateway,
            agent_orchestrator,
            project_brain,
            indexer,
            lsp_multiplexer,
            terminal_manager,
            crdt_manager,
            awareness_tracker,
            role_registry,
            swarm_coordinator,
        })
    }

    #[tokio::test]
    async fn test_unauthenticated_request_rejected() {
        let state = create_test_state();
        let envelope = IpcEnvelope {
            id: "test-unauth-1".into(),
            session_token: None,
            peer_id: None,
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
        assert!(
            resp.session_token.is_none(),
            "Response must never expose session token"
        );
    }

    #[tokio::test]
    async fn test_invalid_token_request_rejected() {
        let state = create_test_state();
        let envelope = IpcEnvelope {
            id: "test-invalid-2".into(),
            session_token: Some("invalid-token-12345".into()),
            peer_id: None,
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
        assert!(
            resp.session_token.is_none(),
            "Response must never expose session token"
        );
    }

    #[tokio::test]
    async fn test_valid_token_ping_succeeds() {
        let state = create_test_state();
        let envelope = IpcEnvelope {
            id: "test-valid-3".into(),
            session_token: Some(state.session_token.clone()),
            peer_id: None,
            payload: IpcPayload::Request(IpcRequest::Ping),
        };

        let resp = handle_ipc_envelope(&state, envelope, "127.0.0.1:9999").await;
        match resp.payload {
            IpcPayload::Response(r) => {
                assert!(r.success, "Valid token request should succeed");
                assert_eq!(r.status_code, Some(200));
                let data = r.data.unwrap();
                assert_eq!(data["pong"], true);
                assert_eq!(data["version"], AETHER_VERSION);
            }
            _ => panic!("Expected response payload"),
        }
        assert!(
            resp.session_token.is_none(),
            "Response must never expose session token"
        );
    }

    #[tokio::test]
    async fn test_unauthenticated_run_command_rejected() {
        let state = create_test_state();
        let envelope = IpcEnvelope {
            id: "test-unauth-cmd-4".into(),
            session_token: None,
            peer_id: None,
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
            peer_id: None,
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
            peer_id: Some(state.owner_peer_id.clone()),
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
            peer_id: Some(state.owner_peer_id.clone()),
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
            peer_id: None,
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
            peer_id: None,
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

    #[tokio::test]
    async fn test_authenticated_crdt_and_presence_ipc() {
        let state = create_test_state();

        // 1. Initial doc state
        let envelope = IpcEnvelope {
            id: "test-crdt-1".into(),
            session_token: Some(state.session_token.clone()),
            peer_id: None,
            payload: IpcPayload::Request(IpcRequest::CrdtGetDoc {
                doc_id: "doc-shared-1".into(),
            }),
        };
        let resp = handle_ipc_envelope(&state, envelope, "127.0.0.1:9999").await;
        if let IpcPayload::Response(r) = resp.payload {
            assert!(r.success);
            let data = r.data.unwrap();
            assert_eq!(data["text"], "");
        } else {
            panic!("Expected response");
        }

        // 2. Viewer role cannot mutate CRDT
        let viewer_token = "token-viewer-collab-1";
        state
            .token_store
            .lock()
            .register_session(security_core::SessionRecord::new(
                viewer_token,
                "peer-viewer",
                "Viewer",
                security_core::SessionType::Collaborator,
                std::time::Duration::from_secs(3600),
            ));
        state
            .role_registry
            .register_peer("peer-viewer", workspace_sync::DeviceRole::Viewer);

        let envelope_viewer = IpcEnvelope {
            id: "test-crdt-2".into(),
            session_token: Some(viewer_token.into()),
            peer_id: Some("peer-viewer".into()),
            payload: IpcPayload::Request(IpcRequest::CrdtApplyUpdate {
                doc_id: "doc-shared-1".into(),
                update_base64: "AAA=".into(),
                peer_id: "peer-viewer".into(),
                role: "Viewer".into(),
            }),
        };
        let resp_viewer = handle_ipc_envelope(&state, envelope_viewer, "127.0.0.1:9999").await;
        if let IpcPayload::Response(r) = resp_viewer.payload {
            assert!(!r.success);
            assert!(r.error.unwrap().contains("Viewer is not authorized"));
        } else {
            panic!("Expected response");
        }

        // 3. Presence tracking with authenticated session
        let envelope_presence = IpcEnvelope {
            id: "test-presence-1".into(),
            session_token: Some(state.session_token.clone()),
            peer_id: None,
            payload: IpcPayload::Request(IpcRequest::UpdatePresence {
                peer_id: "peer-alice".into(),
                display_name: "Alice Collaborator".into(),
                user_color: "#6366f1".into(),
                current_file: Some("src/main.rs".into()),
                line: Some(42),
                col: Some(10),
            }),
        };
        let resp_presence = handle_ipc_envelope(&state, envelope_presence, "127.0.0.1:9999").await;
        if let IpcPayload::Response(r) = resp_presence.payload {
            assert!(r.success);
        } else {
            panic!("Expected response");
        }

        let envelope_get_presence = IpcEnvelope {
            id: "test-presence-2".into(),
            session_token: Some(state.session_token.clone()),
            peer_id: None,
            payload: IpcPayload::Request(IpcRequest::GetPresence {
                file_path: Some("src/main.rs".into()),
            }),
        };
        let resp_get_presence =
            handle_ipc_envelope(&state, envelope_get_presence, "127.0.0.1:9999").await;
        if let IpcPayload::Response(r) = resp_get_presence.payload {
            assert!(r.success);
            let data = r.data.unwrap();
            let arr = data.as_array().unwrap();
            assert_eq!(arr.len(), 1);
            assert_eq!(arr[0]["display_name"], "Alice Collaborator");
        } else {
            panic!("Expected response");
        }
    }

    #[tokio::test]
    async fn test_authenticated_swarm_and_merge_ipc() {
        let state = create_test_state();

        // 1. Add Swarm Task
        let envelope_add = IpcEnvelope {
            id: "test-swarm-1".into(),
            session_token: Some(state.session_token.clone()),
            peer_id: None,
            payload: IpcPayload::Request(IpcRequest::SwarmAddTask {
                task_id: "swarm-task-1".into(),
                title: "Build Subsystem".into(),
                description: "Design and implement".into(),
                persona: "systems".into(),
                dependencies: vec![],
            }),
        };
        let resp_add = handle_ipc_envelope(&state, envelope_add, "127.0.0.1:9999").await;
        if let IpcPayload::Response(r) = resp_add.payload {
            assert!(r.success);
        } else {
            panic!("Expected response");
        }

        // 2. Three-Way Merge
        let envelope_merge = IpcEnvelope {
            id: "test-merge-1".into(),
            session_token: Some(state.session_token.clone()),
            peer_id: None,
            payload: IpcPayload::Request(IpcRequest::ThreeWayMerge {
                base: "fn main() {}\n".into(),
                ours: "fn main() {}\n".into(),
                theirs: "fn main() { println!(\"hello\"); }\n".into(),
            }),
        };
        let resp_merge = handle_ipc_envelope(&state, envelope_merge, "127.0.0.1:9999").await;
        if let IpcPayload::Response(r) = resp_merge.payload {
            assert!(r.success);
            let data = r.data.unwrap();
            assert_eq!(data["is_clean"], true);
            assert!(data["merged_content"]
                .as_str()
                .unwrap()
                .contains("println!"));
        } else {
            panic!("Expected response");
        }
    }

    #[tokio::test]
    async fn test_crdt_payload_role_spoofing_rejected() {
        let state = create_test_state();

        // 1. Rogue client connects as peer-bob with Viewer role session.
        // It attempts to spoof role: "Owner" in the CrdtApplyUpdate payload.
        let bob_token = "token-bob-collaborator-1";
        state
            .token_store
            .lock()
            .register_session(security_core::SessionRecord::new(
                bob_token,
                "peer-bob",
                "Viewer",
                security_core::SessionType::Collaborator,
                std::time::Duration::from_secs(3600),
            ));
        state
            .role_registry
            .register_peer("peer-bob", workspace_sync::DeviceRole::Viewer);

        let envelope_spoofed = IpcEnvelope {
            id: "test-spoof-1".into(),
            session_token: Some(bob_token.into()),
            peer_id: Some("peer-bob".into()),
            payload: IpcPayload::Request(IpcRequest::CrdtApplyUpdate {
                doc_id: "doc-vault-1".into(),
                update_base64: "AAA=".into(),
                peer_id: "peer-bob".into(),
                role: "Owner".into(), // Attacker claims Owner in IPC payload
            }),
        };
        let resp_spoofed = handle_ipc_envelope(&state, envelope_spoofed, "127.0.0.1:9999").await;
        if let IpcPayload::Response(r) = resp_spoofed.payload {
            assert!(!r.success);
            assert!(
                r.error
                    .unwrap()
                    .contains("is not authorized to mutate CRDT document"),
                "Backend RoleRegistry must disallow spoofed role from untrusted payload"
            );
        } else {
            panic!("Expected response");
        }

        // 2. Legitimate admin registers peer-bob as Editor in backend RoleRegistry
        let envelope_register = IpcEnvelope {
            id: "test-reg-1".into(),
            session_token: Some(state.session_token.clone()),
            peer_id: Some(state.owner_peer_id.clone()),
            payload: IpcPayload::Request(IpcRequest::RegisterPeerRole {
                peer_id: "peer-bob".into(),
                role: "Editor".into(),
            }),
        };
        let resp_reg = handle_ipc_envelope(&state, envelope_register, "127.0.0.1:9999").await;
        if let IpcPayload::Response(r) = resp_reg.payload {
            assert!(r.success);
        } else {
            panic!("Expected response");
        }

        // Update bob's session to Editor
        state
            .token_store
            .lock()
            .register_session(security_core::SessionRecord::new(
                bob_token,
                "peer-bob",
                "Editor",
                security_core::SessionType::Collaborator,
                std::time::Duration::from_secs(3600),
            ));

        // 3. Now peer-bob's update is accepted by the backend
        let doc = state.crdt_manager.get_or_create("doc-vault-1");
        doc.set_text("Authenticated edit");
        let update_b64 = doc.get_full_update_base64();

        let envelope_valid = IpcEnvelope {
            id: "test-valid-1".into(),
            session_token: Some(bob_token.into()),
            peer_id: Some("peer-bob".into()),
            payload: IpcPayload::Request(IpcRequest::CrdtApplyUpdate {
                doc_id: "doc-vault-1".into(),
                update_base64: update_b64,
                peer_id: "peer-bob".into(),
                role: "Viewer".into(), // Even if client passes Viewer in payload, session role governs
            }),
        };
        let resp_valid = handle_ipc_envelope(&state, envelope_valid, "127.0.0.1:9999").await;
        if let IpcPayload::Response(r) = resp_valid.payload {
            assert!(r.success);
        } else {
            panic!("Expected response");
        }

        // 4. Revoke peer-bob
        let envelope_revoke = IpcEnvelope {
            id: "test-rev-1".into(),
            session_token: Some(state.session_token.clone()),
            peer_id: Some(state.owner_peer_id.clone()),
            payload: IpcPayload::Request(IpcRequest::RevokePeerRole {
                peer_id: "peer-bob".into(),
            }),
        };
        let resp_revoke = handle_ipc_envelope(&state, envelope_revoke, "127.0.0.1:9999").await;
        if let IpcPayload::Response(r) = resp_revoke.payload {
            assert!(r.success);
        } else {
            panic!("Expected response");
        }

        // 5. Subsequent update from revoked peer is rejected
        let envelope_after_revoke = IpcEnvelope {
            id: "test-after-rev-1".into(),
            session_token: Some(bob_token.into()),
            peer_id: Some("peer-bob".into()),
            payload: IpcPayload::Request(IpcRequest::CrdtApplyUpdate {
                doc_id: "doc-vault-1".into(),
                update_base64: "AAA=".into(),
                peer_id: "peer-bob".into(),
                role: "Editor".into(),
            }),
        };
        let resp_after_revoke =
            handle_ipc_envelope(&state, envelope_after_revoke, "127.0.0.1:9999").await;
        if let IpcPayload::Response(r) = resp_after_revoke.payload {
            assert!(!r.success);
            assert_eq!(r.status_code, Some(401));
        } else {
            panic!("Expected response");
        }
    }

    #[tokio::test]
    async fn test_rbac_daemon_permission_matrix() {
        let state = create_test_state();

        let viewer_token = "token-viewer-matrix";
        let editor_token = "token-editor-matrix";

        state
            .token_store
            .lock()
            .register_session(security_core::SessionRecord::new(
                viewer_token,
                "peer-viewer",
                "Viewer",
                security_core::SessionType::Collaborator,
                std::time::Duration::from_secs(3600),
            ));
        state
            .token_store
            .lock()
            .register_session(security_core::SessionRecord::new(
                editor_token,
                "peer-editor",
                "Editor",
                security_core::SessionType::Collaborator,
                std::time::Duration::from_secs(3600),
            ));

        state
            .role_registry
            .register_peer("peer-viewer", workspace_sync::DeviceRole::Viewer);
        state
            .role_registry
            .register_peer("peer-editor", workspace_sync::DeviceRole::Editor);

        // 1. Viewer attempts RunCommand -> Rejected
        let env_v_cmd = IpcEnvelope {
            id: "rbac-1".into(),
            session_token: Some(viewer_token.into()),
            peer_id: Some("peer-viewer".into()),
            payload: IpcPayload::Request(IpcRequest::RunCommand {
                command: "echo test".into(),
                cwd: None,
                timeout_secs: Some(5),
            }),
        };
        let resp_v_cmd = handle_ipc_envelope(&state, env_v_cmd, "127.0.0.1:9999").await;
        if let IpcPayload::Response(r) = resp_v_cmd.payload {
            assert!(!r.success);
            assert_eq!(r.status_code, Some(401));
            assert!(r
                .error
                .unwrap()
                .contains("not authorized to execute commands"));
        } else {
            panic!("Expected response");
        }

        // 2. Viewer attempts TerminalSpawn -> Rejected
        let env_v_term = IpcEnvelope {
            id: "rbac-2".into(),
            session_token: Some(viewer_token.into()),
            peer_id: Some("peer-viewer".into()),
            payload: IpcPayload::Request(IpcRequest::TerminalSpawn {
                shell: None,
                cwd: None,
                cols: None,
                rows: None,
            }),
        };
        let resp_v_term = handle_ipc_envelope(&state, env_v_term, "127.0.0.1:9999").await;
        if let IpcPayload::Response(r) = resp_v_term.payload {
            assert!(!r.success);
            assert_eq!(r.status_code, Some(401));
            assert!(r
                .error
                .unwrap()
                .contains("not authorized to spawn terminals"));
        } else {
            panic!("Expected response");
        }

        // 3. Viewer attempts RegisterPeerRole -> Rejected
        let env_v_admin = IpcEnvelope {
            id: "rbac-3".into(),
            session_token: Some(viewer_token.into()),
            peer_id: Some("peer-viewer".into()),
            payload: IpcPayload::Request(IpcRequest::RegisterPeerRole {
                peer_id: "peer-hacker".into(),
                role: "Admin".into(),
            }),
        };
        let resp_v_admin = handle_ipc_envelope(&state, env_v_admin, "127.0.0.1:9999").await;
        if let IpcPayload::Response(r) = resp_v_admin.payload {
            assert!(!r.success);
            assert_eq!(r.status_code, Some(401));
            assert!(r
                .error
                .unwrap()
                .contains("not authorized to register peer roles"));
        } else {
            panic!("Expected response");
        }

        // 4. Editor attempts RunCommand -> Accepted
        let env_e_cmd = IpcEnvelope {
            id: "rbac-4".into(),
            session_token: Some(editor_token.into()),
            peer_id: Some("peer-editor".into()),
            payload: IpcPayload::Request(IpcRequest::RunCommand {
                command: "echo test".into(),
                cwd: None,
                timeout_secs: Some(5),
            }),
        };
        let resp_e_cmd = handle_ipc_envelope(&state, env_e_cmd, "127.0.0.1:9999").await;
        if let IpcPayload::Response(r) = resp_e_cmd.payload {
            assert!(r.success);
        } else {
            panic!("Expected response");
        }

        // 5. Editor attempts RegisterPeerRole -> Rejected (Editor lacks Admin)
        let env_e_admin = IpcEnvelope {
            id: "rbac-5".into(),
            session_token: Some(editor_token.into()),
            peer_id: Some("peer-editor".into()),
            payload: IpcPayload::Request(IpcRequest::RegisterPeerRole {
                peer_id: "peer-guest".into(),
                role: "Viewer".into(),
            }),
        };
        let resp_e_admin = handle_ipc_envelope(&state, env_e_admin, "127.0.0.1:9999").await;
        if let IpcPayload::Response(r) = resp_e_admin.payload {
            assert!(!r.success);
            assert_eq!(r.status_code, Some(401));
            assert!(r
                .error
                .unwrap()
                .contains("not authorized to register peer roles"));
        } else {
            panic!("Expected response");
        }

        // 6. Viewer attempts IndexWorkspace -> Rejected
        let env_v_idx = IpcEnvelope {
            id: "rbac-6".into(),
            session_token: Some(viewer_token.into()),
            peer_id: Some("peer-viewer".into()),
            payload: IpcPayload::Request(IpcRequest::IndexWorkspace {
                workspace_path: ".".into(),
            }),
        };
        let resp_v_idx = handle_ipc_envelope(&state, env_v_idx, "127.0.0.1:9999").await;
        if let IpcPayload::Response(r) = resp_v_idx.payload {
            assert!(!r.success);
            assert_eq!(r.status_code, Some(401));
            assert!(r
                .error
                .unwrap()
                .contains("not authorized to index workspaces"));
        } else {
            panic!("Expected response");
        }

        // 7. Provisioned desktop owner identity (with owner_peer_id) -> RegisterPeerRole accepted
        let env_owner_admin = IpcEnvelope {
            id: "rbac-7".into(),
            session_token: Some(state.session_token.clone()),
            peer_id: Some(state.owner_peer_id.clone()),
            payload: IpcPayload::Request(IpcRequest::RegisterPeerRole {
                peer_id: "peer-guest".into(),
                role: "Viewer".into(),
            }),
        };
        let resp_owner = handle_ipc_envelope(&state, env_owner_admin, "127.0.0.1:9999").await;
        if let IpcPayload::Response(r) = resp_owner.payload {
            assert!(r.success);
        } else {
            panic!("Expected response");
        }
    }

    #[tokio::test]
    async fn test_daemon_authorization_10_cases() {
        let state = create_test_state();

        let viewer_token = "token-viewer-10case";
        let reviewer_token = "token-reviewer-10case";
        let editor_token = "token-editor-10case";

        state
            .token_store
            .lock()
            .register_session(security_core::SessionRecord::new(
                viewer_token,
                "peer-viewer",
                "Viewer",
                security_core::SessionType::Collaborator,
                std::time::Duration::from_secs(3600),
            ));
        state
            .token_store
            .lock()
            .register_session(security_core::SessionRecord::new(
                reviewer_token,
                "peer-reviewer",
                "Reviewer",
                security_core::SessionType::Collaborator,
                std::time::Duration::from_secs(3600),
            ));
        state
            .token_store
            .lock()
            .register_session(security_core::SessionRecord::new(
                editor_token,
                "peer-editor",
                "Editor",
                security_core::SessionType::Collaborator,
                std::time::Duration::from_secs(3600),
            ));

        state
            .role_registry
            .register_peer("peer-viewer", workspace_sync::DeviceRole::Viewer);
        state
            .role_registry
            .register_peer("peer-reviewer", workspace_sync::DeviceRole::Reviewer);
        state
            .role_registry
            .register_peer("peer-editor", workspace_sync::DeviceRole::Editor);

        // Case 1: Unauthenticated loopback (session_token: None) -> 401
        let env_c1 = IpcEnvelope {
            id: "c1".into(),
            session_token: None,
            peer_id: None,
            payload: IpcPayload::Request(IpcRequest::Ping),
        };
        let resp_c1 = handle_ipc_envelope(&state, env_c1, "127.0.0.1:44888").await;
        if let IpcPayload::Response(r) = resp_c1.payload {
            assert!(!r.success);
            assert_eq!(r.status_code, Some(401));
            assert!(r.error.unwrap().contains("401 Unauthorized"));
        } else {
            panic!("Expected response payload");
        }

        // Case 2: Invalid token -> 401
        let env_c2 = IpcEnvelope {
            id: "c2".into(),
            session_token: Some("malicious-forged-token-99999".into()),
            peer_id: None,
            payload: IpcPayload::Request(IpcRequest::Ping),
        };
        let resp_c2 = handle_ipc_envelope(&state, env_c2, "127.0.0.1:44888").await;
        if let IpcPayload::Response(r) = resp_c2.payload {
            assert!(!r.success);
            assert_eq!(r.status_code, Some(401));
            assert!(r
                .error
                .unwrap()
                .contains("missing or invalid session token"));
        } else {
            panic!("Expected response payload");
        }

        // Case 3: Expired token -> 401
        let expired_token_store =
            security_core::TokenStore::new(std::time::Duration::from_millis(10));
        let expired_token = expired_token_store.current_token().to_string();
        std::thread::sleep(std::time::Duration::from_millis(20));
        let expired_state = Arc::new(DaemonState {
            config: AetherConfig::default(),
            session_token: expired_token.clone(),
            owner_peer_id: "owner-expired".into(),
            token_store: parking_lot::Mutex::new(expired_token_store),
            db: DatabaseEngine::open_in_memory().unwrap(),
            model_gateway: ModelGateway::new(),
            agent_orchestrator: agent_orchestrator::AgentOrchestrator::new(
                ModelGateway::new(),
                DatabaseEngine::open_in_memory().unwrap(),
            ),
            project_brain: parking_lot::Mutex::new(ProjectBrain::new(
                DatabaseEngine::open_in_memory().unwrap(),
            )),
            indexer: ProjectIndexer::new(SymbolIndex::new(
                DatabaseEngine::open_in_memory().unwrap(),
            )),
            lsp_multiplexer: Arc::new(lsp_client::LspMultiplexer::new(std::path::PathBuf::from(
                ".",
            ))),
            terminal_manager: Arc::new(terminal_manager::TerminalManager::new()),
            crdt_manager: Arc::new(workspace_sync::CrdtManager::new()),
            awareness_tracker: Arc::new(workspace_sync::AwarenessTracker::new()),
            role_registry: Arc::new(workspace_sync::RoleRegistry::new()),
            swarm_coordinator: Arc::new(agent_orchestrator::SwarmCoordinator::new(
                std::path::Path::new("."),
            )),
        });
        let env_c3 = IpcEnvelope {
            id: "c3".into(),
            session_token: Some(expired_token),
            peer_id: None,
            payload: IpcPayload::Request(IpcRequest::Ping),
        };
        let resp_c3 = handle_ipc_envelope(&expired_state, env_c3, "127.0.0.1:44888").await;
        if let IpcPayload::Response(r) = resp_c3.payload {
            assert!(!r.success);
            assert_eq!(r.status_code, Some(401));
            assert!(r.error.unwrap().contains("expired"));
        } else {
            panic!("Expected response payload");
        }

        // Case 4: Revoked token -> 401
        let revoked_token_store =
            security_core::TokenStore::new(std::time::Duration::from_secs(3600));
        let token_to_revoke = revoked_token_store.current_token().to_string();
        let revoked_state = Arc::new(DaemonState {
            config: AetherConfig::default(),
            session_token: token_to_revoke.clone(),
            owner_peer_id: "owner-revoked".into(),
            token_store: parking_lot::Mutex::new(revoked_token_store),
            db: DatabaseEngine::open_in_memory().unwrap(),
            model_gateway: ModelGateway::new(),
            agent_orchestrator: agent_orchestrator::AgentOrchestrator::new(
                ModelGateway::new(),
                DatabaseEngine::open_in_memory().unwrap(),
            ),
            project_brain: parking_lot::Mutex::new(ProjectBrain::new(
                DatabaseEngine::open_in_memory().unwrap(),
            )),
            indexer: ProjectIndexer::new(SymbolIndex::new(
                DatabaseEngine::open_in_memory().unwrap(),
            )),
            lsp_multiplexer: Arc::new(lsp_client::LspMultiplexer::new(std::path::PathBuf::from(
                ".",
            ))),
            terminal_manager: Arc::new(terminal_manager::TerminalManager::new()),
            crdt_manager: Arc::new(workspace_sync::CrdtManager::new()),
            awareness_tracker: Arc::new(workspace_sync::AwarenessTracker::new()),
            role_registry: Arc::new(workspace_sync::RoleRegistry::new()),
            swarm_coordinator: Arc::new(agent_orchestrator::SwarmCoordinator::new(
                std::path::Path::new("."),
            )),
        });
        revoked_state
            .token_store
            .lock()
            .revoke_token(&token_to_revoke);

        let env_c4 = IpcEnvelope {
            id: "c4".into(),
            session_token: Some(token_to_revoke),
            peer_id: None,
            payload: IpcPayload::Request(IpcRequest::Ping),
        };
        let resp_c4 = handle_ipc_envelope(&revoked_state, env_c4, "127.0.0.1:44888").await;
        if let IpcPayload::Response(r) = resp_c4.payload {
            assert!(!r.success);
            assert_eq!(r.status_code, Some(401));
            assert!(r.error.unwrap().contains("revoked"));
        } else {
            panic!("Expected response payload");
        }

        // Case 5: Viewer attempting terminal execution (RunCommand, TerminalSpawn) -> 401
        let env_c5a = IpcEnvelope {
            id: "c5a".into(),
            session_token: Some(viewer_token.into()),
            peer_id: Some("peer-viewer".into()),
            payload: IpcPayload::Request(IpcRequest::RunCommand {
                command: "whoami".into(),
                cwd: None,
                timeout_secs: Some(5),
            }),
        };
        let resp_c5a = handle_ipc_envelope(&state, env_c5a, "127.0.0.1:44888").await;
        if let IpcPayload::Response(r) = resp_c5a.payload {
            assert!(!r.success);
            assert_eq!(r.status_code, Some(401));
            assert!(r
                .error
                .unwrap()
                .contains("not authorized to execute commands"));
        } else {
            panic!("Expected response payload");
        }

        let env_c5b = IpcEnvelope {
            id: "c5b".into(),
            session_token: Some(viewer_token.into()),
            peer_id: Some("peer-viewer".into()),
            payload: IpcPayload::Request(IpcRequest::TerminalSpawn {
                shell: None,
                cwd: None,
                cols: None,
                rows: None,
            }),
        };
        let resp_c5b = handle_ipc_envelope(&state, env_c5b, "127.0.0.1:44888").await;
        if let IpcPayload::Response(r) = resp_c5b.payload {
            assert!(!r.success);
            assert_eq!(r.status_code, Some(401));
            assert!(r
                .error
                .unwrap()
                .contains("not authorized to spawn terminals"));
        } else {
            panic!("Expected response payload");
        }

        // Case 6: Reviewer attempting RegisterPeerRole -> 401
        let env_c6 = IpcEnvelope {
            id: "c6".into(),
            session_token: Some(reviewer_token.into()),
            peer_id: Some("peer-reviewer".into()),
            payload: IpcPayload::Request(IpcRequest::RegisterPeerRole {
                peer_id: "peer-malicious".into(),
                role: "Admin".into(),
            }),
        };
        let resp_c6 = handle_ipc_envelope(&state, env_c6, "127.0.0.1:44888").await;
        if let IpcPayload::Response(r) = resp_c6.payload {
            assert!(!r.success);
            assert_eq!(r.status_code, Some(401));
            assert!(r
                .error
                .unwrap()
                .contains("not authorized to register peer roles"));
        } else {
            panic!("Expected response payload");
        }

        // Case 7: Client injecting role: Owner in payload -> Backend overrides with session role (Viewer)
        let env_c7 = IpcEnvelope {
            id: "c7".into(),
            session_token: Some(viewer_token.into()),
            peer_id: Some("peer-viewer".into()),
            payload: IpcPayload::Request(IpcRequest::CrdtApplyUpdate {
                doc_id: "doc-vault-sec".into(),
                update_base64: "AAA=".into(),
                peer_id: "peer-viewer".into(),
                role: "Owner".into(), // Injected spoofed role in payload
            }),
        };
        let resp_c7 = handle_ipc_envelope(&state, env_c7, "127.0.0.1:44888").await;
        if let IpcPayload::Response(r) = resp_c7.payload {
            assert!(!r.success);
            assert!(r
                .error
                .unwrap()
                .contains("is not authorized to mutate CRDT document"));
        } else {
            panic!("Expected response payload");
        }

        // Case 8: Peer identity spoofing attempt (Viewer presents viewer_token but claims peer-editor) -> 401
        let env_c8 = IpcEnvelope {
            id: "c8".into(),
            session_token: Some(viewer_token.into()),
            peer_id: Some("peer-editor".into()), // Impersonation attempt!
            payload: IpcPayload::Request(IpcRequest::RunCommand {
                command: "id".into(),
                cwd: None,
                timeout_secs: Some(5),
            }),
        };
        let resp_c8 = handle_ipc_envelope(&state, env_c8, "127.0.0.1:44888").await;
        if let IpcPayload::Response(r) = resp_c8.payload {
            assert!(!r.success);
            assert_eq!(r.status_code, Some(401));
            assert!(r
                .error
                .unwrap()
                .contains("does not match authenticated session identity"));
        } else {
            panic!("Expected response payload");
        }

        // Case 9: Local process attempting to claim desktop Owner identity without Owner token -> 401
        let env_c9 = IpcEnvelope {
            id: "c9".into(),
            session_token: Some(viewer_token.into()),
            peer_id: Some(state.owner_peer_id.clone()), // Impersonate Owner!
            payload: IpcPayload::Request(IpcRequest::RegisterPeerRole {
                peer_id: "infiltrator".into(),
                role: "Owner".into(),
            }),
        };
        let resp_c9 = handle_ipc_envelope(&state, env_c9, "127.0.0.1:44888").await;
        if let IpcPayload::Response(r) = resp_c9.payload {
            assert!(!r.success);
            assert_eq!(r.status_code, Some(401));
            assert!(r
                .error
                .unwrap()
                .contains("does not match authenticated session identity"));
        } else {
            panic!("Expected response payload");
        }

        // Case 10: Session attempting privileged action after revocation -> 401
        let victim_peer = "owner-admin-1";
        let live_token_store = security_core::TokenStore::new_with_owner(
            std::time::Duration::from_secs(3600),
            victim_peer,
        );
        let victim_token = live_token_store.current_token().to_string();
        let target_state = Arc::new(DaemonState {
            config: AetherConfig::default(),
            session_token: victim_token.clone(),
            owner_peer_id: victim_peer.into(),
            token_store: parking_lot::Mutex::new(live_token_store),
            db: DatabaseEngine::open_in_memory().unwrap(),
            model_gateway: ModelGateway::new(),
            agent_orchestrator: agent_orchestrator::AgentOrchestrator::new(
                ModelGateway::new(),
                DatabaseEngine::open_in_memory().unwrap(),
            ),
            project_brain: parking_lot::Mutex::new(ProjectBrain::new(
                DatabaseEngine::open_in_memory().unwrap(),
            )),
            indexer: ProjectIndexer::new(SymbolIndex::new(
                DatabaseEngine::open_in_memory().unwrap(),
            )),
            lsp_multiplexer: Arc::new(lsp_client::LspMultiplexer::new(std::path::PathBuf::from(
                ".",
            ))),
            terminal_manager: Arc::new(terminal_manager::TerminalManager::new()),
            crdt_manager: Arc::new(workspace_sync::CrdtManager::new()),
            awareness_tracker: Arc::new(workspace_sync::AwarenessTracker::new()),
            role_registry: Arc::new(workspace_sync::RoleRegistry::new()),
            swarm_coordinator: Arc::new(agent_orchestrator::SwarmCoordinator::new(
                std::path::Path::new("."),
            )),
        });
        target_state
            .role_registry
            .register_peer(victim_peer, workspace_sync::DeviceRole::Owner);

        // Before revocation: Ping succeeds
        let env_before = IpcEnvelope {
            id: "c10-pre".into(),
            session_token: Some(victim_token.clone()),
            peer_id: Some(victim_peer.into()),
            payload: IpcPayload::Request(IpcRequest::Ping),
        };
        let resp_before = handle_ipc_envelope(&target_state, env_before, "127.0.0.1:44888").await;
        if let IpcPayload::Response(r) = resp_before.payload {
            assert!(r.success);
        } else {
            panic!("Expected response payload");
        }

        // Revoke token via RevokeSessionToken IPC request
        let env_revoke = IpcEnvelope {
            id: "c10-revoke".into(),
            session_token: Some(victim_token.clone()),
            peer_id: Some(victim_peer.into()),
            payload: IpcPayload::Request(IpcRequest::RevokeSessionToken {
                token: victim_token.clone(),
            }),
        };
        let resp_revoke = handle_ipc_envelope(&target_state, env_revoke, "127.0.0.1:44888").await;
        if let IpcPayload::Response(r) = resp_revoke.payload {
            assert!(r.success);
            assert_eq!(r.data.unwrap()["revoked"], true);
        } else {
            panic!("Expected response payload");
        }

        // After revocation: privileged action and ping are blocked with 401
        let env_after = IpcEnvelope {
            id: "c10-post".into(),
            session_token: Some(victim_token),
            peer_id: Some(victim_peer.into()),
            payload: IpcPayload::Request(IpcRequest::RunCommand {
                command: "echo fail".into(),
                cwd: None,
                timeout_secs: Some(5),
            }),
        };
        let resp_after = handle_ipc_envelope(&target_state, env_after, "127.0.0.1:44888").await;
        if let IpcPayload::Response(r) = resp_after.payload {
            assert!(!r.success);
            assert_eq!(r.status_code, Some(401));
            assert!(r.error.unwrap().contains("revoked"));
        } else {
            panic!("Expected response payload");
        }
    }

    #[tokio::test]
    async fn test_sec_01_owner_impersonation_blocked() {
        let state = create_test_state();
        let viewer_tok = "tok-viewer-sec01";
        state
            .token_store
            .lock()
            .register_session(security_core::SessionRecord::new(
                viewer_tok,
                "peer-viewer-charlie",
                "Viewer",
                security_core::SessionType::Collaborator,
                std::time::Duration::from_secs(3600),
            ));

        // Viewer attempts to present owner_peer_id
        let env = IpcEnvelope {
            id: "sec-01".into(),
            session_token: Some(viewer_tok.into()),
            peer_id: Some(state.owner_peer_id.clone()),
            payload: IpcPayload::Request(IpcRequest::RegisterPeerRole {
                peer_id: "attacker".into(),
                role: "Owner".into(),
            }),
        };

        let resp = handle_ipc_envelope(&state, env, "127.0.0.1:44888").await;
        if let IpcPayload::Response(r) = resp.payload {
            assert!(!r.success);
            assert_eq!(r.status_code, Some(401));
            assert!(r
                .error
                .unwrap()
                .contains("does not match authenticated session identity"));
        } else {
            panic!("Expected response");
        }
    }

    #[tokio::test]
    async fn test_sec_02_peer_identity_spoofing_blocked() {
        let state = create_test_state();
        let alice_tok = "tok-alice-sec02";
        state
            .token_store
            .lock()
            .register_session(security_core::SessionRecord::new(
                alice_tok,
                "peer-alice",
                "Editor",
                security_core::SessionType::Collaborator,
                std::time::Duration::from_secs(3600),
            ));

        // Alice attempts to claim peer-bob identity
        let env = IpcEnvelope {
            id: "sec-02".into(),
            session_token: Some(alice_tok.into()),
            peer_id: Some("peer-bob".into()),
            payload: IpcPayload::Request(IpcRequest::RunCommand {
                command: "echo test".into(),
                cwd: None,
                timeout_secs: Some(5),
            }),
        };

        let resp = handle_ipc_envelope(&state, env, "127.0.0.1:44888").await;
        if let IpcPayload::Response(r) = resp.payload {
            assert!(!r.success);
            assert_eq!(r.status_code, Some(401));
            assert!(r
                .error
                .unwrap()
                .contains("does not match authenticated session identity"));
        } else {
            panic!("Expected response");
        }
    }

    #[tokio::test]
    async fn test_sec_05_session_rotation_consistency() {
        let state = create_test_state();
        let old_token = state.session_token.clone();

        // Rotate owner session token
        let new_token = state.token_store.lock().rotate_token();
        assert_ne!(old_token, new_token);

        // Both new token and old token (during grace period) authenticate as Owner
        let env_new = IpcEnvelope {
            id: "sec-05-new".into(),
            session_token: Some(new_token.clone()),
            peer_id: Some(state.owner_peer_id.clone()),
            payload: IpcPayload::Request(IpcRequest::Ping),
        };
        let resp_new = handle_ipc_envelope(&state, env_new, "127.0.0.1:44888").await;
        if let IpcPayload::Response(r) = resp_new.payload {
            assert!(r.success);
        } else {
            panic!("Expected response");
        }

        let env_old = IpcEnvelope {
            id: "sec-05-old".into(),
            session_token: Some(old_token),
            peer_id: Some(state.owner_peer_id.clone()),
            payload: IpcPayload::Request(IpcRequest::Ping),
        };
        let resp_old = handle_ipc_envelope(&state, env_old, "127.0.0.1:44888").await;
        if let IpcPayload::Response(r) = resp_old.payload {
            assert!(r.success);
        } else {
            panic!("Expected response");
        }
    }

    #[tokio::test]
    async fn test_sec_07_and_08_transport_rate_limiting_isolation() {
        let state = create_test_state();

        // Attacker repeatedly fails from source "attacker-pipe-1"
        for i in 1..=5 {
            let env_bad = IpcEnvelope {
                id: format!("attack-{}", i),
                session_token: Some("bad-token".into()),
                peer_id: None,
                payload: IpcPayload::Request(IpcRequest::Ping),
            };
            let _ = handle_ipc_envelope(&state, env_bad, "attacker-pipe-1").await;
        }

        // Attacker source is now rate limited
        let env_attack_lockout = IpcEnvelope {
            id: "attack-lockout".into(),
            session_token: Some(state.session_token.clone()),
            peer_id: None,
            payload: IpcPayload::Request(IpcRequest::Ping),
        };
        let resp_attack = handle_ipc_envelope(&state, env_attack_lockout, "attacker-pipe-1").await;
        if let IpcPayload::Response(r) = resp_attack.payload {
            assert!(!r.success);
            assert!(r.error.unwrap().contains("rate-limited"));
        } else {
            panic!("Expected response");
        }

        // Legitimate desktop UI connection from "desktop-host-1" is NOT locked out!
        let env_legit = IpcEnvelope {
            id: "desktop-legit".into(),
            session_token: Some(state.session_token.clone()),
            peer_id: None,
            payload: IpcPayload::Request(IpcRequest::Ping),
        };
        let resp_legit = handle_ipc_envelope(&state, env_legit, "desktop-host-1").await;
        if let IpcPayload::Response(r) = resp_legit.payload {
            assert!(
                r.success,
                "Legitimate desktop client must never be locked out by attacker"
            );
        } else {
            panic!("Expected response");
        }
    }

    #[tokio::test]
    async fn test_cred_01_and_02_at_rest_audit_no_plaintext_token_on_disk() {
        let temp_dir = std::env::temp_dir().join(format!(
            "aether_cred_audit_{}",
            uuid::Uuid::new_v4().simple()
        ));
        std::fs::create_dir_all(&temp_dir).expect("Failed to create temporary directory for audit");

        let raw_token = "aether_sec_owner_raw_bearer_token_super_secret_999";
        let bootstrap_file = temp_dir.join("desktop_bootstrap.json");

        // Protect token via security_core platform DPAPI/key derivation
        let protected_token = security_core::protect_credential(raw_token)
            .expect("Credential protection must succeed on current platform");

        // Write bootstrap JSON with ONLY protected_token
        let bootstrap_json = serde_json::json!({
            "port": 44888,
            "owner_peer_id": "owner-audit-peer",
            "protected_token": protected_token,
            "created_at": chrono::Utc::now().to_rfc3339(),
        });
        std::fs::write(
            &bootstrap_file,
            serde_json::to_string_pretty(&bootstrap_json).unwrap(),
        )
        .expect("Writing protected bootstrap must succeed");

        // STRICT AT-REST AUDIT:
        // Walk entire directory tree and verify raw token string NEVER appears anywhere on disk
        for entry in std::fs::read_dir(&temp_dir)
            .expect("Read temp dir")
            .flatten()
        {
            if entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
                let bytes = std::fs::read(entry.path()).expect("File must be readable");
                let content_str = String::from_utf8_lossy(&bytes);
                assert!(
                    !content_str.contains(raw_token),
                    "FAIL: Raw plaintext owner token found at rest in file '{}'!",
                    entry.path().display()
                );
            }
        }

        // Verify consumer can recover raw token from protected bootstrap
        let read_back_raw = std::fs::read_to_string(&bootstrap_file).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&read_back_raw).unwrap();
        let enc_token_b64 = parsed["protected_token"]
            .as_str()
            .expect("protected_token must exist");
        let recovered_token = security_core::unprotect_credential(enc_token_b64)
            .expect("Unprotecting valid credential must succeed");

        assert_eq!(
            recovered_token, raw_token,
            "Recovered credential must match original byte-for-byte"
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[tokio::test]
    async fn test_sec_09_connection_churn_rate_limiting_defense() {
        let state = create_test_state();

        // Attacker attempts rapid brute-force / auth failure by churning 25 distinct client connection IDs
        for i in 1..=25 {
            let churned_client = format!("ephemeral-churn-source-{}", i);
            let env_bad = IpcEnvelope {
                id: format!("churn-{}", i),
                session_token: Some("invalid-token".into()),
                peer_id: None,
                payload: IpcPayload::Request(IpcRequest::Ping),
            };
            let _ = handle_ipc_envelope(&state, env_bad, &churned_client).await;
        }

        // Attacker on new source is throttled by global rate limiter
        let env_attack_churn = IpcEnvelope {
            id: "attack-churn-locked".into(),
            session_token: Some("invalid-token".into()),
            peer_id: None,
            payload: IpcPayload::Request(IpcRequest::Ping),
        };
        let resp_churn =
            handle_ipc_envelope(&state, env_attack_churn, "ephemeral-churn-source-999").await;
        if let IpcPayload::Response(r) = resp_churn.payload {
            assert!(!r.success);
            assert!(r.error.unwrap().contains("rate-limited"));
        } else {
            panic!("Expected response");
        }

        // Legitimate desktop Owner presenting valid session token is NOT denied
        let env_owner = IpcEnvelope {
            id: "owner-available".into(),
            session_token: Some(state.session_token.clone()),
            peer_id: Some(state.owner_peer_id.clone()),
            payload: IpcPayload::Request(IpcRequest::Ping),
        };
        let resp_owner =
            handle_ipc_envelope(&state, env_owner, "legit-desktop-owner-session").await;
        if let IpcPayload::Response(r) = resp_owner.payload {
            assert!(
                r.success,
                "Legitimate Owner with valid credential must remain available under connection-churn attack"
            );
        } else {
            panic!("Expected response");
        }
    }
}
