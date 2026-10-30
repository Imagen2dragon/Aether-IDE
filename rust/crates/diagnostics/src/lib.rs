use process_manager::ProcessManager;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ToolchainStatus {
    Healthy,
    Warning,
    Missing,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolchainDiagnostic {
    pub name: String,
    pub status: ToolchainStatus,
    pub version: Option<String>,
    pub details: String,
    pub remediation: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DoctorReport {
    pub all_healthy: bool,
    pub diagnostics: Vec<ToolchainDiagnostic>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemObservabilityMetrics {
    pub uptime_secs: u64,
    pub active_agent_tasks: usize,
    pub active_terminals: usize,
    pub memory_rss_mb: f32,
}

pub struct EnvironmentDoctor;

impl EnvironmentDoctor {
    pub async fn diagnose_all() -> DoctorReport {
        let mut list = Vec::new();

        // 1. Rust
        list.push(Self::check_tool("Rust Compiler", "rustc --version", "https://rustup.rs").await);
        // 2. Cargo
        list.push(
            Self::check_tool(
                "Cargo Package Manager",
                "cargo --version",
                "https://rustup.rs",
            )
            .await,
        );
        // 3. Git
        list.push(Self::check_tool("Git SCM", "git --version", "https://git-scm.com").await);
        // 4. Node.js
        list.push(
            Self::check_tool("Node.js Runtime", "node --version", "https://nodejs.org").await,
        );
        // 5. Python
        list.push(
            Self::check_tool(
                "Python Interpreter",
                "python --version",
                "https://python.org",
            )
            .await,
        );

        let all_healthy = list.iter().all(|d| d.status == ToolchainStatus::Healthy);

        DoctorReport {
            all_healthy,
            diagnostics: list,
        }
    }

    async fn check_tool(name: &str, probe_cmd: &str, install_url: &str) -> ToolchainDiagnostic {
        match ProcessManager::run_command_streaming(probe_cmd, None, None, None, None).await {
            Ok(out) if out.exit_code == 0 => {
                let ver = out.stdout.trim().lines().next().unwrap_or("").to_string();
                ToolchainDiagnostic {
                    name: name.to_string(),
                    status: ToolchainStatus::Healthy,
                    version: Some(ver.clone()),
                    details: format!("Verified operational: {}", ver),
                    remediation: None,
                }
            }
            _ => ToolchainDiagnostic {
                name: name.to_string(),
                status: ToolchainStatus::Missing,
                version: None,
                details: "Command not found or failed execution".to_string(),
                remediation: Some(format!("Install via: {}", install_url)),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_doctor_diagnoses_environment() {
        let report = EnvironmentDoctor::diagnose_all().await;
        assert!(!report.diagnostics.is_empty());

        let rust_diag = report
            .diagnostics
            .iter()
            .find(|d| d.name.contains("Rust"))
            .unwrap();
        assert_eq!(rust_diag.status, ToolchainStatus::Healthy);
    }
}
