#[cfg(windows)]
#[tokio::test]
async fn test_conpty_interactive_cmd_lifecycle() {
    use std::time::Duration;
    use terminal_manager::TerminalManager;

    let manager = TerminalManager::new();
    let session = manager
        .spawn_interactive("cmd", None, 80, 24)
        .expect("spawn interactive cmd");

    assert!(session.is_interactive);
    assert!(session.pid.is_some());
    let id = session.id;

    let mut output_rx = manager.subscribe_output(id).expect("subscribe output");

    tokio::time::sleep(Duration::from_millis(300)).await;

    // Write command into ConPTY stdin with carriage return
    manager
        .write_input(id, b"echo AETHER_CONPTY_VERIFIED\r\n")
        .expect("write command");

    let mut collected = manager.get_recent_output(id).unwrap_or_default();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    let mut found = String::from_utf8_lossy(&collected).contains("AETHER_CONPTY_VERIFIED");

    while !found && tokio::time::Instant::now() < deadline {
        match tokio::time::timeout(Duration::from_millis(500), output_rx.recv()).await {
            Ok(Ok(chunk)) => {
                collected.extend_from_slice(&chunk);
                let text = String::from_utf8_lossy(&collected);
                if text.contains("AETHER_CONPTY_VERIFIED") {
                    found = true;
                    break;
                }
            }
            _ => continue,
        }
    }

    assert!(
        found,
        "Failed to receive echo from ConPTY session. Received: {}",
        String::from_utf8_lossy(&collected)
    );

    // Test resize
    manager
        .resize_terminal(id, 120, 40)
        .expect("resize terminal");

    // Close session
    manager.close_session(id).expect("close session");
}
