#[cfg(windows)]
#[tokio::test]
async fn test_multi_session_conpty_lifecycle() {
    use std::time::Duration;
    use terminal_manager::TerminalManager;

    let manager = TerminalManager::new();

    // 1. Spawn Session 1 (PID 1) and Session 2 (PID 2)
    let s1 = manager
        .spawn_interactive("cmd", None, 80, 24)
        .expect("spawn interactive cmd 1");
    let s2 = manager
        .spawn_interactive("cmd", None, 100, 30)
        .expect("spawn interactive cmd 2");

    assert_ne!(s1.id, s2.id, "Session IDs must be distinct");
    assert!(
        s1.pid.is_some() && s2.pid.is_some(),
        "PIDs must be assigned"
    );
    assert_ne!(s1.pid, s2.pid, "Child PIDs must be distinct");

    let pid1 = s1.pid.unwrap();
    let pid2 = s2.pid.unwrap();

    // RAII guard: ensure cleanup runs even if an assertion fails later in the test
    struct SessionCleanupGuard<'a> {
        mgr: &'a TerminalManager,
        sessions: Vec<u32>,
    }
    impl<'a> Drop for SessionCleanupGuard<'a> {
        fn drop(&mut self) {
            for &id in &self.sessions {
                let _ = self.mgr.close_session(id);
            }
        }
    }
    let mut guard = SessionCleanupGuard {
        mgr: &manager,
        sessions: vec![s1.id, s2.id],
    };

    // Verify initial session dimensions
    assert_eq!(
        manager.get_terminal_dimensions(s1.id).expect("get s1 dim"),
        (80, 24),
        "Session 1 initial dimensions must be 80x24"
    );
    assert_eq!(
        manager.get_terminal_dimensions(s2.id).expect("get s2 dim"),
        (100, 30),
        "Session 2 initial dimensions must be 100x30"
    );

    // 2. Subscribe to both output channels
    let mut rx1 = manager.subscribe_output(s1.id).expect("subscribe s1");
    let mut rx2 = manager.subscribe_output(s2.id).expect("subscribe s2");

    tokio::time::sleep(Duration::from_millis(300)).await;

    // Helper to collect output until marker or deadline
    async fn wait_for_marker(
        rx: &mut tokio::sync::broadcast::Receiver<Vec<u8>>,
        initial: Vec<u8>,
        marker: &str,
        timeout_dur: Duration,
    ) -> (bool, String) {
        let mut collected = initial;
        let deadline = tokio::time::Instant::now() + timeout_dur;
        let mut found = String::from_utf8_lossy(&collected).contains(marker);

        while !found && tokio::time::Instant::now() < deadline {
            match tokio::time::timeout(Duration::from_millis(400), rx.recv()).await {
                Ok(Ok(chunk)) => {
                    collected.extend_from_slice(&chunk);
                    if String::from_utf8_lossy(&collected).contains(marker) {
                        found = true;
                        break;
                    }
                }
                _ => continue,
            }
        }
        (found, String::from_utf8_lossy(&collected).to_string())
    }

    // 3. Send distinct echoes to each session
    manager
        .write_input(s1.id, b"echo SESSION_1_ECHO\r\n")
        .expect("write s1");
    manager
        .write_input(s2.id, b"echo SESSION_2_ECHO\r\n")
        .expect("write s2");

    let initial1 = manager.get_recent_output(s1.id).unwrap_or_default();
    let (found1, out1) =
        wait_for_marker(&mut rx1, initial1, "SESSION_1_ECHO", Duration::from_secs(5)).await;
    assert!(
        found1,
        "Session 1 must receive SESSION_1_ECHO. Got: {}",
        out1
    );
    assert!(
        !out1.contains("SESSION_2_ECHO"),
        "Session 1 output stream must not contain Session 2 data"
    );

    let initial2 = manager.get_recent_output(s2.id).unwrap_or_default();
    let (found2, out2) =
        wait_for_marker(&mut rx2, initial2, "SESSION_2_ECHO", Duration::from_secs(5)).await;
    assert!(
        found2,
        "Session 2 must receive SESSION_2_ECHO. Got: {}",
        out2
    );
    assert!(
        !out2.contains("SESSION_1_ECHO"),
        "Session 2 output stream must not contain Session 1 data"
    );

    // 4. Resize Session 1; assert Session 2 actual dimensions remain strictly unchanged
    manager.resize_terminal(s1.id, 120, 40).expect("resize s1");

    assert_eq!(
        manager.get_terminal_dimensions(s1.id).expect("get s1 dim"),
        (120, 40),
        "Session 1 dimensions must be 120x40"
    );
    assert_eq!(
        manager.get_terminal_dimensions(s2.id).expect("get s2 dim"),
        (100, 30),
        "Session 2 dimensions must remain 100x30 and must not be altered by Session 1 resize"
    );

    // 5. Close Session 1; assert Session 2 remains alive and responsive
    manager.close_session(s1.id).expect("close s1");
    guard.sessions.retain(|&id| id != s1.id);

    // Verify Session 2 is still responsive
    manager
        .write_input(s2.id, b"echo SESSION_2_POST_CLOSE_S1\r\n")
        .expect("write s2 after s1 closed");
    let (found2_post, out2_post) = wait_for_marker(
        &mut rx2,
        Vec::new(),
        "SESSION_2_POST_CLOSE_S1",
        Duration::from_secs(5),
    )
    .await;
    assert!(
        found2_post,
        "Session 2 must remain responsive after Session 1 closed. Got: {}",
        out2_post
    );

    // 6. Close Session 2 and verify both processes are terminated without orphans
    manager.close_session(s2.id).expect("close s2");
    guard.sessions.retain(|&id| id != s2.id);

    // Allow JobObject / process teardown to settle
    tokio::time::sleep(Duration::from_millis(300)).await;

    // Check Windows process exit status with rigorous distinction:
    // Distinguish ERROR_INVALID_PARAMETER (PID no longer exists) from query failures (e.g. ERROR_ACCESS_DENIED)
    extern "system" {
        fn OpenProcess(
            dwDesiredAccess: u32,
            bInheritHandle: i32,
            dwProcessId: u32,
        ) -> *mut std::ffi::c_void;
        fn GetExitCodeProcess(hProcess: *mut std::ffi::c_void, lpExitCode: *mut u32) -> i32;
        fn WaitForSingleObject(hHandle: *mut std::ffi::c_void, dwMilliseconds: u32) -> u32;
        fn CloseHandle(hObject: *mut std::ffi::c_void) -> i32;
        fn GetLastError() -> u32;
    }

    const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;
    const SYNCHRONIZE: u32 = 0x00100000;
    const STILL_ACTIVE: u32 = 259;
    const WAIT_OBJECT_0: u32 = 0;
    const WAIT_TIMEOUT: u32 = 0x00000102;
    const ERROR_INVALID_PARAMETER: u32 = 87;

    let verify_process_terminated = |pid: u32, timeout_ms: u32| -> Result<bool, String> {
        unsafe {
            let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION | SYNCHRONIZE, 0, pid);
            if handle.is_null() {
                let err = GetLastError();
                if err == ERROR_INVALID_PARAMETER {
                    // PID does not exist in kernel process table: confirmed dead
                    return Ok(true);
                }
                if err == 5 {
                    // ERROR_ACCESS_DENIED: query permission error, NOT proof of termination
                    return Err(format!(
                        "Query failed for PID {}: Access Denied (error 5). Process exists but cannot be queried.",
                        pid
                    ));
                }
                return Err(format!(
                    "OpenProcess failed unexpectedly for PID {} with Win32 error {}",
                    pid, err
                ));
            }

            let wait_res = WaitForSingleObject(handle, timeout_ms);
            let mut exit_code: u32 = 0;
            let ok = GetExitCodeProcess(handle, &mut exit_code);
            CloseHandle(handle);

            if ok == 0 {
                let err = GetLastError();
                return Err(format!(
                    "GetExitCodeProcess failed for PID {} with Win32 error {}",
                    pid, err
                ));
            }

            if wait_res == WAIT_OBJECT_0 || exit_code != STILL_ACTIVE {
                Ok(true)
            } else if wait_res == WAIT_TIMEOUT {
                Ok(false)
            } else {
                Err(format!(
                    "WaitForSingleObject returned unexpected status 0x{:X} for PID {}",
                    wait_res, pid
                ))
            }
        }
    };

    let res1 = verify_process_terminated(pid1, 3000);
    assert!(
        res1.is_ok(),
        "Session 1 process termination query failed: {:?}",
        res1.err()
    );
    assert!(
        res1.unwrap(),
        "Session 1 child process {} must be confirmed terminated",
        pid1
    );

    let res2 = verify_process_terminated(pid2, 3000);
    assert!(
        res2.is_ok(),
        "Session 2 process termination query failed: {:?}",
        res2.err()
    );
    assert!(
        res2.unwrap(),
        "Session 2 child process {} must be confirmed terminated",
        pid2
    );
}
