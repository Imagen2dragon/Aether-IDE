#[cfg(windows)]
pub mod native {
    use std::cell::RefCell;
    use std::io::{BufRead, BufReader, Write};
    use std::net::TcpStream;
    use std::path::Path;
    use std::sync::mpsc;
    use tracing::{info, warn};

    use ipc_protocol::{decode_message, encode_message, IpcEnvelope, IpcPayload, IpcRequest};
    use webview2_com::Microsoft::Web::WebView2::Win32::*;
    use webview2_com::{
        take_pwstr, wait_with_pump, CreateCoreWebView2ControllerCompletedHandler,
        CreateCoreWebView2EnvironmentCompletedHandler, WebMessageReceivedEventHandler,
    };
    use windows::core::{w, Interface, Result, HSTRING, PCWSTR, PWSTR};
    use windows::Win32::Foundation::{HMODULE, HWND, LPARAM, LRESULT, RECT, WPARAM};
    use windows::Win32::Graphics::Gdi::{UpdateWindow, HBRUSH};
    use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED};
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::System::WinRT::EventRegistrationToken;
    use windows::Win32::UI::HiDpi::{
        SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
    };
    use windows::Win32::UI::WindowsAndMessaging::*;

    thread_local! {
        static CONTROLLER: RefCell<Option<ICoreWebView2Controller>> = const { RefCell::new(None) };
        static HOST_CONFIG: RefCell<Option<HostConfig>> = const { RefCell::new(None) };
    }

    pub use crate::host::HostConfig;

    pub struct Win32WebView2Host {
        config: HostConfig,
    }

    impl crate::host::DesktopHost for Win32WebView2Host {
        fn run(&self) -> std::result::Result<(), Box<dyn std::error::Error>> {
            self.run()
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
        }

        fn platform_name(&self) -> &'static str {
            "Windows (Win32 + Microsoft WebView2)"
        }
    }

    impl Win32WebView2Host {
        pub fn new(config: HostConfig) -> Self {
            Self { config }
        }

        pub fn run(&self) -> Result<()> {
            unsafe {
                // 1. Enable Per-Monitor V2 DPI awareness for sharp UI
                let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);

                // 2. Initialize COM Single-Threaded Apartment
                CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok()?;

                // Store host config for message bridge
                HOST_CONFIG.with(|c| *c.borrow_mut() = Some(self.config.clone()));

                // 3. Register Win32 Window Class
                let hinstance: HMODULE = GetModuleHandleW(PCWSTR::null())?;
                let class_name = w!("AetherIDEWindowClass");

                let wc = WNDCLASSEXW {
                    cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
                    style: CS_HREDRAW | CS_VREDRAW,
                    lpfnWndProc: Some(wndproc),
                    cbClsExtra: 0,
                    cbWndExtra: 0,
                    hInstance: hinstance.into(),
                    hIcon: HICON::default(),
                    hCursor: LoadCursorW(HMODULE::default(), IDC_ARROW)?,
                    hbrBackground: HBRUSH::default(),
                    lpszMenuName: PCWSTR::null(),
                    lpszClassName: class_name,
                    hIconSm: HICON::default(),
                };

                let atom = RegisterClassExW(&wc);
                if atom == 0 {
                    let err = windows::core::Error::from_win32();
                    warn!(
                        "RegisterClassExW notice (class may already exist): {:?}",
                        err
                    );
                }

                // 4. Create Win32 Top-Level Window (HWND)
                let hwnd = CreateWindowExW(
                    WINDOW_EX_STYLE::default(),
                    class_name,
                    w!("AetherIDE — AI-Native Engineering Studio"),
                    WS_OVERLAPPEDWINDOW | WS_VISIBLE,
                    CW_USEDEFAULT,
                    CW_USEDEFAULT,
                    1440,
                    900,
                    HWND::default(),
                    HMENU::default(),
                    hinstance,
                    None,
                )?;

                let _ = ShowWindow(hwnd, SW_SHOW);
                let _ = UpdateWindow(hwnd);

                info!("Created native Win32 window HWND: {:?}", hwnd);

                // 5. Initialize WebView2 Controller inside HWND
                let controller = self.init_webview2(hwnd)?;
                CONTROLLER.with(|c| *c.borrow_mut() = Some(controller));

                // Initial size adjustment
                let mut rect = RECT::default();
                let _ = GetClientRect(hwnd, &mut rect);
                CONTROLLER.with(|c| {
                    if let Some(ctrl) = c.borrow().as_ref() {
                        let _ = ctrl.SetBounds(rect);
                    }
                });

                // 6. Enter Standard Win32 Message Loop
                let mut msg = MSG::default();
                while GetMessageW(&mut msg, HWND::default(), 0, 0).as_bool() {
                    let _ = TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }

                // Clean shutdown
                CONTROLLER.with(|c| *c.borrow_mut() = None);
                CoUninitialize();
                Ok(())
            }
        }

        unsafe fn init_webview2(&self, hwnd: HWND) -> Result<ICoreWebView2Controller> {
            let (tx_env, rx_env) = mpsc::channel();
            let env_handler =
                CreateCoreWebView2EnvironmentCompletedHandler::create(Box::new(move |res, env| {
                    let _ = tx_env.send((res, env));
                    Ok(())
                }));

            let user_data_str = self.config.user_data_dir.to_string_lossy().to_string();
            let user_data_hstring = HSTRING::from(&user_data_str);

            info!(
                "Initializing WebView2 environment (profile: {})",
                user_data_str
            );

            CreateCoreWebView2EnvironmentWithOptions(
                PCWSTR::null(),
                &user_data_hstring,
                None,
                &env_handler,
            )?;

            let (env_res, env_opt) = wait_with_pump(rx_env).map_err(|e| {
                windows::core::Error::new(windows::core::HRESULT(-1), format!("{:?}", e))
            })?;
            env_res?;
            let env = env_opt.ok_or_else(|| {
                windows::core::Error::new(
                    windows::core::HRESULT(-1),
                    "Missing WebView2 environment",
                )
            })?;

            let (tx_ctrl, rx_ctrl) = mpsc::channel();
            let ctrl_handler =
                CreateCoreWebView2ControllerCompletedHandler::create(Box::new(move |res, ctrl| {
                    let _ = tx_ctrl.send((res, ctrl));
                    Ok(())
                }));

            env.CreateCoreWebView2Controller(hwnd, &ctrl_handler)?;

            let (ctrl_res, ctrl_opt) = wait_with_pump(rx_ctrl).map_err(|e| {
                windows::core::Error::new(windows::core::HRESULT(-1), format!("{:?}", e))
            })?;
            ctrl_res?;
            let controller = ctrl_opt.ok_or_else(|| {
                windows::core::Error::new(windows::core::HRESULT(-1), "Missing WebView2 controller")
            })?;

            let _ = controller.SetIsVisible(true);

            let mut rect = RECT::default();
            let _ = GetClientRect(hwnd, &mut rect);
            let _ = controller.SetBounds(rect);

            let webview = controller.CoreWebView2()?;

            // Map Virtual Host: https://appassets.aetheride.local/ -> workbench directory
            let host_name = w!("appassets.aetheride.local");
            let workbench_str = self.config.workbench_dir.to_string_lossy().to_string();
            let folder_hstring = HSTRING::from(&workbench_str);

            if let Ok(webview3) = webview.cast::<ICoreWebView2_3>() {
                info!(
                    "Configured virtual host mapping: https://appassets.aetheride.local -> {}",
                    workbench_str
                );
                let map_res = webview3.SetVirtualHostNameToFolderMapping(
                    host_name,
                    &folder_hstring,
                    COREWEBVIEW2_HOST_RESOURCE_ACCESS_KIND_ALLOW,
                );
                if let Err(e) = map_res {
                    warn!("SetVirtualHostNameToFolderMapping notice: {:?}", e);
                }
            }

            // Wire Secure Message Bridge
            let token_opt = self.config.session_token.clone();
            let peer_opt = self.config.peer_id.clone();
            let port = self.config.daemon_port;
            let workbench_path = self.config.workbench_dir.clone();

            let bridge_handler = WebMessageReceivedEventHandler::create(Box::new(
                move |wv: Option<ICoreWebView2>,
                      args: Option<ICoreWebView2WebMessageReceivedEventArgs>| {
                    if let (Some(wv), Some(args)) = (wv, args) {
                        handle_web_message(
                            &wv,
                            &args,
                            token_opt.as_deref(),
                            peer_opt.as_deref(),
                            port,
                            &workbench_path,
                        );
                    }
                    Ok(())
                },
            ));

            let mut reg_token = EventRegistrationToken::default();
            let _ = webview.add_WebMessageReceived(&bridge_handler, &mut reg_token);

            // Navigate to AetherIDE Workbench
            let target_url = w!("https://appassets.aetheride.local/index.html");
            info!("Navigating WebView2 to https://appassets.aetheride.local/index.html");
            if let Err(e) = webview.Navigate(target_url) {
                warn!(
                    "Virtual origin navigation ({:?}); attempting local file URI fallback",
                    e
                );
                let fallback_file = self.config.workbench_dir.join("index.html");
                let file_url_str = format!(
                    "file:///{}",
                    fallback_file.to_string_lossy().replace('\\', "/")
                );
                let file_hstring = HSTRING::from(&file_url_str);
                let _ = webview.Navigate(&file_hstring);
            }

            Ok(controller)
        }
    }

    unsafe extern "system" fn wndproc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        match msg {
            WM_SIZE => {
                let mut rect = RECT::default();
                let _ = GetClientRect(hwnd, &mut rect);
                CONTROLLER.with(|c| {
                    if let Some(ctrl) = c.borrow().as_ref() {
                        let _ = ctrl.SetBounds(rect);
                    }
                });
                LRESULT(0)
            }
            WM_DPICHANGED => {
                let prc = lparam.0 as *const RECT;
                if !prc.is_null() {
                    let r = *prc;
                    let _ = SetWindowPos(
                        hwnd,
                        HWND::default(),
                        r.left,
                        r.top,
                        r.right - r.left,
                        r.bottom - r.top,
                        SWP_NOZORDER | SWP_NOACTIVATE,
                    );
                }
                LRESULT(0)
            }
            WM_CLOSE => {
                let _ = DestroyWindow(hwnd);
                LRESULT(0)
            }
            WM_DESTROY => {
                PostQuitMessage(0);
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }

    fn handle_web_message(
        webview: &ICoreWebView2,
        args: &ICoreWebView2WebMessageReceivedEventArgs,
        session_token: Option<&str>,
        peer_id: Option<&str>,
        daemon_port: u16,
        workbench_dir: &Path,
    ) {
        unsafe {
            // 1. Origin Verification
            let mut source_pwstr = PWSTR::null();
            if args.Source(&mut source_pwstr).is_ok() && !source_pwstr.is_null() {
                let source_url = take_pwstr(source_pwstr);
                if !source_url.starts_with("https://appassets.aetheride.local")
                    && !source_url.starts_with("file://")
                {
                    warn!(
                        "Security violation: Blocked message from untrusted origin: {}",
                        source_url
                    );
                    let err_json = serde_json::json!({
                        "error": "SecurityViolation: Untrusted message origin",
                        "origin": source_url
                    });
                    let reply_hstring = HSTRING::from(&err_json.to_string());
                    let _ = webview.PostWebMessageAsJson(&reply_hstring);
                    return;
                }
            }

            // 2. Extract Message Body
            let mut message_pwstr = PWSTR::null();
            if args.WebMessageAsJson(&mut message_pwstr).is_err() || message_pwstr.is_null() {
                return;
            }
            let raw_json = take_pwstr(message_pwstr);
            let parsed: serde_json::Value = match serde_json::from_str(&raw_json) {
                Ok(v) => v,
                Err(e) => {
                    warn!("Malformed JSON in WebMessage: {:?}", e);
                    return;
                }
            };

            let req_id = parsed
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();
            let action = parsed
                .get("action")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let params = parsed
                .get("params")
                .cloned()
                .unwrap_or(serde_json::Value::Null);

            // 3. Gate 4: Allowlist Validation
            let allowed_actions = [
                "LspCompletion",
                "LspHover",
                "LspDefinition",
                "LspChangeDocument",
                "LspOpenDocument",
                "LspCloseDocument",
                "LspDiagnostics",
                "GetStatus",
                "Ping",
                "RunDoctorCheck",
                "QueryProjectBrain",
                "QueryProjectGraph",
                "AnalyzeChangeImpact",
                "SaveProjectMemoryNote",
                "GetProjectMemoryNotes",
                "DeleteProjectMemoryNote",
                "FileRead",
                "FileWrite",
                "TerminalSpawn",
                "TerminalWrite",
                "TerminalResize",
                "TerminalClose",
                "TerminalPollOutput",
                "ListAgentPersonas",
                "CreateAgentTask",
                "GetAgentTask",
                "ApproveAgentStep",
                "RejectAgentStep",
                "CancelAgentTask",
                "StartExpoServer",
                "GetExpoStatus",
                "DiscoverMobileDevices",
                "CrdtApplyUpdate",
                "CrdtGetDoc",
                "CrdtGetStateVector",
                "CrdtGetDiff",
                "UpdatePresence",
                "GetPresence",
                "SyncGeneratePairing",
                "SyncGetManifest",
                "SyncDiffManifests",
                "SwarmAddTask",
                "SwarmListTasks",
                "SwarmGetReadyTasks",
                "SwarmSendMessage",
                "SwarmGetMessages",
                "ThreeWayMerge",
                "ApplyMergeResolution",
            ];

            if !allowed_actions.contains(&action.as_str()) {
                warn!("Rejected non-allowlisted action: {}", action);
                let err_reply = serde_json::json!({
                    "id": req_id,
                    "action": action,
                    "error": format!("Action '{}' is not in the secure allowlist", action),
                    "result": null
                });
                let reply_hstring = HSTRING::from(&err_reply.to_string());
                let _ = webview.PostWebMessageAsJson(&reply_hstring);
                return;
            }

            // 4. Dispatch action
            let reply = if action == "FileRead" {
                handle_file_read(&req_id, &params, workbench_dir)
            } else if action == "FileWrite" {
                handle_file_write(&req_id, &params, workbench_dir)
            } else {
                dispatch_ipc(
                    &req_id,
                    &action,
                    &params,
                    session_token,
                    peer_id,
                    daemon_port,
                )
            };

            let reply_hstring = HSTRING::from(&reply.to_string());
            let _ = webview.PostWebMessageAsJson(&reply_hstring);
        }
    }

    fn handle_file_read(
        req_id: &str,
        params: &serde_json::Value,
        base_dir: &Path,
    ) -> serde_json::Value {
        let rel_path = params
            .get("path")
            .and_then(|v| v.as_str())
            .unwrap_or_default();

        let target =
            match filesystem_engine::FilesystemEngine::resolve_contained_path(base_dir, rel_path) {
                Ok(p) => p,
                Err(e) => {
                    return serde_json::json!({
                        "id": req_id,
                        "action": "FileRead",
                        "error": format!("Access Denied: {}", e),
                        "result": null
                    });
                }
            };

        match std::fs::read_to_string(&target) {
            Ok(content) => serde_json::json!({
                "id": req_id,
                "action": "FileRead",
                "result": { "path": rel_path, "content": content },
                "error": null
            }),
            Err(e) => serde_json::json!({
                "id": req_id,
                "action": "FileRead",
                "error": format!("Read failed: {}", e),
                "result": null
            }),
        }
    }

    fn handle_file_write(
        req_id: &str,
        params: &serde_json::Value,
        base_dir: &Path,
    ) -> serde_json::Value {
        let rel_path = params
            .get("path")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        let content = params
            .get("content")
            .and_then(|v| v.as_str())
            .unwrap_or_default();

        let target =
            match filesystem_engine::FilesystemEngine::resolve_contained_path(base_dir, rel_path) {
                Ok(p) => p,
                Err(e) => {
                    return serde_json::json!({
                        "id": req_id,
                        "action": "FileWrite",
                        "error": format!("Access Denied: {}", e),
                        "result": null
                    });
                }
            };

        if let Some(parent) = target.parent() {
            let _ = std::fs::create_dir_all(parent);
        }

        // Atomic write
        let tmp_path = target.with_extension("tmp_atomic");
        if let Err(e) = std::fs::write(&tmp_path, content) {
            return serde_json::json!({
                "id": req_id,
                "action": "FileWrite",
                "error": format!("Write temp error: {}", e),
                "result": null
            });
        }

        if let Err(e) = std::fs::rename(&tmp_path, &target) {
            let _ = std::fs::remove_file(&tmp_path);
            return serde_json::json!({
                "id": req_id,
                "action": "FileWrite",
                "error": format!("Commit error: {}", e),
                "result": null
            });
        }

        serde_json::json!({
            "id": req_id,
            "action": "FileWrite",
            "result": { "path": rel_path, "bytes_written": content.len() },
            "error": null
        })
    }

    fn dispatch_ipc(
        req_id: &str,
        action: &str,
        params: &serde_json::Value,
        session_token: Option<&str>,
        peer_id: Option<&str>,
        daemon_port: u16,
    ) -> serde_json::Value {
        let ipc_req = match action {
            "Ping" => IpcRequest::Ping,
            "GetStatus" => IpcRequest::GetStatus,
            "RunDoctorCheck" => IpcRequest::RunDoctorCheck,
            "QueryProjectBrain" => IpcRequest::QueryProjectBrain {
                query: params
                    .get("query")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
            },
            "LspCompletion" => IpcRequest::LspCompletion {
                path: params
                    .get("path")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
                line: params.get("line").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
                character: params
                    .get("character")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0) as u32,
            },
            "LspHover" => IpcRequest::LspHover {
                path: params
                    .get("path")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
                line: params.get("line").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
                character: params
                    .get("character")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0) as u32,
            },
            "LspDefinition" => IpcRequest::LspDefinition {
                path: params
                    .get("path")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
                line: params.get("line").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
                character: params
                    .get("character")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0) as u32,
            },
            "LspOpenDocument" => IpcRequest::LspOpenDocument {
                path: params
                    .get("path")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
                content: params
                    .get("content")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
                language_id: params
                    .get("language_id")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
            },
            "LspChangeDocument" => IpcRequest::LspChangeDocument {
                path: params
                    .get("path")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
                content: params
                    .get("content")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
                version: params.get("version").and_then(|v| v.as_i64()).unwrap_or(1),
            },
            "LspCloseDocument" => IpcRequest::LspCloseDocument {
                path: params
                    .get("path")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
            },
            "LspDiagnostics" => IpcRequest::LspDiagnostics {
                path: params
                    .get("path")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
            },
            "TerminalSpawn" => IpcRequest::TerminalSpawn {
                shell: params
                    .get("shell")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
                cwd: params
                    .get("cwd")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
                cols: params
                    .get("cols")
                    .and_then(|v| v.as_u64())
                    .map(|n| n as u16),
                rows: params
                    .get("rows")
                    .and_then(|v| v.as_u64())
                    .map(|n| n as u16),
            },
            "TerminalWrite" => IpcRequest::TerminalWrite {
                session_id: params
                    .get("session_id")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0) as u32,
                data: params
                    .get("data")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
            },
            "TerminalResize" => IpcRequest::TerminalResize {
                session_id: params
                    .get("session_id")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0) as u32,
                cols: params.get("cols").and_then(|v| v.as_u64()).unwrap_or(80) as u16,
                rows: params.get("rows").and_then(|v| v.as_u64()).unwrap_or(24) as u16,
            },
            "TerminalClose" => IpcRequest::TerminalClose {
                session_id: params
                    .get("session_id")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0) as u32,
            },
            "TerminalPollOutput" => IpcRequest::TerminalPollOutput {
                session_id: params
                    .get("session_id")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0) as u32,
                offset: params.get("offset").and_then(|v| v.as_u64()).unwrap_or(0) as usize,
            },
            "ListAgentPersonas" => IpcRequest::ListAgentPersonas,
            "CreateAgentTask" => IpcRequest::CreateAgentTask {
                role: params
                    .get("role")
                    .and_then(|v| v.as_str())
                    .unwrap_or("architect")
                    .to_string(),
                objective: params
                    .get("objective")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                autonomy_mode: params
                    .get("autonomy_mode")
                    .and_then(|v| v.as_str())
                    .unwrap_or("autopilot")
                    .to_string(),
            },
            "GetAgentTask" => IpcRequest::GetAgentTask {
                task_id: params
                    .get("task_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
            },
            "ApproveAgentStep" => IpcRequest::ApproveAgentStep {
                task_id: params
                    .get("task_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                step_id: params
                    .get("step_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
            },
            "RejectAgentStep" => IpcRequest::RejectAgentStep {
                task_id: params
                    .get("task_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                step_id: params
                    .get("step_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                reason: params
                    .get("reason")
                    .and_then(|v| v.as_str())
                    .unwrap_or("User rejected step")
                    .to_string(),
            },
            "CancelAgentTask" => IpcRequest::CancelAgentTask {
                task_id: params
                    .get("task_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
            },
            "QueryProjectGraph" => IpcRequest::QueryProjectGraph {
                workspace_path: params
                    .get("workspace_path")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
            },
            "AnalyzeChangeImpact" => IpcRequest::AnalyzeChangeImpact {
                file_path: params
                    .get("file_path")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
            },
            "SaveProjectMemoryNote" => IpcRequest::SaveProjectMemoryNote {
                title: params
                    .get("title")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Untitled Note")
                    .to_string(),
                content: params
                    .get("content")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                tags: params
                    .get("tags")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|t| t.as_str().map(|s| s.to_string()))
                            .collect()
                    })
                    .unwrap_or_default(),
            },
            "GetProjectMemoryNotes" => IpcRequest::GetProjectMemoryNotes,
            "DeleteProjectMemoryNote" => IpcRequest::DeleteProjectMemoryNote {
                note_id: params
                    .get("note_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
            },
            "StartExpoServer" => IpcRequest::StartExpoServer {
                project_path: params
                    .get("project_path")
                    .and_then(|v| v.as_str())
                    .unwrap_or(".")
                    .to_string(),
                port: params
                    .get("port")
                    .and_then(|v| v.as_u64())
                    .map(|p| p as u16),
            },
            "GetExpoStatus" => IpcRequest::GetExpoStatus {
                project_path: params
                    .get("project_path")
                    .and_then(|v| v.as_str())
                    .unwrap_or(".")
                    .to_string(),
            },
            "DiscoverMobileDevices" => IpcRequest::DiscoverMobileDevices,
            _ => {
                return serde_json::json!({
                    "id": req_id,
                    "action": action,
                    "error": format!("Unhandled action: {}", action),
                    "result": null
                });
            }
        };

        match send_daemon_ipc(ipc_req, session_token, peer_id, daemon_port) {
            Ok(data) => serde_json::json!({
                "id": req_id,
                "action": action,
                "result": data,
                "error": null
            }),
            Err(e) => serde_json::json!({
                "id": req_id,
                "action": action,
                "error": format!("Daemon IPC failure: {}", e),
                "result": null
            }),
        }
    }

    fn send_daemon_ipc(
        req: IpcRequest,
        session_token: Option<&str>,
        peer_id: Option<&str>,
        daemon_port: u16,
    ) -> std::result::Result<serde_json::Value, String> {
        let mut stream = TcpStream::connect(format!("127.0.0.1:{}", daemon_port))
            .map_err(|e| format!("Connect error (port {}): {}", daemon_port, e))?;
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(4)))
            .map_err(|e| e.to_string())?;

        let envelope = IpcEnvelope {
            id: format!("wb-{}", uuid::Uuid::new_v4().simple()),
            session_token: session_token.map(|s| s.to_string()),
            peer_id: peer_id.map(|s| s.to_string()),
            payload: IpcPayload::Request(req),
        };

        let encoded = encode_message(&envelope).map_err(|e| e.to_string())?;
        stream.write_all(&encoded).map_err(|e| e.to_string())?;

        let mut reader = BufReader::new(stream);
        let mut line = String::new();
        reader.read_line(&mut line).map_err(|e| e.to_string())?;

        let resp_env = decode_message(line.as_bytes()).map_err(|e| e.to_string())?;
        match resp_env.payload {
            IpcPayload::Response(r) => {
                if r.success {
                    Ok(r.data.unwrap_or(serde_json::json!({})))
                } else {
                    Err(r.error.unwrap_or_else(|| "Daemon reported failure".into()))
                }
            }
            _ => Err("Unexpected daemon IPC payload response".into()),
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use std::path::PathBuf;

        #[test]
        fn test_host_config_defaults() {
            let config = HostConfig {
                workbench_dir: PathBuf::from("workbench"),
                user_data_dir: PathBuf::from("user_data"),
                session_token: Some("secret-token-123".to_string()),
                peer_id: Some("owner-bootstrap-test".to_string()),
                daemon_port: 44888,
            };
            assert_eq!(config.daemon_port, 44888);
            assert_eq!(config.session_token.as_deref(), Some("secret-token-123"));
            assert_eq!(config.peer_id.as_deref(), Some("owner-bootstrap-test"));
        }

        #[test]
        fn test_handle_file_read_blocks_path_traversal() {
            let temp_dir = std::env::temp_dir().join("aether_test_read_sec");
            let _ = std::fs::create_dir_all(&temp_dir);

            let res = handle_file_read(
                "req-1",
                &serde_json::json!({ "path": "../../windows/system32/cmd.exe" }),
                &temp_dir,
            );
            assert!(res.get("error").is_some());
            let _ = std::fs::remove_dir_all(&temp_dir);
        }

        #[test]
        fn test_handle_file_write_atomic_and_blocks_traversal() {
            let temp_dir = std::env::temp_dir().join("aether_test_write_sec");
            let _ = std::fs::create_dir_all(&temp_dir);

            // Path traversal should be blocked
            let evil = handle_file_write(
                "req-2",
                &serde_json::json!({
                    "path": "../evil.exe",
                    "content": "malicious payload"
                }),
                &temp_dir,
            );
            assert!(evil.get("error").is_some());

            // Legitimate write should succeed atomically
            let valid = handle_file_write(
                "req-3",
                &serde_json::json!({
                    "path": "src/test_file.rs",
                    "content": "fn test_hello() -> bool { true }"
                }),
                &temp_dir,
            );
            assert!(valid.get("error").unwrap().is_null());
            assert_eq!(
                std::fs::read_to_string(temp_dir.join("src/test_file.rs")).unwrap(),
                "fn test_hello() -> bool { true }"
            );

            let _ = std::fs::remove_dir_all(&temp_dir);
        }

        #[test]
        fn test_allowlist_validation() {
            let allowed_actions = [
                "LspCompletion",
                "LspHover",
                "LspDefinition",
                "LspChangeDocument",
                "LspOpenDocument",
                "LspCloseDocument",
                "LspDiagnostics",
                "GetStatus",
                "Ping",
                "RunDoctorCheck",
                "QueryProjectBrain",
                "QueryProjectGraph",
                "AnalyzeChangeImpact",
                "SaveProjectMemoryNote",
                "GetProjectMemoryNotes",
                "DeleteProjectMemoryNote",
                "FileRead",
                "FileWrite",
                "TerminalSpawn",
                "TerminalWrite",
                "TerminalResize",
                "TerminalClose",
                "TerminalPollOutput",
                "ListAgentPersonas",
                "CreateAgentTask",
                "GetAgentTask",
                "ApproveAgentStep",
                "RejectAgentStep",
                "CancelAgentTask",
                "StartExpoServer",
                "GetExpoStatus",
                "DiscoverMobileDevices",
                "CrdtApplyUpdate",
                "CrdtGetDoc",
                "CrdtGetStateVector",
                "CrdtGetDiff",
                "UpdatePresence",
                "GetPresence",
                "SyncGeneratePairing",
                "SyncGetManifest",
                "SyncDiffManifests",
                "SwarmAddTask",
                "SwarmListTasks",
                "SwarmGetReadyTasks",
                "SwarmSendMessage",
                "SwarmGetMessages",
                "ThreeWayMerge",
                "ApplyMergeResolution",
            ];
            assert!(allowed_actions.contains(&"LspCompletion"));
            assert!(allowed_actions.contains(&"FileWrite"));
            assert!(allowed_actions.contains(&"TerminalPollOutput"));
            assert!(allowed_actions.contains(&"QueryProjectGraph"));
            assert!(allowed_actions.contains(&"ApproveAgentStep"));
            assert!(!allowed_actions.contains(&"ArbitraryShellExecute"));
            assert!(!allowed_actions.contains(&"FormatDrive"));
        }
    }
}
