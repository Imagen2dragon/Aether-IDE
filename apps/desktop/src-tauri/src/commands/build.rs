use crate::workspace_bridge::SharedWorkspace;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use tauri::{AppHandle, Emitter, Manager};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tracing::info;

pub type SharedActiveBuilds = Arc<Mutex<HashMap<String, u32>>>;

pub const APPROVED_BUILD_TOOLS: &[&str] = &[
    "cargo", "rustc", "npm", "npx", "yarn", "pnpm", "bun", "python", "python3", "py",
    "pytest", "gradle", "gradlew", "./gradlew", "make", "cmake",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectedBuildSystem {
    pub name: String,
    pub kind: String,
    pub build_file: String,
    pub available_commands: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildExecutionResult {
    pub task_id: String,
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
    let ws_arc = app.state::<SharedWorkspace>();
    let ws = ws_arc.lock();
    let root = if let Some(p) = path {
        ws.validate_process_directory(&PathBuf::from(p))?
    } else {
        ws.primary_root()
            .ok_or_else(|| "No active workspace root open to detect build systems".to_string())?
    };
    drop(ws);

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
                "python -m unittest".to_string(),
            ],
        });
    }

    // Check Gradle
    if root.join("build.gradle").exists() || root.join("build.gradle.kts").exists() {
        systems.push(DetectedBuildSystem {
            name: "Gradle (Android / JVM)".to_string(),
            kind: "gradle".to_string(),
            build_file: "build.gradle".to_string(),
            available_commands: vec![
                "gradle build".to_string(),
                "gradle test".to_string(),
                "gradle assembleDebug".to_string(),
            ],
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
    let dir = {
        let ws_arc = app.state::<SharedWorkspace>();
        let ws = ws_arc.lock();
        if let Some(d) = working_dir {
            ws.validate_process_directory(&PathBuf::from(d))?
        } else {
            ws.primary_root()
                .ok_or_else(|| "No active workspace root open for build execution".to_string())?
        }
    };

    // 1. Safe argument tokenization without raw shell injection
    let tokens: Vec<&str> = command.split_whitespace().collect();
    if tokens.is_empty() {
        return Err("Build command cannot be empty".into());
    }

    let program = tokens[0];
    let args = &tokens[1..];

    // 2. Validate executable against approved build tool whitelist
    let is_whitelisted = APPROVED_BUILD_TOOLS.iter().any(|&tool| {
        program.eq_ignore_ascii_case(tool)
            || program.ends_with(tool)
            || program.eq_ignore_ascii_case(&format!("{}.exe", tool))
            || program.eq_ignore_ascii_case(&format!("{}.cmd", tool))
    });

    if !is_whitelisted {
        return Err(format!(
            "Security Denial: '{}' is not an approved build tool executable. Allowed tools: {:?}",
            program, APPROVED_BUILD_TOOLS
        ));
    }

    let task_id = format!("build-{}", uuid::Uuid::new_v4().simple());
    let start = Instant::now();

    // 3. Spawn validated process with direct argument array
    #[cfg(windows)]
    let mut cmd = {
        let is_batch = program == "npm" || program == "npx" || program == "yarn" || program == "pnpm";
        if is_batch {
            let mut c = Command::new("cmd.exe");
            c.arg("/C").arg(program).args(args);
            c
        } else {
            let mut c = Command::new(program);
            c.args(args);
            c
        }
    };

    #[cfg(not(windows))]
    let mut cmd = {
        let mut c = Command::new(program);
        c.args(args);
        c
    };

    cmd.current_dir(&dir);
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());

    let mut child = cmd
        .spawn()
        .map_err(|e| format!("Failed to spawn build command '{}': {}", command, e))?;

    let pid = child.id().unwrap_or(0);

    // Track active process ID for cancellation
    let active_builds = app.state::<SharedActiveBuilds>();
    {
        let mut map = active_builds.lock();
        map.insert(task_id.clone(), pid);
    }

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

    // Remove from active map
    {
        let mut map = active_builds.lock();
        map.remove(&task_id);
    }

    let out_str = stdout_handle.await.unwrap_or_default();
    let err_str = stderr_handle.await.unwrap_or_default();
    let duration = start.elapsed().as_millis();

    let result = BuildExecutionResult {
        task_id,
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

#[tauri::command]
pub async fn build_cancel_task(app: AppHandle, task_id: String) -> Result<bool, String> {
    let active_builds = app.state::<SharedActiveBuilds>();
    let mut map = active_builds.lock();
    if let Some(pid) = map.remove(&task_id) {
        #[cfg(windows)]
        let _ = std::process::Command::new("taskkill")
            .args(["/F", "/T", "/PID", &pid.to_string()])
            .status();

        #[cfg(not(windows))]
        let _ = std::process::Command::new("kill")
            .args(["-9", &pid.to_string()])
            .status();

        info!("Cancelled build task {} (PID {})", task_id, pid);
        Ok(true)
    } else {
        Ok(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_approved_build_tools_whitelist() {
        let allowed = ["cargo", "npm", "npx", "yarn", "pnpm", "bun", "python", "gradle", "make", "cmake"];
        for tool in allowed {
            assert!(
                APPROVED_BUILD_TOOLS.iter().any(|&t| t == tool || tool.ends_with(t)),
                "Tool '{}' should be in approved build tools",
                tool
            );
        }

        let dangerous = ["cmd", "cmd.exe", "powershell", "pwsh", "bash", "sh", "curl", "wget", "rm", "del"];
        for tool in dangerous {
            assert!(
                !APPROVED_BUILD_TOOLS.iter().any(|&t| t.eq_ignore_ascii_case(tool)),
                "Dangerous tool '{}' must NOT be in approved build tools",
                tool
            );
        }
    }
}
