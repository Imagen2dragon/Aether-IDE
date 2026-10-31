use application_core::{AetherError, Result};
use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SwarmTaskStatus {
    Pending,
    Ready,
    Running,
    Completed,
    Failed,
    Blocked,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SwarmTaskNode {
    pub id: String,
    pub title: String,
    pub description: String,
    pub assigned_persona: String,
    pub dependencies: Vec<String>,
    pub status: SwarmTaskStatus,
    pub worktree_path: Option<PathBuf>,
    pub artifacts: Vec<String>,
    pub created_at: DateTime<Utc>,
}

impl SwarmTaskNode {
    pub fn new(
        id: impl Into<String>,
        title: impl Into<String>,
        description: impl Into<String>,
        assigned_persona: impl Into<String>,
        dependencies: Vec<String>,
    ) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            description: description.into(),
            assigned_persona: assigned_persona.into(),
            dependencies,
            status: SwarmTaskStatus::Pending,
            worktree_path: None,
            artifacts: Vec::new(),
            created_at: Utc::now(),
        }
    }
}

/// Directed Acyclic Graph (DAG) for swarm task dependencies and scheduling.
#[derive(Default, Clone, Serialize, Deserialize)]
pub struct SwarmTaskGraph {
    nodes: HashMap<String, SwarmTaskNode>,
}

impl SwarmTaskGraph {
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
        }
    }

    pub fn add_task(&mut self, task: SwarmTaskNode) {
        self.nodes.insert(task.id.clone(), task);
    }

    pub fn get_task(&self, task_id: &str) -> Option<&SwarmTaskNode> {
        self.nodes.get(task_id)
    }

    pub fn get_task_mut(&mut self, task_id: &str) -> Option<&mut SwarmTaskNode> {
        self.nodes.get_mut(task_id)
    }

    /// Detects if there is any cycle in the task graph using Kahn's algorithm.
    pub fn has_cycles(&self) -> bool {
        let mut in_degree: HashMap<String, usize> = HashMap::new();
        let mut adj: HashMap<String, Vec<String>> = HashMap::new();

        for id in self.nodes.keys() {
            in_degree.insert(id.clone(), 0);
            adj.insert(id.clone(), Vec::new());
        }

        for (id, node) in &self.nodes {
            for dep in &node.dependencies {
                if self.nodes.contains_key(dep) {
                    *in_degree.entry(id.clone()).or_insert(0) += 1;
                    adj.entry(dep.clone()).or_default().push(id.clone());
                }
            }
        }

        let mut queue: VecDeque<String> = in_degree
            .iter()
            .filter(|(_, &deg)| deg == 0)
            .map(|(id, _)| id.clone())
            .collect();

        let mut visited_count = 0;
        while let Some(u) = queue.pop_front() {
            visited_count += 1;
            if let Some(neighbors) = adj.get(&u) {
                for v in neighbors {
                    if let Some(deg) = in_degree.get_mut(v) {
                        *deg -= 1;
                        if *deg == 0 {
                            queue.push_back(v.clone());
                        }
                    }
                }
            }
        }

        visited_count != self.nodes.len()
    }

    /// Finds all tasks ready to run (all dependencies completed, status is Pending or Ready).
    pub fn get_ready_tasks(&self) -> Vec<SwarmTaskNode> {
        let completed_ids: HashSet<&str> = self
            .nodes
            .values()
            .filter(|n| n.status == SwarmTaskStatus::Completed)
            .map(|n| n.id.as_str())
            .collect();

        self.nodes
            .values()
            .filter(|n| {
                (n.status == SwarmTaskStatus::Pending || n.status == SwarmTaskStatus::Ready)
                    && n.dependencies
                        .iter()
                        .all(|dep| completed_ids.contains(dep.as_str()))
            })
            .cloned()
            .collect()
    }

    pub fn mark_running(&mut self, task_id: &str, worktree: PathBuf) -> Result<()> {
        let node = self
            .nodes
            .get_mut(task_id)
            .ok_or_else(|| AetherError::Agent(format!("Task {task_id} not found")))?;
        node.status = SwarmTaskStatus::Running;
        node.worktree_path = Some(worktree);
        Ok(())
    }

    pub fn mark_completed(&mut self, task_id: &str, artifacts: Vec<String>) -> Result<()> {
        let node = self
            .nodes
            .get_mut(task_id)
            .ok_or_else(|| AetherError::Agent(format!("Task {task_id} not found")))?;
        node.status = SwarmTaskStatus::Completed;
        node.artifacts = artifacts;
        Ok(())
    }

    pub fn mark_failed(&mut self, task_id: &str) -> Result<()> {
        let node = self
            .nodes
            .get_mut(task_id)
            .ok_or_else(|| AetherError::Agent(format!("Task {task_id} not found")))?;
        node.status = SwarmTaskStatus::Failed;
        Ok(())
    }

    pub fn all_tasks(&self) -> Vec<SwarmTaskNode> {
        self.nodes.values().cloned().collect()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentMessageType {
    Query,
    ProposeDiff,
    ReviewFeedback,
    Handoff,
    Alert,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AgentMessage {
    pub id: String,
    pub sender_persona: String,
    pub recipient_persona: String,
    pub task_id: String,
    pub message_type: AgentMessageType,
    pub payload: String,
    pub timestamp_utc: DateTime<Utc>,
}

impl AgentMessage {
    pub fn new(
        sender_persona: impl Into<String>,
        recipient_persona: impl Into<String>,
        task_id: impl Into<String>,
        message_type: AgentMessageType,
        payload: impl Into<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            sender_persona: sender_persona.into(),
            recipient_persona: recipient_persona.into(),
            task_id: task_id.into(),
            message_type,
            payload: payload.into(),
            timestamp_utc: Utc::now(),
        }
    }
}

/// Swarm Coordinator managing task DAG scheduling, worktree isolation, and message bus.
pub struct SwarmCoordinator {
    graph: RwLock<SwarmTaskGraph>,
    messages: RwLock<Vec<AgentMessage>>,
    workspace_root: PathBuf,
}

impl SwarmCoordinator {
    pub fn new(workspace_root: impl Into<PathBuf>) -> Self {
        Self {
            graph: RwLock::new(SwarmTaskGraph::new()),
            messages: RwLock::new(Vec::new()),
            workspace_root: workspace_root.into(),
        }
    }

    pub fn add_task(&self, task: SwarmTaskNode) -> Result<()> {
        let mut graph = self.graph.write();
        graph.add_task(task);
        if graph.has_cycles() {
            return Err(AetherError::Agent(
                "Cannot add task: creates cyclic dependency in swarm task graph".into(),
            ));
        }
        Ok(())
    }

    /// Allocates an isolated Git worktree path for an agent task.
    pub fn allocate_worktree(&self, persona: &str, task_id: &str) -> PathBuf {
        let dir = self
            .workspace_root
            .join(".aether")
            .join("worktrees")
            .join(format!("agent-{persona}-{task_id}"));
        let _ = std::fs::create_dir_all(&dir);
        dir
    }

    /// Post an inter-agent message to the bus.
    pub fn send_message(&self, message: AgentMessage) {
        self.messages.write().push(message);
    }

    /// Read messages destined for a specific persona or task.
    pub fn get_messages_for_persona(&self, persona: &str) -> Vec<AgentMessage> {
        self.messages
            .read()
            .iter()
            .filter(|m| m.recipient_persona == persona || m.recipient_persona == "*")
            .cloned()
            .collect()
    }

    /// Read messages related to a specific task.
    pub fn get_messages_for_task(&self, task_id: &str) -> Vec<AgentMessage> {
        self.messages
            .read()
            .iter()
            .filter(|m| m.task_id == task_id)
            .cloned()
            .collect()
    }

    pub fn get_ready_tasks(&self) -> Vec<SwarmTaskNode> {
        self.graph.read().get_ready_tasks()
    }

    pub fn mark_running(&self, task_id: &str, worktree: PathBuf) -> Result<()> {
        self.graph.write().mark_running(task_id, worktree)
    }

    pub fn mark_completed(&self, task_id: &str, artifacts: Vec<String>) -> Result<()> {
        self.graph.write().mark_completed(task_id, artifacts)
    }

    pub fn mark_failed(&self, task_id: &str) -> Result<()> {
        self.graph.write().mark_failed(task_id)
    }

    pub fn list_tasks(&self) -> Vec<SwarmTaskNode> {
        self.graph.read().all_tasks()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_swarm_dag_ready_tasks_and_cycle_detection() {
        let mut graph = SwarmTaskGraph::new();

        let t1 = SwarmTaskNode::new("t1", "Schema Design", "DB Schema", "database", vec![]);
        let t2 = SwarmTaskNode::new(
            "t2",
            "API Implementation",
            "Backend",
            "backend",
            vec!["t1".into()],
        );
        let t3 = SwarmTaskNode::new(
            "t3",
            "UI Components",
            "Frontend",
            "frontend",
            vec!["t2".into()],
        );

        graph.add_task(t1);
        graph.add_task(t2);
        graph.add_task(t3);

        assert!(!graph.has_cycles());

        // Initially, only t1 is ready
        let ready = graph.get_ready_tasks();
        assert_eq!(ready.len(), 1);
        assert_eq!(ready[0].id, "t1");

        // Complete t1 -> t2 becomes ready
        graph
            .mark_completed("t1", vec!["schema.sql".into()])
            .unwrap();
        let ready2 = graph.get_ready_tasks();
        assert_eq!(ready2.len(), 1);
        assert_eq!(ready2[0].id, "t2");

        // Complete t2 -> t3 becomes ready
        graph.mark_completed("t2", vec!["api.rs".into()]).unwrap();
        let ready3 = graph.get_ready_tasks();
        assert_eq!(ready3.len(), 1);
        assert_eq!(ready3[0].id, "t3");
    }

    #[test]
    fn test_swarm_cycle_rejection() {
        let mut graph = SwarmTaskGraph::new();
        let t1 = SwarmTaskNode::new("t1", "A", "desc", "architect", vec!["t2".into()]);
        let t2 = SwarmTaskNode::new("t2", "B", "desc", "architect", vec!["t1".into()]);
        graph.add_task(t1);
        graph.add_task(t2);
        assert!(graph.has_cycles());
    }

    #[test]
    fn test_swarm_coordinator_messaging() {
        let temp_dir = std::env::temp_dir().join("aether_test_swarm_coord");
        let coord = SwarmCoordinator::new(&temp_dir);

        let msg = AgentMessage::new(
            "frontend",
            "backend",
            "task-42",
            AgentMessageType::Query,
            "What is the schema for user profile?",
        );
        coord.send_message(msg);

        let msgs = coord.get_messages_for_persona("backend");
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0].sender_persona, "frontend");
        assert_eq!(msgs[0].payload, "What is the schema for user profile?");
    }
}
