#[cfg(windows)]
#[tokio::test]
async fn test_multi_session_conpty_lifecycle() {
    use std::time::Duration;
    use terminal_manager::TerminalManager;

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

    // Acquire and retain real Win32 process handles IMMEDIATELY while child processes are alive.
    // Holding open kernel handles guarantees:
    // 1. The Win32 process objects cannot be recycled or destroyed until handles are closed.
    // 2. Waiting on these handles via WaitForSingleObject is 100% identity-bound to these specific child instances.
    let h_proc1 = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION | SYNCHRONIZE, 0, pid1) };
    assert!(
        !h_proc1.is_null(),
        "Must acquire identity-bound handle to Session 1 process. Win32 error: {}",
        unsafe { GetLastError() }
    );

    let h_proc2 = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION | SYNCHRONIZE, 0, pid2) };
    assert!(
        !h_proc2.is_null(),
        "Must acquire identity-bound handle to Session 2 process. Win32 error: {}",
        unsafe { GetLastError() }
    );

    // RAII guard: ensure cleanup runs even if an assertion fails later in the test
    struct SessionCleanupGuard<'a> {
        mgr: &'a TerminalManager,
        sessions: Vec<u32>,
        handles: Vec<*mut std::ffi::c_void>,
    }
    impl<'a> Drop for SessionCleanupGuard<'a> {
        fn drop(&mut self) {
            for &id in &self.sessions {
                let _ = self.mgr.close_session(id);
            }
            for &h in &self.handles {
                if !h.is_null() {
                    unsafe {
                        CloseHandle(h);
                    }
                }
            }
        }
    }
    let mut guard = SessionCleanupGuard {
        mgr: &manager,
        sessions: vec![s1.id, s2.id],
        handles: vec![h_proc1, h_proc2],
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

    // Verify Session 1 process termination on the retained handle
    let wait_res1 = unsafe { WaitForSingleObject(h_proc1, 5000) };
    assert_ne!(
        wait_res1, WAIT_TIMEOUT,
        "Session 1 process timed out waiting for termination"
    );
    assert_eq!(
        wait_res1, WAIT_OBJECT_0,
        "Session 1 process must transition to signaled (terminated) within 5s"
    );
    let mut exit_code1: u32 = 0;
    let ok1 = unsafe { GetExitCodeProcess(h_proc1, &mut exit_code1) };
    assert_ne!(
        ok1,
        0,
        "GetExitCodeProcess failed for Session 1 with error {}",
        unsafe { GetLastError() }
    );
    assert_ne!(
        exit_code1, STILL_ACTIVE,
        "Session 1 process must not be STILL_ACTIVE after close"
    );

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

    // 6. Close Session 2 and verify its termination on its retained handle
    manager.close_session(s2.id).expect("close s2");
    guard.sessions.retain(|&id| id != s2.id);

    let wait_res2 = unsafe { WaitForSingleObject(h_proc2, 5000) };
    assert_ne!(
        wait_res2, WAIT_TIMEOUT,
        "Session 2 process timed out waiting for termination"
    );
    assert_eq!(
        wait_res2, WAIT_OBJECT_0,
        "Session 2 process must transition to signaled (terminated) within 5s"
    );
    let mut exit_code2: u32 = 0;
    let ok2 = unsafe { GetExitCodeProcess(h_proc2, &mut exit_code2) };
    assert_ne!(
        ok2,
        0,
        "GetExitCodeProcess failed for Session 2 with error {}",
        unsafe { GetLastError() }
    );
    assert_ne!(
        exit_code2, STILL_ACTIVE,
        "Session 2 process must not be STILL_ACTIVE after close"
    );
}
