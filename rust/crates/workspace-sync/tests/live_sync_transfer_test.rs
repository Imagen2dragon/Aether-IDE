use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use workspace_sync::{
    diff_manifests, reassemble_chunks, slice_file, AeadChannel, FileChunk, PairingPayload,
    SyncRole, WorkspaceManifest,
};

#[tokio::test]
async fn test_two_endpoint_encrypted_workspace_file_transfer_bidirectional() {
    // 1. Establish Ephemeral Pairing Handshake
    let (pairing, psk) = PairingPayload::new("127.0.0.1", 45678, 300).unwrap();
    let qr_uri = pairing.to_pairing_uri();
    assert!(qr_uri.starts_with("aether://pair?"));

    let parsed_pairing = PairingPayload::parse_pairing_uri(&qr_uri).unwrap();
    assert_eq!(parsed_pairing.session_id, pairing.session_id);

    // 2. Start TCP Listener for Peer B (Companion Endpoint)
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let peer_addr = listener.local_addr().unwrap();

    // Setup temporary test directories for Endpoint A and Endpoint B
    let temp_base = std::env::temp_dir().join(format!("aether_sync_test_{}", uuid::Uuid::new_v4()));
    let dir_a = temp_base.join("endpoint_a");
    let dir_b = temp_base.join("endpoint_b");
    std::fs::create_dir_all(&dir_a).unwrap();
    std::fs::create_dir_all(&dir_b).unwrap();

    // Create a real source code file on Endpoint A
    let file_a_path = dir_a.join("main.rs");
    let test_source_content = b"// AetherIDE High-Assurance Systems Code\npub fn calculate_merkle_root() -> [u8; 32] { [42u8; 32] }\n".repeat(200); // ~20 KB
    std::fs::write(&file_a_path, &test_source_content).unwrap();

    // Spawn Peer B (Receiver: Companion)
    let psk_b = psk;
    let dir_b_clone = dir_b.clone();
    let server_task = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut companion_channel = AeadChannel::new(psk_b, SyncRole::Companion);

        // Read framed encrypted payload from Desktop
        let mut frame_len_buf = [0u8; 4];
        socket.read_exact(&mut frame_len_buf).await.unwrap();
        let frame_len = u32::from_be_bytes(frame_len_buf) as usize;

        let mut encrypted_frame = vec![0u8; frame_len];
        socket.read_exact(&mut encrypted_frame).await.unwrap();

        // Decrypt with Companion AEAD channel
        let decrypted_payload = companion_channel.decrypt_frame(&encrypted_frame).unwrap();

        // Parse chunks from decrypted payload
        let chunks: Vec<FileChunk> = serde_json::from_slice(&decrypted_payload).unwrap();
        assert!(!chunks.is_empty(), "Chunks received should not be empty");

        // Reassemble chunks
        let reassembled = reassemble_chunks(chunks).unwrap();
        let dest_file = dir_b_clone.join("main.rs");
        std::fs::write(&dest_file, &reassembled).unwrap();

        // Now respond in Reverse Direction: send a file from Companion to Desktop
        let companion_reply_content = b"// Received and verified by Companion Device\npub const DEVICE_CONFIRMED: bool = true;\n";
        let reply_chunks = slice_file("companion_ack.rs", companion_reply_content, 64 * 1024);
        let reply_json = serde_json::to_vec(&reply_chunks).unwrap();

        let reply_frame = companion_channel.encrypt_frame(&reply_json).unwrap();
        socket
            .write_all(&(reply_frame.len() as u32).to_be_bytes())
            .await
            .unwrap();
        socket.write_all(&reply_frame).await.unwrap();
        socket.flush().await.unwrap();
    });

    // Spawn Peer A (Sender: Desktop)
    let psk_a = psk;
    let mut desktop_channel = AeadChannel::new(psk_a, SyncRole::Desktop);
    let mut client_socket = TcpStream::connect(peer_addr).await.unwrap();

    // Slice file on Endpoint A
    let chunks_a = slice_file("main.rs", &test_source_content, 64 * 1024);
    let payload_bytes = serde_json::to_vec(&chunks_a).unwrap();

    // Encrypt frame with Desktop AEAD channel
    let frame_a = desktop_channel.encrypt_frame(&payload_bytes).unwrap();
    client_socket
        .write_all(&(frame_a.len() as u32).to_be_bytes())
        .await
        .unwrap();
    client_socket.write_all(&frame_a).await.unwrap();
    client_socket.flush().await.unwrap();

    // Read reverse-direction response from Companion
    let mut reply_len_buf = [0u8; 4];
    client_socket.read_exact(&mut reply_len_buf).await.unwrap();
    let reply_len = u32::from_be_bytes(reply_len_buf) as usize;

    let mut reply_frame = vec![0u8; reply_len];
    client_socket.read_exact(&mut reply_frame).await.unwrap();

    // Decrypt reverse message
    let decrypted_reply = desktop_channel.decrypt_frame(&reply_frame).unwrap();
    let reply_chunks: Vec<FileChunk> = serde_json::from_slice(&decrypted_reply).unwrap();
    let reply_content = reassemble_chunks(reply_chunks).unwrap();

    assert!(reply_content.starts_with(b"// Received and verified by Companion Device"));

    server_task.await.unwrap();

    // Verify file on Endpoint B matches Endpoint A byte-for-byte
    let received_file_b = dir_b.join("main.rs");
    assert!(received_file_b.exists());
    let content_b = std::fs::read(&received_file_b).unwrap();
    assert_eq!(
        content_b, test_source_content,
        "File on Endpoint B must match Endpoint A byte-for-byte"
    );

    // Clean up temporary test directories
    let _ = std::fs::remove_dir_all(&temp_base);
}

#[tokio::test]
async fn test_workspace_manifest_diff_and_exclusions() {
    let temp_base =
        std::env::temp_dir().join(format!("aether_manifest_test_{}", uuid::Uuid::new_v4()));
    let ws_a = temp_base.join("ws_a");
    let ws_b = temp_base.join("ws_b");
    std::fs::create_dir_all(&ws_a).unwrap();
    std::fs::create_dir_all(&ws_b).unwrap();

    // Create normal files and excluded files in Workspace A
    std::fs::write(ws_a.join("lib.rs"), b"pub fn a() {}").unwrap();
    std::fs::write(ws_a.join("README.md"), b"# Test Project").unwrap();

    // Create ignored folder/files (.git, target)
    let git_dir = ws_a.join(".git");
    std::fs::create_dir_all(&git_dir).unwrap();
    std::fs::write(git_dir.join("config"), b"git config data").unwrap();

    let target_dir = ws_a.join("target");
    std::fs::create_dir_all(&target_dir).unwrap();
    std::fs::write(target_dir.join("binary.exe"), b"executable binary").unwrap();

    // Create partial file in Workspace B
    std::fs::write(ws_b.join("lib.rs"), b"pub fn a() {}").unwrap();

    let ignores = vec![
        ".git".to_string(),
        "target".to_string(),
        "node_modules".to_string(),
    ];
    let manifest_a = WorkspaceManifest::build(&ws_a, "ws_a", &ignores).unwrap();
    let manifest_b = WorkspaceManifest::build(&ws_b, "ws_b", &ignores).unwrap();

    // Verify exclusions were honored
    assert!(
        !manifest_a.entries.contains_key(".git/config"),
        ".git files must be excluded from sync manifest"
    );
    assert!(
        !manifest_a.entries.contains_key("target/binary.exe"),
        "target files must be excluded from sync manifest"
    );
    assert!(manifest_a.entries.contains_key("lib.rs"));
    assert!(manifest_a.entries.contains_key("README.md"));

    // Compare manifests
    let diff = diff_manifests(&manifest_a, &manifest_b);
    assert_eq!(
        diff.to_upload,
        vec!["README.md".to_string()],
        "README.md should be flagged to upload to B"
    );
    assert!(
        diff.conflicts.is_empty(),
        "There should be zero conflicts for matching lib.rs"
    );

    let _ = std::fs::remove_dir_all(&temp_base);
}
