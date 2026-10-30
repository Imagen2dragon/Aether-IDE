use application_core::{AetherError, Result};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminalSessionInfo {
    pub id: u32,
    pub title: String,
    pub shell: String,
    pub cwd: Option<PathBuf>,
}

pub struct TerminalManager {
    next_id: AtomicU32,
    sessions: Arc<Mutex<HashMap<u32, TerminalSessionInfo>>>,
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
        }
    }

    pub fn create_session(&self, title: &str, shell: &str, cwd: Option<PathBuf>) -> TerminalSessionInfo {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let info = TerminalSessionInfo {
            id,
            title: title.to_string(),
            shell: shell.to_string(),
            cwd,
        };
        self.sessions.lock().insert(id, info.clone());
        info
    }

    pub fn list_sessions(&self) -> Vec<TerminalSessionInfo> {
        self.sessions.lock().values().cloned().collect()
    }

    pub fn close_session(&self, id: u32) -> Result<()> {
        let mut lock = self.sessions.lock();
        if lock.remove(&id).is_some() {
            Ok(())
        } else {
            Err(AetherError::Process(format!("Terminal session {} not found", id)))
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
            lock.get(&id).cloned().ok_or_else(|| {
                AetherError::Process(format!("Terminal session {} not found", id))
            })?
        };

        process_manager::ProcessManager::run_command_streaming(
            command_str,
            session.cwd,
            None,
            stdout_tx,
            stderr_tx,
        ).await
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
}
