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

/// Validates that the build process working directory is within an authorized and trusted workspace root.
/// BLD-007: Validates workspace trust and boundary for both explicit working_dir and default (working_dir = None).
pub fn validate_build_working_directory(
    ws: &crate::workspace_bridge::WorkspaceSession,
    working_dir: Option<&str>,
) -> Result<(PathBuf, bool), String> {
    let target = if let Some(d) = working_dir {
        PathBuf::from(d)
    } else {
        ws.primary_root()
            .ok_or_else(|| "No active workspace root open for build execution".to_string())?
    };
    let validated = ws.validate_process_directory(&target)?;
    let trusted = ws.is_path_trusted(&validated);
    Ok((validated, trusted))
}

/// Resolves an approved build tool executable securely from system PATH,
/// strictly refusing project-local binary shadowing and never invoking a shell.
pub fn resolve_approved_tool_executable(
    tool_name: &str,
    working_dir: &std::path::Path,
    is_trusted: bool,
) -> Result<PathBuf, String> {
    let trimmed = tool_name.trim();
    if trimmed.is_empty() {
        return Err("Security Denial: Empty tool name specified.".into());
    }

    // Prohibit arbitrary directory navigation in tool name.
    // The only allowed path forms are "./gradlew", ".\gradlew", or a direct executable name.
    let is_gradlew_relative = trimmed == "./gradlew" || trimmed == ".\\gradlew";
    if (trimmed.contains('/') || trimmed.contains('\\')) && !is_gradlew_relative {
        return Err(format!(
            "Security Denial: Tool path navigation is not allowed: '{}'. Only approved tool names or './gradlew' are permitted.",
            trimmed
        ));
    }

    let p = std::path::Path::new(trimmed);
    let stem = if is_gradlew_relative {
        "gradlew"
    } else {
        p.file_stem().and_then(|s| s.to_str()).unwrap_or(trimmed)
    };
    let ext = if is_gradlew_relative {
        ""
    } else {
        p.extension().and_then(|s| s.to_str()).unwrap_or("")
    };

    // Extension validation
    #[cfg(windows)]
    {
        if !ext.is_empty()
            && !ext.eq_ignore_ascii_case("exe")
            && !ext.eq_ignore_ascii_case("cmd")
            && !ext.eq_ignore_ascii_case("bat")
        {
            return Err(format!(
                "Security Denial: Unapproved executable extension '.{}' for tool '{}'.",
                ext, trimmed
            ));
        }

        // Script extensions (.cmd and .bat) are only permitted for script-based tools (npm, npx, yarn, pnpm, gradlew)
        if ext.eq_ignore_ascii_case("bat") || ext.eq_ignore_ascii_case("cmd") {
            let allows_batch = ["npm", "npx", "yarn", "pnpm", "gradlew"]
                .iter()
                .any(|t| stem.eq_ignore_ascii_case(t));
            if !allows_batch {
                return Err(format!(
                    "Security Denial: Binary tool '{}' cannot have script extension '.{}'.",
                    stem, ext
                ));
            }
        }
    }
    #[cfg(not(windows))]
    {
        if !ext.is_empty() && !ext.eq_ignore_ascii_case("sh") {
            return Err(format!(
                "Security Denial: Unapproved executable extension '.{}' for tool '{}'.",
                ext, trimmed
            ));
        }
    }

    // Exact stem match against approved build tools whitelist (BLD-008: no ends_with suffix spoofing!)
    let is_whitelisted = APPROVED_BUILD_TOOLS.iter().any(|&tool| {
        let tool_base = tool.strip_prefix("./").unwrap_or(tool);
        stem.eq_ignore_ascii_case(tool_base)
    });

    if !is_whitelisted {
        return Err(format!(
            "Security Denial: '{}' is not an approved build tool executable. Allowed tools: {:?}",
            trimmed, APPROVED_BUILD_TOOLS
        ));
    }

    // Handle gradlew wrapper policy: only allowed if workspace is trusted!
    if stem.eq_ignore_ascii_case("gradlew") {
        if !is_trusted {
            return Err(
                "Security Denial: Gradle wrapper execution is denied in untrusted workspace."
                    .to_string(),
            );
        }

        #[cfg(windows)]
        {
            let gradlew_bat = working_dir.join("gradlew.bat");
            if gradlew_bat.is_file() {
                return Ok(gradlew_bat);
            }
            let gradlew_cmd = working_dir.join("gradlew.cmd");
            if gradlew_cmd.is_file() {
                return Ok(gradlew_cmd);
            }
        }
        let gradlew_script = working_dir.join("gradlew");
        if gradlew_script.is_file() {
            return Ok(gradlew_script);
        }
    }

    // Rejection of executable shadowed by project-local binary (BLD-002)
    #[cfg(windows)]
    let shadow_exts = [".exe", ".cmd", ".bat", ""];
    #[cfg(not(windows))]
    let shadow_exts = [""];

    for shadow_ext in shadow_exts {
        let local_shadow = working_dir.join(format!("{}{}", stem, shadow_ext));
        if local_shadow.is_file() {
            return Err(format!(
                "Security Denial: Project-local binary '{:?}' shadows approved build tool '{}'. Refusing execution to prevent local binary substitution.",
                local_shadow, stem
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
                if !ext.is_empty() {
                    let cand = dir.join(format!("{}.{}", stem, ext));
                    if cand.is_file() {
                        return Ok(cand);
                    }
                } else {
                    for e in [".exe", ".cmd", ".bat", ""] {
                        let cand = dir.join(format!("{}{}", stem, e));
                        if cand.is_file() {
                            return Ok(cand);
                        }
                    }
                }
            }
            #[cfg(not(windows))]
            {
                let cand = dir.join(stem);
                if cand.is_file() {
                    return Ok(cand);
                }
            }
        }
    }

    #[cfg(windows)]
    {
        if stem.eq_ignore_ascii_case("npm")
            || stem.eq_ignore_ascii_case("npx")
            || stem.eq_ignore_ascii_case("yarn")
            || stem.eq_ignore_ascii_case("pnpm")
        {
            return Ok(PathBuf::from(format!("{}.cmd", stem)));
        }
        if !ext.is_empty() {
            return Ok(PathBuf::from(format!("{}.{}", stem, ext)));
        }
    }

    Ok(PathBuf::from(stem))
}

#[tauri::command]
pub async fn build_run_task(
    app: AppHandle,
    command: String,
    working_dir: Option<String>,
) -> Result<BuildExecutionResult, String> {
    let (dir, is_trusted) = {
        let ws_arc = app.state::<SharedWorkspace>();
        let ws = ws_arc.lock();
        validate_build_working_directory(&ws, working_dir.as_deref())?
    };

    // 1. Safe argument tokenization preserving quotes and spaces (no split_whitespace!)
    let tokens = parse_command_tokens(&command);
    if tokens.is_empty() {
        return Err("Build command cannot be empty".into());
    }

    let program = &tokens[0];
    let args = &tokens[1..];

    // 2. Resolve executable through controlled registry and reject local shadowing
    let resolved_executable = resolve_approved_tool_executable(program, &dir, is_trusted)?;

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
        let res = resolve_approved_tool_executable("npm", &temp_dir, true);
        assert!(
            res.is_err(),
            "Must reject project-local shadow of approved tool"
        );
        let err_str = res.err().unwrap();
        assert!(err_str.contains("shadows approved build tool"));

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_bld_007_untrusted_default_workspace_execution_denied() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_bld_007_{}", uuid::Uuid::new_v4().simple()));
        let user_data = temp_dir.join("user_data");
        let ws_root = temp_dir.join("project");
        std::fs::create_dir_all(&user_data).unwrap();
        std::fs::create_dir_all(&ws_root).unwrap();

        let ws = crate::workspace_bridge::WorkspaceSession::new(user_data);
        // Open workspace root and mark it untrusted explicitly
        let _ = ws.set_single_root(ws_root.clone()).unwrap();
        let _ = ws.set_trust(ws_root.to_str().unwrap(), false).unwrap();

        // 1. Explicit working_dir in untrusted workspace must be rejected
        let res_explicit = validate_build_working_directory(&ws, Some(ws_root.to_str().unwrap()));
        assert!(res_explicit.is_err());
        assert!(res_explicit
            .unwrap_err()
            .contains("untrusted workspace root"));

        // 2. Default working_dir (None) in untrusted workspace MUST ALSO be rejected (BLD-007)
        let res_default = validate_build_working_directory(&ws, None);
        assert!(
            res_default.is_err(),
            "Default working_dir in untrusted workspace must be rejected"
        );
        assert!(res_default
            .unwrap_err()
            .contains("untrusted workspace root"));

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_bld_008_executable_suffix_spoofing_rejected() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_bld_008_{}", uuid::Uuid::new_v4().simple()));
        std::fs::create_dir_all(&temp_dir).unwrap();

        // Suffix spoofing attacks must all be rejected!
        let spoofed_tools = [
            "evilcargo",
            "maliciouscargo",
            "fakecargo.exe",
            "evilcargo.exe",
            "fakepnpm",
            "my_npm",
            "badpython",
            "evilpython3",
            "c:/temp/cargo",
            "../../cargo",
            "npm.sh",
            "cargo.bat",
        ];

        for tool in spoofed_tools {
            let res = resolve_approved_tool_executable(tool, &temp_dir, true);
            assert!(
                res.is_err(),
                "Spoofed tool '{}' must be rejected but got {:?}",
                tool,
                res
            );
            let err = res.unwrap_err();
            assert!(
                err.contains("Security Denial"),
                "Expected Security Denial for '{}', got: {}",
                tool,
                err
            );
        }

        // Exact approved tools must be permitted
        let valid_cargo = resolve_approved_tool_executable("cargo", &temp_dir, true);
        assert!(valid_cargo.is_ok(), "Exact 'cargo' must be approved");

        let valid_npm = resolve_approved_tool_executable("npm", &temp_dir, true);
        assert!(valid_npm.is_ok(), "Exact 'npm' must be approved");

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_bld_009_windows_command_wrapper_safety() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_bld_009_{}", uuid::Uuid::new_v4().simple()));
        std::fs::create_dir_all(&temp_dir).unwrap();

        // 1. Verify Gradle wrapper in untrusted workspace is denied
        #[cfg(windows)]
        let gradlew_script = temp_dir.join("gradlew.bat");
        #[cfg(not(windows))]
        let gradlew_script = temp_dir.join("gradlew");
        std::fs::write(&gradlew_script, b"#!/bin/sh\necho gradle").unwrap();

        let res_untrusted_gradle = resolve_approved_tool_executable("./gradlew", &temp_dir, false);
        assert!(res_untrusted_gradle.is_err());
        assert!(res_untrusted_gradle
            .unwrap_err()
            .contains("untrusted workspace"));

        // 2. In trusted workspace, Gradle wrapper resolution succeeds
        let res_trusted_gradle = resolve_approved_tool_executable("./gradlew", &temp_dir, true);
        assert!(res_trusted_gradle.is_ok());

        // 3. Shell metacharacter injection in command string is tokenized into literal arguments
        let dangerous_cmd = r#"cargo test "foo & echo pwned | calc.exe" --name 'bar > test.txt'"#;
        let tokens = parse_command_tokens(dangerous_cmd);
        assert_eq!(tokens[0], "cargo");
        assert_eq!(tokens[1], "test");
        assert_eq!(tokens[2], "foo & echo pwned | calc.exe");
        assert_eq!(tokens[3], "--name");
        assert_eq!(tokens[4], "bar > test.txt");

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_bld_010_windows_process_argument_escaping_integration() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_bld_010_{}", uuid::Uuid::new_v4().simple()));
        std::fs::create_dir_all(&temp_dir).unwrap();

        let cargo_exe = resolve_approved_tool_executable("cargo", &temp_dir, true)
            .expect("Cargo must resolve in trusted workspace");

        // Construct command with dangerous shell metacharacters (&, |, >, <, %TEMP%)
        let injection_cmd = r#"cargo --version "test & echo PWNED > injected.txt | type injected.txt < NUL %TEMP%""#;
        let tokens = parse_command_tokens(injection_cmd);
        assert_eq!(tokens[0], "cargo");
        assert_eq!(tokens[1], "--version");
        assert_eq!(
            tokens[2],
            "test & echo PWNED > injected.txt | type injected.txt < NUL %TEMP%"
        );

        // Execute directly via std::process::Command (mirroring build_run_task direct execution)
        let output = std::process::Command::new(&cargo_exe)
            .args(&tokens[1..])
            .current_dir(&temp_dir)
            .output();

        // Execution completes (cargo prints version or handles extra arg)
        assert!(output.is_ok(), "Process execution must not crash");

        // The injected file MUST NOT exist because shell interpretation never occurred!
        let injected_file = temp_dir.join("injected.txt");
        assert!(
            !injected_file.exists(),
            "Security Failure: Injected file '{:?}' was created! Shell metacharacters escaped into execution.",
            injected_file
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
