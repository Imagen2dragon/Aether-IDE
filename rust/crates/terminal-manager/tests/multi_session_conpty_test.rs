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

    // 4. Resize Session 1; assert Session 2 remains unaffected and responsive
    manager.resize_terminal(s1.id, 120, 40).expect("resize s1");

    // 5. Close Session 1; assert Session 2 remains alive and responsive
    manager.close_session(s1.id).expect("close s1");

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

    // Allow JobObject / process teardown to settle
    tokio::time::sleep(Duration::from_millis(300)).await;

    // Check Windows process exit status
    extern "system" {
        fn OpenProcess(
            dwDesiredAccess: u32,
            bInheritHandle: i32,
            dwProcessId: u32,
        ) -> *mut std::ffi::c_void;
        fn GetExitCodeProcess(hProcess: *mut std::ffi::c_void, lpExitCode: *mut u32) -> i32;
        fn CloseHandle(hObject: *mut std::ffi::c_void) -> i32;
    }

    const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;
    const STILL_ACTIVE: u32 = 259;

    let check_process_dead = |pid: u32| -> bool {
        unsafe {
            let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            if handle.is_null() {
                // Process already terminated and handle table entry cleaned up
                return true;
            }
            let mut exit_code: u32 = 0;
            let ok = GetExitCodeProcess(handle, &mut exit_code);
            CloseHandle(handle);
            if ok != 0 {
                exit_code != STILL_ACTIVE
            } else {
                true
            }
        }
    };

    assert!(
        check_process_dead(pid1),
        "Session 1 process {} must be terminated",
        pid1
    );
    assert!(
        check_process_dead(pid2),
        "Session 2 process {} must be terminated",
        pid2
    );
}
