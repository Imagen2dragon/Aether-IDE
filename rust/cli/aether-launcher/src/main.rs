use std::env;
use std::net::TcpStream;
use std::path::{Path, PathBuf};
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
        let cli_path = exe_dir.join("bin").join("aether-cli.exe");
        if cli_path.exists() {
            let status = Command::new(&cli_path).args(&args).status()?;
            std::process::exit(status.code().unwrap_or(0));
        }
    }

    println!("============================================================");
    println!(" AetherIDE — Production AI-Native Software Engineering Studio");
    println!(" Version: v0.1.0-alpha (Standalone Windows x64 Distribution)");
    println!("============================================================\n");

    // 1. Ensure Daemon is Running
    let is_daemon_running = TcpStream::connect("127.0.0.1:44888").is_ok();
    if is_daemon_running {
        println!(" [✓] Aether Daemon: Active on 127.0.0.1:44888");
    } else {
        println!(" [*] Starting Aether Native Daemon in background...");
        let daemon_path = exe_dir.join("bin").join("aether-daemon.exe");
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

    // 2. Read Session Token
    let data_dir = if let Ok(appdata) = env::var("LOCALAPPDATA") {
        PathBuf::from(appdata).join("AetherIDE")
    } else {
        PathBuf::from(".").join(".aether")
    };
    let token_file = data_dir.join("daemon_session.token");
    if let Ok(token) = std::fs::read_to_string(&token_file) {
        println!(
            " [✓] Daemon Session Token verified ({} chars)",
            token.trim().len()
        );
    }

    // 3. Launch Standalone Desktop Workbench
    println!(" [*] Launching AetherIDE Standalone Desktop Workbench...");
    let workbench_html = exe_dir.join("workbench").join("index.html");
    let electron_exe = exe_dir.join("workbench").join("electron.exe");
    let electron_main = exe_dir.join("workbench").join("dist").join("main.js");

    if electron_exe.exists() && electron_main.exists() {
        println!(" [✓] Launching bundled Electron runtime...");
        Command::new(electron_exe).arg(electron_main).spawn()?;
        return Ok(());
    }

    // Launch standalone window via system Webview/Edge in app mode (no browser chrome)
    let edge_paths = [
        r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe",
        r"C:\Program Files\Microsoft\Edge\Application\msedge.exe",
    ];

    let mut launched = false;
    for edge in &edge_paths {
        if Path::new(edge).exists() {
            println!(" [✓] Starting Standalone Desktop Window: {}", edge);
            let app_arg = format!(
                "--app=file:///{}",
                workbench_html.to_string_lossy().replace('\\', "/")
            );
            let status = Command::new(edge)
                .arg(app_arg)
                .arg("--window-size=1440,900")
                .arg("--window-name=AetherIDE")
                .spawn();

            if status.is_ok() {
                launched = true;
                break;
            }
        }
    }

    if !launched {
        // Fallback: Open workbench in default browser
        println!(" [✓] Opening Aether Workbench in default browser...");
        let url = format!(
            "file:///{}",
            workbench_html.to_string_lossy().replace('\\', "/")
        );
        let _ = Command::new("cmd").args(["/c", "start", &url]).spawn();
    }

    println!("\nAetherIDE Environment Ready. Press Ctrl+C or close window to exit.");
    Ok(())
}
