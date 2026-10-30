use database_engine::DatabaseEngine;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrainNode {
    pub id: String,
    pub label: String,
    pub kind: String, // "module", "api", "database", "component", "file"
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrainEdge {
    pub source: String,
    pub target: String,
    pub relationship: String, // "imports", "calls", "renders", "migrates"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectBrainGraph {
    pub nodes: Vec<BrainNode>,
    pub edges: Vec<BrainEdge>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImpactReport {
    pub target_file: String,
    pub direct_dependents: Vec<String>,
    pub transitive_dependents: Vec<String>,
    pub affected_tests: Vec<String>,
    pub risk_score: f32, // 0.0 (safe) to 1.0 (high blast radius)
}

pub struct ProjectBrain {
    _db: DatabaseEngine,
    graph: ProjectBrainGraph,
}

impl ProjectBrain {
    pub fn new(db: DatabaseEngine) -> Self {
        Self {
            _db: db,
            graph: ProjectBrainGraph {
                nodes: Vec::new(),
                edges: Vec::new(),
            },
        }
    }

    pub fn add_node(&mut self, node: BrainNode) {
        if !self.graph.nodes.iter().any(|n| n.id == node.id) {
            self.graph.nodes.push(node);
        }
    }

    pub fn add_edge(&mut self, edge: BrainEdge) {
        self.graph.edges.push(edge);
    }

    pub fn get_graph(&self) -> &ProjectBrainGraph {
        &self.graph
    }

    pub fn calculate_impact_radar(&self, modified_file: &str) -> ImpactReport {
        let mut direct = Vec::new();
        let mut visited = HashSet::new();
        let mut queue = Vec::new();

        for edge in &self.graph.edges {
            if edge.target == modified_file && edge.relationship == "imports" {
                direct.push(edge.source.clone());
                queue.push(edge.source.clone());
                visited.insert(edge.source.clone());
            }
        }

        let mut transitive = Vec::new();
        while let Some(current) = queue.pop() {
            for edge in &self.graph.edges {
                if edge.target == current && !visited.contains(&edge.source) {
                    visited.insert(edge.source.clone());
                    transitive.push(edge.source.clone());
                    queue.push(edge.source.clone());
                }
            }
        }

        let mut tests = Vec::new();
        for dep in visited
            .iter()
            .chain(std::iter::once(&modified_file.to_string()))
        {
            if dep.contains("test") || dep.ends_with("_test.rs") || dep.ends_with(".spec.ts") {
                tests.push(dep.clone());
            }
        }

        let total_impacted = direct.len() + transitive.len();
        let risk_score = ((total_impacted as f32) * 0.15).min(1.0);

        ImpactReport {
            target_file: modified_file.to_string(),
            direct_dependents: direct,
            transitive_dependents: transitive,
            affected_tests: tests,
            risk_score,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_project_brain_and_impact_radar() {
        let db = DatabaseEngine::open_in_memory().unwrap();
        let mut brain = ProjectBrain::new(db);

        brain.add_node(BrainNode {
            id: "src/auth/token.rs".into(),
            label: "Token Service".into(),
            kind: "module".into(),
            metadata: HashMap::new(),
        });
        brain.add_node(BrainNode {
            id: "src/api/routes.rs".into(),
            label: "API Router".into(),
            kind: "module".into(),
            metadata: HashMap::new(),
        });
        brain.add_node(BrainNode {
            id: "tests/auth_test.rs".into(),
            label: "Auth Integration Tests".into(),
            kind: "test".into(),
            metadata: HashMap::new(),
        });

        brain.add_edge(BrainEdge {
            source: "src/api/routes.rs".into(),
            target: "src/auth/token.rs".into(),
            relationship: "imports".into(),
        });
        brain.add_edge(BrainEdge {
            source: "tests/auth_test.rs".into(),
            target: "src/api/routes.rs".into(),
            relationship: "imports".into(),
        });

        let radar = brain.calculate_impact_radar("src/auth/token.rs");
        assert_eq!(radar.direct_dependents, vec!["src/api/routes.rs"]);
        assert_eq!(radar.transitive_dependents, vec!["tests/auth_test.rs"]);
        assert!(radar.risk_score > 0.0);
    }
}
