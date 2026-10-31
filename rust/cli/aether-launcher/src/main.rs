pub mod host;
pub mod linux_host;
pub mod macos_host;
#[cfg(windows)]
mod webview2_runtime;
#[cfg(windows)]
mod win32_host;

use std::env;
use std::net::TcpStream;
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

const CREATE_NO_WINDOW: u32 = 0x08000000;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().skip(1).collect();

    let exe_dir = env::current_exe()?
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."));

    // If subcommands or CLI flags passed, forward directly to aether-cli
    if !args.is_empty()
        && (args[0] == "doctor"
            || args[0] == "ping"
            || args[0] == "status"
            || args[0] == "index"
            || args[0] == "search"
            || args[0] == "agent"
            || args[0] == "mobile")
    {
        let cli_bin = if cfg!(windows) {
            "aether-cli.exe"
        } else {
            "aether-cli"
        };
        let cli_path = exe_dir.join("bin").join(cli_bin);
        if cli_path.exists() {
            let status = Command::new(&cli_path).args(&args).status()?;
            std::process::exit(status.code().unwrap_or(0));
        }
    }

    println!("============================================================");
    println!(" AetherIDE — Production AI-Native Software Engineering Studio");
    println!(
        " Version: v{} (Desktop Distribution)",
        application_core::AETHER_VERSION
    );
    println!("============================================================\n");

    // 1. Ensure Daemon is Running
    let is_daemon_running = TcpStream::connect("127.0.0.1:44888").is_ok();
    if is_daemon_running {
        println!(" [✓] Aether Daemon: Active on 127.0.0.1:44888");
    } else {
        println!(" [*] Starting Aether Native Daemon in background...");
        let daemon_bin = if cfg!(windows) {
            "aether-daemon.exe"
        } else {
            "aether-daemon"
        };
        let daemon_path = exe_dir.join("bin").join(daemon_bin);
        if !daemon_path.exists() {
            eprintln!("Error: aether-daemon binary not found at {:?}", daemon_path);
            return Err("Missing daemon binary".into());
        }

        let mut cmd = Command::new(&daemon_path);
        #[cfg(windows)]
        cmd.creation_flags(CREATE_NO_WINDOW);

        match cmd.spawn() {
            Ok(_) => {
                println!(" [✓] Daemon process spawned: {:?}", daemon_path);
            }
            Err(e) => {
                eprintln!("Failed to spawn daemon: {}", e);
                return Err(e.into());
            }
        }

        // Wait up to 3 seconds for daemon to accept connections
        let mut connected = false;
        for _ in 0..30 {
            std::thread::sleep(Duration::from_millis(100));
            if TcpStream::connect("127.0.0.1:44888").is_ok() {
                connected = true;
                break;
            }
        }

        if connected {
            println!(" [✓] Aether Daemon IPC listening on 127.0.0.1:44888");
        } else {
            eprintln!(
                "Warning: Daemon started but did not respond on 127.0.0.1:44888 within timeout."
            );
        }
    }

    // 2. Read Session Identity from desktop_bootstrap.json
    let data_dir = if let Ok(appdata) = env::var("LOCALAPPDATA") {
        PathBuf::from(appdata).join("AetherIDE")
    } else if let Ok(home) = env::var("HOME") {
        PathBuf::from(home).join(".aether")
    } else {
        PathBuf::from(".").join(".aether")
    };

    let bootstrap_file = data_dir.join("desktop_bootstrap.json");
    let (session_token, peer_id) = if let Ok(data) = std::fs::read_to_string(&bootstrap_file) {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&data) {
            (
                v.get("session_token")
                    .and_then(|t| t.as_str())
                    .map(|s| s.trim().to_string()),
                v.get("owner_peer_id")
                    .and_then(|p| p.as_str())
                    .map(|s| s.trim().to_string()),
            )
        } else {
            (None, None)
        }
    } else {
        (None, None)
    };

    if let Some(token) = &session_token {
        println!(
            " [✓] Authenticated Desktop Owner Session Verified ({} chars, Peer: {})",
            token.len(),
            peer_id.as_deref().unwrap_or("unknown")
        );
    } else {
        println!(" [!] No active session token found; daemon will enforce authorization.");
    }

    // 3. Launch Standalone Desktop Workbench
    println!(" [*] Launching AetherIDE Standalone Desktop Workbench...");
    let workbench_dir = exe_dir.join("workbench");
    let workbench_html = workbench_dir.join("index.html");
    let electron_exe = workbench_dir.join("electron.exe");
    let electron_main = workbench_dir.join("dist").join("main.js");

    let use_electron = args.iter().any(|a| a == "--electron");
    if use_electron && electron_exe.exists() && electron_main.exists() {
        println!(" [✓] Launching bundled Electron runtime (--electron requested)...");
        Command::new(electron_exe).arg(electron_main).spawn()?;
        return Ok(());
    }

    // Native Desktop Host execution via cross-platform DesktopHost trait
    #[cfg(windows)]
    {
        println!(" [✓] Initializing True Native Win32 + Microsoft WebView2 Embedded Host...");

        let host_config = host::HostConfig {
            workbench_dir: workbench_dir.clone(),
            user_data_dir: data_dir.clone(),
            session_token: session_token.clone(),
            peer_id: peer_id.clone(),
            daemon_port: 44888,
        };
        let host = win32_host::native::Win32WebView2Host::new(host_config);
        match host.run() {
            Ok(()) => {
                println!("\nAetherIDE session terminated cleanly.");
                return Ok(());
            }
            Err(e) => {
                eprintln!(
                    "Warning: Native Win32 WebView2 Host failed ({:?}); attempting fallback...",
                    e
                );
                if electron_exe.exists() && electron_main.exists() {
                    println!(" [✓] Attempting fallback to bundled Electron runtime...");
                    if Command::new(&electron_exe)
                        .arg(&electron_main)
                        .spawn()
                        .is_ok()
                    {
                        return Ok(());
                    }
                }
            }
        }
    }

    #[cfg(target_os = "linux")]
    {
        use host::DesktopHost;
        let host_config = host::HostConfig {
            workbench_dir: workbench_dir.clone(),
            user_data_dir: data_dir.clone(),
            session_token: session_token.clone(),
            peer_id: peer_id.clone(),
            daemon_port: 44888,
        };
        let host = linux_host::LinuxGtkHost::new(host_config);
        if let Err(e) = host.run() {
            eprintln!("Warning: Linux GTK host returned: {}", e);
        } else {
            return Ok(());
        }
    }

    #[cfg(target_os = "macos")]
    {
        use host::DesktopHost;
        let host_config = host::HostConfig {
            workbench_dir: workbench_dir.clone(),
            user_data_dir: data_dir.clone(),
            session_token: session_token.clone(),
            peer_id: peer_id.clone(),
            daemon_port: 44888,
        };
        let host = macos_host::MacWkWebViewHost::new(host_config);
        if let Err(e) = host.run() {
            eprintln!("Warning: macOS WKWebView host returned: {}", e);
        } else {
            return Ok(());
        }
    }

    // Fallback on Windows: Detect and launch external Microsoft WebView2 App Host
    #[cfg(windows)]
    {
        let webview2 = webview2_runtime::detect_webview2_runtime();
        if webview2.is_available {
            let ver = webview2.version.as_deref().unwrap_or("Evergreen");
            println!(
                " [✓] Starting Fallback Microsoft WebView2 App Host (version: {})...",
                ver
            );
            match webview2_runtime::launch_webview2_host(&webview2, &workbench_html, &data_dir) {
                Ok(_) => {
                    println!(
                        "\nAetherIDE Environment Ready. Press Ctrl+C or close window to exit."
                    );
                    return Ok(());
                }
                Err(e) => {
                    eprintln!(
                        "Warning: WebView2 fallback launch failed ({}); attempting default browser...",
                        e
                    );
                }
            }
        }
    }

    // Fallback: Open workbench in default browser
    println!(" [!] Opening Aether Workbench in default browser...");
    let url = format!(
        "file:///{}",
        workbench_html.to_string_lossy().replace('\\', "/")
    );
    let _ = Command::new("cmd").args(["/c", "start", &url]).spawn();

    println!("\nAetherIDE Environment Ready. Press Ctrl+C or close window to exit.");
    Ok(())
}
