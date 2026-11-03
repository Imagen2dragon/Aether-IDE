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
    "cargo",
    "rustc",
    "npm",
    "npx",
    "yarn",
    "pnpm",
    "bun",
    "python",
    "python3",
    "py",
    "pytest",
    "gradle",
    "gradlew",
    "./gradlew",
    "make",
    "cmake",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildCommand {
    pub tool: String,
    pub args: Vec<String>,
    pub workspace_root_id: Option<String>,
    pub working_directory: Option<String>,
}

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

/// Robust shell-independent argument tokenizer that preserves quoted strings, spaces, and escape sequences.
pub fn parse_command_tokens(cmd: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_single = false;
    let mut in_double = false;
    let mut escaped = false;

    for c in cmd.chars() {
        if escaped {
            current.push(c);
            escaped = false;
            continue;
        }

        if c == '\\' && !in_single {
            escaped = true;
            continue;
        }

        if c == '\'' && !in_double {
            in_single = !in_single;
            continue;
        }

        if c == '"' && !in_single {
            in_double = !in_double;
            continue;
        }

        if c.is_whitespace() && !in_single && !in_double {
            if !current.is_empty() {
                tokens.push(current);
                current = String::new();
            }
        } else {
            current.push(c);
        }
    }

    if !current.is_empty() {
        tokens.push(current);
    }

    tokens
}

/// Resolves an approved build tool executable securely from system PATH,
/// strictly refusing project-local binary shadowing and never invoking a shell.
pub fn resolve_approved_tool_executable(
    tool_name: &str,
    working_dir: &std::path::Path,
) -> Result<PathBuf, String> {
    let is_whitelisted = APPROVED_BUILD_TOOLS.iter().any(|&tool| {
        tool_name.eq_ignore_ascii_case(tool)
            || tool_name.ends_with(tool)
            || tool_name.eq_ignore_ascii_case(&format!("{}.exe", tool))
            || tool_name.eq_ignore_ascii_case(&format!("{}.cmd", tool))
    });

    if !is_whitelisted {
        return Err(format!(
            "Security Denial: '{}' is not an approved build tool executable. Allowed tools: {:?}",
            tool_name, APPROVED_BUILD_TOOLS
        ));
    }

    // Rejection of executable shadowed by project-local binary (BLD-002)
    #[cfg(windows)]
    let exts = [".exe", ".cmd", ".bat", ""];
    #[cfg(not(windows))]
    let exts = [""];

    for ext in exts {
        let local_shadow = working_dir.join(format!("{}{}", tool_name, ext));
        if local_shadow.is_file() {
            if tool_name == "gradlew" || tool_name == "./gradlew" {
                return Ok(local_shadow);
            }
            return Err(format!(
                "Security Denial: Project-local binary '{:?}' shadows approved build tool '{}'. Refusing execution to prevent local binary substitution.",
                local_shadow, tool_name
            ));
        }
    }

    // Explicit search of system PATH directories (strictly excluding working_dir)
    if let Some(path_var) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&path_var) {
            if dir.starts_with(working_dir) {
                continue;
            }
            #[cfg(windows)]
            {
                for ext in [".exe", ".cmd", ".bat", ""] {
                    let cand = dir.join(format!("{}{}", tool_name, ext));
                    if cand.is_file() {
                        return Ok(cand);
                    }
                }
            }
            #[cfg(not(windows))]
            {
                let cand = dir.join(tool_name);
                if cand.is_file() {
                    return Ok(cand);
                }
            }
        }
    }

    #[cfg(windows)]
    {
        if tool_name == "npm" || tool_name == "npx" || tool_name == "yarn" || tool_name == "pnpm" {
            return Ok(PathBuf::from(format!("{}.cmd", tool_name)));
        }
    }

    Ok(PathBuf::from(tool_name))
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

    // 1. Safe argument tokenization preserving quotes and spaces (no split_whitespace!)
    let tokens = parse_command_tokens(&command);
    if tokens.is_empty() {
        return Err("Build command cannot be empty".into());
    }

    let program = &tokens[0];
    let args = &tokens[1..];

    // 2. Resolve executable through controlled registry and reject local shadowing
    let resolved_executable = resolve_approved_tool_executable(program, &dir)?;

    let task_id = format!("build-{}", uuid::Uuid::new_v4().simple());
    let start = Instant::now();

    // 3. Direct executable spawning without intermediate shell interpreter (NO cmd.exe /C, NO sh -c)
    let mut cmd = Command::new(&resolved_executable);
    cmd.args(args);
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
            let _ = app_stdout.emit(
                "build-task-output",
                json!({ "line": line, "stream": "stdout" }),
            );
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
            let _ = app_stderr.emit(
                "build-task-output",
                json!({ "line": line, "stream": "stderr" }),
            );
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
        let allowed = [
            "cargo", "npm", "npx", "yarn", "pnpm", "bun", "python", "gradle", "make", "cmake",
        ];
        for tool in allowed {
            assert!(
                APPROVED_BUILD_TOOLS
                    .iter()
                    .any(|&t| t == tool || tool.ends_with(t)),
                "Tool '{}' should be in approved build tools",
                tool
            );
        }

        let dangerous = [
            "cmd",
            "cmd.exe",
            "powershell",
            "pwsh",
            "bash",
            "sh",
            "curl",
            "wget",
            "rm",
            "del",
        ];
        for tool in dangerous {
            assert!(
                !APPROVED_BUILD_TOOLS
                    .iter()
                    .any(|&t| t.eq_ignore_ascii_case(tool)),
                "Dangerous tool '{}' must NOT be in approved build tools",
                tool
            );
        }
    }

    #[test]
    fn test_parse_command_tokens_quotes_and_spaces() {
        let cmd = r#"cargo build --message-format "json-diagnostic" --package 'my project'"#;
        let tokens = parse_command_tokens(cmd);
        assert_eq!(
            tokens,
            vec![
                "cargo",
                "build",
                "--message-format",
                "json-diagnostic",
                "--package",
                "my project"
            ]
        );

        // Metacharacters preserved literally, not parsed by shell
        let injection_cmd = r#"cargo test "arg1 & echo evil | rm -rf""#;
        let tokens2 = parse_command_tokens(injection_cmd);
        assert_eq!(tokens2.len(), 3);
        assert_eq!(tokens2[2], "arg1 & echo evil | rm -rf");
    }

    #[test]
    fn test_resolve_approved_tool_executable_shadowing() {
        let temp_dir = std::env::temp_dir().join(format!(
            "aether_build_shadow_{}",
            uuid::Uuid::new_v4().simple()
        ));
        std::fs::create_dir_all(&temp_dir).unwrap();

        // Create a local shadow file named "npm.cmd" inside project working_dir
        #[cfg(windows)]
        let shadow_file = temp_dir.join("npm.cmd");
        #[cfg(not(windows))]
        let shadow_file = temp_dir.join("npm");
        std::fs::write(&shadow_file, b"malicious script").unwrap();

        // Resolution must reject project-local shadow
        let res = resolve_approved_tool_executable("npm", &temp_dir);
        assert!(
            res.is_err(),
            "Must reject project-local shadow of approved tool"
        );
        let err_str = res.err().unwrap();
        assert!(err_str.contains("shadows approved build tool"));

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
