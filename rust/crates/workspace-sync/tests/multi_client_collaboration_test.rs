use std::time::Duration;
use workspace_sync::{
    AwarenessTracker, CrdtDocument, CursorPosition, DeviceRole, PeerPresence, RoleRegistry,
};

#[tokio::test]
async fn test_two_endpoint_crdt_concurrent_convergence() {
    // 1. Initialize two independent in-memory CRDT client endpoints
    let peer_a = CrdtDocument::new("doc-shared-main");
    let peer_b = CrdtDocument::new("doc-shared-main");

    // 2. Set initial shared baseline document
    peer_a.set_text("// AetherIDE Commercial Beta Collaborative Document\nfn main() {\n");
    let initial_update = peer_a.get_full_update_base64();
    peer_b.apply_update_base64(&initial_update).unwrap();

    assert_eq!(peer_a.get_text(), peer_b.get_text());

    // 3. Simulate concurrent independent edits with network latency
    // Peer A (Alice) adds a logging call
    peer_a.insert_text(
        peer_a.get_text().len() as u32,
        "    println!(\"Peer A connected\");\n",
    );

    // Simulated network jitter / latency
    tokio::time::sleep(Duration::from_millis(5)).await;

    // Peer B (Bob) adds a return statement
    peer_b.insert_text(peer_b.get_text().len() as u32, "    // Peer B edit\n}\n");

    // 4. Exchange State Vectors (Two-Way CRDT Handshake)
    let sv_a = peer_a.get_state_vector_base64();
    let sv_b = peer_b.get_state_vector_base64();

    // Compute minimal delta diffs
    let diff_for_b = peer_a.get_diff_base64(&sv_b).unwrap();
    let diff_for_a = peer_b.get_diff_base64(&sv_a).unwrap();

    // Simulated network transit
    tokio::time::sleep(Duration::from_millis(5)).await;

    // Apply delta updates
    peer_b.apply_update_base64(&diff_for_b).unwrap();
    peer_a.apply_update_base64(&diff_for_a).unwrap();

    // 5. Verification: Both endpoints must deterministically converge to the exact same text
    let text_a = peer_a.get_text();
    let text_b = peer_b.get_text();

    assert_eq!(
        text_a, text_b,
        "CRDT documents must achieve deterministic mathematical convergence"
    );
    assert!(text_a.contains("Peer A connected"));
    assert!(text_a.contains("Peer B edit"));
}

#[tokio::test]
async fn test_multi_client_role_lifecycle_and_revocation() {
    let registry = RoleRegistry::new();

    // 1. Establish roles for multi-client cluster
    registry.register_peer("alice-desktop", DeviceRole::Owner);
    registry.register_peer("bob-laptop", DeviceRole::Editor);
    registry.register_peer("charlie-tablet", DeviceRole::Viewer);

    assert_eq!(registry.get_role("alice-desktop"), DeviceRole::Owner);
    assert!(registry.get_role("alice-desktop").can_write());
    assert!(registry.get_role("alice-desktop").can_admin());

    assert_eq!(registry.get_role("bob-laptop"), DeviceRole::Editor);
    assert!(registry.get_role("bob-laptop").can_write());
    assert!(!registry.get_role("bob-laptop").can_admin());

    assert_eq!(registry.get_role("charlie-tablet"), DeviceRole::Viewer);
    assert!(!registry.get_role("charlie-tablet").can_write());
    assert!(!registry.get_role("charlie-tablet").can_admin());

    // 2. Unregistered peer default
    assert_eq!(registry.get_role("unregistered-device"), DeviceRole::Viewer);
    assert!(!registry.get_role("unregistered-device").can_write());

    // 3. Admin demotes bob to Viewer
    registry.register_peer("bob-laptop", DeviceRole::Viewer);
    assert_eq!(registry.get_role("bob-laptop"), DeviceRole::Viewer);
    assert!(!registry.get_role("bob-laptop").can_write());

    // 4. Admin revokes Charlie entirely
    assert!(registry.revoke_peer("charlie-tablet"));
    assert_eq!(registry.get_role("charlie-tablet"), DeviceRole::Viewer);
}

#[tokio::test]
async fn test_multi_client_awareness_and_presence_heartbeats() {
    let tracker = AwarenessTracker::new();

    // 1. Peer A heartbeat
    let mut presence_a = PeerPresence::new("peer-a", "Alice", "#3b82f6");
    presence_a.current_file = Some("src/main.rs".into());
    presence_a.cursor = Some(CursorPosition { line: 12, col: 4 });
    tracker.update_presence(presence_a);

    // 2. Peer B heartbeat
    let mut presence_b = PeerPresence::new("peer-b", "Bob", "#10b981");
    presence_b.current_file = Some("src/main.rs".into());
    presence_b.cursor = Some(CursorPosition { line: 25, col: 8 });
    tracker.update_presence(presence_b);

    // 3. Query presence on src/main.rs with 60-second freshness window
    let presences = tracker.get_peers_for_file("src/main.rs", 60);
    assert_eq!(presences.len(), 2);

    let alice = presences.iter().find(|p| p.peer_id == "peer-a").unwrap();
    assert_eq!(alice.cursor, Some(CursorPosition { line: 12, col: 4 }));
    assert_eq!(alice.display_name, "Alice");

    let bob = presences.iter().find(|p| p.peer_id == "peer-b").unwrap();
    assert_eq!(bob.cursor, Some(CursorPosition { line: 25, col: 8 }));
    assert_eq!(bob.display_name, "Bob");

    // 4. Query presence on another file
    let empty_presences = tracker.get_peers_for_file("src/lib.rs", 60);
    assert_eq!(empty_presences.len(), 0);
}
