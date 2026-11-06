use aether_desktop_lib::commands::build::{parse_command_tokens, resolve_approved_tool_executable};
use std::path::PathBuf;

#[test]
fn test_windows_process_argument_escaping_integration_test_binary() {
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

    // 1. Resolve through production approved-tool resolver
    let resolved_tool = resolve_approved_tool_executable("test_arg_echo", &work_dir, true)
        .expect("test_arg_echo must resolve from PATH");

    // 2. Command containing Windows shell metacharacters (&, |, >, <, %TEMP%)
    let command_str = r#"test_arg_echo --flag "injected & echo PWNED > hacked.txt | type hacked.txt < NUL %TEMP%""#;

    // 3. Tokenize using production tokenizer
    let tokens = parse_command_tokens(command_str);
    assert_eq!(tokens[0], "test_arg_echo");
    assert_eq!(tokens[1], "--flag");
    assert_eq!(
        tokens[2],
        "injected & echo PWNED > hacked.txt | type hacked.txt < NUL %TEMP%"
    );

    // 4. Launch using production process execution pattern
    let output = std::process::Command::new(&resolved_tool)
        .args(&tokens[1..])
        .current_dir(&work_dir)
        .output()
        .expect("Failed to execute test_arg_echo");

    assert!(output.status.success());

    // 5. Verify machine-verifiable output
    let stdout_str = String::from_utf8_lossy(&output.stdout);
    let lines: Vec<&str> = stdout_str.lines().map(|l| l.trim()).collect();

    assert!(
        lines.contains(&"COUNT:2"),
        "Expected COUNT:2, got:\n{}",
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

    // 6. Prove that NO shell interpretation occurred and NO file was created on disk
    let hacked_file = work_dir.join("hacked.txt");
    assert!(
        !hacked_file.exists(),
        "Security Failure: hacked.txt was created on disk! Shell metacharacters escaped into command execution."
    );

    std::env::set_var("PATH", orig_path);
    let _ = std::fs::remove_dir_all(&temp_dir);
}
