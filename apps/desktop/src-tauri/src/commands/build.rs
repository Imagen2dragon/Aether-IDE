use crate::workspace_bridge::SharedWorkspace;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::path::PathBuf;
use std::time::Instant;
use tauri::{AppHandle, Emitter, Manager};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectedBuildSystem {
    pub name: String,
    pub kind: String,
    pub build_file: String,
    pub available_commands: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildExecutionResult {
    pub command: String,
    pub success: bool,
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub duration_ms: u128,
}

#[tauri::command]
pub async fn build_detect_systems(
    app: AppHandle,
    path: Option<String>,
) -> Result<Vec<DetectedBuildSystem>, String> {
    let root = if let Some(p) = path {
        PathBuf::from(p)
    } else {
        let ws_arc = app.state::<SharedWorkspace>();
        let ws = ws_arc.lock();
        ws.get_roots()
            .into_iter()
            .next()
            .unwrap_or_else(|| PathBuf::from("."))
    };

    let mut systems = Vec::new();

    // Check Cargo / Rust
    if root.join("Cargo.toml").exists() {
        systems.push(DetectedBuildSystem {
            name: "Cargo (Rust)".to_string(),
            kind: "rust".to_string(),
            build_file: "Cargo.toml".to_string(),
            available_commands: vec![
                "cargo check".to_string(),
                "cargo build".to_string(),
                "cargo test".to_string(),
                "cargo clippy".to_string(),
            ],
        });
    }

    // Check Node.js
    let pkg_json = root.join("package.json");
    if pkg_json.exists() {
        let pm = if root.join("bun.lockb").exists() {
            "bun"
        } else if root.join("pnpm-lock.yaml").exists() {
            "pnpm"
        } else if root.join("yarn.lock").exists() {
            "yarn"
        } else {
            "npm"
        };

        let mut available = vec![format!("{} run build", pm), format!("{} test", pm)];
        if let Ok(content) = std::fs::read_to_string(&pkg_json) {
            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&content) {
                if let Some(scripts) = parsed.get("scripts").and_then(|s| s.as_object()) {
                    for key in scripts.keys() {
                        let cmd = format!("{} run {}", pm, key);
                        if !available.contains(&cmd) {
                            available.push(cmd);
                        }
                    }
                }
            }
        }

        systems.push(DetectedBuildSystem {
            name: format!("Node.js ({})", pm),
            kind: "node".to_string(),
            build_file: "package.json".to_string(),
            available_commands: available,
        });
    }

    // Check Python
    if root.join("pyproject.toml").exists() || root.join("requirements.txt").exists() {
        systems.push(DetectedBuildSystem {
            name: "Python".to_string(),
            kind: "python".to_string(),
            build_file: if root.join("pyproject.toml").exists() {
                "pyproject.toml".to_string()
            } else {
                "requirements.txt".to_string()
            },
            available_commands: vec![
                "python -m pytest".to_string(),
                "pip install -r requirements.txt".to_string(),
            ],
        });
    }

    // Check Gradle
    if root.join("build.gradle").exists()
        || root.join("build.gradle.kts").exists()
        || root.join("gradlew").exists()
        || root.join("gradlew.bat").exists()
    {
        let cmd = if cfg!(windows) {
            "gradlew.bat build"
        } else {
            "./gradlew build"
        };
        systems.push(DetectedBuildSystem {
            name: "Gradle (Android/JVM)".to_string(),
            kind: "gradle".to_string(),
            build_file: "build.gradle".to_string(),
            available_commands: vec![cmd.to_string()],
        });
    }

    Ok(systems)
}

#[tauri::command]
pub async fn build_run_task(
    app: AppHandle,
    command: String,
    working_dir: Option<String>,
) -> Result<BuildExecutionResult, String> {
    let dir = if let Some(d) = working_dir {
        PathBuf::from(d)
    } else {
        let ws_arc = app.state::<SharedWorkspace>();
        let ws = ws_arc.lock();
        ws.get_roots()
            .into_iter()
            .next()
            .unwrap_or_else(|| PathBuf::from("."))
    };

    let start = Instant::now();

    #[cfg(windows)]
    let mut cmd = {
        let mut c = Command::new("cmd.exe");
        c.args(["/C", &command]);
        c
    };

    #[cfg(not(windows))]
    let mut cmd = {
        let mut c = Command::new("sh");
        c.args(["-c", &command]);
        c
    };

    cmd.current_dir(&dir);
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());

    let mut child = cmd
        .spawn()
        .map_err(|e| format!("Failed to spawn command '{}': {}", command, e))?;

    let stdout = child.stdout.take().ok_or("Failed to capture stdout")?;
    let stderr = child.stderr.take().ok_or("Failed to capture stderr")?;

    let app_stdout = app.clone();
    let stdout_handle = tokio::spawn(async move {
        let mut reader = BufReader::new(stdout).lines();
        let mut full = String::new();
        while let Ok(Some(line)) = reader.next_line().await {
            full.push_str(&line);
            full.push('\n');
            let _ = app_stdout.emit("build-task-output", json!({ "line": line, "stream": "stdout" }));
        }
        full
    });

    let app_stderr = app.clone();
    let stderr_handle = tokio::spawn(async move {
        let mut reader = BufReader::new(stderr).lines();
        let mut full = String::new();
        while let Ok(Some(line)) = reader.next_line().await {
            full.push_str(&line);
            full.push('\n');
            let _ = app_stderr.emit("build-task-output", json!({ "line": line, "stream": "stderr" }));
        }
        full
    });

    let status = child
        .wait()
        .await
        .map_err(|e| format!("Failed to wait for build process: {}", e))?;

    let out_str = stdout_handle.await.unwrap_or_default();
    let err_str = stderr_handle.await.unwrap_or_default();
    let duration = start.elapsed().as_millis();

    let result = BuildExecutionResult {
        command,
        success: status.success(),
        exit_code: status.code(),
        stdout: out_str,
        stderr: err_str,
        duration_ms: duration,
    };

    let _ = app.emit("build-task-finished", json!({ "result": &result }));

    Ok(result)
}
