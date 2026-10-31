use ipc_protocol::{
    decode_message, encode_message, IpcEnvelope, IpcPayload, IpcRequest, IpcResponse,
};
use parking_lot::Mutex;
use security_core::{AuthVerificationResult, TokenStore};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

#[tokio::test]
async fn test_tcp_loopback_connection_churn_and_owner_availability() {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("Failed to bind ephemeral TCP loopback");
    let addr = listener.local_addr().expect("Failed to get local addr");

    let token_store = Arc::new(Mutex::new(TokenStore::new_with_owner(
        Duration::from_secs(3600),
        "desktop-owner-id",
    )));
    let owner_token = token_store.lock().current_token().to_string();

    let store_clone = token_store.clone();
    let conn_counter = Arc::new(AtomicUsize::new(0));
    let conn_clone = conn_counter.clone();

    // Spawn mock daemon IPC server handling authentication with TokenStore
    let server_task = tokio::spawn(async move {
        loop {
            let (mut socket, peer) = match listener.accept().await {
                Ok(res) => res,
                Err(_) => break,
            };
            let conn_id = conn_clone.fetch_add(1, Ordering::Relaxed);
            let peer_source = format!("tcp-{}-conn-{}", peer.ip(), conn_id);
            let store = store_clone.clone();

            tokio::spawn(async move {
                let mut buf = vec![0u8; 4096];
                if let Ok(n) = socket.read(&mut buf).await {
                    if n > 0 {
                        if let Ok(envelope) = decode_message(&buf[..n]) {
                            let (auth_res, _session) = store.lock().authenticate_session(
                                envelope.session_token.as_deref(),
                                &peer_source,
                            );

                            let resp_payload = match auth_res {
                                AuthVerificationResult::Success => {
                                    IpcResponse::ok(serde_json::json!({ "pong": true }))
                                }
                                AuthVerificationResult::RateLimited { wait_seconds } => {
                                    IpcResponse {
                                        success: false,
                                        data: None,
                                        error: Some(format!(
                                            "Rate limited: retry after {} seconds",
                                            wait_seconds
                                        )),
                                        status_code: Some(429),
                                    }
                                }
                                _ => IpcResponse::unauthorized("Unauthorized bearer token"),
                            };

                            let resp_envelope = IpcEnvelope::new(
                                envelope.id,
                                None,
                                IpcPayload::Response(resp_payload),
                            );
                            if let Ok(bytes) = encode_message(&resp_envelope) {
                                let _ = socket.write_all(&bytes).await;
                            }
                        }
                    }
                }
            });
        }
    });

    // 1. Attacker generates connection churn flood (20 rapid fresh connections with invalid tokens)
    let mut attacker_401_count = 0;
    let mut attacker_429_count = 0;

    for i in 0..20 {
        let mut stream = TcpStream::connect(addr)
            .await
            .expect("Attacker connection must succeed at TCP level");
        let req = IpcEnvelope::new(
            format!("attacker-req-{}", i),
            Some("invalid-attacker-token".into()),
            IpcPayload::Request(IpcRequest::Ping),
        );
        let bytes = encode_message(&req).unwrap();
        stream.write_all(&bytes).await.unwrap();

        let mut resp_buf = vec![0u8; 4096];
        let n = stream.read(&mut resp_buf).await.unwrap();
        let resp_env = decode_message(&resp_buf[..n]).unwrap();

        match resp_env.payload {
            IpcPayload::Response(resp) => {
                let code = resp.status_code.unwrap_or(0);
                if code == 401 {
                    attacker_401_count += 1;
                } else if code == 429 {
                    attacker_429_count += 1;
                }
            }
            other => panic!("Unexpected response for attacker: {:?}", other),
        }
    }

    println!(
        " [TCP Churn Metrics] Attacker 401 Unauthorized: {}, Attacker 429 RateLimited: {}",
        attacker_401_count, attacker_429_count
    );
    assert!(
        attacker_429_count > 0,
        "Global churn protection must engage and return 429 RateLimited under flood!"
    );

    // 2. Legitimate Owner connects concurrently and under active churn lockout
    for i in 0..5 {
        let mut owner_stream = TcpStream::connect(addr)
            .await
            .expect("Owner connection must succeed");
        let req = IpcEnvelope::new(
            format!("owner-req-{}", i),
            Some(owner_token.clone()),
            IpcPayload::Request(IpcRequest::Ping),
        );
        let bytes = encode_message(&req).unwrap();
        owner_stream.write_all(&bytes).await.unwrap();

        let mut resp_buf = vec![0u8; 4096];
        let n = owner_stream.read(&mut resp_buf).await.unwrap();
        let resp_env = decode_message(&resp_buf[..n]).unwrap();

        match resp_env.payload {
            IpcPayload::Response(resp) => {
                assert!(
                    resp.success,
                    "Legitimate Owner must NEVER be locked out by attacker flood! Error: {:?}",
                    resp.error
                );
            }
            other => panic!(
                "Legitimate Owner must NEVER be locked out by attacker flood! Got: {:?}",
                other
            ),
        }
    }

    server_task.abort();
}

#[cfg(windows)]
#[tokio::test]
async fn test_windows_named_pipe_transport_integration() {
    use tokio::net::windows::named_pipe::{ClientOptions, ServerOptions};

    let pipe_name = format!(
        r"\\.\pipe\aether-test-named-pipe-{}",
        uuid::Uuid::new_v4().simple()
    );
    let token_store = Arc::new(Mutex::new(TokenStore::new_with_owner(
        Duration::from_secs(3600),
        "desktop-owner-pipe",
    )));
    let owner_token = token_store.lock().current_token().to_string();

    let server = ServerOptions::new()
        .first_pipe_instance(true)
        .reject_remote_clients(true)
        .create(&pipe_name)
        .expect("Failed to create test named pipe server");

    let pipe_name_clone = pipe_name.clone();
    let store_clone = token_store.clone();

    // Spawn server accept task
    let server_task = tokio::spawn(async move {
        if server.connect().await.is_ok() {
            let mut server = server;
            let mut buf = vec![0u8; 4096];
            if let Ok(n) = server.read(&mut buf).await {
                if let Ok(envelope) = decode_message(&buf[..n]) {
                    let (auth_res, _session) = store_clone.lock().authenticate_session(
                        envelope.session_token.as_deref(),
                        "named-pipe-client-1",
                    );

                    let resp_payload = match auth_res {
                        AuthVerificationResult::Success => {
                            IpcResponse::ok(serde_json::json!({ "pong": true }))
                        }
                        _ => IpcResponse::unauthorized("Unauthorized token"),
                    };
                    let resp =
                        IpcEnvelope::new(envelope.id, None, IpcPayload::Response(resp_payload));
                    if let Ok(bytes) = encode_message(&resp) {
                        let _ = server.write_all(&bytes).await;
                    }
                }
            }
        }
    });

    // Client connects to Windows Named Pipe
    let mut client = ClientOptions::new()
        .open(&pipe_name_clone)
        .expect("Failed to open named pipe client");

    let req = IpcEnvelope::new(
        "pipe-msg-01",
        Some(owner_token),
        IpcPayload::Request(IpcRequest::Ping),
    );
    let bytes = encode_message(&req).unwrap();
    client.write_all(&bytes).await.unwrap();

    let mut resp_buf = vec![0u8; 4096];
    let n = client.read(&mut resp_buf).await.unwrap();
    let resp = decode_message(&resp_buf[..n]).unwrap();

    match resp.payload {
        IpcPayload::Response(resp) => {
            assert!(resp.success);
            println!(" [Named Pipe Metrics] Successfully verified Windows Named Pipe IPC roundtrip with Owner token");
        }
        other => panic!("Unexpected named pipe response: {:?}", other),
    }

    server_task.abort();
}
