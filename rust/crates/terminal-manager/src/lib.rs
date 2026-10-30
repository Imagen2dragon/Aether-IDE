pub mod conpty;

use application_core::{AetherError, Result};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc;

#[cfg(windows)]
use conpty::win32::ConPtySession;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminalSessionInfo {
    pub id: u32,
    pub title: String,
    pub shell: String,
    pub cwd: Option<PathBuf>,
    pub pid: Option<u32>,
    pub is_interactive: bool,
}

pub struct TerminalManager {
    next_id: AtomicU32,
    sessions: Arc<Mutex<HashMap<u32, TerminalSessionInfo>>>,
    #[cfg(windows)]
    conpty_sessions: Arc<Mutex<HashMap<u32, ConPtySession>>>,
}

impl Default for TerminalManager {
    fn default() -> Self {
        Self::new()
    }
}

impl TerminalManager {
    pub fn new() -> Self {
        Self {
            next_id: AtomicU32::new(1),
            sessions: Arc::new(Mutex::new(HashMap::new())),
            #[cfg(windows)]
            conpty_sessions: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn create_session(
        &self,
        title: &str,
        shell: &str,
        cwd: Option<PathBuf>,
    ) -> TerminalSessionInfo {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let info = TerminalSessionInfo {
            id,
            title: title.to_string(),
            shell: shell.to_string(),
            cwd,
            pid: None,
            is_interactive: false,
        };
        self.sessions.lock().insert(id, info.clone());
        info
    }

    pub fn list_sessions(&self) -> Vec<TerminalSessionInfo> {
        self.sessions.lock().values().cloned().collect()
    }

    #[cfg(windows)]
    pub fn spawn_interactive(
        &self,
        shell_name: &str,
        cwd: Option<PathBuf>,
        cols: u16,
        rows: u16,
    ) -> Result<TerminalSessionInfo> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let cmd = resolve_shell_command(shell_name);

        let session = ConPtySession::spawn(id, &cmd, cwd.as_deref(), cols, rows)
            .map_err(|e| AetherError::Process(format!("ConPTY spawn failed: {}", e)))?;

        let pid = session.pid;
        self.conpty_sessions.lock().insert(id, session);

        let info = TerminalSessionInfo {
            id,
            title: format!("{} (ConPTY)", shell_name),
            shell: cmd,
            cwd,
            pid: Some(pid),
            is_interactive: true,
        };
        self.sessions.lock().insert(id, info.clone());
        Ok(info)
    }

    #[cfg(windows)]
    pub fn write_input(&self, id: u32, data: &[u8]) -> Result<usize> {
        let lock = self.conpty_sessions.lock();
        if let Some(session) = lock.get(&id) {
            session
                .write_input(data)
                .map_err(|e| AetherError::Process(format!("ConPTY write failed: {}", e)))
        } else {
            Err(AetherError::Process(format!(
                "Terminal session {} not found",
                id
            )))
        }
    }

    #[cfg(windows)]
    pub fn resize_terminal(&self, id: u32, cols: u16, rows: u16) -> Result<()> {
        let lock = self.conpty_sessions.lock();
        if let Some(session) = lock.get(&id) {
            session
                .resize(cols, rows)
                .map_err(|e| AetherError::Process(format!("ConPTY resize failed: {}", e)))
        } else {
            Err(AetherError::Process(format!(
                "Terminal session {} not found",
                id
            )))
        }
    }

    #[cfg(windows)]
    pub fn subscribe_output(&self, id: u32) -> Result<tokio::sync::broadcast::Receiver<Vec<u8>>> {
        let lock = self.conpty_sessions.lock();
        if let Some(session) = lock.get(&id) {
            Ok(session.subscribe_output())
        } else {
            Err(AetherError::Process(format!(
                "Terminal session {} not found",
                id
            )))
        }
    }

    #[cfg(windows)]
    pub fn get_recent_output(&self, id: u32) -> Result<Vec<u8>> {
        let lock = self.conpty_sessions.lock();
        if let Some(session) = lock.get(&id) {
            Ok(session.get_recent_output())
        } else {
            Err(AetherError::Process(format!(
                "Terminal session {} not found",
                id
            )))
        }
    }

    pub fn close_session(&self, id: u32) -> Result<()> {
        #[cfg(windows)]
        {
            if let Some(mut session) = self.conpty_sessions.lock().remove(&id) {
                session.close();
            }
        }

        let mut lock = self.sessions.lock();
        if lock.remove(&id).is_some() {
            Ok(())
        } else {
            Err(AetherError::Process(format!(
                "Terminal session {} not found",
                id
            )))
        }
    }

    pub async fn execute_in_session(
        &self,
        id: u32,
        command_str: &str,
        stdout_tx: Option<mpsc::Sender<String>>,
        stderr_tx: Option<mpsc::Sender<String>>,
    ) -> Result<process_manager::ProcessOutput> {
        let session = {
            let lock = self.sessions.lock();
            lock.get(&id)
                .cloned()
                .ok_or_else(|| AetherError::Process(format!("Terminal session {} not found", id)))?
        };

        process_manager::ProcessManager::run_command_streaming(
            command_str,
            session.cwd,
            None,
            stdout_tx,
            stderr_tx,
        )
        .await
    }
}

pub fn resolve_shell_command(shell: &str) -> String {
    let lower = shell.to_lowercase();
    if lower == "powershell" || lower == "powershell.exe" {
        "powershell.exe -NoLogo".to_string()
    } else if lower == "pwsh" || lower == "pwsh.exe" {
        "pwsh.exe -NoLogo".to_string()
    } else if lower == "cmd" || lower == "cmd.exe" {
        "cmd.exe".to_string()
    } else if lower == "bash" || lower == "git-bash" {
        let git_bash = PathBuf::from(r"C:\Program Files\Git\bin\bash.exe");
        if git_bash.exists() {
            format!("\"{}\" --login -i", git_bash.to_string_lossy())
        } else {
            "bash.exe --login -i".to_string()
        }
    } else {
        shell.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_terminal_session_lifecycle() {
        let manager = TerminalManager::new();
        let session = manager.create_session("PowerShell 1", "powershell.exe", None);
        assert_eq!(session.id, 1);
        assert_eq!(manager.list_sessions().len(), 1);

        manager.close_session(1).expect("failed to close");
        assert_eq!(manager.list_sessions().len(), 0);
    }

    #[test]
    fn test_resolve_shell_command() {
        assert!(resolve_shell_command("powershell").contains("powershell.exe"));
        assert_eq!(resolve_shell_command("cmd"), "cmd.exe");
    }
}
