use crate::client::{ClientState, LspClient, LspClientConfig, LspError};
use crate::document::{path_to_uri, DocumentState};
use crate::types::{CompletionItem, Diagnostic, HoverResult, Location, PublishDiagnosticsParams};
use parking_lot::RwLock;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::broadcast;
use tracing::{error, info};

#[derive(Debug, Clone)]
pub struct LanguageServerDefinition {
    pub language_id: String,
    pub file_extensions: Vec<String>,
    pub command: String,
    pub args: Vec<String>,
    pub initialization_options: Option<serde_json::Value>,
}

#[derive(Debug, Clone)]
struct ServerHealth {
    restart_count: u32,
    last_spawn: Instant,
}

pub struct LspMultiplexer {
    workspace_root: PathBuf,
    server_defs: Arc<RwLock<HashMap<String, LanguageServerDefinition>>>,
    active_clients: Arc<RwLock<HashMap<String, Arc<LspClient>>>>,
    open_documents: Arc<RwLock<HashMap<PathBuf, DocumentState>>>,
    diagnostics_store: Arc<RwLock<HashMap<String, Vec<Diagnostic>>>>,
    diagnostics_tx: broadcast::Sender<PublishDiagnosticsParams>,
    server_health: Arc<RwLock<HashMap<String, ServerHealth>>>,
}

impl LspMultiplexer {
    pub fn new(workspace_root: PathBuf) -> Self {
        let (diagnostics_tx, _) = broadcast::channel(1024);
        let mut multiplexer = Self {
            workspace_root,
            server_defs: Arc::new(RwLock::new(HashMap::new())),
            active_clients: Arc::new(RwLock::new(HashMap::new())),
            open_documents: Arc::new(RwLock::new(HashMap::new())),
            diagnostics_store: Arc::new(RwLock::new(HashMap::new())),
            diagnostics_tx,
            server_health: Arc::new(RwLock::new(HashMap::new())),
        };

        multiplexer.register_default_servers();
        multiplexer
    }

    pub fn register_default_servers(&mut self) {
        // Rust -> rust-analyzer
        self.register_server(LanguageServerDefinition {
            language_id: "rust".to_string(),
            file_extensions: vec!["rs".to_string()],
            command: "rust-analyzer".to_string(),
            args: Vec::new(),
            initialization_options: None,
        });

        // TypeScript / JavaScript -> typescript-language-server
        self.register_server(LanguageServerDefinition {
            language_id: "typescript".to_string(),
            file_extensions: vec![
                "ts".to_string(),
                "tsx".to_string(),
                "js".to_string(),
                "jsx".to_string(),
                "json".to_string(),
            ],
            command: "typescript-language-server".to_string(),
            args: vec!["--stdio".to_string()],
            initialization_options: None,
        });

        // Python -> pyright-langserver
        self.register_server(LanguageServerDefinition {
            language_id: "python".to_string(),
            file_extensions: vec!["py".to_string()],
            command: "pyright-langserver".to_string(),
            args: vec!["--stdio".to_string()],
            initialization_options: None,
        });
    }

    pub fn register_server(&self, def: LanguageServerDefinition) {
        self.server_defs
            .write()
            .insert(def.language_id.clone(), def);
    }

    pub fn subscribe_diagnostics(&self) -> broadcast::Receiver<PublishDiagnosticsParams> {
        self.diagnostics_tx.subscribe()
    }

    pub fn resolve_language_id(&self, path: &Path, override_lang: Option<&str>) -> Option<String> {
        if let Some(lang) = override_lang {
            return Some(lang.to_string());
        }

        let ext = path.extension()?.to_str()?.to_lowercase();
        let defs = self.server_defs.read();
        for (lang_id, def) in defs.iter() {
            if def
                .file_extensions
                .iter()
                .any(|e| e.eq_ignore_ascii_case(&ext))
            {
                return Some(lang_id.clone());
            }
        }
        None
    }

    pub async fn get_or_spawn_client(&self, language_id: &str) -> Result<Arc<LspClient>, LspError> {
        // Check if existing client is healthy and ready
        {
            let clients = self.active_clients.read();
            if let Some(client) = clients.get(language_id) {
                if client.state() == ClientState::Ready || client.state() == ClientState::Starting {
                    return Ok(client.clone());
                }
            }
        }

        let def = {
            let defs = self.server_defs.read();
            defs.get(language_id).cloned().ok_or_else(|| {
                LspError::NotReady(format!(
                    "No language server registered for '{}'",
                    language_id
                ))
            })?
        };

        // Throttle rapid crash loops
        {
            let mut health = self.server_health.write();
            let entry = health
                .entry(language_id.to_string())
                .or_insert_with(|| ServerHealth {
                    restart_count: 0,
                    last_spawn: Instant::now(),
                });

            if entry.last_spawn.elapsed() < Duration::from_secs(5) {
                entry.restart_count += 1;
                if entry.restart_count > 5 {
                    return Err(LspError::NotReady(format!(
                        "Language server '{}' is crashing repeatedly (exponential backoff active)",
                        language_id
                    )));
                }
            } else {
                entry.restart_count = 0;
            }
            entry.last_spawn = Instant::now();
        }

        let config = LspClientConfig {
            server_id: language_id.to_string(),
            command: def.command.clone(),
            args: def.args.clone(),
            workspace_root: self.workspace_root.clone(),
            initialization_options: def.initialization_options.clone(),
        };

        let client = Arc::new(LspClient::spawn(config).await?);

        // Subscribe to diagnostics from this client and bridge to multiplexer
        let mut diag_rx = client.subscribe_diagnostics();
        let store_clone = self.diagnostics_store.clone();
        let tx_clone = self.diagnostics_tx.clone();

        tokio::spawn(async move {
            while let Ok(params) = diag_rx.recv().await {
                store_clone
                    .write()
                    .insert(params.uri.clone(), params.diagnostics.clone());
                let _ = tx_clone.send(params);
            }
        });

        // Initialize with 10s timeout
        if let Err(e) = client.initialize(Duration::from_secs(10)).await {
            error!("Failed to initialize LSP server '{}': {}", language_id, e);
            let _ = client.shutdown(Duration::from_secs(2)).await;
            return Err(e);
        }

        // Re-open all existing open documents for this language
        let docs_to_open: Vec<DocumentState> = {
            let docs = self.open_documents.read();
            docs.values()
                .filter(|d| d.language_id == language_id)
                .cloned()
                .collect()
        };
        for doc in docs_to_open {
            let _ = client.did_open(&doc).await;
        }

        self.active_clients
            .write()
            .insert(language_id.to_string(), client.clone());

        Ok(client)
    }

    pub async fn open_document(
        &self,
        path: &Path,
        content: &str,
        language_id: Option<&str>,
    ) -> Result<(), LspError> {
        let lang = self
            .resolve_language_id(path, language_id)
            .ok_or_else(|| LspError::NotReady(format!("Unsupported file type: {:?}", path)))?;

        let uri = path_to_uri(path);
        let doc = DocumentState::new(uri, &lang, content);

        self.open_documents
            .write()
            .insert(path.to_path_buf(), doc.clone());

        let client = self.get_or_spawn_client(&lang).await?;
        client.did_open(&doc).await
    }

    pub async fn change_document(
        &self,
        path: &Path,
        content: &str,
        version: i64,
    ) -> Result<(), LspError> {
        let uri = path_to_uri(path);
        let lang = self
            .resolve_language_id(path, None)
            .ok_or_else(|| LspError::NotReady(format!("Unsupported file type: {:?}", path)))?;

        let doc = DocumentState {
            uri,
            language_id: lang.clone(),
            version,
            content: content.to_string(),
        };

        self.open_documents
            .write()
            .insert(path.to_path_buf(), doc.clone());

        let client_opt = self.active_clients.read().get(&lang).cloned();
        if let Some(client) = client_opt {
            client.did_change(&doc).await?;
        }
        Ok(())
    }

    pub async fn save_document(&self, path: &Path) -> Result<(), LspError> {
        let doc = self
            .open_documents
            .read()
            .get(path)
            .cloned()
            .ok_or_else(|| LspError::NotReady(format!("Document not open: {:?}", path)))?;

        let client_opt = self.active_clients.read().get(&doc.language_id).cloned();
        if let Some(client) = client_opt {
            client.did_save(&doc).await?;
        }
        Ok(())
    }

    pub async fn close_document(&self, path: &Path) -> Result<(), LspError> {
        let doc_opt = self.open_documents.write().remove(path);
        if let Some(doc) = doc_opt {
            let client_opt = self.active_clients.read().get(&doc.language_id).cloned();
            if let Some(client) = client_opt {
                client.did_close(&doc).await?;
            }
        }
        Ok(())
    }

    pub async fn completion(
        &self,
        path: &Path,
        line: u32,
        character: u32,
    ) -> Result<Vec<CompletionItem>, LspError> {
        let lang = self
            .resolve_language_id(path, None)
            .ok_or_else(|| LspError::NotReady(format!("Unsupported file type: {:?}", path)))?;

        let client = self.get_or_spawn_client(&lang).await?;
        client
            .completion(path, line, character, Duration::from_secs(5))
            .await
    }

    pub async fn hover(
        &self,
        path: &Path,
        line: u32,
        character: u32,
    ) -> Result<Option<HoverResult>, LspError> {
        let lang = self
            .resolve_language_id(path, None)
            .ok_or_else(|| LspError::NotReady(format!("Unsupported file type: {:?}", path)))?;

        let client = self.get_or_spawn_client(&lang).await?;
        client
            .hover(path, line, character, Duration::from_secs(5))
            .await
    }

    pub async fn definition(
        &self,
        path: &Path,
        line: u32,
        character: u32,
    ) -> Result<Vec<Location>, LspError> {
        let lang = self
            .resolve_language_id(path, None)
            .ok_or_else(|| LspError::NotReady(format!("Unsupported file type: {:?}", path)))?;

        let client = self.get_or_spawn_client(&lang).await?;
        client
            .definition(path, line, character, Duration::from_secs(5))
            .await
    }

    pub fn get_diagnostics(&self, path: &Path) -> Vec<Diagnostic> {
        let uri = path_to_uri(path);
        self.diagnostics_store
            .read()
            .get(&uri)
            .cloned()
            .unwrap_or_default()
    }

    pub async fn shutdown_all(&self) -> Result<(), LspError> {
        let clients: Vec<Arc<LspClient>> = self
            .active_clients
            .write()
            .drain()
            .map(|(_, c)| c)
            .collect();
        for client in clients {
            let _ = client.shutdown(Duration::from_secs(2)).await;
        }
        info!("All active LSP clients shut down cleanly");
        Ok(())
    }
}
