use application_core::{AetherError, AutonomyMode, Result};
use database_engine::{DatabaseEngine, ProofOfWorkRecord};
use filesystem_engine::WorkspaceCapability;
use model_gateway::{CompletionRequest, ModelGateway};
use permission_engine::{ActionType, AuthorizationDecision, PermissionEngine};
use process_manager::ProcessManager;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;

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

/// AgentExecutionContext enforces cryptographic and filesystem-level capability boundaries
/// for all tool operations invoked by autonomous agents.
#[derive(Clone)]
pub struct AgentExecutionContext {
    pub workspace_id: String,
    pub root_id: String,
    pub canonical_root: PathBuf,
    pub is_trusted: bool,
    pub task_id: String,
    pub capability: Arc<WorkspaceCapability>,
}

impl AgentExecutionContext {
    pub fn new(
        workspace_id: impl Into<String>,
        root_id: impl Into<String>,
        canonical_root: PathBuf,
        is_trusted: bool,
        task_id: impl Into<String>,
        capability: Arc<WorkspaceCapability>,
    ) -> Self {
        Self {
            workspace_id: workspace_id.into(),
            root_id: root_id.into(),
            canonical_root,
            is_trusted,
            task_id: task_id.into(),
            capability,
        }
    }

    /// Creates an execution context for a directory, opening a new WorkspaceCapability.
    pub fn from_directory<P: AsRef<Path>>(
        root: P,
        is_trusted: bool,
        task_id: impl Into<String>,
    ) -> Result<Self> {
        let root_ref = root.as_ref();
        let canonical_root = root_ref.canonicalize().map_err(AetherError::Io)?;
        let capability = Arc::new(WorkspaceCapability::open(&canonical_root)?);
        let tid = task_id.into();
        Ok(Self {
            workspace_id: format!("ws-{}", uuid::Uuid::new_v4().simple()),
            root_id: "root-0".to_string(),
            canonical_root,
            is_trusted,
            task_id: tid,
            capability,
        })
    }

    /// Authorizes and reads a text file anchored strictly to the capability.
    pub fn authorize_read(&self, user_path: &str) -> Result<String> {
        self.capability.read_file(user_path)
    }

    /// Authorizes and reads raw bytes anchored strictly to the capability.
    pub fn authorize_read_bytes(&self, user_path: &str) -> Result<Vec<u8>> {
        self.capability.read_file_bytes(user_path)
    }

    /// Authorizes and writes text atomically anchored strictly to the capability.
    pub fn authorize_write(&self, user_path: &str, content: &str) -> Result<PathBuf> {
        self.capability.write_file_atomic(user_path, content)
    }

    /// Authorizes and writes raw bytes atomically anchored strictly to the capability.
    pub fn authorize_write_bytes(&self, user_path: &str, bytes: &[u8]) -> Result<PathBuf> {
        self.capability.write_file_bytes_atomic(user_path, bytes)
    }

    /// Authorizes and deletes a file anchored strictly to the capability.
    pub fn authorize_delete(&self, user_path: &str) -> Result<()> {
        self.capability.delete_file(user_path)
    }

    /// Authorizes a process working directory, enforcing boundary containment and trust status.
    pub fn authorize_process(&self, requested_cwd: Option<&str>) -> Result<PathBuf> {
        if !self.is_trusted {
            return Err(AetherError::PermissionDenied(format!(
                "Security Denial: Untrusted workspace '{}'. Process execution denied.",
                self.workspace_id
            )));
        }

        let target = match requested_cwd {
            None => self.canonical_root.clone(),
            Some(cwd) if cwd.trim().is_empty() => self.canonical_root.clone(),
            Some(cwd) => {
                let p = Path::new(cwd);
                let full = if p.is_absolute() {
                    p.to_path_buf()
                } else {
                    self.canonical_root.join(p)
                };

                let canon = full.canonicalize().map_err(|e| {
                    AetherError::PermissionDenied(format!(
                        "Invalid process working directory {:?}: {}",
                        cwd, e
                    ))
                })?;

                if !filesystem_engine::FilesystemEngine::is_within_boundary(
                    &canon,
                    &self.canonical_root,
                ) {
                    return Err(AetherError::PermissionDenied(format!(
                        "Security Denial: Working directory {:?} escapes workspace boundary {:?}",
                        cwd, self.canonical_root
                    )));
                }

                // Reject if path is an external symlink
                if let Ok(meta) = std::fs::symlink_metadata(&full) {
                    if meta.file_type().is_symlink() {
                        let link_target = std::fs::read_link(&full).map_err(AetherError::Io)?;
                        let resolved_target = if link_target.is_absolute() {
                            link_target
                        } else {
                            full.parent()
                                .unwrap_or(&self.canonical_root)
                                .join(link_target)
                        };
                        let target_canon =
                            resolved_target.canonicalize().map_err(AetherError::Io)?;
                        if !filesystem_engine::FilesystemEngine::is_within_boundary(
                            &target_canon,
                            &self.canonical_root,
                        ) {
                            return Err(AetherError::PermissionDenied(format!(
                                "Security Denial: Working directory symlink {:?} points outside workspace boundary",
                                cwd
                            )));
                        }
                    }
                }

                canon
            }
        };

        Ok(target)
    }
}

pub struct ToolDispatcher;

impl ToolDispatcher {
    pub async fn dispatch(
        tool: &str,
        input: &serde_json::Value,
        ctx: &AgentExecutionContext,
        db: &DatabaseEngine,
    ) -> Result<ToolOutput> {
        match tool {
            "read_file" => {
                let rel_path = input["path"].as_str().ok_or_else(|| {
                    AetherError::Agent("read_file requires 'path' parameter".into())
                })?;
                let content = ctx.authorize_read(rel_path)?;
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
                let safe_path = ctx.authorize_write(rel_path, content)?;

                let diff = git_engine::GitEngine::get_diff(&ctx.canonical_root)
                    .await
                    .ok();
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
                let cwd_opt = input.get("cwd").and_then(|v| v.as_str());
                let safe_cwd = ctx.authorize_process(cwd_opt)?;

                let res = ProcessManager::run_command_streaming(
                    command,
                    Some(safe_cwd),
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
                let diff = git_engine::GitEngine::get_diff(&ctx.canonical_root).await?;
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
}

pub struct AgentRuntime {
    pub task: AgentTask,
    pub context: AgentExecutionContext,
    model_gateway: ModelGateway,
    db: DatabaseEngine,
}

impl AgentRuntime {
    pub fn new(
        role: &str,
        objective: &str,
        autonomy_mode: AutonomyMode,
        context: AgentExecutionContext,
        model_gateway: ModelGateway,
        db: DatabaseEngine,
    ) -> Self {
        let task_id = context.task_id.clone();
        let task = AgentTask {
            task_id,
            role: role.to_string(),
            objective: objective.to_string(),
            autonomy_mode,
            status: TaskStatus::Pending,
            worktree_path: Some(context.canonical_root.clone()),
            steps: Vec::new(),
            created_at: chrono::Utc::now().to_rfc3339(),
        };

        Self {
            task,
            context,
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

                let tool_res =
                    ToolDispatcher::dispatch(&tool, &params_json, &self.context, &self.db).await?;

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

        let tool_res =
            ToolDispatcher::dispatch(&step.tool, &params, &self.context, &self.db).await?;

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

        let ctx = AgentExecutionContext::from_directory(&temp_dir, true, "task-test-1")
            .expect("Failed to create agent execution context");

        // 1. write_file
        let write_input = serde_json::json!({
            "path": "test.txt",
            "content": "AetherIDE autonomous tool execution verified."
        });
        let write_res = ToolDispatcher::dispatch("write_file", &write_input, &ctx, &db)
            .await
            .expect("write_file failed");
        assert!(write_res.success);
        assert_eq!(write_res.exit_code, 0);

        // 2. read_file
        let read_input = serde_json::json!({ "path": "test.txt" });
        let read_res = ToolDispatcher::dispatch("read_file", &read_input, &ctx, &db)
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
        let temp_dir =
            std::env::temp_dir().join(format!("aether-test-cmd-{}", uuid::Uuid::new_v4().simple()));
        std::fs::create_dir_all(&temp_dir).unwrap();

        let ctx = AgentExecutionContext::from_directory(&temp_dir, true, "task-cmd-1")
            .expect("Failed to create execution context");

        let cmd_input = serde_json::json!({ "command": "echo aether_agent_ok" });
        let res = ToolDispatcher::dispatch("run_command", &cmd_input, &ctx, &db)
            .await
            .expect("run_command failed");
        assert!(res.success);
        assert!(res.output.contains("aether_agent_ok"));
        assert_eq!(res.exit_code, 0);

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[tokio::test]
    async fn test_sec_001_path_traversal_read_rejected() {
        let db = DatabaseEngine::open_in_memory().unwrap();
        let temp_dir =
            std::env::temp_dir().join(format!("aether-sec001-{}", uuid::Uuid::new_v4().simple()));
        std::fs::create_dir_all(&temp_dir).unwrap();
        let ctx = AgentExecutionContext::from_directory(&temp_dir, true, "task-sec-1").unwrap();

        let read_input = serde_json::json!({ "path": "../../etc/passwd" });
        let res = ToolDispatcher::dispatch("read_file", &read_input, &ctx, &db).await;
        assert!(res.is_err(), "Path traversal in read_file must be rejected");

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[tokio::test]
    async fn test_sec_002_path_traversal_write_rejected() {
        let db = DatabaseEngine::open_in_memory().unwrap();
        let temp_dir =
            std::env::temp_dir().join(format!("aether-sec002-{}", uuid::Uuid::new_v4().simple()));
        std::fs::create_dir_all(&temp_dir).unwrap();
        let ctx = AgentExecutionContext::from_directory(&temp_dir, true, "task-sec-2").unwrap();

        let write_input = serde_json::json!({
            "path": "../../../escaped_secret.txt",
            "content": "pwned"
        });
        let res = ToolDispatcher::dispatch("write_file", &write_input, &ctx, &db).await;
        assert!(
            res.is_err(),
            "Path traversal in write_file must be rejected"
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[tokio::test]
    async fn test_sec_004_run_command_cwd_escape_rejected() {
        let db = DatabaseEngine::open_in_memory().unwrap();
        let temp_dir =
            std::env::temp_dir().join(format!("aether-sec004-{}", uuid::Uuid::new_v4().simple()));
        std::fs::create_dir_all(&temp_dir).unwrap();
        let ctx = AgentExecutionContext::from_directory(&temp_dir, true, "task-sec-4").unwrap();

        // Target an outside directory
        #[cfg(windows)]
        let outside_dir = "C:\\Windows";
        #[cfg(not(windows))]
        let outside_dir = "/etc";

        let cmd_input = serde_json::json!({
            "command": "echo test",
            "cwd": outside_dir
        });
        let res = ToolDispatcher::dispatch("run_command", &cmd_input, &ctx, &db).await;
        assert!(
            res.is_err(),
            "run_command with cwd outside workspace root must be rejected"
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[tokio::test]
    async fn test_sec_005_untrusted_workspace_execution_denied() {
        let db = DatabaseEngine::open_in_memory().unwrap();
        let temp_dir =
            std::env::temp_dir().join(format!("aether-sec005-{}", uuid::Uuid::new_v4().simple()));
        std::fs::create_dir_all(&temp_dir).unwrap();
        let ctx = AgentExecutionContext::from_directory(&temp_dir, false, "task-sec-5").unwrap();

        let cmd_input = serde_json::json!({ "command": "echo test" });
        let res = ToolDispatcher::dispatch("run_command", &cmd_input, &ctx, &db).await;
        assert!(
            res.is_err(),
            "run_command in untrusted workspace must be denied"
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[tokio::test]
    async fn test_sec_006_reserved_dos_names_rejected() {
        let db = DatabaseEngine::open_in_memory().unwrap();
        let temp_dir =
            std::env::temp_dir().join(format!("aether-sec006-{}", uuid::Uuid::new_v4().simple()));
        std::fs::create_dir_all(&temp_dir).unwrap();
        let ctx = AgentExecutionContext::from_directory(&temp_dir, true, "task-sec-6").unwrap();

        let write_input = serde_json::json!({
            "path": "CON",
            "content": "crash"
        });
        let res = ToolDispatcher::dispatch("write_file", &write_input, &ctx, &db).await;
        assert!(
            res.is_err(),
            "Writing to reserved DOS name CON must be rejected"
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[tokio::test]
    async fn test_permission_engine_blocks_writes_in_observe_mode() {
        let db = DatabaseEngine::open_in_memory().unwrap();
        let gateway = ModelGateway::new();
        let temp_dir = std::env::temp_dir().join(format!(
            "aether-test-observe-{}",
            uuid::Uuid::new_v4().simple()
        ));
        std::fs::create_dir_all(&temp_dir).unwrap();

        let ctx = AgentExecutionContext::from_directory(&temp_dir, true, "task-observe-1").unwrap();

        let mut runtime = AgentRuntime::new(
            "Security Auditor",
            "write malicious changes",
            AutonomyMode::Observe,
            ctx,
            gateway,
            db,
        );

        let res = runtime.execute_next_step().await;
        assert!(res.is_err());
        assert_eq!(runtime.task.status, TaskStatus::Failed);

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[tokio::test]
    async fn test_agent_runtime_step_execution_and_proof_of_work() {
        let db = DatabaseEngine::open_in_memory().unwrap();
        let gateway = ModelGateway::new();
        let temp_dir =
            std::env::temp_dir().join(format!("aether-test-pow-{}", uuid::Uuid::new_v4().simple()));
        std::fs::create_dir_all(&temp_dir).unwrap();

        let ctx = AgentExecutionContext::from_directory(&temp_dir, true, "task-pow-1").unwrap();

        let mut runtime = AgentRuntime::new(
            "Rust Systems Architect",
            "run git status check",
            AutonomyMode::Autopilot,
            ctx,
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

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
