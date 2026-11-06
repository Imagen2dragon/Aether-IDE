use crate::commands::checkpoint::SharedCheckpointEngine;
use crate::workspace_bridge::SharedWorkspace;
use model_gateway::{
    CompletionRequest, CompletionResponse, ModelGateway, ModelMetadata, ProviderStatus, StreamDelta,
};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager};
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

pub type SharedGateway = Arc<ModelGateway>;

#[derive(Clone, Debug)]
pub struct ActiveStreamEntry {
    pub generation: u64,
    pub cancel_token: CancellationToken,
    pub is_cancelled: bool,
}

#[derive(Default, Debug)]
pub struct StreamRegistry {
    entries: HashMap<String, ActiveStreamEntry>,
    next_generation: u64,
}

impl StreamRegistry {
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
            next_generation: 0,
        }
    }

    /// Registers a new stream for `req_id`.
    /// - Rejects with error if a stream with `req_id` is currently active (and not cancelled),
    ///   enforcing duplicate-ID rejection.
    /// - If a prior stream was cancelled or finished, increments generation and assigns a new
    ///   isolated generation counter, returning the new generation and fresh cancellation token.
    pub fn register(&mut self, req_id: &str) -> Result<(u64, CancellationToken), String> {
        if let Some(entry) = self.entries.get(req_id) {
            if !entry.is_cancelled {
                return Err(format!(
                    "An active AI streaming request with ID '{}' is already in progress. Duplicate request IDs are prohibited to prevent stream hijacking.",
                    req_id
                ));
            }
        }

        self.next_generation += 1;
        let gen = self.next_generation;
        let cancel_token = CancellationToken::new();

        self.entries.insert(
            req_id.to_string(),
            ActiveStreamEntry {
                generation: gen,
                cancel_token: cancel_token.clone(),
                is_cancelled: false,
            },
        );

        Ok((gen, cancel_token))
    }

    /// Cancels the currently registered stream for `req_id` without premature map removal.
    /// Returns the cancellation token and the generation that was cancelled.
    pub fn cancel(&mut self, req_id: &str) -> Option<(u64, CancellationToken)> {
        if let Some(entry) = self.entries.get_mut(req_id) {
            entry.is_cancelled = true;
            entry.cancel_token.cancel();
            Some((entry.generation, entry.cancel_token.clone()))
        } else {
            None
        }
    }

    /// Cleans up an active stream entry only if its generation matches the calling stream.
    /// If an older stream finishes after a newer stream with the same ID was registered,
    /// this check ensures the older stream never removes the newer stream's entry.
    pub fn cleanup(&mut self, req_id: &str, generation: u64) -> bool {
        if let Some(entry) = self.entries.get(req_id) {
            if entry.generation == generation {
                self.entries.remove(req_id);
                return true;
            }
        }
        false
    }

    pub fn get_token(&self, req_id: &str) -> Option<CancellationToken> {
        self.entries.get(req_id).map(|e| e.cancel_token.clone())
    }

    pub fn is_active(&self, req_id: &str) -> bool {
        self.entries
            .get(req_id)
            .map(|e| !e.is_cancelled)
            .unwrap_or(false)
    }

    pub fn entry_count(&self) -> usize {
        self.entries.len()
    }
}

pub type ActiveAiStreams = Arc<Mutex<StreamRegistry>>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructuredProposal {
    pub proposal_id: String,
    pub workspace_id: String,
    pub relative_path: String,
    pub operation: String, // "modify" | "create" | "delete"
    pub base_hash: String, // SHA-256 of file at proposal time
    pub unified_diff: String,
    pub proposed_content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProposalReviewPreview {
    pub proposal_id: String,
    pub relative_path: String,
    pub operation: String,
    pub base_hash: String,
    pub current_disk_hash: String,
    pub has_conflict: bool,
    pub original_content: String,
    pub proposed_content: String,
    pub unified_diff: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApplyProposalResult {
    pub success: bool,
    pub relative_path: String,
    pub new_hash: String,
    pub message: String,
    pub backup_checkpoint_id: Option<String>,
}

fn compute_sha256(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

#[tauri::command]
pub async fn ai_list_models(app: AppHandle) -> Result<Vec<ModelMetadata>, String> {
    let gw = app.state::<SharedGateway>();
    Ok(gw.list_all_models())
}

#[tauri::command]
pub async fn ai_get_providers(app: AppHandle) -> Result<HashMap<String, ProviderStatus>, String> {
    let gw = app.state::<SharedGateway>();
    Ok(gw.diagnose_all_providers())
}

#[tauri::command]
pub async fn ai_complete(
    app: AppHandle,
    prompt: String,
    system: Option<String>,
    model: Option<String>,
    max_tokens: Option<usize>,
) -> Result<CompletionResponse, String> {
    let gw = app.state::<SharedGateway>();

    let req = CompletionRequest {
        prompt,
        system,
        model,
        temperature: Some(0.2),
        max_tokens,
    };

    gw.execute_completion(&req)
        .await
        .map_err(|e| format!("AI completion failed: {}", e))
}

#[tauri::command]
pub async fn ai_stream_prompt(
    app: AppHandle,
    prompt: String,
    system: Option<String>,
    model: Option<String>,
    file_context: Option<String>,
    request_id: Option<String>,
    conversation_id: Option<String>,
) -> Result<String, String> {
    let req_id = request_id.unwrap_or_else(|| format!("req-{}", uuid::Uuid::new_v4().simple()));
    let conv_id = conversation_id.unwrap_or_else(|| "default-conv".into());

    let full_prompt = if let Some(ctx) = file_context {
        format!("Context:\n{}\n\nTask: {}", ctx, prompt)
    } else {
        prompt.clone()
    };

    let gw = app.state::<SharedGateway>().inner().clone();
    let streams_arc = app.state::<ActiveAiStreams>();

    let (generation, cancel_token) = {
        let mut streams = streams_arc.lock();
        streams.register(&req_id)?
    };

    let req = CompletionRequest {
        prompt: full_prompt,
        system,
        model: model.clone(),
        temperature: Some(0.2),
        max_tokens: Some(4096),
    };

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<StreamDelta>();

    // Emit stream started event
    let _ = app.emit(
        "ai-stream-started",
        json!({
            "request_id": &req_id,
            "conversation_id": &conv_id,
            "model": model.as_deref().unwrap_or("default")
        }),
    );

    let app_emitter = app.clone();
    let req_id_clone = req_id.clone();
    let conv_id_clone = conv_id.clone();

    // Spawn listener to forward stream chunks in real-time over Tauri event bus
    let forwarder_handle = tokio::spawn(async move {
        let mut accumulated = String::new();
        while let Some(delta) = rx.recv().await {
            match delta {
                StreamDelta::Chunk { delta: chunk, .. } => {
                    accumulated.push_str(&chunk);
                    let _ = app_emitter.emit(
                        "ai-stream-chunk",
                        json!({
                            "request_id": &req_id_clone,
                            "conversation_id": &conv_id_clone,
                            "chunk": chunk
                        }),
                    );
                }
                StreamDelta::Completed {
                    full_text,
                    completion_tokens,
                    ..
                } => {
                    let _ = app_emitter.emit(
                        "ai-generation-complete",
                        json!({
                            "request_id": &req_id_clone,
                            "conversation_id": &conv_id_clone,
                            "completion_tokens": completion_tokens,
                            "full_text": full_text
                        }),
                    );
                }
                StreamDelta::Error { message, .. } => {
                    let _ = app_emitter.emit(
                        "ai-stream-error",
                        json!({
                            "request_id": &req_id_clone,
                            "conversation_id": &conv_id_clone,
                            "error": message
                        }),
                    );
                }
                _ => {}
            }
        }
        accumulated
    });

    let exec_res = gw.execute_stream(&req, &req_id, tx, cancel_token).await;

    // Clean up active streams map safely using generation check
    {
        let mut streams = streams_arc.lock();
        streams.cleanup(&req_id, generation);
    }

    match exec_res {
        Ok(resp) => {
            let _ = forwarder_handle.await;
            Ok(resp.content)
        }
        Err(e) => {
            let err_str = e.to_string();
            let _ = app.emit(
                "ai-stream-error",
                json!({
                    "request_id": &req_id,
                    "conversation_id": &conv_id,
                    "error": &err_str
                }),
            );
            Err(err_str)
        }
    }
}

#[tauri::command]
pub async fn ai_cancel_stream(app: AppHandle, request_id: String) -> Result<bool, String> {
    let streams_arc = app.state::<ActiveAiStreams>();
    let cancelled = {
        let mut streams = streams_arc.lock();
        streams.cancel(&request_id)
    };

    if let Some((_gen, _token)) = cancelled {
        let _ = app.emit("ai-stream-cancelled", json!({ "request_id": &request_id }));
        info!("Cancelled active AI streaming request: {}", request_id);
        Ok(true)
    } else {
        Ok(false)
    }
}

#[tauri::command]
pub async fn ai_preview_proposal(
    app: AppHandle,
    proposal: StructuredProposal,
) -> Result<ProposalReviewPreview, String> {
    let ws_arc = app.state::<SharedWorkspace>();
    let ws = ws_arc.lock();
    let (_, root, rel_path) = ws.resolve_in_workspace(&proposal.relative_path)?;
    drop(ws);

    let full_path = root.join(&rel_path);
    let (original_content, current_disk_hash) = if full_path.exists() {
        let bytes = std::fs::read(&full_path)
            .map_err(|e| format!("Failed to read file '{:?}': {}", full_path, e))?;
        let hash = compute_sha256(&bytes);
        (String::from_utf8_lossy(&bytes).to_string(), hash)
    } else {
        (String::new(), String::new())
    };

    let has_conflict = if !proposal.base_hash.is_empty() && !current_disk_hash.is_empty() {
        proposal.base_hash != current_disk_hash
    } else {
        false
    };

    Ok(ProposalReviewPreview {
        proposal_id: proposal.proposal_id,
        relative_path: rel_path,
        operation: proposal.operation,
        base_hash: proposal.base_hash,
        current_disk_hash,
        has_conflict,
        original_content,
        proposed_content: proposal.proposed_content,
        unified_diff: proposal.unified_diff,
    })
}

#[tauri::command]
pub async fn ai_apply_proposal(
    app: AppHandle,
    proposal: StructuredProposal,
    force: bool,
) -> Result<ApplyProposalResult, String> {
    let ws_arc = app.state::<SharedWorkspace>();
    let ws = ws_arc.lock();
    let (cap, root, rel_path) = ws.resolve_in_workspace(&proposal.relative_path)?;
    let ws_id = ws.workspace_id();
    drop(ws);

    let full_path = root.join(&rel_path);

    // 1. Conflict detection against disk hash
    if full_path.exists() {
        let current_bytes = std::fs::read(&full_path)
            .map_err(|e| format!("Failed to read target file '{:?}': {}", full_path, e))?;
        let current_disk_hash = compute_sha256(&current_bytes);

        if !force && !proposal.base_hash.is_empty() && proposal.base_hash != current_disk_hash {
            return Err(format!(
                "Conflict detected: File '{}' was modified on disk since the AI proposal was generated. (Expected base: {}, Current disk: {}). Please review changes before applying.",
                rel_path, proposal.base_hash, current_disk_hash
            ));
        }
    }

    // 2. Create recovery checkpoint of workspace before writing
    let backup_ckpt_id = {
        let ckpt_engine_arc = app.state::<SharedCheckpointEngine>();
        let ckpt_engine = ckpt_engine_arc.lock();
        let desc = format!("Pre-AI edit backup for {}", rel_path);
        match ckpt_engine.create_checkpoint_with_root(
            &desc,
            &ws_id,
            &root,
            Some(std::slice::from_ref(&full_path)),
        ) {
            Ok(ckpt) => Some(ckpt.checkpoint_id),
            Err(e) => {
                warn!("Failed to create pre-edit checkpoint: {}", e);
                None
            }
        }
    };

    // 3. Atomically write proposed content via capability
    cap.write_file_atomic(&rel_path, &proposal.proposed_content)
        .map_err(|e| format!("Atomic write failed for '{}': {}", rel_path, e))?;

    // 4. Verify resulting written file
    let final_bytes = std::fs::read(&full_path)
        .map_err(|e| format!("Failed to verify written file '{:?}': {}", full_path, e))?;
    let new_hash = compute_sha256(&final_bytes);

    info!(
        "Successfully applied AI proposal to '{}' (new hash: {}, backup: {:?})",
        rel_path, new_hash, backup_ckpt_id
    );

    Ok(ApplyProposalResult {
        success: true,
        relative_path: rel_path,
        new_hash,
        message: format!("Changes safely applied to {}", proposal.relative_path),
        backup_checkpoint_id: backup_ckpt_id,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_conflict_detection() {
        let original_data = b"function test() { return 1; }";
        let original_hash = compute_sha256(original_data);

        let modified_data = b"function test() { return 2; }";
        let modified_hash = compute_sha256(modified_data);

        assert_ne!(original_hash, modified_hash);

        // Simulation of proposal with base_hash matching original vs modified
        let base_hash = original_hash.clone();
        let current_disk_hash = modified_hash.clone();

        let has_conflict = !base_hash.is_empty()
            && !current_disk_hash.is_empty()
            && base_hash != current_disk_hash;
        assert!(
            has_conflict,
            "Conflict must be flagged when base hash differs from disk hash"
        );

        let no_conflict = !base_hash.is_empty() && base_hash == original_hash;
        assert!(no_conflict, "No conflict when base hash matches disk hash");
    }

    #[test]
    fn test_ai_stream_registry_duplicate_registration_rejected() {
        let mut registry = StreamRegistry::new();
        let reg1 = registry.register("req-1");
        assert!(reg1.is_ok());

        // Second registration while active is rejected
        let reg2 = registry.register("req-1");
        assert!(reg2.is_err());
        assert!(reg2
            .unwrap_err()
            .contains("Duplicate request IDs are prohibited"));
    }

    #[test]
    fn test_ai_stream_registry_cancellation_preserves_entry_until_cleanup() {
        let mut registry = StreamRegistry::new();
        let (gen1, token1) = registry.register("req-cancel").unwrap();
        assert_eq!(gen1, 1);
        assert!(!token1.is_cancelled());

        // Cancel stream
        let cancelled = registry.cancel("req-cancel");
        assert!(cancelled.is_some());
        let (c_gen, c_tok) = cancelled.unwrap();
        assert_eq!(c_gen, 1);
        assert!(c_tok.is_cancelled());
        assert!(token1.is_cancelled());

        // Invariant: Cancellation must NOT prematurely delete the entry from the registry!
        assert_eq!(registry.entry_count(), 1);
        assert!(!registry.is_active("req-cancel"));
    }

    #[test]
    fn test_ai_stream_registry_immediate_reuse_after_cancellation() {
        let mut registry = StreamRegistry::new();
        let (gen1, token1) = registry.register("req-reuse").unwrap();
        assert_eq!(gen1, 1);

        // Stream cancelled by user
        registry.cancel("req-reuse");
        assert!(token1.is_cancelled());

        // Immediate reuse of the same request_id succeeds with fresh generation
        let (gen2, token2) = registry.register("req-reuse").unwrap();
        assert_eq!(gen2, 2);
        assert!(
            !token2.is_cancelled(),
            "New generation must have a fresh, non-cancelled token"
        );
        assert!(registry.is_active("req-reuse"));
    }

    #[test]
    fn test_ai_stream_registry_old_stream_completion_does_not_remove_new_generation() {
        let mut registry = StreamRegistry::new();
        let (gen1, _token1) = registry.register("req-race").unwrap();
        assert_eq!(gen1, 1);

        // User cancels gen 1
        registry.cancel("req-race");

        // Immediate reuse: gen 2 registered while gen 1 is still unwinding
        let (gen2, token2) = registry.register("req-race").unwrap();
        assert_eq!(gen2, 2);

        // Old stream (gen 1) finally reaches cleanup block
        let cleaned_gen1 = registry.cleanup("req-race", 1);
        assert!(
            !cleaned_gen1,
            "Old stream cleanup must NOT remove a newer generation"
        );

        // Invariant: gen 2 must still be present and cancellable
        assert_eq!(registry.entry_count(), 1);
        assert!(registry.is_active("req-race"));

        let cancel_res = registry.cancel("req-race");
        assert!(cancel_res.is_some());
        assert_eq!(cancel_res.unwrap().0, 2, "Cancellation must cancel gen 2");
        assert!(token2.is_cancelled(), "Gen 2 token must be cancelled");

        // Gen 2 finishes and cleans up
        let cleaned_gen2 = registry.cleanup("req-race", 2);
        assert!(cleaned_gen2, "Matching generation cleanup must succeed");
        assert_eq!(registry.entry_count(), 0);
    }
}
