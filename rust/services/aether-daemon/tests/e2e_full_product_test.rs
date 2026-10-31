use agent_orchestrator::AgentOrchestrator;
use application_core::AutonomyMode;
use checkpoint_engine::CheckpointEngine;
use database_engine::{DatabaseEngine, ProofOfWorkRecord};
use lsp_client::LspMultiplexer;
use model_gateway::ModelGateway;
use std::process::Command;
#[cfg(windows)]
use std::time::Duration;
use symbol_index::SymbolIndex;
use syntax_engine::{SupportedLanguage, SyntaxEngine};
#[cfg(windows)]
use terminal_manager::TerminalManager;
use workspace_sync::{
    reassemble_chunks, slice_file, AeadChannel, FileChunk, PairingPayload, SyncRole,
    WorkspaceManifest,
};

#[tokio::test]
async fn test_e2e_full_product_lifecycle_disposable_project() {
    println!("\n=== Starting AetherIDE End-to-End Product Lifecycle Integration Test ===");

    // STEP 1: Create Disposable Project Directory & Git Repo
    let random_id = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!("aether_e2e_project_{}", random_id));
    let src_dir = temp_dir.join("src");
    std::fs::create_dir_all(&src_dir).expect("Failed to create disposable test project directory");

    // Initialize real git repo in the disposable directory
    let git_init = Command::new("git")
        .args(["init"])
        .current_dir(&temp_dir)
        .output();
    let is_git_available = git_init.map(|o| o.status.success()).unwrap_or(false);
    println!(
        " [Step 1] Disposable project created at: {:?} (git init: {})",
        temp_dir, is_git_available
    );

    let initial_file = src_dir.join("calculator.rs");
    let initial_code = r#"// AetherIDE Sample Calculation Module
pub fn calculate_sum(a: i32, b: i32) -> i32 {
    a + b
}

pub fn multiply(a: i32, b: i32) -> i32 {
    a * b
}
"#;
    std::fs::write(&initial_file, initial_code).expect("Failed to write initial file");

    if is_git_available {
        let _ = Command::new("git")
            .args(["config", "user.email", "test@aetheride.local"])
            .current_dir(&temp_dir)
            .output();
        let _ = Command::new("git")
            .args(["config", "user.name", "Aether Test Runner"])
            .current_dir(&temp_dir)
            .output();
        let _ = Command::new("git")
            .args(["add", "."])
            .current_dir(&temp_dir)
            .output();
        let _ = Command::new("git")
            .args(["commit", "-m", "Initial commit"])
            .current_dir(&temp_dir)
            .output();
    }

    // STEP 2: Database Initialization & Symbol Indexing
    let db = DatabaseEngine::open_in_memory().expect("Failed to open test database");
    let symbol_index = SymbolIndex::new(db.clone());

    // Extract symbols using AST parser and insert into index
    let symbols = SyntaxEngine::extract_symbols(initial_code, SupportedLanguage::Rust);
    assert!(!symbols.is_empty(), "Extracted symbols must not be empty");
    let insert_count = symbol_index
        .insert_file_symbols(&initial_file.to_string_lossy(), &symbols)
        .expect("Symbol insertion must succeed");
    assert!(insert_count >= 2, "Must insert at least 2 functions");

    let found_symbols = db.search_symbols_fts("calculate_sum", 10).unwrap();
    assert!(
        !found_symbols.is_empty(),
        "Symbol 'calculate_sum' must be found in FTS5 index"
    );
    assert_eq!(found_symbols[0].name, "calculate_sum");
    println!(
        " [Step 2] Symbol index verified: found '{}' in FTS5 index",
        found_symbols[0].name
    );

    // STEP 3: Language Server Protocol Multiplexer
    let lsp_multiplexer = LspMultiplexer::new(temp_dir.clone());
    let open_res = lsp_multiplexer
        .open_document(&initial_file, initial_code, Some("rust"))
        .await;
    println!(
        " [Step 3] LSP open document response: {:?}",
        open_res.is_ok()
    );

    // Query diagnostics
    let diags = lsp_multiplexer.get_diagnostics(&initial_file);
    println!(
        " [Step 3] LSP diagnostics queried: {} items found",
        diags.len()
    );

    // STEP 4: Interactive ConPTY Terminal Session
    #[cfg(windows)]
    {
        let term_mgr = TerminalManager::new();
        let session_info = term_mgr
            .spawn_interactive("cmd", Some(temp_dir.clone()), 80, 24)
            .expect("ConPTY session spawn must succeed");
        let session_id = session_info.id;
        println!(" [Step 4] ConPTY session spawned with ID: {}", session_id);

        let mut output_rx = term_mgr.subscribe_output(session_id).unwrap();

        // Write a test echo command
        let test_cmd = "echo E2E_CONPTY_STREAM_OK\r\n";
        let write_res = term_mgr.write_input(session_id, test_cmd.as_bytes());
        assert!(write_res.is_ok(), "Writing to ConPTY session must succeed");

        // Read output from the broadcast stream
        let mut captured = String::new();
        let timeout_deadline = tokio::time::Instant::now() + Duration::from_secs(3);
        while tokio::time::Instant::now() < timeout_deadline {
            if let Ok(Ok(chunk)) =
                tokio::time::timeout(Duration::from_millis(200), output_rx.recv()).await
            {
                captured.push_str(&String::from_utf8_lossy(&chunk));
                if captured.contains("E2E_CONPTY_STREAM_OK") {
                    break;
                }
            }
        }
        println!(
            " [Step 4] ConPTY captured output: contains 'E2E_CONPTY_STREAM_OK' = {}",
            captured.contains("E2E_CONPTY_STREAM_OK")
        );
        assert!(
            captured.contains("E2E_CONPTY_STREAM_OK"),
            "ConPTY must stream back written command output"
        );

        let close_res = term_mgr.close_session(session_id);
        assert!(close_res.is_ok(), "ConPTY session close must succeed");
    }

    // STEP 5: Autonomous Agent Task Execution & Proof-of-Work
    let model_gateway = ModelGateway::new();
    let orchestrator = AgentOrchestrator::new(model_gateway, db.clone());

    let task_id = orchestrator.spawn_task_with_worktree(
        "systems",
        "Implement divide function and run unit tests",
        AutonomyMode::Autopilot,
        Some(temp_dir.clone()),
    );
    println!(" [Step 5] Agent task spawned with ID: {}", task_id);

    // Simulate Agent Tool Dispatch: write a new function to calculator.rs
    let updated_code = r#"// AetherIDE Sample Calculation Module
pub fn calculate_sum(a: i32, b: i32) -> i32 {
    a + b
}

pub fn multiply(a: i32, b: i32) -> i32 {
    a * b
}

pub fn divide(a: i32, b: i32) -> Result<i32, &'static str> {
    if b == 0 {
        return Err("Cannot divide by zero");
    }
    Ok(a / b)
}
"#;

    let tool_input = serde_json::json!({
        "path": "src/calculator.rs",
        "content": updated_code
    });

    let tool_res =
        agent_runtime::ToolDispatcher::dispatch("write_file", &tool_input, Some(&temp_dir), &db)
            .await;
    assert!(tool_res.is_ok(), "Tool dispatch write_file must succeed");
    println!(" [Step 5] Agent tool dispatched write_file successfully");

    // Cryptographic Proof of Work Logging
    let pow_record = ProofOfWorkRecord {
        id: None,
        task_id: task_id.clone(),
        step_index: 1,
        tool_name: "write_file".to_string(),
        input_json: tool_input.to_string(),
        output_json: "exit_code: 0".to_string(),
        diff: Some("sha256-verified-diff".to_string()),
        exit_code: 0,
        timestamp: "2026-10-31T00:20:00Z".to_string(),
    };
    let pow_res = db.record_proof_of_work(&pow_record);
    assert!(
        pow_res.is_ok(),
        "Proof of Work must be logged in SQLite ledger"
    );

    let ledger_entries = db.get_proof_of_work(&task_id).unwrap();
    assert_eq!(ledger_entries.len(), 1);
    assert_eq!(ledger_entries[0].tool_name, "write_file");
    println!(
        " [Step 5] Proof of work ledger verified: {} entry found",
        ledger_entries.len()
    );

    // STEP 6: Workspace Checkpoint Snapshot
    let ckpt_engine = CheckpointEngine::new(temp_dir.join(".checkpoints"));
    let snapshot_res =
        ckpt_engine.create_checkpoint("Pre-sync state", std::slice::from_ref(&initial_file));
    assert!(snapshot_res.is_ok(), "Checkpoint creation must succeed");
    let checkpoint = snapshot_res.unwrap();
    println!(
        " [Step 6] Workspace checkpoint created: id={}, files={}",
        checkpoint.checkpoint_id,
        checkpoint.file_hashes.len()
    );

    // STEP 7: Encrypted Local Device Synchronization (Desktop -> Companion)
    let (pairing, psk) = PairingPayload::new("192.168.1.100", 44888, 300).unwrap();
    assert!(pairing.to_pairing_uri().starts_with("aether://pair?"));

    let ignores = vec![".git".to_string(), "target".to_string()];
    let manifest = WorkspaceManifest::build(&temp_dir, "e2e_project", &ignores).unwrap();
    assert!(manifest.entries.contains_key("src/calculator.rs"));
    println!(
        " [Step 7] Workspace Merkle manifest built: root_hash={}",
        manifest.root_hash
    );

    // Slicing into 64KB content-addressable chunks
    let file_bytes = std::fs::read(&initial_file).unwrap();
    let chunks = slice_file("src/calculator.rs", &file_bytes, 64 * 1024);
    let chunks_json = serde_json::to_vec(&chunks).unwrap();

    // ChaCha20-Poly1305 AEAD Framing
    let mut desktop_channel = AeadChannel::new(psk, SyncRole::Desktop);
    let mut companion_channel = AeadChannel::new(psk, SyncRole::Companion);

    let encrypted_frame = desktop_channel.encrypt_frame(&chunks_json).unwrap();
    assert!(!encrypted_frame.is_empty());

    // Decrypt on Companion endpoint
    let decrypted_payload = companion_channel.decrypt_frame(&encrypted_frame).unwrap();
    let received_chunks: Vec<FileChunk> = serde_json::from_slice(&decrypted_payload).unwrap();
    let reassembled_file = reassemble_chunks(received_chunks).unwrap();

    assert_eq!(
        reassembled_file, file_bytes,
        "Reassembled content on Companion must match original file byte-for-byte"
    );
    println!(" [Step 7] Encrypted workspace sync verified: 100% byte match on companion endpoint");

    // Clean up disposable project
    let _ = std::fs::remove_dir_all(&temp_dir);
    println!("=== AetherIDE End-to-End Product Lifecycle Integration Test PASSED ===\n");
}
