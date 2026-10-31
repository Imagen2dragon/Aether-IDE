pub mod merge;
pub mod swarm;

pub use merge::{MergeConflictHunk, MergeReport, MergeResolutionAction, ThreeWayMergeEngine};
pub use swarm::{
    AgentMessage, AgentMessageType, SwarmCoordinator, SwarmTaskGraph, SwarmTaskNode,
    SwarmTaskStatus,
};

use agent_runtime::{AgentRuntime, AgentTask};
use application_core::{AetherError, AutonomyMode, Result};
use database_engine::DatabaseEngine;
use model_gateway::ModelGateway;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentPersona {
    pub role_id: String,
    pub display_name: String,
    pub description: String,
    pub system_prompt: String,
    pub default_tools: Vec<String>,
}

pub struct AgentOrchestrator {
    personas: Vec<AgentPersona>,
    tasks: Arc<Mutex<HashMap<String, AgentRuntime>>>,
    model_gateway: ModelGateway,
    db: DatabaseEngine,
}

impl AgentOrchestrator {
    pub fn new(model_gateway: ModelGateway, db: DatabaseEngine) -> Self {
        let personas = Self::init_personas();
        Self {
            personas,
            tasks: Arc::new(Mutex::new(HashMap::new())),
            model_gateway,
            db,
        }
    }

    fn init_personas() -> Vec<AgentPersona> {
        vec![
            AgentPersona {
                role_id: "architect".into(),
                display_name: "Principal Software Architect".into(),
                description: "Oversees high-level system topology, module boundaries, and architectural coherence.".into(),
                system_prompt: "You are the Principal Software Architect. Focus on modularity, clear interface contracts, scalability, and long-term maintainability.".into(),
                default_tools: vec!["read_symbol_graph".into(), "analyze_dependencies".into(), "propose_rfc".into()],
            },
            AgentPersona {
                role_id: "frontend".into(),
                display_name: "Frontend Developer".into(),
                description: "Specializes in TypeScript, React, responsive UI, CSS layout, and state management.".into(),
                system_prompt: "You are an expert Frontend Developer. Build clean, accessible, component-driven UI with TypeScript and modern frameworks.".into(),
                default_tools: vec!["inspect_component".into(), "render_preview".into(), "edit_jsx".into()],
            },
            AgentPersona {
                role_id: "backend".into(),
                display_name: "Backend Developer".into(),
                description: "Builds high-performance APIs, microservices, business logic, and server integrations.".into(),
                system_prompt: "You are a senior Backend Developer. Prioritize error handling, security, concurrency, and idempotent API contracts.".into(),
                default_tools: vec!["edit_api_endpoint".into(), "run_service_test".into(), "inspect_network".into()],
            },
            AgentPersona {
                role_id: "database".into(),
                display_name: "Database Engineer".into(),
                description: "Designs schemas, optimizes queries, ensures transactional integrity, and manages migrations.".into(),
                system_prompt: "You are a Database Engineer. Protect data integrity, structure foreign keys, design optimal indexes, and plan migrations.".into(),
                default_tools: vec!["inspect_schema".into(), "explain_query".into(), "generate_migration".into()],
            },
            AgentPersona {
                role_id: "mobile".into(),
                display_name: "Mobile Developer".into(),
                description: "Builds cross-platform React Native/Expo apps, Flutter solutions, and native integrations.".into(),
                system_prompt: "You are an expert Mobile Developer. Prioritize touch responsiveness, device permissions, offline sync, and app store compliance.".into(),
                default_tools: vec!["expo_cli".into(), "eas_build".into(), "preview_device_frame".into()],
            },
            AgentPersona {
                role_id: "systems".into(),
                display_name: "Rust/C++ Systems Engineer".into(),
                description: "Designs safe, low-level high-performance native systems, FFI bridges, and IPC protocols.".into(),
                system_prompt: "You are a Rust Systems Engineer. Forbid unsafe code where possible, eliminate memory leaks, and guarantee zero-cost abstractions.".into(),
                default_tools: vec!["cargo_check".into(), "valgrind_inspect".into(), "bench_system".into()],
            },
            AgentPersona {
                role_id: "debugging".into(),
                display_name: "Debugging Specialist".into(),
                description: "Isolates root causes, reproduces edge cases, and provides surgical minimal patches.".into(),
                system_prompt: "You are a Master Debugger. Collect reproduction traces, formulate hypotheses, test assumptions, and resolve root causes.".into(),
                default_tools: vec!["inspect_stack_trace".into(), "set_breakpoint".into(), "trace_execution".into()],
            },
            AgentPersona {
                role_id: "testing".into(),
                display_name: "Testing Specialist".into(),
                description: "Generates unit, integration, property-based, and end-to-end regression test suites.".into(),
                system_prompt: "You are a Test Automation Director. Ensure test suites prove behavioral assertions and never invent passing tests.".into(),
                default_tools: vec!["run_test_suite".into(), "generate_property_test".into(), "calc_coverage".into()],
            },
            AgentPersona {
                role_id: "security".into(),
                display_name: "Security Reviewer".into(),
                description: "Audits code for vulnerabilities, injection vectors, secret leaks, and authorization flaws.".into(),
                system_prompt: "You are a Security Reviewer. Assume all external input is hostile. Enforce strict sanitization and permission tiers.".into(),
                default_tools: vec!["scan_secrets".into(), "audit_dependencies".into(), "threat_model".into()],
            },
            AgentPersona {
                role_id: "reviewer".into(),
                display_name: "Code Reviewer".into(),
                description: "Evaluates PRs and diffs for code style, idiomatic patterns, readability, and regression risks.".into(),
                system_prompt: "You are a Senior Code Reviewer. Review diffs for correctness, edge case handling, and style consistency.".into(),
                default_tools: vec!["inspect_diff".into(), "evaluate_complexity".into(), "add_review_comment".into()],
            },
            AgentPersona {
                role_id: "devops".into(),
                display_name: "DevOps Engineer".into(),
                description: "Manages CI/CD pipelines, containerization, deployment orchestration, and cloud infrastructure.".into(),
                system_prompt: "You are a DevOps Engineer. Create repeatable, deterministic build and release pipelines.".into(),
                default_tools: vec!["lint_ci_workflow".into(), "build_dockerfile".into(), "verify_deployment".into()],
            },
            AgentPersona {
                role_id: "researcher".into(),
                display_name: "Technical Researcher".into(),
                description: "Investigates libraries, RFCs, APIs, and benchmarks before complex implementation decisions.".into(),
                system_prompt: "You are a Technical Researcher. Synthesize documentation, contrast tradeoffs, and cite official documentation.".into(),
                default_tools: vec!["search_docs".into(), "benchmark_libraries".into(), "summarize_spec".into()],
            },
            AgentPersona {
                role_id: "documentation".into(),
                display_name: "Documentation Engineer".into(),
                description: "Authors comprehensive guides, architecture diagrams, API references, and release notes.".into(),
                system_prompt: "You are a Technical Writer. Produce clear, accurate, unambiguous technical documentation.".into(),
                default_tools: vec!["generate_api_docs".into(), "validate_links".into(), "render_mermaid".into()],
            },
            AgentPersona {
                role_id: "performance".into(),
                display_name: "Performance Engineer".into(),
                description: "Profiles CPU/memory consumption, identifies hot loops, and optimizes algorithmic complexity.".into(),
                system_prompt: "You are a Performance Engineer. Measure before optimizing, identify memory bottlenecks, and verify latency budgets.".into(),
                default_tools: vec!["run_profiler".into(), "analyze_flamegraph".into(), "measure_rss".into()],
            },
        ]
    }

    pub fn list_personas(&self) -> Vec<AgentPersona> {
        self.personas.clone()
    }

    pub fn spawn_task(&self, role: &str, objective: &str, autonomy_mode: AutonomyMode) -> String {
        self.spawn_task_with_worktree(role, objective, autonomy_mode, None)
    }

    pub fn spawn_task_with_worktree(
        &self,
        role: &str,
        objective: &str,
        autonomy_mode: AutonomyMode,
        worktree_path: Option<std::path::PathBuf>,
    ) -> String {
        let runtime = AgentRuntime::new(
            role,
            objective,
            autonomy_mode,
            worktree_path,
            self.model_gateway.clone(),
            self.db.clone(),
        );

        let task_id = runtime.task.task_id.clone();
        self.tasks.lock().insert(task_id.clone(), runtime);
        task_id
    }

    pub fn get_task(&self, task_id: &str) -> Option<AgentTask> {
        self.tasks.lock().get(task_id).map(|r| r.task.clone())
    }

    pub async fn step_task(&self, task_id: &str) -> Result<AgentTask> {
        let mut runtime = {
            let mut lock = self.tasks.lock();
            lock.remove(task_id)
                .ok_or_else(|| AetherError::Agent(format!("Task {} not found", task_id)))?
        };

        let _ = runtime.execute_next_step().await;
        let task_clone = runtime.task.clone();
        self.tasks.lock().insert(task_id.to_string(), runtime);
        Ok(task_clone)
    }

    pub async fn approve_step(&self, task_id: &str, step_index: usize) -> Result<AgentTask> {
        let mut runtime = {
            let mut lock = self.tasks.lock();
            lock.remove(task_id)
                .ok_or_else(|| AetherError::Agent(format!("Task {} not found", task_id)))?
        };

        let _ = runtime.approve_and_execute_step(step_index).await?;
        let task_clone = runtime.task.clone();
        self.tasks.lock().insert(task_id.to_string(), runtime);
        Ok(task_clone)
    }

    pub fn reject_step(&self, task_id: &str, step_index: usize, reason: &str) -> Result<AgentTask> {
        let mut runtime = {
            let mut lock = self.tasks.lock();
            lock.remove(task_id)
                .ok_or_else(|| AetherError::Agent(format!("Task {} not found", task_id)))?
        };

        if step_index < runtime.task.steps.len() {
            runtime.task.steps[step_index].output_json = Some(format!("Rejected: {}", reason));
            runtime.task.status = agent_runtime::TaskStatus::Cancelled;
        }

        let task_clone = runtime.task.clone();
        self.tasks.lock().insert(task_id.to_string(), runtime);
        Ok(task_clone)
    }

    pub fn cancel_task(&self, task_id: &str) -> Result<AgentTask> {
        let mut runtime = {
            let mut lock = self.tasks.lock();
            lock.remove(task_id)
                .ok_or_else(|| AetherError::Agent(format!("Task {} not found", task_id)))?
        };

        runtime.task.status = agent_runtime::TaskStatus::Cancelled;
        let task_clone = runtime.task.clone();
        self.tasks.lock().insert(task_id.to_string(), runtime);
        Ok(task_clone)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_orchestrator_initializes_14_personas() {
        let db = DatabaseEngine::open_in_memory().unwrap();
        let gateway = ModelGateway::new();
        let orchestrator = AgentOrchestrator::new(gateway, db);

        let personas = orchestrator.list_personas();
        assert_eq!(personas.len(), 14);

        let task_id =
            orchestrator.spawn_task("architect", "Design IPC schema", AutonomyMode::Autopilot);
        assert!(!task_id.is_empty());

        let task = orchestrator.get_task(&task_id);
        assert!(task.is_some());
        assert_eq!(task.unwrap().role, "architect");
    }
}
