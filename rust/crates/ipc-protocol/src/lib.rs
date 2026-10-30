pub mod ffi;

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const DEFAULT_NAMED_PIPE_PATH: &str = r"\\.\pipe\aether-daemon-ipc";
pub const DEFAULT_WEBSOCKET_PORT: u16 = 44888;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpcEnvelope {
    pub id: String,
    pub session_token: Option<String>,
    #[serde(flatten)]
    pub payload: IpcPayload,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum IpcPayload {
    Request(IpcRequest),
    Response(IpcResponse),
    Event(IpcEvent),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action", content = "params")]
pub enum IpcRequest {
    Ping,
    GetStatus,
    IndexWorkspace {
        workspace_path: String,
    },
    SearchSymbols {
        query: String,
        limit: Option<usize>,
    },
    FullTextSearch {
        query: String,
        limit: Option<usize>,
    },
    RunCommand {
        command: String,
        cwd: Option<String>,
        timeout_secs: Option<u64>,
    },
    KillCommand {
        process_id: u32,
    },
    CreateAgentTask {
        role: String,
        objective: String,
        autonomy_mode: String,
    },
    GetAgentTask {
        task_id: String,
    },
    ApproveAgentStep {
        task_id: String,
        step_id: String,
    },
    RejectAgentStep {
        task_id: String,
        step_id: String,
        reason: String,
    },
    QueryProjectBrain {
        query: String,
    },
    RunDoctorCheck,
    GenerateMobileApp {
        prompt: String,
        target_framework: String,
        app_name: String,
    },
    TriggerMobileBuild {
        project_path: String,
        platform: String,
        build_type: String,
    },
    ModelComplete {
        prompt: String,
        model: Option<String>,
        system: Option<String>,
    },
    LspOpenDocument {
        path: String,
        content: String,
        language_id: Option<String>,
    },
    LspChangeDocument {
        path: String,
        content: String,
        version: i64,
    },
    LspCloseDocument {
        path: String,
    },
    LspCompletion {
        path: String,
        line: u32,
        character: u32,
    },
    LspHover {
        path: String,
        line: u32,
        character: u32,
    },
    LspDefinition {
        path: String,
        line: u32,
        character: u32,
    },
    LspDiagnostics {
        path: String,
    },
    TerminalSpawn {
        shell: Option<String>,
        cwd: Option<String>,
        cols: Option<u16>,
        rows: Option<u16>,
    },
    TerminalWrite {
        session_id: u32,
        data: String,
    },
    TerminalResize {
        session_id: u32,
        cols: u16,
        rows: u16,
    },
    TerminalClose {
        session_id: u32,
    },
    SyncGeneratePairing {
        device_name: Option<String>,
        host: Option<String>,
        port: Option<u16>,
    },
    SyncGetManifest {
        workspace_path: String,
        workspace_name: Option<String>,
    },
    SyncDiffManifests {
        local_manifest: serde_json::Value,
        remote_manifest: serde_json::Value,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpcResponse {
    pub success: bool,
    pub data: Option<Value>,
    pub error: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status_code: Option<u16>,
}

impl IpcResponse {
    pub fn ok(data: Value) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
            status_code: Some(200),
        }
    }

    pub fn err(message: impl Into<String>) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(message.into()),
            status_code: Some(500),
        }
    }

    pub fn unauthorized(message: impl Into<String>) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(format!("401 Unauthorized: {}", message.into())),
            status_code: Some(401),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event", content = "data")]
pub enum IpcEvent {
    CommandStdout {
        process_id: u32,
        chunk: String,
    },
    CommandStderr {
        process_id: u32,
        chunk: String,
    },
    CommandExited {
        process_id: u32,
        exit_code: i32,
    },
    IndexingProgress {
        files_scanned: usize,
        total_symbols: usize,
        status: String,
    },
    AgentStepStarted {
        task_id: String,
        step_id: String,
        description: String,
    },
    AgentStepApprovalRequired {
        task_id: String,
        step_id: String,
        diff: String,
        tool: String,
    },
    AgentStepCompleted {
        task_id: String,
        step_id: String,
        success: bool,
        output: String,
    },
    ModelTokenStream {
        request_id: String,
        token: String,
    },
    MobileBuildProgress {
        build_id: String,
        status: String,
        log_line: String,
    },
    LspPublishDiagnostics {
        path: String,
        diagnostics: Vec<serde_json::Value>,
    },
    TerminalOutput {
        session_id: u32,
        data: String,
    },
    SyncProgress {
        session_id: String,
        files_transferred: usize,
        total_files: usize,
        bytes_transferred: u64,
        status: String,
    },
}

pub fn encode_message(envelope: &IpcEnvelope) -> Result<Vec<u8>, serde_json::Error> {
    let mut bytes = serde_json::to_vec(envelope)?;
    bytes.push(b'\n'); // newline delimited JSON framing
    Ok(bytes)
}

pub fn decode_message(bytes: &[u8]) -> Result<IpcEnvelope, serde_json::Error> {
    let slice = if bytes.ends_with(b"\n") {
        &bytes[..bytes.len() - 1]
    } else {
        bytes
    };
    serde_json::from_slice(slice)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ipc_envelope_roundtrip() {
        let envelope = IpcEnvelope {
            id: "msg-123".to_string(),
            session_token: Some("token-abc".to_string()),
            payload: IpcPayload::Request(IpcRequest::Ping),
        };

        let encoded = encode_message(&envelope).expect("failed to encode");
        let decoded = decode_message(&encoded).expect("failed to decode");

        assert_eq!(decoded.id, "msg-123");
        match decoded.payload {
            IpcPayload::Request(IpcRequest::Ping) => {}
            _ => panic!("Expected Ping request"),
        }
    }
}
