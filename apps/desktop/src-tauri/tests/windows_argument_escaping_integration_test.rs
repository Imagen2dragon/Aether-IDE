use aether_desktop_lib::commands::build::ApprovedBuildProcess;
use std::path::PathBuf;

#[tokio::test]
async fn test_windows_process_argument_escaping_integration_test_binary() {
    let temp_dir = std::env::temp_dir().join(format!(
        "aether_bld_010_integ_{}",
        uuid::Uuid::new_v4().simple()
    ));
    let work_dir = temp_dir.join("work");
    std::fs::create_dir_all(&work_dir).unwrap();

    // In integration tests, Cargo provides the absolute path to [[bin]] targets via env!("CARGO_BIN_EXE_<name>")
    let helper_exe = PathBuf::from(env!("CARGO_BIN_EXE_test_arg_echo"));
    assert!(
        helper_exe.is_file(),
        "CARGO_BIN_EXE_test_arg_echo must point to compiled binary: {:?}",
        helper_exe
    );

    // Prepend the directory of the binary to PATH so resolve_approved_tool_executable finds it
    let helper_dir = helper_exe.parent().unwrap();
    let orig_path = std::env::var_os("PATH").unwrap_or_default();
    let mut new_paths = vec![helper_dir.to_path_buf()];
    new_paths.extend(std::env::split_paths(&orig_path));
    let new_path_os = std::env::join_paths(new_paths).unwrap();
    std::env::set_var("PATH", &new_path_os);

    // 1. Invariant: test_arg_echo MUST be rejected by default without explicit extra tools
    let default_rejection =
        ApprovedBuildProcess::prepare("test_arg_echo --flag", &work_dir, true, &[]);
    assert!(
        default_rejection.is_err(),
        "test_arg_echo must not be in default production approved whitelist"
    );

    // 2. Command containing Windows shell metacharacters (&, |, >, <, %TEMP%), quotes, spaces, and backslashes
    let command_str = r#"test_arg_echo --flag "injected & echo PWNED > hacked.txt | type hacked.txt < NUL %TEMP%" "argument with spaces and \"quotes\"" "C:\Windows\System32\drivers\etc\hosts""#;

    // 3. Prepare using shared production process launcher
    let prepared = ApprovedBuildProcess::prepare(command_str, &work_dir, true, &["test_arg_echo"])
        .expect("ApprovedBuildProcess preparation must succeed");

    assert_eq!(prepared.args.len(), 4);
    assert_eq!(prepared.args[0], "--flag");
    assert_eq!(
        prepared.args[1],
        "injected & echo PWNED > hacked.txt | type hacked.txt < NUL %TEMP%"
    );
    assert_eq!(prepared.args[2], "argument with spaces and \"quotes\"");
    assert_eq!(prepared.args[3], r"C:\Windows\System32\drivers\etc\hosts");

    // 4. Spawn using shared production process spawn implementation
    let child = prepared
        .spawn()
        .expect("Failed to spawn prepared build process");

    let output = child
        .wait_with_output()
        .await
        .expect("Failed to await child execution");

    assert!(output.status.success());

    // 5. Verify machine-verifiable output
    let stdout_str = String::from_utf8_lossy(&output.stdout);
    let lines: Vec<&str> = stdout_str.lines().map(|l| l.trim()).collect();

    assert!(
        lines.contains(&"COUNT:4"),
        "Expected COUNT:4, got:\n{}",
        stdout_str
    );
    assert!(
        lines.contains(&"ARG[0]:--flag"),
        "Expected ARG[0]:--flag, got:\n{}",
        stdout_str
    );
    assert!(
        lines.contains(&"ARG[1]:injected & echo PWNED > hacked.txt | type hacked.txt < NUL %TEMP%"),
        "Metacharacters must be preserved verbatim without shell evaluation! Got:\n{}",
        stdout_str
    );
    assert!(
        lines.contains(&"ARG[2]:argument with spaces and \"quotes\""),
        "Spaces and quotes must be preserved verbatim! Got:\n{}",
        stdout_str
    );
    assert!(
        lines.contains(&r"ARG[3]:C:\Windows\System32\drivers\etc\hosts"),
        "Backslashes must be preserved verbatim! Got:\n{}",
        stdout_str
    );

    // 6. Prove that NO shell interpretation occurred and NO file was created on disk
    let hacked_file = work_dir.join("hacked.txt");
    assert!(
        !hacked_file.exists(),
        "Security Failure: hacked.txt was created on disk! Shell metacharacters escaped into command execution."
    );

    std::env::set_var("PATH", orig_path);
    let _ = std::fs::remove_dir_all(&temp_dir);
}
