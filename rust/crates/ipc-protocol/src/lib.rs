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
    IndexWorkspace { workspace_path: String },
    SearchSymbols { query: String, limit: Option<usize> },
    FullTextSearch { query: String, limit: Option<usize> },
    RunCommand { command: String, cwd: Option<String>, timeout_secs: Option<u64> },
    KillCommand { process_id: u32 },
    CreateAgentTask { role: String, objective: String, autonomy_mode: String },
    GetAgentTask { task_id: String },
    ApproveAgentStep { task_id: String, step_id: String },
    RejectAgentStep { task_id: String, step_id: String, reason: String },
    QueryProjectBrain { query: String },
    RunDoctorCheck,
    GenerateMobileApp { prompt: String, target_framework: String, app_name: String },
    TriggerMobileBuild { project_path: String, platform: String, build_type: String },
    ModelComplete { prompt: String, model: Option<String>, system: Option<String> },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpcResponse {
    pub success: bool,
    pub data: Option<Value>,
    pub error: Option<String>,
}

impl IpcResponse {
    pub fn ok(data: Value) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
        }
    }

    pub fn err(message: impl Into<String>) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(message.into()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event", content = "data")]
pub enum IpcEvent {
    CommandStdout { process_id: u32, chunk: String },
    CommandStderr { process_id: u32, chunk: String },
    CommandExited { process_id: u32, exit_code: i32 },
    IndexingProgress { files_scanned: usize, total_symbols: usize, status: String },
    AgentStepStarted { task_id: String, step_id: String, description: String },
    AgentStepApprovalRequired { task_id: String, step_id: String, diff: String, tool: String },
    AgentStepCompleted { task_id: String, step_id: String, success: bool, output: String },
    ModelTokenStream { request_id: String, token: String },
    MobileBuildProgress { build_id: String, status: String, log_line: String },
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
