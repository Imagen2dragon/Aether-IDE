pub mod update;

pub use update::{
    AetherUpdateManifest, StagedUpdateApplicator, StagedUpdateStatus, UpdateVerifier,
};

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AetherError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("IPC communication error: {0}")]
    Ipc(String),

    #[error("Database error: {0}")]
    Database(String),

    #[error("Process execution error: {0}")]
    Process(String),

    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    #[error("AI model provider error: {0}")]
    ModelProvider(String),

    #[error("Agent error: {0}")]
    Agent(String),

    #[error("Mobile build error: {0}")]
    MobileBuild(String),

    #[error("Index error: {0}")]
    Index(String),

    #[error("Internal error: {0}")]
    Internal(String),
}

pub type Result<T> = std::result::Result<T, AetherError>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AetherConfig {
    pub instance_id: String,
    pub data_dir: PathBuf,
    pub default_model: String,
    pub autonomy_mode: AutonomyMode,
    pub max_parallel_agents: usize,
    pub index_excluded_patterns: Vec<String>,
}

impl Default for AetherConfig {
    fn default() -> Self {
        let appdata = std::env::var("LOCALAPPDATA").unwrap_or_else(|_| ".".to_string());
        let data_dir = PathBuf::from(appdata).join("AetherIDE");

        Self {
            instance_id: uuid::Uuid::new_v4().to_string(),
            data_dir,
            default_model: "claude-3-5-sonnet".to_string(),
            autonomy_mode: AutonomyMode::Assist,
            max_parallel_agents: 4,
            index_excluded_patterns: vec![
                "node_modules".into(),
                "target".into(),
                ".git".into(),
                "dist".into(),
                "build".into(),
                ".expo".into(),
                ".gradle".into(),
                "Pods".into(),
            ],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AutonomyMode {
    Observe,
    Assist,
    Autopilot,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceMeta {
    pub id: String,
    pub name: String,
    pub root_path: PathBuf,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub last_opened_at: chrono::DateTime<chrono::Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = AetherConfig::default();
        assert_eq!(config.autonomy_mode, AutonomyMode::Assist);
        assert_eq!(config.max_parallel_agents, 4);
        assert!(!config.index_excluded_patterns.is_empty());
    }
}
