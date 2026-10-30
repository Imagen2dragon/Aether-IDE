use application_core::{AetherError, AutonomyMode, Result};
use database_engine::{DatabaseEngine, ProofOfWorkRecord};
use model_gateway::{CompletionRequest, ModelGateway};
use permission_engine::{ActionType, AuthorizationDecision, PermissionEngine};
use process_manager::ProcessManager;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskStatus {
    Pending,
    Planning,
    Executing,
    WaitingForApproval,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentStep {
    pub step_id: String,
    pub index: usize,
    pub tool: String,
    pub description: String,
    pub input_json: String,
    pub output_json: Option<String>,
    pub diff: Option<String>,
    pub exit_code: Option<i32>,
    pub approved: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentTask {
    pub task_id: String,
    pub role: String,
    pub objective: String,
    pub autonomy_mode: AutonomyMode,
    pub status: TaskStatus,
    pub worktree_path: Option<PathBuf>,
    pub steps: Vec<AgentStep>,
    pub created_at: String,
}

pub struct AgentRuntime {
    pub task: AgentTask,
    model_gateway: ModelGateway,
    db: DatabaseEngine,
}

impl AgentRuntime {
    pub fn new(
        role: &str,
        objective: &str,
        autonomy_mode: AutonomyMode,
        worktree_path: Option<PathBuf>,
        model_gateway: ModelGateway,
        db: DatabaseEngine,
    ) -> Self {
        let task_id = format!("task-{}", uuid::Uuid::new_v4().simple());
        let task = AgentTask {
            task_id,
            role: role.to_string(),
            objective: objective.to_string(),
            autonomy_mode,
            status: TaskStatus::Pending,
            worktree_path,
            steps: Vec::new(),
            created_at: chrono::Utc::now().to_rfc3339(),
        };

        Self {
            task,
            model_gateway,
            db,
        }
    }

    pub async fn execute_next_step(&mut self) -> Result<Option<AgentStep>> {
        self.task.status = TaskStatus::Planning;

        // Prompt model for next step
        let prompt = format!(
            "Role: {}\nObjective: {}\nSteps executed so far: {}\nWhat is the single next concrete action?",
            self.task.role,
            self.task.objective,
            self.task.steps.len()
        );

        let req = CompletionRequest {
            prompt,
            system: Some(format!("You are an expert autonomous software engineer operating as: {}", self.task.role)),
            model: None,
            temperature: Some(0.1),
            max_tokens: Some(512),
        };

        let resp = self.model_gateway.execute_completion(&req).await?;

        let step_index = self.task.steps.len();
        let step_id = format!("step-{}", step_index);

        // Formulate step
        let mut step = AgentStep {
            step_id: step_id.clone(),
            index: step_index,
            tool: "code_inspection".into(),
            description: resp.content.lines().next().unwrap_or("Next step").to_string(),
            input_json: serde_json::json!({ "objective": self.task.objective }).to_string(),
            output_json: None,
            diff: None,
            exit_code: None,
            approved: false,
        };

        // Check permission engine
        let action = ActionType::ReadFile { path: "src/lib.rs".into() };
        let decision = PermissionEngine::evaluate(self.task.autonomy_mode, &action);

        match decision {
            AuthorizationDecision::Allow => {
                step.approved = true;
                self.task.status = TaskStatus::Executing;

                // Execute action
                let out = ProcessManager::run_command_streaming(
                    "cargo --version",
                    self.task.worktree_path.clone(),
                    None,
                    None,
                    None,
                ).await?;

                step.output_json = Some(out.stdout.clone());
                step.exit_code = Some(out.exit_code);

                // Record in Proof of Work Ledger
                let pow = ProofOfWorkRecord {
                    id: None,
                    task_id: self.task.task_id.clone(),
                    step_index,
                    tool_name: step.tool.clone(),
                    input_json: step.input_json.clone(),
                    output_json: out.stdout,
                    diff: step.diff.clone(),
                    exit_code: out.exit_code,
                    timestamp: chrono::Utc::now().to_rfc3339(),
                };
                self.db.record_proof_of_work(&pow)?;

                self.task.steps.push(step.clone());
                self.task.status = TaskStatus::Completed;
                Ok(Some(step))
            }
            AuthorizationDecision::RequiresUserApproval => {
                self.task.status = TaskStatus::WaitingForApproval;
                self.task.steps.push(step.clone());
                Ok(Some(step))
            }
            AuthorizationDecision::Deny(reason) => {
                self.task.status = TaskStatus::Failed;
                Err(AetherError::PermissionDenied(format!("Action denied: {:?}", reason)))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_agent_runtime_step_execution() {
        let db = DatabaseEngine::open_in_memory().unwrap();
        let gateway = ModelGateway::new();

        let mut runtime = AgentRuntime::new(
            "Rust Systems Architect",
            "Verify compiler toolchain and project structure",
            AutonomyMode::Autopilot,
            None,
            gateway,
            db,
        );

        let step = runtime.execute_next_step().await.expect("step execution failed");
        assert!(step.is_some());
        assert_eq!(runtime.task.status, TaskStatus::Completed);
    }
}
