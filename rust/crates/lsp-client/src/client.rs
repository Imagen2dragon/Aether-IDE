use crate::capabilities::default_client_capabilities;
use crate::document::{path_to_uri, DocumentState};
use crate::framing::{encode_frame, LspFrameDecoder};
use crate::jsonrpc::{
    error_codes, IncomingMessage, RequestId, RpcError, RpcNotification, RpcRequest, RpcResponse,
};
use crate::types::{
    CompletionItem, CompletionList, HoverResult, Location, PublishDiagnosticsParams, Range,
};
use parking_lot::Mutex;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt};
use tokio::process::{Child, Command};
use tokio::sync::{broadcast, oneshot};
use tracing::{debug, error, info, warn};

#[derive(Debug, Error)]
pub enum LspError {
    #[error("Language server binary '{command}' not found on PATH. {install_hint}")]
    ServerNotFound {
        command: String,
        install_hint: String,
    },
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("Framing error: {0}")]
    Framing(#[from] crate::framing::FramingError),
    #[error("RPC error: {0}")]
    Rpc(#[from] RpcError),
    #[error("Client not ready: {0}")]
    NotReady(String),
    #[error("Request timed out after {0:?}")]
    Timeout(Duration),
    #[error("Server terminated unexpectedly")]
    ServerTerminated,
    #[error("Channel closed")]
    ChannelClosed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientState {
    Created,
    Starting,
    Ready,
    ShuttingDown,
    Terminated,
    Failed,
}

#[derive(Debug, Clone)]
pub struct LspClientConfig {
    pub server_id: String,
    pub command: String,
    pub args: Vec<String>,
    pub workspace_root: PathBuf,
    pub initialization_options: Option<Value>,
}

pub type RpcResultSender = oneshot::Sender<Result<Value, RpcError>>;
pub type PendingRequests = Arc<Mutex<HashMap<RequestId, RpcResultSender>>>;

pub struct LspClient {
    config: LspClientConfig,
    state: Arc<Mutex<ClientState>>,
    next_request_id: AtomicI64,
    write_tx: tokio::sync::mpsc::Sender<Vec<u8>>,
    pending_requests: PendingRequests,
    diagnostics_tx: broadcast::Sender<PublishDiagnosticsParams>,
    child_handle: Arc<Mutex<Option<Child>>>,
}

impl LspClient {
    pub async fn spawn(config: LspClientConfig) -> Result<Self, LspError> {
        info!(
            "Spawning LSP server '{}' with command: {} {:?}",
            config.server_id, config.command, config.args
        );

        let mut cmd = Command::new(&config.command);
        cmd.args(&config.args)
            .current_dir(&config.workspace_root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        #[cfg(windows)]
        {
            // Set process creation flag to prevent console window popup
            const CREATE_NO_WINDOW: u32 = 0x08000000;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }

        let mut child = cmd.spawn().map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                let install_hint = match config.server_id.as_str() {
                    "rust" => {
                        "Please install rust-analyzer via 'rustup component add rust-analyzer'"
                            .to_string()
                    }
                    "typescript" => {
                        "Please install typescript-language-server via 'npm install -g typescript-language-server typescript'"
                            .to_string()
                    }
                    "python" => "Please install pyright via 'pip install pyright'".to_string(),
                    _ => format!(
                        "Please ensure '{}' is installed and available on your system PATH.",
                        config.command
                    ),
                };
                LspError::ServerNotFound {
                    command: config.command.clone(),
                    install_hint,
                }
            } else {
                LspError::Io(e)
            }
        })?;

        let stdin = child.stdin.take().expect("Child stdin must be piped");
        let stdout = child.stdout.take().expect("Child stdout must be piped");
        let stderr = child.stderr.take().expect("Child stderr must be piped");

        let (write_tx, mut write_rx) = tokio::sync::mpsc::channel::<Vec<u8>>(256);
        let pending_requests: PendingRequests = Arc::new(Mutex::new(HashMap::new()));
        let (diagnostics_tx, _) = broadcast::channel(512);
        let state = Arc::new(Mutex::new(ClientState::Starting));

        // Background Writer Task
        tokio::spawn(async move {
            let mut writer = stdin;
            while let Some(msg) = write_rx.recv().await {
                if let Err(e) = writer.write_all(&msg).await {
                    error!("LSP stdin write error: {}", e);
                    break;
                }
                let _ = writer.flush().await;
            }
        });

        // Background Reader Task
        let pending_clone = pending_requests.clone();
        let diag_clone = diagnostics_tx.clone();
        let state_clone = state.clone();
        let write_tx_reply = write_tx.clone();

        tokio::spawn(async move {
            let mut reader = stdout;
            let mut decoder = LspFrameDecoder::new();
            let mut buf = vec![0u8; 16384];

            loop {
                let n = match reader.read(&mut buf).await {
                    Ok(0) => {
                        debug!("LSP stdout EOF reached");
                        break;
                    }
                    Ok(n) => n,
                    Err(e) => {
                        error!("LSP stdout read error: {}", e);
                        break;
                    }
                };

                decoder.accept_chunk(&buf[..n]);

                loop {
                    match decoder.next_frame() {
                        Ok(Some(frame)) => {
                            if let Ok(incoming) = IncomingMessage::parse(&frame) {
                                match incoming {
                                    IncomingMessage::Response(resp) => {
                                        if let Some(id) = resp.id {
                                            if let Some(sender) = pending_clone.lock().remove(&id) {
                                                if let Some(err) = resp.error {
                                                    let _ = sender.send(Err(err));
                                                } else {
                                                    let _ = sender.send(Ok(resp
                                                        .result
                                                        .unwrap_or(Value::Null)));
                                                }
                                            }
                                        }
                                    }
                                    IncomingMessage::Notification(notif) => {
                                        if notif.method == "textDocument/publishDiagnostics" {
                                            if let Some(params) = notif.params {
                                                if let Ok(diag_params) = serde_json::from_value::<
                                                    PublishDiagnosticsParams,
                                                >(
                                                    params
                                                ) {
                                                    let _ = diag_clone.send(diag_params);
                                                }
                                            }
                                        } else {
                                            debug!(
                                                "LSP notification [{}]: {:?}",
                                                notif.method, notif.params
                                            );
                                        }
                                    }
                                    IncomingMessage::Request(req) => {
                                        debug!("LSP server sent request: {}", req.method);
                                        // Auto-respond to common requests from language servers
                                        let reply = if req.method == "workspace/configuration" {
                                            RpcResponse::ok(req.id, json!([]))
                                        } else if req.method == "client/registerCapability" {
                                            RpcResponse::ok(req.id, json!(null))
                                        } else {
                                            RpcResponse::err(
                                                Some(req.id),
                                                error_codes::METHOD_NOT_FOUND,
                                                "Method unsupported by client",
                                            )
                                        };
                                        if let Ok(bytes) = serde_json::to_vec(&reply) {
                                            let framed = encode_frame(&bytes);
                                            let _ = write_tx_reply.send(framed).await;
                                        }
                                    }
                                }
                            }
                        }
                        Ok(None) => break,
                        Err(e) => {
                            warn!("LSP framing error: {}", e);
                            break;
                        }
                    }
                }
            }

            *state_clone.lock() = ClientState::Terminated;
        });

        // Background Stderr Logger Task
        tokio::spawn(async move {
            let mut reader = tokio::io::BufReader::new(stderr);
            let mut line = String::new();
            while let Ok(n) = reader.read_line(&mut line).await {
                if n == 0 {
                    break;
                }
                debug!(target: "lsp-stderr", "{}", line.trim_end());
                line.clear();
            }
        });

        Ok(Self {
            config,
            state,
            next_request_id: AtomicI64::new(1),
            write_tx,
            pending_requests,
            diagnostics_tx,
            child_handle: Arc::new(Mutex::new(Some(child))),
        })
    }

    pub fn state(&self) -> ClientState {
        *self.state.lock()
    }

    pub fn subscribe_diagnostics(&self) -> broadcast::Receiver<PublishDiagnosticsParams> {
        self.diagnostics_tx.subscribe()
    }

    pub async fn send_request(
        &self,
        method: &str,
        params: Option<Value>,
        timeout: Duration,
    ) -> Result<Value, LspError> {
        let id_val = self.next_request_id.fetch_add(1, Ordering::SeqCst);
        let id = RequestId::Int(id_val);

        let req = RpcRequest::new(id.clone(), method, params);
        let body = serde_json::to_vec(&req)?;
        let frame = encode_frame(&body);

        let (resp_tx, resp_rx) = oneshot::channel();
        self.pending_requests.lock().insert(id.clone(), resp_tx);

        if self.write_tx.send(frame).await.is_err() {
            self.pending_requests.lock().remove(&id);
            return Err(LspError::ServerTerminated);
        }

        match tokio::time::timeout(timeout, resp_rx).await {
            Ok(Ok(Ok(val))) => Ok(val),
            Ok(Ok(Err(rpc_err))) => Err(LspError::Rpc(rpc_err)),
            Ok(Err(_)) => {
                self.pending_requests.lock().remove(&id);
                Err(LspError::ChannelClosed)
            }
            Err(_) => {
                self.pending_requests.lock().remove(&id);
                // Optionally send $/cancelRequest
                let cancel = RpcNotification::new("$/cancelRequest", Some(json!({ "id": id_val })));
                if let Ok(c_bytes) = serde_json::to_vec(&cancel) {
                    let _ = self.write_tx.send(encode_frame(&c_bytes)).await;
                }
                Err(LspError::Timeout(timeout))
            }
        }
    }

    pub async fn send_notification(
        &self,
        method: &str,
        params: Option<Value>,
    ) -> Result<(), LspError> {
        let notif = RpcNotification::new(method, params);
        let body = serde_json::to_vec(&notif)?;
        let frame = encode_frame(&body);
        self.write_tx
            .send(frame)
            .await
            .map_err(|_| LspError::ServerTerminated)?;
        Ok(())
    }

    pub async fn initialize(&self, timeout: Duration) -> Result<Value, LspError> {
        let root_uri = path_to_uri(&self.config.workspace_root);
        let params = json!({
            "processId": std::process::id(),
            "rootUri": root_uri,
            "rootPath": self.config.workspace_root.to_string_lossy(),
            "workspaceFolders": [
                {
                    "uri": root_uri,
                    "name": self.config.workspace_root.file_name().and_then(|n| n.to_str()).unwrap_or("workspace")
                }
            ],
            "capabilities": default_client_capabilities(),
            "initializationOptions": self.config.initialization_options
        });

        let result = self
            .send_request("initialize", Some(params), timeout)
            .await?;
        self.send_notification("initialized", Some(json!({})))
            .await?;

        *self.state.lock() = ClientState::Ready;
        info!(
            "LSP server '{}' initialized successfully",
            self.config.server_id
        );
        Ok(result)
    }

    pub async fn did_open(&self, doc: &DocumentState) -> Result<(), LspError> {
        self.send_notification("textDocument/didOpen", Some(doc.to_did_open_params()))
            .await
    }

    pub async fn did_change(&self, doc: &DocumentState) -> Result<(), LspError> {
        self.send_notification("textDocument/didChange", Some(doc.to_did_change_params()))
            .await
    }

    pub async fn did_save(&self, doc: &DocumentState) -> Result<(), LspError> {
        self.send_notification("textDocument/didSave", Some(doc.to_did_save_params()))
            .await
    }

    pub async fn did_close(&self, doc: &DocumentState) -> Result<(), LspError> {
        self.send_notification("textDocument/didClose", Some(doc.to_did_close_params()))
            .await
    }

    pub async fn completion(
        &self,
        path: &Path,
        line: u32,
        character: u32,
        timeout: Duration,
    ) -> Result<Vec<CompletionItem>, LspError> {
        let uri = path_to_uri(path);
        let params = json!({
            "textDocument": { "uri": uri },
            "position": { "line": line, "character": character }
        });

        let val = self
            .send_request("textDocument/completion", Some(params), timeout)
            .await?;

        if val.is_null() {
            return Ok(Vec::new());
        }

        if let Ok(list) = serde_json::from_value::<CompletionList>(val.clone()) {
            return Ok(list.items);
        }

        if let Ok(items) = serde_json::from_value::<Vec<CompletionItem>>(val) {
            return Ok(items);
        }

        Ok(Vec::new())
    }

    pub async fn hover(
        &self,
        path: &Path,
        line: u32,
        character: u32,
        timeout: Duration,
    ) -> Result<Option<HoverResult>, LspError> {
        let uri = path_to_uri(path);
        let params = json!({
            "textDocument": { "uri": uri },
            "position": { "line": line, "character": character }
        });

        let val = self
            .send_request("textDocument/hover", Some(params), timeout)
            .await?;
        if val.is_null() {
            return Ok(None);
        }

        // Contents can be string, MarkupContent { kind, value }, or MarkedString array
        let mut contents_text = String::new();
        if let Some(c) = val.get("contents") {
            if let Some(s) = c.as_str() {
                contents_text = s.to_string();
            } else if let Some(obj) = c.as_object() {
                if let Some(val_str) = obj.get("value").and_then(|v| v.as_str()) {
                    contents_text = val_str.to_string();
                }
            } else if let Some(arr) = c.as_array() {
                for item in arr {
                    if let Some(s) = item.as_str() {
                        contents_text.push_str(s);
                        contents_text.push('\n');
                    } else if let Some(v) = item.get("value").and_then(|v| v.as_str()) {
                        contents_text.push_str(v);
                        contents_text.push('\n');
                    }
                }
            }
        }

        let range = val
            .get("range")
            .and_then(|r| serde_json::from_value::<Range>(r.clone()).ok());

        Ok(Some(HoverResult {
            contents: contents_text,
            range,
        }))
    }

    pub async fn definition(
        &self,
        path: &Path,
        line: u32,
        character: u32,
        timeout: Duration,
    ) -> Result<Vec<Location>, LspError> {
        let uri = path_to_uri(path);
        let params = json!({
            "textDocument": { "uri": uri },
            "position": { "line": line, "character": character }
        });

        let val = self
            .send_request("textDocument/definition", Some(params), timeout)
            .await?;
        if val.is_null() {
            return Ok(Vec::new());
        }

        // Can be a single Location, array of Locations, or LocationLink array
        if let Ok(loc) = serde_json::from_value::<Location>(val.clone()) {
            return Ok(vec![loc]);
        }

        if let Ok(locs) = serde_json::from_value::<Vec<Location>>(val.clone()) {
            return Ok(locs);
        }

        // Fallback: LocationLink array with targetUri and targetRange
        if let Some(links) = val.as_array() {
            let mut out = Vec::new();
            for link in links {
                if let (Some(target_uri), Some(target_range)) = (
                    link.get("targetUri").and_then(|u| u.as_str()),
                    link.get("targetSelectionRange")
                        .or_else(|| link.get("targetRange")),
                ) {
                    if let Ok(range) = serde_json::from_value::<Range>(target_range.clone()) {
                        out.push(Location {
                            uri: target_uri.to_string(),
                            range,
                        });
                    }
                }
            }
            return Ok(out);
        }

        Ok(Vec::new())
    }

    pub async fn shutdown(&self, timeout: Duration) -> Result<(), LspError> {
        *self.state.lock() = ClientState::ShuttingDown;
        let _ = self.send_request("shutdown", None, timeout).await;
        let _ = self.send_notification("exit", None).await;

        let child_opt = { self.child_handle.lock().take() };
        if let Some(mut child) = child_opt {
            match tokio::time::timeout(Duration::from_millis(1500), child.wait()).await {
                Ok(Ok(exit_status)) => {
                    debug!("LSP server reaped cleanly: {}", exit_status);
                }
                _ => {
                    warn!("LSP server did not exit in time; killing process");
                    let _ = child.kill().await;
                }
            }
        }

        *self.state.lock() = ClientState::Terminated;
        Ok(())
    }
}
