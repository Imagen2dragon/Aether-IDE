use lsp_client::{path_to_uri, DocumentState, LspClient, LspClientConfig, LspMultiplexer};
use std::process::Command;
use std::time::Duration;

#[tokio::test]
async fn test_live_rust_analyzer_or_actionable_error() {
    let has_ra = Command::new("rust-analyzer")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    let workspace_dir = std::env::current_dir().unwrap();

    if has_ra {
        println!("[Live LSP Test] rust-analyzer found on host, testing live LSP lifecycle...");

        let config = LspClientConfig {
            server_id: "rust".to_string(),
            command: "rust-analyzer".to_string(),
            args: Vec::new(),
            workspace_root: workspace_dir.clone(),
            initialization_options: None,
        };

        let client = match LspClient::spawn(config).await {
            Ok(c) => c,
            Err(e) => panic!("Failed to spawn rust-analyzer: {}", e),
        };

        // Initialize live server
        let init_res = client.initialize(Duration::from_secs(15)).await;
        assert!(
            init_res.is_ok(),
            "rust-analyzer live initialize should succeed: {:?}",
            init_res.err()
        );

        // Open a sample rust file in the client
        let test_file = workspace_dir.join("src").join("lib.rs");
        let content = "pub fn add_numbers(a: i32, b: i32) -> i32 { a + b }\n";
        let doc = DocumentState::new(path_to_uri(&test_file), "rust", content);
        let open_res = client.did_open(&doc).await;
        assert!(open_res.is_ok(), "did_open should succeed on live server");

        // Request completions on the live server
        let comp_res = client
            .completion(&test_file, 0, 10, Duration::from_secs(5))
            .await;
        assert!(
            comp_res.is_ok(),
            "completion should return a result from rust-analyzer"
        );

        // Request hover
        let hover_res = client.hover(&test_file, 0, 8, Duration::from_secs(5)).await;
        assert!(hover_res.is_ok(), "hover query should be processed");

        // Clean shutdown
        let shutdown_res = client.shutdown(Duration::from_secs(5)).await;
        assert!(shutdown_res.is_ok(), "clean shutdown should succeed");
    } else {
        println!("[Live LSP Test] rust-analyzer not on PATH; testing actionable error message...");
        let multiplexer = LspMultiplexer::new(workspace_dir);
        let res = multiplexer.get_or_spawn_client("nonexistent-lang").await;
        assert!(res.is_err());
        let err_msg = res.err().unwrap().to_string();
        assert!(err_msg.contains("No language server registered"));
    }
}

#[tokio::test]
async fn test_missing_language_server_provides_actionable_instructions() {
    let workspace_dir = std::env::current_dir().unwrap();
    let config = LspClientConfig {
        server_id: "typescript".to_string(),
        command: "definitely-not-installed-tsserver-xyz".to_string(),
        args: vec!["--stdio".to_string()],
        workspace_root: workspace_dir,
        initialization_options: None,
    };

    let result = LspClient::spawn(config).await;
    assert!(result.is_err(), "Spawn of nonexistent binary must fail");
    let err = result.err().unwrap();
    let err_msg = err.to_string();
    assert!(
        err_msg.contains("not found on PATH"),
        "Error message should mention not found on PATH: {}",
        err_msg
    );
    assert!(
        err_msg.contains("npm install -g typescript-language-server"),
        "Error message should provide installation instructions: {}",
        err_msg
    );
}
