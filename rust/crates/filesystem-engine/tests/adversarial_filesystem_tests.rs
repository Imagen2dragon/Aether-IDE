use filesystem_engine::FilesystemEngine;
use std::fs;
use std::path::PathBuf;

fn setup_temp_workspace() -> PathBuf {
    let random = uuid::Uuid::new_v4().simple();
    let dir = std::env::temp_dir().join(format!("aether_adv_ws_{}", random));
    fs::create_dir_all(&dir).expect("Failed to create temporary workspace");
    dir
}

#[test]
fn test_01_traversal_dot_dot_variations() {
    let ws = setup_temp_workspace();

    let malicious_paths = [
        "../outside.txt",
        "..\\outside.txt",
        "foo/../../outside.txt",
        "nested/../../../../etc/passwd",
        "a/b/c/../../../..",
        "./../../secret",
    ];

    for path in &malicious_paths {
        let res = FilesystemEngine::resolve_contained_path(&ws, path);
        assert!(
            res.is_err(),
            "Path '{}' should be rejected as traversal escape",
            path
        );
    }

    let _ = fs::remove_dir_all(&ws);
}

#[test]
fn test_02_absolute_paths_and_drive_letters() {
    let ws = setup_temp_workspace();

    let abs_paths = [
        "C:\\Windows\\System32\\cmd.exe",
        "D:\\data\\secrets.json",
        "/etc/shadow",
        "/usr/bin/bash",
        "\\Windows\\explorer.exe",
    ];

    for path in &abs_paths {
        let res = FilesystemEngine::resolve_contained_path(&ws, path);
        assert!(res.is_err(), "Absolute path '{}' must be rejected", path);
    }

    let _ = fs::remove_dir_all(&ws);
}

#[test]
fn test_03_unc_network_paths() {
    let ws = setup_temp_workspace();

    let unc_paths = [
        r"\\localhost\c$\windows\system32",
        r"\\192.168.1.1\share\payload.dll",
        "//10.0.0.1/public/malware.exe",
        r"\\?\UNC\server\share\file.txt",
    ];

    for path in &unc_paths {
        let res = FilesystemEngine::resolve_contained_path(&ws, path);
        assert!(res.is_err(), "UNC path '{}' must be rejected", path);
    }

    let _ = fs::remove_dir_all(&ws);
}

#[test]
fn test_04_extended_device_namespaces() {
    let ws = setup_temp_workspace();

    let dev_ns = [
        r"\\?\C:\Windows\System32",
        r"\??\C:\Windows",
        "//?/C:/Windows",
        r"\\.\PhysicalDrive0",
    ];

    for path in &dev_ns {
        let res = FilesystemEngine::resolve_contained_path(&ws, path);
        assert!(
            res.is_err(),
            "Extended device namespace '{}' must be rejected",
            path
        );
    }

    let _ = fs::remove_dir_all(&ws);
}

#[test]
fn test_05_alternate_data_streams() {
    let ws = setup_temp_workspace();

    let ads_paths = [
        "app.exe:hidden.ps1",
        "index.html::$DATA",
        "src/main.rs:zone.identifier",
        "test.txt:stream:$DATA",
        ":root_stream",
    ];

    for path in &ads_paths {
        let res = FilesystemEngine::resolve_contained_path(&ws, path);
        assert!(res.is_err(), "ADS path '{}' must be rejected", path);
    }

    let _ = fs::remove_dir_all(&ws);
}

#[test]
fn test_06_trailing_dot_and_space_truncation() {
    let ws = setup_temp_workspace();

    let truncation_paths = [
        "secret.txt.",
        "secret.txt ",
        "foo/bar./baz",
        "nested/dir /file.rs",
        "trailing_spaces   ",
        "dots.....",
    ];

    for path in &truncation_paths {
        let res = FilesystemEngine::resolve_contained_path(&ws, path);
        assert!(
            res.is_err(),
            "Trailing dot/space path '{}' must be rejected",
            path
        );
    }

    let _ = fs::remove_dir_all(&ws);
}

#[test]
fn test_07_win32_reserved_devices() {
    let ws = setup_temp_workspace();

    let devices = [
        "CON",
        "PRN",
        "AUX",
        "NUL",
        "COM1",
        "COM2",
        "COM3",
        "COM4",
        "COM5",
        "COM6",
        "COM7",
        "COM8",
        "COM9",
        "LPT1",
        "LPT2",
        "LPT3",
        "LPT4",
        "LPT5",
        "LPT6",
        "LPT7",
        "LPT8",
        "LPT9",
        "con.txt",
        "aux.rs",
        "nul.json",
        "COM1.dat",
        "subdir/PRN.exe",
        "nested/con",
    ];

    for dev in &devices {
        let res = FilesystemEngine::resolve_contained_path(&ws, dev);
        assert!(
            res.is_err(),
            "Reserved device name '{}' must be rejected",
            dev
        );
    }

    let _ = fs::remove_dir_all(&ws);
}

#[test]
fn test_08_url_encoded_traversal_evasion() {
    let ws = setup_temp_workspace();

    let encoded_paths = [
        "%2e%2e/secret.txt",
        "..%2foutside.txt",
        "%2e%2e%5cwindows",
        "foo/%2e%2e/bar",
        "evil%00.png",
    ];

    for path in &encoded_paths {
        let res = FilesystemEngine::resolve_contained_path(&ws, path);
        assert!(
            res.is_err(),
            "Encoded traversal sequence '{}' must be rejected",
            path
        );
    }

    let _ = fs::remove_dir_all(&ws);
}

#[test]
fn test_09_component_boundary_prefix_collision() {
    let ws = setup_temp_workspace();
    let ws_str = ws.to_string_lossy().to_string();

    // Sibling directory sharing the exact string prefix but different component
    let evil_sibling = PathBuf::from(format!("{}-external", ws_str));
    let evil_sub = evil_sibling.join("exploit.txt");

    // Rust path component comparison must strictly return false
    assert!(
        !FilesystemEngine::is_within_boundary(&evil_sibling, &ws),
        "Sibling folder sharing name prefix must not be considered within boundary"
    );
    assert!(
        !FilesystemEngine::is_within_boundary(&evil_sub, &ws),
        "File inside sibling folder must not be within boundary"
    );

    // True child must return true
    let genuine_child = ws.join("genuine").join("child.txt");
    assert!(
        FilesystemEngine::is_within_boundary(&genuine_child, &ws),
        "Genuine subpath must be within boundary"
    );

    let _ = fs::remove_dir_all(&ws);
}

#[test]
fn test_10_nonexistent_deep_path_traversal() {
    let ws = setup_temp_workspace();

    // Even if intermediate directories do not exist, traversal attempts escaping the root must fail
    let path = "does/not/exist/yet/../../../../../../escaped.txt";
    let res = FilesystemEngine::resolve_contained_path(&ws, path);
    assert!(
        res.is_err(),
        "Nonexistent deep traversal '{}' must be rejected",
        path
    );

    let _ = fs::remove_dir_all(&ws);
}

#[test]
fn test_11_contained_atomic_write_and_read_lifecycle() {
    let ws = setup_temp_workspace();

    // Normal safe write
    let created =
        FilesystemEngine::write_file_contained(&ws, "src/engine/mod.rs", "pub fn start() {}")
            .expect("write_file_contained should succeed for safe path");
    assert!(created.exists());

    let content = FilesystemEngine::read_file_contained(&ws, "src/engine/mod.rs")
        .expect("read_file_contained should succeed");
    assert_eq!(content, "pub fn start() {}");

    // Traversal write attempt
    let bad_write = FilesystemEngine::write_file_contained(&ws, "../../escape.txt", "payload");
    assert!(bad_write.is_err());

    // Rename
    let rename = FilesystemEngine::rename_contained(&ws, "src/engine/mod.rs", "src/engine/lib.rs");
    assert!(rename.is_ok());

    let renamed_content = FilesystemEngine::read_file_contained(&ws, "src/engine/lib.rs")
        .expect("read_file_contained after rename should succeed");
    assert_eq!(renamed_content, "pub fn start() {}");

    // Delete
    let delete = FilesystemEngine::delete_file_contained(&ws, "src/engine/lib.rs");
    assert!(delete.is_ok());
    assert!(FilesystemEngine::read_file_contained(&ws, "src/engine/lib.rs").is_err());

    let _ = fs::remove_dir_all(&ws);
}

#[test]
fn test_12_forbidden_control_and_wildcard_characters() {
    let ws = setup_temp_workspace();

    let invalid_chars = [
        "file<name.txt",
        "file>name.txt",
        "file|pipe.txt",
        "wild*card.txt",
        "query?param.txt",
        "quote\"name.txt",
        "null\0byte.txt",
        "control\x01char.txt",
        "escape\x1bchar.txt",
    ];

    for path in &invalid_chars {
        let res = FilesystemEngine::resolve_contained_path(&ws, path);
        assert!(
            res.is_err(),
            "Path with forbidden characters '{}' must be rejected",
            path
        );
    }

    let _ = fs::remove_dir_all(&ws);
}

#[test]
fn test_13_concurrent_adversarial_symlink_race() {
    let ws = setup_temp_workspace();
    let outside_dir = std::env::temp_dir().join(format!(
        "aether_outside_sentinel_{}",
        uuid::Uuid::new_v4().simple()
    ));
    fs::create_dir_all(&outside_dir).expect("Failed to create outside directory");

    let sentinel_file = outside_dir.join("sentinel.txt");
    let canary_file = outside_dir.join("canary.txt");
    fs::write(&sentinel_file, "CRITICAL_SENTINEL_ROOT_KEY_DO_NOT_TAMPER").unwrap();
    fs::write(&canary_file, "OUTSIDE_CANARY_DO_NOT_DELETE").unwrap();

    let race_sub = ws.join("race_target");
    let _ = fs::create_dir_all(&race_sub);

    let stop_flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let stop_clone = stop_flag.clone();
    let ws_clone = ws.clone();
    let outside_clone = outside_dir.clone();

    // Background adversarial thread: repeatedly attempts to substitute directory with symlink/junction
    let attacker = std::thread::spawn(move || {
        let mut iter = 0;
        while !stop_clone.load(std::sync::atomic::Ordering::Relaxed) && iter < 150 {
            iter += 1;
            let target = ws_clone.join("race_target");
            let _ = fs::remove_dir_all(&target);

            #[cfg(windows)]
            let _ = std::os::windows::fs::symlink_dir(&outside_clone, &target);
            #[cfg(unix)]
            let _ = std::os::unix::fs::symlink(&outside_clone, &target);

            std::thread::sleep(std::time::Duration::from_millis(1));

            let _ = fs::remove_dir(&target);
            let _ = fs::create_dir_all(&target);
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
    });

    // Worker thread: repeatedly executes contained filesystem operations
    for _ in 0..100 {
        // Attempt contained write
        let _ = FilesystemEngine::write_file_contained(
            &ws,
            "race_target/payload.txt",
            "WORKER_SAFE_CONTENT",
        );

        // Attempt contained read
        let _ = FilesystemEngine::read_file_contained(&ws, "race_target/payload.txt");

        // Attempt contained create dir
        let _ = FilesystemEngine::create_dir_contained(&ws, "race_target/inner_dir");

        // Attempt contained rename
        let _ = FilesystemEngine::rename_contained(
            &ws,
            "race_target/payload.txt",
            "race_target/payload_moved.txt",
        );

        // Attempt contained delete
        let _ = FilesystemEngine::delete_file_contained(&ws, "race_target/payload_moved.txt");
    }

    stop_flag.store(true, std::sync::atomic::Ordering::Relaxed);
    let _ = attacker.join();

    // RIGOROUS VERIFICATION:
    // 1. Outside sentinel file content must remain completely unchanged!
    let current_sentinel =
        fs::read_to_string(&sentinel_file).expect("Sentinel file must still exist");
    assert_eq!(
        current_sentinel, "CRITICAL_SENTINEL_ROOT_KEY_DO_NOT_TAMPER",
        "Sentinel file was tampered during concurrent attack race!"
    );

    // 2. Outside canary file must NOT have been deleted!
    let current_canary = fs::read_to_string(&canary_file).expect("Canary file must still exist");
    assert_eq!(
        current_canary, "OUTSIDE_CANARY_DO_NOT_DELETE",
        "Canary file was tampered or deleted!"
    );

    // 3. No unauthorized files or directories created in outside directory!
    let outside_entries: Vec<String> = fs::read_dir(&outside_dir)
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();

    for name in &outside_entries {
        assert!(
            name == "sentinel.txt" || name == "canary.txt",
            "Unauthorized file '{}' was created in outside directory during race attack!",
            name
        );
    }

    // 4. Indexer scan of workspace never ingests outside sentinel
    let scanned = FilesystemEngine::scan_workspace_files(&ws, &[]).unwrap_or_default();
    for path in scanned {
        let content = fs::read_to_string(&path).unwrap_or_default();
        assert!(
            !content.contains("CRITICAL_SENTINEL"),
            "Outside sentinel data leaked into workspace indexer!"
        );
    }

    let _ = fs::remove_dir_all(&ws);
    let _ = fs::remove_dir_all(&outside_dir);
}
