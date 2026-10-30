use application_core::{AetherError, Result};
use process_manager::ProcessManager;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitStatusResult {
    pub branch: String,
    pub is_clean: bool,
    pub modified_files: Vec<String>,
    pub staged_files: Vec<String>,
    pub untracked_files: Vec<String>,
}

pub struct GitEngine;

impl GitEngine {
    pub async fn get_status<P: AsRef<Path>>(repo_path: P) -> Result<GitStatusResult> {
        let repo = repo_path.as_ref().to_path_buf();

        let branch_out = ProcessManager::run_command_streaming(
            "git rev-parse --abbrev-ref HEAD",
            Some(repo.clone()),
            None,
            None,
            None,
        ).await.map_err(|e| AetherError::Process(format!("Git rev-parse failed: {}", e)))?;

        let branch = branch_out.stdout.trim().to_string();

        let status_out = ProcessManager::run_command_streaming(
            "git status --porcelain",
            Some(repo),
            None,
            None,
            None,
        ).await.map_err(|e| AetherError::Process(format!("Git status failed: {}", e)))?;

        let mut modified = Vec::new();
        let mut staged = Vec::new();
        let mut untracked = Vec::new();

        for line in status_out.stdout.lines() {
            if line.len() < 3 {
                continue;
            }
            let index_state = &line[0..1];
            let worktree_state = &line[1..2];
            let file = line[3..].trim().to_string();

            if index_state == "?" {
                untracked.push(file);
            } else {
                if index_state != " " && index_state != "?" {
                    staged.push(file.clone());
                }
                if worktree_state != " " {
                    modified.push(file);
                }
            }
        }

        let is_clean = modified.is_empty() && staged.is_empty() && untracked.is_empty();

        Ok(GitStatusResult {
            branch,
            is_clean,
            modified_files: modified,
            staged_files: staged,
            untracked_files: untracked,
        })
    }

    pub async fn create_agent_worktree<P: AsRef<Path>>(
        repo_path: P,
        agent_id: &str,
    ) -> Result<PathBuf> {
        let repo = repo_path.as_ref();
        let worktree_dir = repo.join(".aether").join("worktrees").join(format!("agent-{}", agent_id));
        let branch_name = format!("aether-agent/{}", agent_id);

        if let Some(parent) = worktree_dir.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let cmd = format!("git worktree add -b \"{}\" \"{}\"", branch_name, worktree_dir.display());
        ProcessManager::run_command_streaming(
            &cmd,
            Some(repo.to_path_buf()),
            None,
            None,
            None,
        ).await.map_err(|e| AetherError::Process(format!("Failed to create worktree: {}", e)))?;

        Ok(worktree_dir)
    }

    pub async fn remove_agent_worktree<P: AsRef<Path>>(
        repo_path: P,
        worktree_path: P,
    ) -> Result<()> {
        let cmd = format!("git worktree remove --force \"{}\"", worktree_path.as_ref().display());
        ProcessManager::run_command_streaming(
            &cmd,
            Some(repo_path.as_ref().to_path_buf()),
            None,
            None,
            None,
        ).await.map_err(|e| AetherError::Process(format!("Failed to remove worktree: {}", e)))?;
        Ok(())
    }

    pub async fn get_diff<P: AsRef<Path>>(repo_path: P) -> Result<String> {
        let out = ProcessManager::run_command_streaming(
            "git diff",
            Some(repo_path.as_ref().to_path_buf()),
            None,
            None,
            None,
        ).await.map_err(|e| AetherError::Process(format!("Git diff failed: {}", e)))?;
        Ok(out.stdout)
    }
}
