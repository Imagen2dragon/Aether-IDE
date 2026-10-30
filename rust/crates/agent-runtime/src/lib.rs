use application_core::{AetherError, AutonomyMode, Result};
use database_engine::{DatabaseEngine, ProofOfWorkRecord};
use model_gateway::{CompletionRequest, ModelGateway};
use permission_engine::{ActionType, AuthorizationDecision, PermissionEngine};
use process_manager::ProcessManager;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

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

#[derive(Debug, Clone)]
pub struct ToolOutput {
    pub success: bool,
    pub output: String,
    pub diff: Option<String>,
    pub exit_code: i32,
}

pub struct ToolDispatcher;

impl ToolDispatcher {
    pub async fn dispatch(
        tool: &str,
        input: &serde_json::Value,
        base_dir: Option<&Path>,
        db: &DatabaseEngine,
    ) -> Result<ToolOutput> {
        let base_path = base_dir.unwrap_or_else(|| Path::new("."));

        match tool {
            "read_file" => {
                let rel_path = input["path"].as_str().ok_or_else(|| {
                    AetherError::Agent("read_file requires 'path' parameter".into())
                })?;
                let safe_path = Self::resolve_safe_path(base_path, rel_path)?;
                let content = filesystem_engine::FilesystemEngine::read_file_string(&safe_path)?;
                Ok(ToolOutput {
                    success: true,
                    output: content,
                    diff: None,
                    exit_code: 0,
                })
            }
            "write_file" => {
                let rel_path = input["path"].as_str().ok_or_else(|| {
                    AetherError::Agent("write_file requires 'path' parameter".into())
                })?;
                let content = input["content"].as_str().ok_or_else(|| {
                    AetherError::Agent("write_file requires 'content' parameter".into())
                })?;
                let safe_path = Self::resolve_safe_path(base_path, rel_path)?;
                filesystem_engine::FilesystemEngine::write_file_atomic(&safe_path, content)?;

                let diff = git_engine::GitEngine::get_diff(base_path).await.ok();
                Ok(ToolOutput {
                    success: true,
                    output: format!(
                        "Successfully wrote {} bytes to {:?}",
                        content.len(),
                        safe_path
                    ),
                    diff,
                    exit_code: 0,
                })
            }
            "run_command" => {
                let command = input["command"].as_str().ok_or_else(|| {
                    AetherError::Agent("run_command requires 'command' parameter".into())
                })?;
                let cwd = input["cwd"]
                    .as_str()
                    .map(PathBuf::from)
                    .or_else(|| Some(base_path.to_path_buf()));

                let res = ProcessManager::run_command_streaming(
                    command,
                    cwd,
                    Some(std::time::Duration::from_secs(60)),
                    None,
                    None,
                )
                .await?;

                let output = if !res.stderr.is_empty() {
                    format!("STDOUT:\n{}\nSTDERR:\n{}", res.stdout, res.stderr)
                } else {
                    res.stdout
                };

                Ok(ToolOutput {
                    success: res.exit_code == 0,
                    output,
                    diff: None,
                    exit_code: res.exit_code,
                })
            }
            "git_diff" => {
                let diff = git_engine::GitEngine::get_diff(base_path).await?;
                Ok(ToolOutput {
                    success: true,
                    output: if diff.is_empty() {
                        "No changes in worktree".into()
                    } else {
                        diff.clone()
                    },
                    diff: Some(diff),
                    exit_code: 0,
                })
            }
            "search_symbols" => {
                let query = input["query"].as_str().unwrap_or("");
                let limit = input["limit"].as_u64().unwrap_or(20) as usize;
                let symbols = db.search_symbols_fts(query, limit)?;
                let output = serde_json::to_string_pretty(&symbols)?;
                Ok(ToolOutput {
                    success: true,
                    output,
                    diff: None,
                    exit_code: 0,
                })
            }
            "semantic_search" => {
                let query = input["query"].as_str().unwrap_or("");
                let symbols = db.search_symbols_fts(query, 10)?;
                let output = serde_json::to_string_pretty(&symbols)?;
                Ok(ToolOutput {
                    success: true,
                    output,
                    diff: None,
                    exit_code: 0,
                })
            }
            unknown => Err(AetherError::Agent(format!(
                "Unsupported agent tool: {}",
                unknown
            ))),
        }
    }

    fn resolve_safe_path(base: &Path, rel: &str) -> Result<PathBuf> {
        let clean_rel = rel.trim_start_matches(['/', '\\']);
        if rel.contains("..") {
            return Err(AetherError::PermissionDenied(format!(
                "Path traversal rejected: {}",
                rel
            )));
        }
        Ok(base.join(clean_rel))
    }
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

    pub fn determine_action_type(tool: &str, input: &serde_json::Value) -> ActionType {
        match tool {
            "read_file" => ActionType::ReadFile {
                path: input["path"].as_str().unwrap_or("").to_string(),
            },
            "write_file" => ActionType::WriteFile {
                path: input["path"].as_str().unwrap_or("").to_string(),
            },
            "run_command" => {
                let cmd = input["command"].as_str().unwrap_or("");
                let parts: Vec<&str> = cmd.split_whitespace().collect();
                let binary = parts.first().copied().unwrap_or("").to_string();
                let args = parts.iter().skip(1).map(|s| s.to_string()).collect();
                ActionType::ExecuteProcess { binary, args }
            }
            "git_diff" | "search_symbols" | "semantic_search" => ActionType::ReadFile {
                path: format!("virtual:{}", tool),
            },
            _ => ActionType::ReadFile {
                path: "unknown".into(),
            },
        }
    }

    pub async fn execute_next_step(&mut self) -> Result<Option<AgentStep>> {
        self.task.status = TaskStatus::Planning;

        // Prompt model for next concrete step
        let prompt = format!(
            "Role: {}\nObjective: {}\nSteps executed: {}\nChoose the next concrete tool to execute: read_file, write_file, run_command, git_diff, search_symbols.\nRespond in JSON format: {{\"tool\": \"...\", \"params\": {{...}}, \"description\": \"...\"}}",
            self.task.role,
            self.task.objective,
            self.task.steps.len()
        );

        let req = CompletionRequest {
            prompt: prompt.clone(),
            system: Some(format!(
                "You are an expert autonomous software engineer operating as: {}",
                self.task.role
            )),
            model: None,
            temperature: Some(0.1),
            max_tokens: Some(512),
        };

        // Try to obtain planned step or synthesize from objective
        let (tool, params_json, description) =
            match self.model_gateway.execute_completion(&req).await {
                Ok(resp) => {
                    if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&resp.content) {
                        let t = parsed["tool"].as_str().unwrap_or("run_command").to_string();
                        let p = parsed
                            .get("params")
                            .cloned()
                            .unwrap_or_else(|| serde_json::json!({}));
                        let d = parsed["description"]
                            .as_str()
                            .unwrap_or("Executing step")
                            .to_string();
                        (t, p, d)
                    } else {
                        Self::infer_step_from_objective(&self.task.objective)
                    }
                }
                Err(_) => Self::infer_step_from_objective(&self.task.objective),
            };

        let step_index = self.task.steps.len();
        let step_id = format!("step-{}", step_index);

        let mut step = AgentStep {
            step_id: step_id.clone(),
            index: step_index,
            tool: tool.clone(),
            description,
            input_json: params_json.to_string(),
            output_json: None,
            diff: None,
            exit_code: None,
            approved: false,
        };

        // Check permission engine
        let action = Self::determine_action_type(&tool, &params_json);
        let decision = PermissionEngine::evaluate(self.task.autonomy_mode, &action);

        match decision {
            AuthorizationDecision::Allow => {
                step.approved = true;
                self.task.status = TaskStatus::Executing;

                let base_path = self.task.worktree_path.as_deref();
                let tool_res =
                    ToolDispatcher::dispatch(&tool, &params_json, base_path, &self.db).await?;

                step.output_json = Some(tool_res.output.clone());
                step.diff = tool_res.diff.clone();
                step.exit_code = Some(tool_res.exit_code);

                // Record in SQLite Proof of Work Ledger
                let pow = ProofOfWorkRecord {
                    id: None,
                    task_id: self.task.task_id.clone(),
                    step_index,
                    tool_name: step.tool.clone(),
                    input_json: step.input_json.clone(),
                    output_json: tool_res.output,
                    diff: step.diff.clone(),
                    exit_code: tool_res.exit_code,
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
                Err(AetherError::PermissionDenied(format!(
                    "Action denied by permission policy: {:?}",
                    reason
                )))
            }
        }
    }

    pub async fn approve_and_execute_step(&mut self, step_index: usize) -> Result<AgentStep> {
        if step_index >= self.task.steps.len() {
            return Err(AetherError::Agent("Step index out of bounds".into()));
        }

        let mut step = self.task.steps[step_index].clone();
        step.approved = true;
        self.task.status = TaskStatus::Executing;

        let params: serde_json::Value =
            serde_json::from_str(&step.input_json).unwrap_or_else(|_| serde_json::json!({}));

        let base_path = self.task.worktree_path.as_deref();
        let tool_res = ToolDispatcher::dispatch(&step.tool, &params, base_path, &self.db).await?;

        step.output_json = Some(tool_res.output.clone());
        step.diff = tool_res.diff.clone();
        step.exit_code = Some(tool_res.exit_code);

        let pow = ProofOfWorkRecord {
            id: None,
            task_id: self.task.task_id.clone(),
            step_index,
            tool_name: step.tool.clone(),
            input_json: step.input_json.clone(),
            output_json: tool_res.output,
            diff: step.diff.clone(),
            exit_code: tool_res.exit_code,
            timestamp: chrono::Utc::now().to_rfc3339(),
        };
        self.db.record_proof_of_work(&pow)?;

        self.task.steps[step_index] = step.clone();
        self.task.status = TaskStatus::Completed;
        Ok(step)
    }

    fn infer_step_from_objective(objective: &str) -> (String, serde_json::Value, String) {
        let obj_lower = objective.to_lowercase();
        if obj_lower.contains("read") || obj_lower.contains("inspect") {
            (
                "read_file".into(),
                serde_json::json!({ "path": "Cargo.toml" }),
                format!("Read file relevant to: {}", objective),
            )
        } else if obj_lower.contains("write") || obj_lower.contains("create") {
            (
                "write_file".into(),
                serde_json::json!({ "path": "AETHER_SCRATCH.md", "content": "# Aether Scratch\nVerified step execution.\n" }),
                format!("Write file for: {}", objective),
            )
        } else if obj_lower.contains("diff") {
            (
                "git_diff".into(),
                serde_json::json!({}),
                format!("Inspect git diff for: {}", objective),
            )
        } else if obj_lower.contains("symbol") || obj_lower.contains("search") {
            (
                "search_symbols".into(),
                serde_json::json!({ "query": "main", "limit": 10 }),
                format!("Search codebase symbols for: {}", objective),
            )
        } else {
            (
                "run_command".into(),
                serde_json::json!({ "command": "git status" }),
                format!("Inspect repository status for: {}", objective),
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_tool_dispatcher_read_and_write_file() {
        let db = DatabaseEngine::open_in_memory().unwrap();
        let temp_dir = std::env::temp_dir().join(format!(
            "aether-test-tool-{}",
            uuid::Uuid::new_v4().simple()
        ));
        std::fs::create_dir_all(&temp_dir).unwrap();

        // 1. write_file
        let write_input = serde_json::json!({
            "path": "test.txt",
            "content": "AetherIDE autonomous tool execution verified."
        });
        let write_res = ToolDispatcher::dispatch("write_file", &write_input, Some(&temp_dir), &db)
            .await
            .expect("write_file failed");
        assert!(write_res.success);
        assert_eq!(write_res.exit_code, 0);

        // 2. read_file
        let read_input = serde_json::json!({ "path": "test.txt" });
        let read_res = ToolDispatcher::dispatch("read_file", &read_input, Some(&temp_dir), &db)
            .await
            .expect("read_file failed");
        assert!(read_res.success);
        assert!(read_res
            .output
            .contains("AetherIDE autonomous tool execution verified."));

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[tokio::test]
    async fn test_tool_dispatcher_run_command() {
        let db = DatabaseEngine::open_in_memory().unwrap();
        let cmd_input = serde_json::json!({ "command": "echo aether_agent_ok" });
        let res = ToolDispatcher::dispatch("run_command", &cmd_input, None, &db)
            .await
            .expect("run_command failed");
        assert!(res.success);
        assert!(res.output.contains("aether_agent_ok"));
        assert_eq!(res.exit_code, 0);
    }

    #[tokio::test]
    async fn test_permission_engine_blocks_writes_in_observe_mode() {
        let db = DatabaseEngine::open_in_memory().unwrap();
        let gateway = ModelGateway::new();

        let mut runtime = AgentRuntime::new(
            "Security Auditor",
            "write malicious changes",
            AutonomyMode::Observe,
            None,
            gateway,
            db,
        );

        let res = runtime.execute_next_step().await;
        assert!(res.is_err());
        assert_eq!(runtime.task.status, TaskStatus::Failed);
    }

    #[tokio::test]
    async fn test_agent_runtime_step_execution_and_proof_of_work() {
        let db = DatabaseEngine::open_in_memory().unwrap();
        let gateway = ModelGateway::new();

        let mut runtime = AgentRuntime::new(
            "Rust Systems Architect",
            "run git status check",
            AutonomyMode::Autopilot,
            None,
            gateway,
            db.clone(),
        );

        let step = runtime
            .execute_next_step()
            .await
            .expect("step execution failed");
        assert!(step.is_some());
        let step = step.unwrap();
        assert_eq!(step.tool, "run_command");
        assert_eq!(runtime.task.status, TaskStatus::Completed);

        // Verify SQLite proof of work was persisted
        let pows = db.get_proof_of_work(&runtime.task.task_id).unwrap();
        assert_eq!(pows.len(), 1);
        assert_eq!(pows[0].tool_name, "run_command");
    }
}
