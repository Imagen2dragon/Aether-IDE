use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RequestId {
    Int(i64),
    String(String),
}

impl fmt::Display for RequestId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RequestId::Int(i) => write!(f, "{}", i),
            RequestId::String(s) => write!(f, "{}", s),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcRequest {
    pub jsonrpc: String,
    pub id: RequestId,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
}

impl RpcRequest {
    pub fn new(id: RequestId, method: impl Into<String>, params: Option<Value>) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            method: method.into(),
            params,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcNotification {
    pub jsonrpc: String,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
}

impl RpcNotification {
    pub fn new(method: impl Into<String>, params: Option<Value>) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            method: method.into(),
            params,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcError {
    pub code: i64,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

impl fmt::Display for RpcError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "RPC Error [{}]: {}", self.code, self.message)
    }
}

impl std::error::Error for RpcError {}

pub mod error_codes {
    pub const PARSE_ERROR: i64 = -32700;
    pub const INVALID_REQUEST: i64 = -32600;
    pub const METHOD_NOT_FOUND: i64 = -32601;
    pub const INVALID_PARAMS: i64 = -32602;
    pub const INTERNAL_ERROR: i64 = -32603;
    pub const SERVER_NOT_INITIALIZED: i64 = -32002;
    pub const UNKNOWN_ERROR_CODE: i64 = -32001;
    pub const REQUEST_CANCELLED: i64 = -32800;
    pub const CONTENT_MODIFIED: i64 = -32801;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcResponse {
    pub jsonrpc: String,
    pub id: Option<RequestId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<RpcError>,
}

impl RpcResponse {
    pub fn ok(id: RequestId, result: Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id: Some(id),
            result: Some(result),
            error: None,
        }
    }

    pub fn err(id: Option<RequestId>, code: i64, message: impl Into<String>) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: None,
            error: Some(RpcError {
                code,
                message: message.into(),
                data: None,
            }),
        }
    }
}

#[derive(Debug, Clone)]
pub enum IncomingMessage {
    Response(RpcResponse),
    Notification(RpcNotification),
    Request(RpcRequest),
}

impl IncomingMessage {
    pub fn parse(bytes: &[u8]) -> Result<Self, serde_json::Error> {
        let val: Value = serde_json::from_slice(bytes)?;

        // If it has "id" and ("result" or "error"), it is a response
        if val.get("id").is_some() && (val.get("result").is_some() || val.get("error").is_some()) {
            let resp: RpcResponse = serde_json::from_value(val)?;
            return Ok(IncomingMessage::Response(resp));
        }

        // If it has "id" and "method", it is a request from the server (e.g. client/registerCapability or workspace/configuration)
        if val.get("id").is_some() && val.get("method").is_some() {
            let req: RpcRequest = serde_json::from_value(val)?;
            return Ok(IncomingMessage::Request(req));
        }

        // If it has "method" without "id", it is a notification
        if val.get("method").is_some() {
            let notif: RpcNotification = serde_json::from_value(val)?;
            return Ok(IncomingMessage::Notification(notif));
        }

        // Fallback: try parsing as response (some servers send null result)
        let resp: RpcResponse = serde_json::from_value(val)?;
        Ok(IncomingMessage::Response(resp))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rpc_request_serialization() {
        let req = RpcRequest::new(RequestId::Int(1), "textDocument/completion", None);
        let s = serde_json::to_string(&req).unwrap();
        assert!(s.contains("\"method\":\"textDocument/completion\""));
        assert!(s.contains("\"id\":1"));
    }

    #[test]
    fn test_incoming_response_parse() {
        let json = r#"{"jsonrpc":"2.0","id":1,"result":{"items":[]}}"#;
        let msg = IncomingMessage::parse(json.as_bytes()).unwrap();
        match msg {
            IncomingMessage::Response(r) => {
                assert_eq!(r.id, Some(RequestId::Int(1)));
                assert!(r.result.is_some());
            }
            _ => panic!("Expected Response"),
        }
    }

    #[test]
    fn test_incoming_notification_parse() {
        let json = r#"{"jsonrpc":"2.0","method":"textDocument/publishDiagnostics","params":{"uri":"file:///a.rs","diagnostics":[]}}"#;
        let msg = IncomingMessage::parse(json.as_bytes()).unwrap();
        match msg {
            IncomingMessage::Notification(n) => {
                assert_eq!(n.method, "textDocument/publishDiagnostics");
                assert!(n.params.is_some());
            }
            _ => panic!("Expected Notification"),
        }
    }
}
