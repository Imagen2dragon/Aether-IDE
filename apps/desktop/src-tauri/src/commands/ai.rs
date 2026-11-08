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
use tracing::info;

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
    pub operation: String, // "replace_file" | "apply_patch" | "insert_at" | "create_file" | "delete_file" | "modify"
    pub base_hash: String, // SHA-256 of file at proposal time
    pub unified_diff: String,
    pub proposed_content: String,
    #[serde(default)]
    pub insertion_line: Option<u32>,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiffLine {
    Context(String),
    Delete(String),
    Insert(String),
}

#[derive(Debug, Clone)]
pub struct DiffHunk {
    pub old_start: usize,
    pub old_count: usize,
    pub new_start: usize,
    pub new_count: usize,
    pub lines: Vec<DiffLine>,
}

fn validate_hunk(hunk: &DiffHunk) -> Result<(), String> {
    if hunk.old_start == 0 && hunk.old_count > 0 {
        return Err(format!(
            "Malformed patch: Invalid hunk header -{},{}: old_start cannot be 0 when old_count is non-zero",
            hunk.old_start, hunk.old_count
        ));
    }
    if hunk.new_start == 0 && hunk.new_count > 0 {
        return Err(format!(
            "Malformed patch: Invalid hunk header +{},{}: new_start cannot be 0 when new_count is non-zero",
            hunk.new_start, hunk.new_count
        ));
    }

    let consumed = hunk
        .lines
        .iter()
        .filter(|l| matches!(l, DiffLine::Context(_) | DiffLine::Delete(_)))
        .count();
    if consumed != hunk.old_count {
        return Err(format!(
            "Malformed patch: Hunk header claims -{},{} but body consumes {} lines",
            hunk.old_start, hunk.old_count, consumed
        ));
    }

    let produced = hunk
        .lines
        .iter()
        .filter(|l| matches!(l, DiffLine::Context(_) | DiffLine::Insert(_)))
        .count();
    if produced != hunk.new_count {
        return Err(format!(
            "Malformed patch: Hunk header claims +{},{} but body produces {} lines",
            hunk.new_start, hunk.new_count, produced
        ));
    }

    Ok(())
}

pub fn parse_unified_diff_internal(patch: &str) -> Result<(Vec<DiffHunk>, bool), String> {
    let re = regex::Regex::new(r"^@@\s+-(\d+)(?:,(\d+))?\s+\+(\d+)(?:,(\d+))?\s+@@")
        .map_err(|e| e.to_string())?;
    let mut hunks = Vec::new();
    let mut current_hunk: Option<DiffHunk> = None;
    let mut has_seen_hunk_header = false;
    let mut last_produced_has_no_newline = false;

    for raw_line in patch.lines() {
        let line = raw_line.strip_suffix('\r').unwrap_or(raw_line);

        if let Some(caps) = re.captures(line) {
            has_seen_hunk_header = true;
            if let Some(h) = current_hunk.take() {
                validate_hunk(&h)?;
                hunks.push(h);
            }

            let old_start: usize = caps[1]
                .parse()
                .map_err(|_| "Invalid old_start in hunk header".to_string())?;
            let old_count: usize = caps
                .get(2)
                .map_or(Ok(1), |m| m.as_str().parse())
                .map_err(|_| "Invalid old_count in hunk header".to_string())?;
            let new_start: usize = caps[3]
                .parse()
                .map_err(|_| "Invalid new_start in hunk header".to_string())?;
            let new_count: usize = caps
                .get(4)
                .map_or(Ok(1), |m| m.as_str().parse())
                .map_err(|_| "Invalid new_count in hunk header".to_string())?;

            current_hunk = Some(DiffHunk {
                old_start,
                old_count,
                new_start,
                new_count,
                lines: Vec::new(),
            });
            continue;
        }

        if let Some(ref mut h) = current_hunk {
            if let Some(stripped) = line.strip_prefix('+') {
                h.lines.push(DiffLine::Insert(stripped.to_string()));
                last_produced_has_no_newline = false;
            } else if let Some(stripped) = line.strip_prefix('-') {
                h.lines.push(DiffLine::Delete(stripped.to_string()));
            } else if let Some(stripped) = line.strip_prefix(' ') {
                h.lines.push(DiffLine::Context(stripped.to_string()));
                last_produced_has_no_newline = false;
            } else if line.starts_with('\\') {
                if let Some(last) = h.lines.last() {
                    if matches!(last, DiffLine::Insert(_) | DiffLine::Context(_)) {
                        last_produced_has_no_newline = true;
                    }
                }
                continue;
            } else if line.is_empty() {
                return Err("Malformed patch: malformed empty line in hunk body without diff prefix (' ', '+', '-')".into());
            } else {
                return Err(format!(
                    "Malformed patch line: '{}'. Must start with '+', '-', or ' '",
                    line
                ));
            }
        } else {
            if line.starts_with("---")
                || line.starts_with("+++")
                || line.starts_with("diff ")
                || line.starts_with("index ")
            {
                continue;
            }
            if !line.trim().is_empty() {
                return Err(format!(
                    "Malformed patch: Unexpected line before hunk header: '{}'",
                    line
                ));
            }
        }
    }

    if let Some(h) = current_hunk {
        validate_hunk(&h)?;
        hunks.push(h);
    }

    if !has_seen_hunk_header || hunks.is_empty() {
        return Err("Malformed patch: No unified diff hunk headers ('@@ -start,count +start,count @@') found in patch content".into());
    }

    let mut last_old_end = 0;
    let mut last_new_end = 0;
    let mut cumulative_delta: isize = 0;

    for (idx, hunk) in hunks.iter().enumerate() {
        let hunk_old_end = if hunk.old_count == 0 {
            hunk.old_start
        } else {
            hunk.old_start + hunk.old_count - 1
        };

        let hunk_new_end = if hunk.new_count == 0 {
            hunk.new_start
        } else {
            hunk.new_start + hunk.new_count - 1
        };

        if idx > 0 {
            if hunk.old_start <= last_old_end
                && (hunk.old_count > 0 || hunk.old_start < last_old_end)
            {
                return Err(format!(
                    "Ambiguous or overlapping hunks detected: hunk {} starts at line {} before or at previous hunk end {}",
                    idx + 1,
                    hunk.old_start,
                    last_old_end
                ));
            }
            if hunk.new_start <= last_new_end
                && (hunk.new_count > 0 || hunk.new_start < last_new_end)
            {
                return Err(format!(
                    "Invalid hunk ordering or overlapping new range: hunk {} new_start {} is not strictly after previous hunk new_end {}",
                    idx + 1,
                    hunk.new_start,
                    last_new_end
                ));
            }
        }

        // Validate expected new_start position based on preceding hunks and unchanged lines
        let lines_before = if hunk.old_count == 0 {
            (hunk.old_start as isize) + cumulative_delta
        } else if hunk.old_start == 0 {
            0isize
        } else {
            (hunk.old_start as isize - 1) + cumulative_delta
        };

        if lines_before < 0 {
            return Err(format!(
                "Malformed patch: Hunk {} implies negative output line count before hunk: {}",
                idx + 1,
                lines_before
            ));
        }

        let lb = lines_before as usize;
        if hunk.new_count > 0 {
            let expected_new_start = lb + 1;
            if hunk.new_start != expected_new_start {
                return Err(format!(
                    "Malformed patch: Hunk {} new_start ({}) does not match expected output position ({}) based on preceding hunks",
                    idx + 1,
                    hunk.new_start,
                    expected_new_start
                ));
            }
        } else {
            // Zero-length produced lines (e.g. deletion hunk).
            // In POSIX/GNU diff, new_start indicates the line after which the change occurs (0 if at start).
            let expected_new_start = lb;
            if hunk.new_start != expected_new_start {
                return Err(format!(
                    "Malformed patch: Hunk {} zero-length new_start ({}) does not match expected output position ({})",
                    idx + 1,
                    hunk.new_start,
                    expected_new_start
                ));
            }
        }

        cumulative_delta += (hunk.new_count as isize) - (hunk.old_count as isize);
        last_old_end = hunk_old_end;
        last_new_end = hunk_new_end;
    }

    Ok((hunks, last_produced_has_no_newline))
}

pub fn parse_unified_diff(patch: &str) -> Result<Vec<DiffHunk>, String> {
    parse_unified_diff_internal(patch).map(|(hunks, _)| hunks)
}

pub fn apply_unified_diff(original: &str, patch: &str) -> Result<String, String> {
    let (hunks, no_final_newline) = parse_unified_diff_internal(patch)?;
    apply_diff_hunks_internal(original, &hunks, no_final_newline)
}

pub fn apply_diff_hunks_internal(
    original: &str,
    hunks: &[DiffHunk],
    no_final_newline: bool,
) -> Result<String, String> {
    let is_crlf = original.contains("\r\n")
        || (original.is_empty()
            && hunks.iter().any(|h| {
                h.lines.iter().any(|l| match l {
                    DiffLine::Insert(s) | DiffLine::Context(s) => s.contains("\r\n"),
                    _ => false,
                })
            }));
    let orig_has_trailing_newline = original.ends_with('\n');

    let orig_lines: Vec<&str> = if original.is_empty() {
        Vec::new()
    } else {
        original
            .split('\n')
            .map(|l| l.strip_suffix('\r').unwrap_or(l))
            .collect()
    };
    let orig_len =
        if orig_has_trailing_newline && !orig_lines.is_empty() && orig_lines.last() == Some(&"") {
            orig_lines.len() - 1
        } else {
            orig_lines.len()
        };
    let active_orig = &orig_lines[..orig_len];

    let mut output_lines: Vec<String> = Vec::new();
    let mut curr_orig_idx: usize = 0;

    for (hunk_idx, hunk) in hunks.iter().enumerate() {
        let target_idx = if hunk.old_count == 0 {
            hunk.old_start
        } else {
            if hunk.old_start == 0 {
                return Err("Malformed patch: old_start cannot be 0 when old_count > 0".into());
            }
            hunk.old_start - 1
        };

        if target_idx < curr_orig_idx {
            return Err("Hunk overlaps with previously applied lines".into());
        }

        if target_idx > active_orig.len()
            || (hunk.old_count > 0 && target_idx + hunk.old_count > active_orig.len())
        {
            return Err(format!(
                "Hunk {} target range (start: {}, count: {}) exceeds original file length {}",
                hunk_idx + 1,
                hunk.old_start,
                hunk.old_count,
                active_orig.len()
            ));
        }

        while curr_orig_idx < target_idx {
            output_lines.push(active_orig[curr_orig_idx].to_string());
            curr_orig_idx += 1;
        }

        let lines_before = output_lines.len();
        if hunk.new_count > 0 {
            let expected_new_start = lines_before + 1;
            if hunk.new_start != expected_new_start {
                return Err(format!(
                    "Malformed patch: Hunk {} new_start ({}) does not match actual output line position ({})",
                    hunk_idx + 1,
                    hunk.new_start,
                    expected_new_start
                ));
            }
        } else {
            let expected_new_start = lines_before;
            if hunk.new_start != expected_new_start {
                return Err(format!(
                    "Malformed patch: Hunk {} zero-length new_start ({}) does not match actual output line position ({})",
                    hunk_idx + 1,
                    hunk.new_start,
                    expected_new_start
                ));
            }
        }

        for line in &hunk.lines {
            match line {
                DiffLine::Context(expected) => {
                    if curr_orig_idx >= active_orig.len() {
                        return Err(format!(
                            "Patch context mismatch: expected '{}' at line {}, but reached end of file",
                            expected,
                            curr_orig_idx + 1
                        ));
                    }
                    if active_orig[curr_orig_idx] != expected {
                        return Err(format!(
                            "Patch context mismatch at line {}: expected '{}', found '{}'",
                            curr_orig_idx + 1,
                            expected,
                            active_orig[curr_orig_idx]
                        ));
                    }
                    output_lines.push(expected.clone());
                    curr_orig_idx += 1;
                }
                DiffLine::Delete(expected) => {
                    if curr_orig_idx >= active_orig.len() {
                        return Err(format!(
                            "Patch delete mismatch: expected '{}' at line {}, but reached end of file",
                            expected,
                            curr_orig_idx + 1
                        ));
                    }
                    if active_orig[curr_orig_idx] != expected {
                        return Err(format!(
                            "Patch delete mismatch at line {}: expected '{}', found '{}'",
                            curr_orig_idx + 1,
                            expected,
                            active_orig[curr_orig_idx]
                        ));
                    }
                    curr_orig_idx += 1;
                }
                DiffLine::Insert(added) => {
                    output_lines.push(added.clone());
                }
            }
        }
    }

    while curr_orig_idx < active_orig.len() {
        output_lines.push(active_orig[curr_orig_idx].to_string());
        curr_orig_idx += 1;
    }

    if output_lines.is_empty() {
        return Ok(String::new());
    }

    let line_sep = if is_crlf { "\r\n" } else { "\n" };
    let mut result = output_lines.join(line_sep);

    let should_have_trailing_newline = if no_final_newline {
        false
    } else if original.is_empty() {
        true
    } else {
        orig_has_trailing_newline
    };

    if should_have_trailing_newline {
        result.push_str(line_sep);
    }
    Ok(result)
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

    let proposed_preview_content = if proposal.operation == "apply_patch" {
        if !full_path.exists() {
            return Err(format!(
                "Cannot preview patch on non-existent file '{}'. Use create_file instead.",
                rel_path
            ));
        }
        let patch_text = if !proposal.unified_diff.is_empty() {
            &proposal.unified_diff
        } else {
            &proposal.proposed_content
        };
        apply_unified_diff(&original_content, patch_text)?
    } else {
        proposal.proposed_content.clone()
    };

    Ok(ProposalReviewPreview {
        proposal_id: proposal.proposal_id,
        relative_path: rel_path,
        operation: proposal.operation,
        base_hash: proposal.base_hash,
        current_disk_hash,
        has_conflict,
        original_content,
        proposed_content: proposed_preview_content,
        unified_diff: proposal.unified_diff,
    })
}

/// Core production execution logic for applying AI proposals.
/// Shared across Tauri command dispatch and real adversarial integration verification.
pub fn execute_proposal_internal(
    ws: &crate::workspace_bridge::WorkspaceSession,
    ckpt_engine: Option<&checkpoint_engine::CheckpointEngine>,
    proposal: &StructuredProposal,
) -> Result<ApplyProposalResult, String> {
    // 0. Operation contract validation
    let valid_ops = [
        "replace_file",
        "apply_patch",
        "insert_at",
        "create_file",
        "delete_file",
        "modify",
    ];
    if !valid_ops.contains(&proposal.operation.as_str()) {
        return Err(format!(
            "Invalid proposal operation '{}'. Permitted operations: replace_file, apply_patch, insert_at, create_file, delete_file.",
            proposal.operation
        ));
    }

    let (cap, root, rel_path) = ws.resolve_in_workspace(&proposal.relative_path)?;
    let ws_id = ws.workspace_id();

    let full_path = root.join(&rel_path);
    let target_exists = full_path.exists();

    // 1. Mandatory base_hash validation and conflict detection for existing files
    if target_exists {
        if proposal.operation == "create_file" {
            return Err(format!(
                "Cannot create file '{}': file already exists on disk. Use replace_file, apply_patch, or insert_at.",
                rel_path
            ));
        }

        // CRITICAL: Reject empty base_hash on existing file modifications
        if proposal.base_hash.trim().is_empty() {
            return Err(format!(
                "Safety Violation: Proposal base_hash cannot be empty when modifying existing file '{}'. A valid base hash must be obtained via preview before applying.",
                rel_path
            ));
        }

        let current_bytes = std::fs::read(&full_path)
            .map_err(|e| format!("Failed to read target file '{:?}': {}", full_path, e))?;
        let current_disk_hash = compute_sha256(&current_bytes);

        // Strict conflict check: never allow silent overwrite of concurrent modifications
        if proposal.base_hash != current_disk_hash {
            return Err(format!(
                "Conflict detected: File '{}' was modified on disk since the AI proposal was reviewed. (Expected base: {}, Current disk: {}). Please review a fresh preview before applying.",
                rel_path, proposal.base_hash, current_disk_hash
            ));
        }
    } else if proposal.operation != "create_file"
        && proposal.operation != "replace_file"
        && proposal.operation != "modify"
    {
        return Err(format!(
            "Cannot perform operation '{}' on non-existent file '{}'. Use create_file instead.",
            proposal.operation, rel_path
        ));
    }

    // 2. Mandatory Pre-Edit Checkpoint for modifications to existing files (MANDATORY FAIL-CLOSED)
    let mut backup_ckpt_id = None;
    if target_exists {
        if let Some(engine) = ckpt_engine {
            let desc = format!("Pre-AI edit backup for {}", rel_path);
            let ckpt = engine
                .create_checkpoint_with_root(
                    &desc,
                    &ws_id,
                    &root,
                    Some(std::slice::from_ref(&full_path)),
                )
                .map_err(|e| {
                    format!(
                        "Mandatory pre-edit checkpoint failed for '{}': {}. Write aborted to prevent unrecoverable data loss.",
                        rel_path, e
                    )
                })?;
            backup_ckpt_id = Some(ckpt.checkpoint_id);
        } else {
            return Err(format!(
                "Mandatory pre-edit checkpoint failed for '{}': Checkpoint engine not available. Write aborted.",
                rel_path
            ));
        }
    }

    // 3. Compute final content based on operation
    let final_content = match proposal.operation.as_str() {
        "delete_file" => {
            cap.delete_file_conditional(&rel_path, &proposal.base_hash)
                .map_err(|e| format!("Conditional deletion failed for '{}': {}", rel_path, e))?;
            return Ok(ApplyProposalResult {
                success: true,
                relative_path: rel_path.clone(),
                new_hash: String::new(),
                message: format!("File '{}' safely deleted", rel_path),
                backup_checkpoint_id: backup_ckpt_id,
            });
        }
        "apply_patch" => {
            if !target_exists {
                return Err(format!(
                    "Cannot apply patch to non-existent file '{}'. Use create_file instead.",
                    rel_path
                ));
            }
            let original_bytes = std::fs::read(&full_path)
                .map_err(|e| format!("Failed to read target file '{:?}': {}", full_path, e))?;
            let original_str = String::from_utf8(original_bytes)
                .map_err(|e| format!("Target file '{}' is not valid UTF-8: {}", rel_path, e))?;
            let patch_text = if !proposal.unified_diff.is_empty() {
                &proposal.unified_diff
            } else {
                &proposal.proposed_content
            };
            apply_unified_diff(&original_str, patch_text)?
        }
        "insert_at" => {
            let original_bytes = std::fs::read(&full_path).map_err(|e| {
                format!(
                    "Failed to read target file for insertion '{:?}': {}",
                    full_path, e
                )
            })?;
            let original_str = String::from_utf8_lossy(&original_bytes);
            let lines: Vec<&str> = original_str.lines().collect();
            let insert_idx = proposal.insertion_line.unwrap_or(1).saturating_sub(1) as usize;
            let mut new_lines = Vec::new();
            let mut inserted = false;
            for (idx, line) in lines.iter().enumerate() {
                if idx == insert_idx {
                    new_lines.push(proposal.proposed_content.as_str());
                    inserted = true;
                }
                new_lines.push(*line);
            }
            if !inserted {
                new_lines.push(proposal.proposed_content.as_str());
            }
            new_lines.join("\n")
        }
        "replace_file" | "create_file" | "modify" => {
            // Safety guard: prevent accidental wipe of substantial existing file by a trivial snippet
            if target_exists && proposal.proposed_content.trim().is_empty() {
                return Err(format!(
                    "Safety Violation: Refusing to replace non-empty file '{}' with empty content. Use delete_file to intentionally remove a file.",
                    rel_path
                ));
            }
            proposal.proposed_content.clone()
        }
        _ => proposal.proposed_content.clone(),
    };

    // 4. Atomically & conditionally write proposed content via capability
    // Eliminates TOCTOU race: replace_file_bytes_conditional verifies identity & hash at commit point.
    let expected_hash_opt = if target_exists {
        Some(proposal.base_hash.as_str())
    } else {
        None
    };

    cap.replace_file_bytes_conditional(&rel_path, final_content.as_bytes(), expected_hash_opt, None)
        .map_err(|e| {
            format!(
                "Conditional replacement failed for '{}': {}. File identity or hash mismatch at point of write.",
                rel_path, e
            )
        })?;

    // 5. Verify resulting written file
    let final_bytes = std::fs::read(&full_path)
        .map_err(|e| format!("Failed to verify written file '{:?}': {}", full_path, e))?;
    let new_hash = compute_sha256(&final_bytes);

    info!(
        "Successfully applied AI proposal to '{}' (new hash: {}, backup: {:?})",
        rel_path, new_hash, backup_ckpt_id
    );

    Ok(ApplyProposalResult {
        success: true,
        relative_path: rel_path.clone(),
        new_hash,
        message: format!("Changes safely applied to {}", rel_path),
        backup_checkpoint_id: backup_ckpt_id,
    })
}

#[tauri::command]
pub async fn ai_apply_proposal(
    app: AppHandle,
    proposal: StructuredProposal,
    _force: Option<bool>,
) -> Result<ApplyProposalResult, String> {
    let ws_arc = app.state::<SharedWorkspace>();
    let ws = ws_arc.lock();
    let ckpt_engine_arc = app.state::<SharedCheckpointEngine>();
    let ckpt_engine = ckpt_engine_arc.lock();
    execute_proposal_internal(&ws, Some(&ckpt_engine), &proposal)
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

    use crate::workspace_bridge::WorkspaceSession;
    use checkpoint_engine::CheckpointEngine;

    fn setup_test_workspace() -> (tempfile::TempDir, WorkspaceSession, CheckpointEngine) {
        let temp = tempfile::tempdir().unwrap();
        let session = WorkspaceSession::new(temp.path().join(".aether-data"));
        session.set_single_root(temp.path().to_path_buf()).unwrap();

        let ckpt_engine = CheckpointEngine::new(temp.path().join(".quarantine"));
        (temp, session, ckpt_engine)
    }

    #[test]
    fn test_ai_safety_empty_base_hash_rejected_for_existing_file() {
        let (temp, session, ckpt) = setup_test_workspace();
        let file_path = temp.path().join("main.rs");
        std::fs::write(&file_path, "initial content").unwrap();

        let proposal = StructuredProposal {
            proposal_id: "prop-empty-hash".into(),
            workspace_id: session.workspace_id(),
            relative_path: "main.rs".into(),
            operation: "replace_file".into(),
            base_hash: "".into(),
            unified_diff: "".into(),
            proposed_content: "replaced content".into(),
            insertion_line: None,
        };

        let res = execute_proposal_internal(&session, Some(&ckpt), &proposal);
        assert!(res.is_err(), "Empty base hash must be rejected");
        assert!(res
            .unwrap_err()
            .contains("Safety Violation: Proposal base_hash cannot be empty"));
        assert_eq!(
            std::fs::read_to_string(&file_path).unwrap(),
            "initial content"
        );
    }

    #[test]
    fn test_ai_safety_stale_hash_rejected() {
        let (temp, session, ckpt) = setup_test_workspace();
        let file_path = temp.path().join("main.rs");
        std::fs::write(&file_path, "v1 content").unwrap();

        let proposal = StructuredProposal {
            proposal_id: "prop-stale".into(),
            workspace_id: session.workspace_id(),
            relative_path: "main.rs".into(),
            operation: "replace_file".into(),
            base_hash: "0000000000000000000000000000000000000000000000000000000000000000".into(),
            unified_diff: "".into(),
            proposed_content: "v2-replaced".into(),
            insertion_line: None,
        };

        let res = execute_proposal_internal(&session, Some(&ckpt), &proposal);
        assert!(res.is_err(), "Stale hash must trigger conflict error");
        assert!(res.unwrap_err().contains("Conflict detected"));
        assert_eq!(std::fs::read_to_string(&file_path).unwrap(), "v1 content");
    }

    #[test]
    fn test_ai_safety_checkpoint_failure_prevents_writes() {
        let (temp, session, _ckpt) = setup_test_workspace();
        let file_path = temp.path().join("main.rs");
        std::fs::write(&file_path, "original before ckpt").unwrap();
        let hash = compute_sha256(b"original before ckpt");

        let proposal = StructuredProposal {
            proposal_id: "prop-no-ckpt".into(),
            workspace_id: session.workspace_id(),
            relative_path: "main.rs".into(),
            operation: "replace_file".into(),
            base_hash: hash,
            unified_diff: "".into(),
            proposed_content: "should not be written".into(),
            insertion_line: None,
        };

        // Pass None for ckpt_engine to simulate checkpoint backend failure/unavailability
        let res = execute_proposal_internal(&session, None, &proposal);
        assert!(res.is_err(), "Checkpoint failure must abort application");
        assert!(res
            .unwrap_err()
            .contains("Mandatory pre-edit checkpoint failed"));
        assert_eq!(
            std::fs::read_to_string(&file_path).unwrap(),
            "original before ckpt"
        );
    }

    #[test]
    fn test_ai_safety_malformed_patch_rejected() {
        let (temp, session, ckpt) = setup_test_workspace();
        let file_path = temp.path().join("lib.rs");
        std::fs::write(&file_path, "fn hello() {}\n").unwrap();
        let hash = compute_sha256(b"fn hello() {}\n");

        let proposal = StructuredProposal {
            proposal_id: "prop-malformed".into(),
            workspace_id: session.workspace_id(),
            relative_path: "lib.rs".into(),
            operation: "apply_patch".into(),
            base_hash: hash,
            unified_diff: "".into(),
            proposed_content: "this is completely invalid patch without hunk headers\n+foo\n"
                .into(),
            insertion_line: None,
        };

        let res = execute_proposal_internal(&session, Some(&ckpt), &proposal);
        assert!(res.is_err(), "Malformed patch must be rejected");
        assert!(res.unwrap_err().contains("Malformed patch"));
        assert_eq!(
            std::fs::read_to_string(&file_path).unwrap(),
            "fn hello() {}\n"
        );
    }

    #[test]
    fn test_ai_safety_patch_context_mismatch_rejected() {
        let (temp, session, ckpt) = setup_test_workspace();
        let file_path = temp.path().join("lib.rs");
        std::fs::write(&file_path, "fn alpha() {}\nfn beta() {}\n").unwrap();
        let hash = compute_sha256(b"fn alpha() {}\nfn beta() {}\n");

        // Patch expects fn gamma() instead of fn beta()
        let patch = "@@ -1,2 +1,2 @@\n fn alpha() {}\n-fn gamma() {}\n+fn delta() {}\n";

        let proposal = StructuredProposal {
            proposal_id: "prop-mismatch".into(),
            workspace_id: session.workspace_id(),
            relative_path: "lib.rs".into(),
            operation: "apply_patch".into(),
            base_hash: hash,
            unified_diff: "".into(),
            proposed_content: patch.into(),
            insertion_line: None,
        };

        let res = execute_proposal_internal(&session, Some(&ckpt), &proposal);
        assert!(res.is_err(), "Patch context mismatch must be rejected");
        let err = res.unwrap_err();
        assert!(
            err.contains("context mismatch") || err.contains("delete mismatch"),
            "Error was: {}",
            err
        );
        assert_eq!(
            std::fs::read_to_string(&file_path).unwrap(),
            "fn alpha() {}\nfn beta() {}\n"
        );
    }

    #[test]
    fn test_ai_safety_valid_patch_changes_only_intended_lines() {
        let (temp, session, ckpt) = setup_test_workspace();
        let file_path = temp.path().join("lib.rs");
        let orig = "line 1\nline 2\nline 3\nline 4\nline 5\n";
        std::fs::write(&file_path, orig).unwrap();
        let hash = compute_sha256(orig.as_bytes());

        let patch = "@@ -2,3 +2,3 @@\n line 2\n-line 3\n+line 3 MODIFIED\n line 4\n";

        let proposal = StructuredProposal {
            proposal_id: "prop-valid-patch".into(),
            workspace_id: session.workspace_id(),
            relative_path: "lib.rs".into(),
            operation: "apply_patch".into(),
            base_hash: hash,
            unified_diff: "".into(),
            proposed_content: patch.into(),
            insertion_line: None,
        };

        let res = execute_proposal_internal(&session, Some(&ckpt), &proposal);
        assert!(
            res.is_ok(),
            "Valid patch application must succeed: {:?}",
            res.err()
        );
        let expected = "line 1\nline 2\nline 3 MODIFIED\nline 4\nline 5\n";
        assert_eq!(std::fs::read_to_string(&file_path).unwrap(), expected);
    }

    #[test]
    fn test_ai_safety_external_modification_between_preview_and_apply_rejected() {
        let (temp, session, ckpt) = setup_test_workspace();
        let file_path = temp.path().join("concurrent.rs");
        std::fs::write(&file_path, "initial state").unwrap();
        let preview_hash = compute_sha256(b"initial state");

        // Concurrent modification occurs on disk before application
        std::fs::write(&file_path, "external concurrent modification").unwrap();

        let proposal = StructuredProposal {
            proposal_id: "prop-race".into(),
            workspace_id: session.workspace_id(),
            relative_path: "concurrent.rs".into(),
            operation: "replace_file".into(),
            base_hash: preview_hash,
            unified_diff: "".into(),
            proposed_content: "ai overwrite attempt".into(),
            insertion_line: None,
        };

        let res = execute_proposal_internal(&session, Some(&ckpt), &proposal);
        assert!(
            res.is_err(),
            "Concurrent modification before apply must be rejected"
        );
        assert_eq!(
            std::fs::read_to_string(&file_path).unwrap(),
            "external concurrent modification"
        );
    }

    #[test]
    fn test_ai_safety_path_traversal_rejected() {
        let (_temp, session, ckpt) = setup_test_workspace();
        let proposal = StructuredProposal {
            proposal_id: "prop-traversal".into(),
            workspace_id: session.workspace_id(),
            relative_path: "../../outside_target.txt".into(),
            operation: "create_file".into(),
            base_hash: "".into(),
            unified_diff: "".into(),
            proposed_content: "malicious escape".into(),
            insertion_line: None,
        };

        let res = execute_proposal_internal(&session, Some(&ckpt), &proposal);
        assert!(res.is_err(), "Path traversal proposal must be rejected");
    }

    #[test]
    fn test_ai_safety_symlink_escape_rejected() {
        let (temp, session, ckpt) = setup_test_workspace();
        let outside = tempfile::tempdir().unwrap();
        let outside_target = outside.path().join("external_secret.txt");
        std::fs::write(&outside_target, "secret data").unwrap();

        let link_path = temp.path().join("symlink_target.txt");
        #[cfg(windows)]
        {
            let _ = std::os::windows::fs::symlink_file(&outside_target, &link_path);
        }
        #[cfg(not(windows))]
        {
            let _ = std::os::unix::fs::symlink(&outside_target, &link_path);
        }

        if link_path.exists() {
            let hash = compute_sha256(b"secret data");
            let proposal = StructuredProposal {
                proposal_id: "prop-symlink".into(),
                workspace_id: session.workspace_id(),
                relative_path: "symlink_target.txt".into(),
                operation: "replace_file".into(),
                base_hash: hash,
                unified_diff: "".into(),
                proposed_content: "pwned".into(),
                insertion_line: None,
            };
            let res = execute_proposal_internal(&session, Some(&ckpt), &proposal);
            assert!(res.is_err(), "Symlink escape modification must fail closed");
            assert_eq!(
                std::fs::read_to_string(&outside_target).unwrap(),
                "secret data"
            );
        }
    }

    #[test]
    fn test_ai_safety_failed_write_preserves_original_content() {
        let (temp, session, ckpt) = setup_test_workspace();
        let file_path = temp.path().join("important.rs");
        let original = "const IMPORTANT: u32 = 42;\n";
        std::fs::write(&file_path, original).unwrap();
        let hash = compute_sha256(original.as_bytes());

        let proposal = StructuredProposal {
            proposal_id: "prop-wipe".into(),
            workspace_id: session.workspace_id(),
            relative_path: "important.rs".into(),
            operation: "replace_file".into(),
            base_hash: hash,
            unified_diff: "".into(),
            proposed_content: "   ".into(), // Empty content rejected
            insertion_line: None,
        };

        let res = execute_proposal_internal(&session, Some(&ckpt), &proposal);
        assert!(res.is_err(), "Safety guard must reject empty file wipe");
        assert_eq!(std::fs::read_to_string(&file_path).unwrap(), original);
    }

    #[test]
    fn test_diff_old_count_mismatch_rejected() {
        // Hunk claims -1,3 (3 lines consumed) but only supplies 2 lines (1 context + 1 delete)
        let patch = "@@ -1,3 +1,2 @@\n context\n-removed\n";
        let res = parse_unified_diff(patch);
        assert!(res.is_err());
        let err = res.unwrap_err();
        assert!(
            err.contains("claims -1,3 but body consumes 2 lines"),
            "Error was: {}",
            err
        );
    }

    #[test]
    fn test_diff_new_count_mismatch_rejected() {
        // Hunk claims +1,3 (3 lines produced) but only supplies 2 lines (1 context + 1 insert)
        // old_count is 1, which matches the 1 context line consumed
        let patch = "@@ -1,1 +1,3 @@\n context\n+added\n";
        let res = parse_unified_diff(patch);
        assert!(res.is_err());
        let err = res.unwrap_err();
        assert!(
            err.contains("claims +1,3 but body produces 2 lines"),
            "Error was: {}",
            err
        );
    }

    #[test]
    fn test_diff_invalid_positions_rejected() {
        // Invalid old_start=0 when old_count > 0
        let patch1 = "@@ -0,2 +1,2 @@\n-line1\n-line2\n+newline1\n+newline2\n";
        let res1 = parse_unified_diff(patch1);
        assert!(res1.is_err());
        assert!(res1.unwrap_err().contains("old_start cannot be 0"));

        // Invalid new_start=0 when new_count > 0
        let patch2 = "@@ -1,2 +0,2 @@\n-line1\n-line2\n+newline1\n+newline2\n";
        let res2 = parse_unified_diff(patch2);
        assert!(res2.is_err());
        assert!(res2.unwrap_err().contains("new_start cannot be 0"));

        // Overlapping hunks: hunk 2 starts at line 2 before previous hunk ends at line 3
        let patch3 = "@@ -1,3 +1,3 @@\n line1\n line2\n line3\n@@ -2,2 +2,2 @@\n line2\n line3\n";
        let res3 = parse_unified_diff(patch3);
        assert!(res3.is_err());
        assert!(res3
            .unwrap_err()
            .contains("Ambiguous or overlapping hunks detected"));
    }

    #[test]
    fn test_diff_zero_length_hunks_empty_file_insertion() {
        // Inserting into empty file
        let patch = "@@ -0,0 +1,2 @@\n+first line\n+second line\n";
        let res = apply_unified_diff("", patch);
        assert!(
            res.is_ok(),
            "Applying patch to empty file failed: {:?}",
            res.err()
        );
        assert_eq!(res.unwrap(), "first line\nsecond line\n");

        // Inserting lines after line 2 in existing file
        let orig = "line 1\nline 2\nline 3\n";
        let insert_patch = "@@ -2,0 +3,2 @@\n+inserted A\n+inserted B\n";
        let res_ins = apply_unified_diff(orig, insert_patch);
        assert!(
            res_ins.is_ok(),
            "Zero-length insertion failed: {:?}",
            res_ins.err()
        );
        assert_eq!(
            res_ins.unwrap(),
            "line 1\nline 2\ninserted A\ninserted B\nline 3\n"
        );
    }

    #[test]
    fn test_diff_multiple_hunks_applied_in_order() {
        let orig = "line 1\nline 2\nline 3\nline 4\nline 5\nline 6\n";
        let patch = "@@ -2,1 +2,1 @@\n-line 2\n+LINE TWO\n@@ -5,1 +5,1 @@\n-line 5\n+LINE FIVE\n";
        let res = apply_unified_diff(orig, patch);
        assert!(
            res.is_ok(),
            "Multi-hunk application failed: {:?}",
            res.err()
        );
        assert_eq!(
            res.unwrap(),
            "line 1\nLINE TWO\nline 3\nline 4\nLINE FIVE\nline 6\n"
        );
    }

    #[test]
    fn test_diff_no_final_newline_preserved() {
        let orig = "first line\nsecond line\n";
        let patch = "@@ -1,2 +1,2 @@\n first line\n-second line\n+second line modified\n\\ No newline at end of file\n";
        let res = apply_unified_diff(orig, patch);
        assert!(
            res.is_ok(),
            "No final newline application failed: {:?}",
            res.err()
        );
        let output = res.unwrap();
        assert_eq!(output, "first line\nsecond line modified");
        assert!(!output.ends_with('\n'), "Output must not end with newline");
    }

    #[test]
    fn test_diff_malformed_empty_lines_rejected() {
        // Hunk body contains an empty line without prefix (' ', '+', '-')
        let patch = "@@ -1,2 +1,2 @@\n line 1\n\n";
        let res = parse_unified_diff(patch);
        assert!(
            res.is_err(),
            "Empty line without diff prefix must be rejected"
        );
        assert!(res
            .unwrap_err()
            .contains("malformed empty line in hunk body"));
    }

    #[test]
    fn test_diff_preview_and_apply_produce_identical_output() {
        let (temp, session, ckpt) = setup_test_workspace();
        let file_path = temp.path().join("service.rs");
        let initial_content =
            "pub fn execute() {\n    let step = 1;\n    println!(\"{}\", step);\n}\n";
        std::fs::write(&file_path, initial_content).unwrap();
        let base_hash = compute_sha256(initial_content.as_bytes());

        let patch = "@@ -2,2 +2,2 @@\n-    let step = 1;\n-    println!(\"{}\", step);\n+    let step = 2;\n+    info!(\"Step {}\", step);\n";

        let proposal = StructuredProposal {
            proposal_id: "prop-preview-equiv".into(),
            workspace_id: session.workspace_id(),
            relative_path: "service.rs".into(),
            operation: "apply_patch".into(),
            base_hash,
            unified_diff: patch.into(),
            proposed_content: "".into(),
            insertion_line: None,
        };

        // Preview logic
        let preview_applied = apply_unified_diff(initial_content, patch).unwrap();

        // Execution logic
        let res = execute_proposal_internal(&session, Some(&ckpt), &proposal);
        assert!(res.is_ok(), "Execution failed: {:?}", res.err());
        let on_disk_content = std::fs::read_to_string(&file_path).unwrap();

        assert_eq!(
            preview_applied, on_disk_content,
            "Preview output and on-disk execution output must be byte-for-byte identical"
        );
    }

    #[test]
    fn test_ai_safety_simulated_checkpoint_failure_aborts_without_mutation() {
        let (temp, session, ckpt) = setup_test_workspace();
        let file_path = temp.path().join("production.rs");
        let original_bytes = b"pub fn critical_system() -> bool { true }\n";
        std::fs::write(&file_path, original_bytes).unwrap();
        let hash = compute_sha256(original_bytes);

        // Arm simulated early pre-storage failure in checkpoint engine
        ckpt.set_simulate_backup_failure(true);

        let proposal = StructuredProposal {
            proposal_id: "prop-fail-ckpt".into(),
            workspace_id: session.workspace_id(),
            relative_path: "production.rs".into(),
            operation: "replace_file".into(),
            base_hash: hash,
            unified_diff: "".into(),
            proposed_content: "pub fn critical_system() -> bool { false }\n".into(),
            insertion_line: None,
        };

        let res = execute_proposal_internal(&session, Some(&ckpt), &proposal);
        assert!(
            res.is_err(),
            "Simulated checkpoint failure must fail closed"
        );
        let err = res.unwrap_err();
        assert!(
            err.contains("Mandatory pre-edit checkpoint failed"),
            "Error must clearly indicate mandatory checkpoint failure, got: {}",
            err
        );

        // Verify target file remains byte-for-byte unchanged
        assert_eq!(
            std::fs::read(&file_path).unwrap(),
            original_bytes,
            "Target file must remain byte-for-byte identical after checkpoint failure"
        );

        // Verify no temporary or partial staging artifacts remain in directory
        for entry in std::fs::read_dir(temp.path()).unwrap() {
            let entry = entry.unwrap();
            let name = entry.file_name().to_string_lossy().to_string();
            assert!(
                !name.starts_with(".aether-stage-"),
                "No staging artifacts should remain: {}",
                name
            );
        }
    }

    #[test]
    fn test_ai_safety_genuine_checkpoint_blob_storage_write_failure_aborts_without_mutation() {
        let (temp, session, ckpt) = setup_test_workspace();
        let file_path = temp.path().join("core_database.rs");
        let original_bytes = b"pub fn execute_transaction() -> Result<(), ()> { Ok(()) }\n";
        std::fs::write(&file_path, original_bytes).unwrap();
        let hash = compute_sha256(original_bytes);

        // Arm deterministic failure at the genuine blob storage write operation
        // (after snapshot directory creation, workspace file discovery, and hashing have begun)
        ckpt.set_fault_inject_blob_storage_failure(true);

        let proposal = StructuredProposal {
            proposal_id: "prop-genuine-storage-fail".into(),
            workspace_id: session.workspace_id(),
            relative_path: "core_database.rs".into(),
            operation: "replace_file".into(),
            base_hash: hash,
            unified_diff: "".into(),
            proposed_content: "pub fn execute_transaction() -> Result<(), ()> { Err(()) }\n".into(),
            insertion_line: None,
        };

        // Invoke the real production execute_proposal_internal path
        let res = execute_proposal_internal(&session, Some(&ckpt), &proposal);
        assert!(
            res.is_err(),
            "Proposal execution must fail closed on genuine checkpoint blob storage failure"
        );
        let err = res.unwrap_err();
        assert!(
            err.contains("Mandatory pre-edit checkpoint failed"),
            "Error must indicate mandatory pre-edit checkpoint failure: {}",
            err
        );

        // Assert that the target file remains 100% byte-for-byte unchanged
        assert_eq!(
            std::fs::read(&file_path).unwrap(),
            original_bytes,
            "Target file must remain byte-for-byte unchanged"
        );

        // Assert that no replacement is published and zero staging artifacts remain
        for entry in std::fs::read_dir(temp.path()).unwrap() {
            let entry = entry.unwrap();
            let name = entry.file_name().to_string_lossy().to_string();
            assert!(
                !name.starts_with(".aether-stage-"),
                "No staging artifacts should remain: {}",
                name
            );
        }

        // Assert that no incomplete checkpoint is registered or treated as valid
        let stored_checkpoints = ckpt.list_checkpoints().unwrap();
        assert!(
            stored_checkpoints.is_empty(),
            "No incomplete checkpoint must be treated as valid or listed in metadata"
        );

        // Assert that no incomplete snapshot directory is left behind in storage
        let storage_dir = temp.path().join(".aether").join("checkpoints");
        if storage_dir.exists() {
            let subdirs: Vec<_> = std::fs::read_dir(&storage_dir)
                .unwrap()
                .map(|e| e.unwrap().path())
                .collect();
            assert!(
                subdirs.is_empty(),
                "No orphaned snapshot directory must remain in storage: {:?}",
                subdirs
            );
        }
    }

    #[test]
    fn test_ai_safety_genuine_checkpoint_partial_blob_write_failure_aborts_without_mutation() {
        let (temp, session, ckpt) = setup_test_workspace();
        let file_path = temp.path().join("partial_blob_target.rs");
        let original_bytes = b"pub fn critical_computation() -> u64 { 42 }\n";
        std::fs::write(&file_path, original_bytes).unwrap();
        let hash = compute_sha256(original_bytes);

        // Arm failure during blob writing (after partial write / truncated bytes)
        ckpt.set_fault_inject_blob_storage_failure_partial(true);

        let proposal = StructuredProposal {
            proposal_id: "prop-partial-blob-fail".into(),
            workspace_id: session.workspace_id(),
            relative_path: "partial_blob_target.rs".into(),
            operation: "replace_file".into(),
            base_hash: hash,
            unified_diff: "".into(),
            proposed_content: "pub fn critical_computation() -> u64 { 999 }\n".into(),
            insertion_line: None,
        };

        let res = execute_proposal_internal(&session, Some(&ckpt), &proposal);
        assert!(
            res.is_err(),
            "Proposal must fail closed on partial blob write failure"
        );

        // Assert that target file remains untouched
        assert_eq!(
            std::fs::read(&file_path).unwrap(),
            original_bytes,
            "Target file must remain byte-for-byte untouched"
        );

        // Assert no staging artifacts remain
        for entry in std::fs::read_dir(temp.path()).unwrap() {
            let entry = entry.unwrap();
            let name = entry.file_name().to_string_lossy().to_string();
            assert!(
                !name.starts_with(".aether-stage-"),
                "No staging artifacts should remain: {}",
                name
            );
        }

        // Assert no incomplete checkpoint is registered
        assert!(
            ckpt.list_checkpoints().unwrap().is_empty(),
            "No incomplete checkpoint must be listed"
        );

        // Assert no orphaned snapshot directory remains in storage
        let storage_dir = temp.path().join(".aether").join("checkpoints");
        if storage_dir.exists() {
            let subdirs: Vec<_> = std::fs::read_dir(&storage_dir)
                .unwrap()
                .map(|e| e.unwrap().path())
                .collect();
            assert!(
                subdirs.is_empty(),
                "No orphaned snapshot directory must remain in storage: {:?}",
                subdirs
            );
        }
    }

    #[test]
    fn test_ai_safety_genuine_checkpoint_metadata_publication_failure_aborts_without_mutation() {
        let (temp, session, ckpt) = setup_test_workspace();
        let file_path = temp.path().join("metadata_fail_target.rs");
        let original_bytes = b"pub fn durable_transaction() -> bool { true }\n";
        std::fs::write(&file_path, original_bytes).unwrap();
        let hash = compute_sha256(original_bytes);

        // Arm failure at metadata publication
        ckpt.set_fault_inject_metadata_publication_failure(true);

        let proposal = StructuredProposal {
            proposal_id: "prop-meta-pub-fail".into(),
            workspace_id: session.workspace_id(),
            relative_path: "metadata_fail_target.rs".into(),
            operation: "replace_file".into(),
            base_hash: hash,
            unified_diff: "".into(),
            proposed_content: "pub fn durable_transaction() -> bool { false }\n".into(),
            insertion_line: None,
        };

        let res = execute_proposal_internal(&session, Some(&ckpt), &proposal);
        assert!(
            res.is_err(),
            "Proposal must fail closed on checkpoint metadata publication failure"
        );

        // Target file untouched
        assert_eq!(
            std::fs::read(&file_path).unwrap(),
            original_bytes,
            "Target file must remain byte-for-byte untouched"
        );

        // Zero staging artifacts
        for entry in std::fs::read_dir(temp.path()).unwrap() {
            let entry = entry.unwrap();
            let name = entry.file_name().to_string_lossy().to_string();
            assert!(
                !name.starts_with(".aether-stage-"),
                "No staging artifacts should remain: {}",
                name
            );
        }

        // Zero incomplete checkpoints listed
        assert!(
            ckpt.list_checkpoints().unwrap().is_empty(),
            "No incomplete checkpoint must be listed"
        );

        // Zero orphaned snapshot directories left behind
        let storage_dir = temp.path().join(".aether").join("checkpoints");
        if storage_dir.exists() {
            let subdirs: Vec<_> = std::fs::read_dir(&storage_dir)
                .unwrap()
                .map(|e| e.unwrap().path())
                .collect();
            assert!(
                subdirs.is_empty(),
                "No orphaned snapshot directory must remain in storage: {:?}",
                subdirs
            );
        }
    }

    #[test]
    fn test_diff_inconsistent_new_start_rejected() {
        // Hunk claims to replace the first line of a one-line file at new_start 99
        let patch = "@@ -1,1 +99,1 @@\n-old line\n+new line\n";
        let res = parse_unified_diff(patch);
        assert!(
            res.is_err(),
            "Hunk with inconsistent new_start (99) must be rejected"
        );
        let err = res.unwrap_err();
        assert!(
            err.contains("new_start (99) does not match expected output position"),
            "Error must identify inconsistent new_start: {}",
            err
        );

        let apply_res = apply_unified_diff("old line\n", patch);
        assert!(
            apply_res.is_err(),
            "apply_unified_diff must reject inconsistent new_start"
        );
    }

    #[test]
    fn test_diff_valid_insertions_and_deletions_start_middle_end() {
        let base_content = "line 1\nline 2\nline 3\nline 4\n";

        // 1. Beginning insertion
        let patch_ins_start = "@@ -0,0 +1,1 @@\n+line 0\n";
        let res_ins_start = apply_unified_diff(base_content, patch_ins_start).unwrap();
        assert_eq!(
            res_ins_start, "line 0\nline 1\nline 2\nline 3\nline 4\n",
            "Beginning insertion failed"
        );

        // 2. Middle insertion (after line 2)
        let patch_ins_mid = "@@ -2,0 +3,1 @@\n+line 2.5\n";
        let res_ins_mid = apply_unified_diff(base_content, patch_ins_mid).unwrap();
        assert_eq!(
            res_ins_mid, "line 1\nline 2\nline 2.5\nline 3\nline 4\n",
            "Middle insertion failed"
        );

        // 3. End insertion (after line 4)
        let patch_ins_end = "@@ -4,0 +5,1 @@\n+line 5\n";
        let res_ins_end = apply_unified_diff(base_content, patch_ins_end).unwrap();
        assert_eq!(
            res_ins_end, "line 1\nline 2\nline 3\nline 4\nline 5\n",
            "End insertion failed"
        );

        // 4. Beginning deletion (delete line 1)
        let patch_del_start = "@@ -1,1 +0,0 @@\n-line 1\n";
        let res_del_start = apply_unified_diff(base_content, patch_del_start).unwrap();
        assert_eq!(
            res_del_start, "line 2\nline 3\nline 4\n",
            "Beginning deletion failed"
        );

        // 5. Middle deletion (delete line 2)
        let patch_del_mid = "@@ -2,1 +1,0 @@\n-line 2\n";
        let res_del_mid = apply_unified_diff(base_content, patch_del_mid).unwrap();
        assert_eq!(
            res_del_mid, "line 1\nline 3\nline 4\n",
            "Middle deletion failed"
        );

        // 6. End deletion (delete line 4)
        let patch_del_end = "@@ -4,1 +3,0 @@\n-line 4\n";
        let res_del_end = apply_unified_diff(base_content, patch_del_end).unwrap();
        assert_eq!(
            res_del_end, "line 1\nline 2\nline 3\n",
            "End deletion failed"
        );

        // 7. CRLF preservation with beginning, middle, end edits
        let crlf_base = "line 1\r\nline 2\r\nline 3\r\n";
        let crlf_patch = "@@ -2,1 +2,1 @@\n-line 2\n+line 2 edited\n";
        let crlf_res = apply_unified_diff(crlf_base, crlf_patch).unwrap();
        assert_eq!(
            crlf_res, "line 1\r\nline 2 edited\r\nline 3\r\n",
            "CRLF preservation failed"
        );
    }

    #[test]
    fn test_diff_off_by_one_deletion_coordinate_rejected() {
        let base_content = "line 1\nline 2\nline 3\n";

        // Off-by-one deletion header: claims +2,0 when deleting line 2, but correct new-side coordinate is +1,0
        let off_by_one_patch = "@@ -2,1 +2,0 @@\n-line 2\n";

        // Must be rejected by parsing
        let parse_res = parse_unified_diff(off_by_one_patch);
        assert!(
            parse_res.is_err(),
            "parse_unified_diff must reject off-by-one deletion coordinate (+2,0)"
        );
        let parse_err = parse_res.unwrap_err();
        assert!(
            parse_err
                .contains("zero-length new_start (2) does not match expected output position (1)"),
            "Parse error must report coordinate mismatch: {}",
            parse_err
        );

        // Must be rejected by application via apply_unified_diff
        let apply_res = apply_unified_diff(base_content, off_by_one_patch);
        assert!(
            apply_res.is_err(),
            "apply_unified_diff must reject off-by-one deletion coordinate (+2,0)"
        );
        let apply_err = apply_res.unwrap_err();
        assert!(
            apply_err.contains("zero-length new_start (2) does not match"),
            "Apply error must report coordinate mismatch: {}",
            apply_err
        );

        // Also test apply_diff_hunks_internal directly to verify application runtime coordinate rejection
        let off_by_one_hunk = DiffHunk {
            old_start: 2,
            old_count: 1,
            new_start: 2, // Off-by-one! Correct coordinate is 1
            new_count: 0,
            lines: vec![DiffLine::Delete("line 2".to_string())],
        };
        let hunk_apply_res = apply_diff_hunks_internal(base_content, &[off_by_one_hunk], false);
        assert!(
            hunk_apply_res.is_err(),
            "apply_diff_hunks_internal runtime check must reject off-by-one deletion coordinate"
        );
        let hunk_apply_err = hunk_apply_res.unwrap_err();
        assert!(
            hunk_apply_err.contains(
                "zero-length new_start (2) does not match actual output line position (1)"
            ),
            "Runtime hunk apply error must report actual output position mismatch: {}",
            hunk_apply_err
        );

        // Correct deletion header (+1,0) succeeds in both parsing and application
        let correct_patch = "@@ -2,1 +1,0 @@\n-line 2\n";
        let correct_parse = parse_unified_diff(correct_patch);
        assert!(
            correct_parse.is_ok(),
            "Correct deletion header must parse successfully"
        );

        let correct_apply = apply_unified_diff(base_content, correct_patch);
        assert!(
            correct_apply.is_ok(),
            "Correct deletion header must apply successfully"
        );
        assert_eq!(
            correct_apply.unwrap(),
            "line 1\nline 3\n",
            "Correct deletion must produce expected output"
        );
    }
}
