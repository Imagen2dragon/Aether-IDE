use application_core::{AetherError, Result};
use security_core::SecretRedactor;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc;

#[derive(Debug, Clone)]
pub struct ProcessOutput {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
}

pub struct ProcessManager;

impl ProcessManager {
    pub async fn run_command_streaming(
        command_str: &str,
        cwd: Option<PathBuf>,
        timeout: Option<Duration>,
        stdout_tx: Option<mpsc::Sender<String>>,
        stderr_tx: Option<mpsc::Sender<String>>,
    ) -> Result<ProcessOutput> {
        #[cfg(target_os = "windows")]
        let mut cmd = {
            let mut c = Command::new("cmd.exe");
            c.args(["/C", command_str]);
            c
        };

        #[cfg(not(target_os = "windows"))]
        let mut cmd = {
            let mut c = Command::new("sh");
            c.args(["-c", command_str]);
            c
        };

        if let Some(dir) = cwd {
            cmd.current_dir(dir);
        }

        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        let mut child = cmd.spawn().map_err(|e| AetherError::Process(e.to_string()))?;

        let stdout_handle = child.stdout.take();
        let stderr_handle = child.stderr.take();



        let stdout_task = tokio::spawn(async move {
            let mut captured = String::new();
            if let Some(pipe) = stdout_handle {
                let reader = BufReader::new(pipe);
                let mut lines = reader.lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    let sanitized = SecretRedactor::redact(&line);
                    captured.push_str(&sanitized);
                    captured.push('\n');
                    if let Some(tx) = &stdout_tx {
                        let _ = tx.send(sanitized).await;
                    }
                }
            }
            captured
        });

        let stderr_task = tokio::spawn(async move {
            let mut captured = String::new();
            if let Some(pipe) = stderr_handle {
                let reader = BufReader::new(pipe);
                let mut lines = reader.lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    let sanitized = SecretRedactor::redact(&line);
                    captured.push_str(&sanitized);
                    captured.push('\n');
                    if let Some(tx) = &stderr_tx {
                        let _ = tx.send(sanitized).await;
                    }
                }
            }
            captured
        });

        let wait_fut = async {
            let status = child.wait().await?;
            let out = stdout_task.await.unwrap_or_default();
            let err = stderr_task.await.unwrap_or_default();
            Ok::<ProcessOutput, std::io::Error>(ProcessOutput {
                exit_code: status.code().unwrap_or(-1),
                stdout: out,
                stderr: err,
            })
        };

        let result = if let Some(limit) = timeout {
            match tokio::time::timeout(limit, wait_fut).await {
                Ok(res) => res.map_err(|e| AetherError::Process(e.to_string()))?,
                Err(_) => {
                    let _ = child.kill().await;
                    return Err(AetherError::Process(format!("Process timed out after {:?}", limit)));
                }
            }
        } else {
            wait_fut.await.map_err(|e| AetherError::Process(e.to_string()))?
        };

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_run_command_echo() {
        let out = ProcessManager::run_command_streaming(
            "echo AetherIDE-Test",
            None,
            Some(Duration::from_secs(5)),
            None,
            None,
        ).await.expect("command execution failed");

        assert_eq!(out.exit_code, 0);
        assert!(out.stdout.contains("AetherIDE-Test"));
    }
}
