use lsp_client::{LanguageServerDefinition, LspClient, LspClientConfig, LspMultiplexer};
use std::time::Duration;

const MOCK_SERVER_SCRIPT: &str = r#"
import sys, json

def read_message():
    content_len = None
    while True:
        line = sys.stdin.buffer.readline()
        if not line:
            return None
        line_str = line.decode('utf-8', errors='replace').strip()
        if not line_str:
            break
        if line_str.lower().startswith('content-length:'):
            content_len = int(line_str.split(':', 1)[1].strip())
    if content_len is None:
        return None
    body = sys.stdin.buffer.read(content_len)
    return json.loads(body.decode('utf-8'))

def send_message(payload):
    body = json.dumps(payload).encode('utf-8')
    header = f"Content-Length: {len(body)}\r\n\r\n".encode('ascii')
    sys.stdout.buffer.write(header + body)
    sys.stdout.buffer.flush()

while True:
    msg = read_message()
    if not msg:
        break
    msg_id = msg.get('id')
    method = msg.get('method')
    
    if method == 'initialize':
        send_message({
            "jsonrpc": "2.0",
            "id": msg_id,
            "result": {
                "capabilities": {
                    "textDocumentSync": 1,
                    "completionProvider": {},
                    "hoverProvider": True,
                    "definitionProvider": True
                }
            }
        })
    elif method == 'initialized':
        pass
    elif method == 'textDocument/didOpen':
        uri = msg['params']['textDocument']['uri']
        send_message({
            "jsonrpc": "2.0",
            "method": "textDocument/publishDiagnostics",
            "params": {
                "uri": uri,
                "diagnostics": [{
                    "range": {
                        "start": {"line": 0, "character": 0},
                        "end": {"line": 0, "character": 4}
                    },
                    "severity": 1,
                    "message": "Mock test error"
                }]
            }
        })
    elif method == 'textDocument/completion':
        send_message({
            "jsonrpc": "2.0",
            "id": msg_id,
            "result": {
                "isIncomplete": False,
                "items": [
                    {
                        "label": "compute_hash",
                        "kind": 3,
                        "detail": "fn compute_hash() -> u64"
                    }
                ]
            }
        })
    elif method == 'textDocument/hover':
        send_message({
            "jsonrpc": "2.0",
            "id": msg_id,
            "result": {
                "contents": "Mock hover signature: compute_hash()"
            }
        })
    elif method == 'textDocument/definition':
        uri = msg['params']['textDocument']['uri']
        send_message({
            "jsonrpc": "2.0",
            "id": msg_id,
            "result": [
                {
                    "uri": uri,
                    "range": {
                        "start": {"line": 42, "character": 0},
                        "end": {"line": 42, "character": 12}
                    }
                }
            ]
        })
    elif method == 'shutdown':
        send_message({"jsonrpc": "2.0", "id": msg_id, "result": None})
    elif method == 'exit':
        break
"#;

#[tokio::test]
async fn test_lsp_client_mock_lifecycle_and_features() {
    let temp_dir = std::env::temp_dir().join("aether_lsp_test");
    let _ = std::fs::create_dir_all(&temp_dir);
    let script_path = temp_dir.join("mock_lsp_server.py");
    std::fs::write(&script_path, MOCK_SERVER_SCRIPT).expect("write mock server script");

    let config = LspClientConfig {
        server_id: "mock-test".to_string(),
        command: "python".to_string(),
        args: vec![script_path.to_string_lossy().to_string()],
        workspace_root: temp_dir.clone(),
        initialization_options: None,
    };

    let client = LspClient::spawn(config).await.expect("spawn client");
    let mut diag_rx = client.subscribe_diagnostics();

    // 1. Initialize
    let init_res = client
        .initialize(Duration::from_secs(20))
        .await
        .expect("initialize");
    assert!(init_res.get("capabilities").is_some());

    // 2. Open document and receive diagnostics notification
    let test_file = temp_dir.join("main.rs");
    let doc =
        lsp_client::DocumentState::new(lsp_client::path_to_uri(&test_file), "rust", "fn main() {}");
    client.did_open(&doc).await.expect("did_open");

    let diag = tokio::time::timeout(Duration::from_secs(15), diag_rx.recv())
        .await
        .expect("diag timeout")
        .expect("diag recv");
    assert_eq!(diag.diagnostics.len(), 1);
    assert_eq!(diag.diagnostics[0].message, "Mock test error");

    // 3. Completion
    let items = client
        .completion(&test_file, 0, 5, Duration::from_secs(15))
        .await
        .expect("completion");
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].label, "compute_hash");

    // 4. Hover
    let hover = client
        .hover(&test_file, 0, 5, Duration::from_secs(15))
        .await
        .expect("hover");
    assert!(hover.is_some());
    assert!(hover.unwrap().contents.contains("compute_hash()"));

    // 5. Definition
    let defs = client
        .definition(&test_file, 0, 5, Duration::from_secs(15))
        .await
        .expect("definition");
    assert_eq!(defs.len(), 1);
    assert_eq!(defs[0].range.start.line, 42);

    // 6. Shutdown
    client
        .shutdown(Duration::from_secs(15))
        .await
        .expect("shutdown");
    assert_eq!(client.state(), lsp_client::ClientState::Terminated);
}

#[tokio::test]
async fn test_lsp_multiplexer_routing() {
    let temp_dir = std::env::temp_dir().join("aether_lsp_mux_test");
    let _ = std::fs::create_dir_all(&temp_dir);
    let script_path = temp_dir.join("mock_lsp_server.py");
    std::fs::write(&script_path, MOCK_SERVER_SCRIPT).expect("write mock server script");

    let mux = LspMultiplexer::new(temp_dir.clone());
    // Register mock server for .rs
    mux.register_server(LanguageServerDefinition {
        language_id: "rust".to_string(),
        file_extensions: vec!["rs".to_string()],
        command: "python".to_string(),
        args: vec![script_path.to_string_lossy().to_string()],
        initialization_options: None,
    });

    let rs_file = temp_dir.join("lib.rs");
    mux.open_document(&rs_file, "pub fn calculate() {}", None)
        .await
        .expect("open rs document");

    // Request completions via multiplexer
    let completions = mux
        .completion(&rs_file, 0, 10)
        .await
        .expect("mux completion");
    assert_eq!(completions.len(), 1);
    assert_eq!(completions[0].label, "compute_hash");

    // Check shutdown_all
    mux.shutdown_all().await.expect("mux shutdown");
}
