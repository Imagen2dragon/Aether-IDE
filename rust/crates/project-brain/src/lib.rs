use application_core::Result;
use database_engine::{DatabaseEngine, ProjectMemoryNote};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrainNode {
    pub id: String,
    pub label: String,
    pub kind: String, // "module", "api", "database", "component", "file", "test", "crate"
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrainEdge {
    pub source: String,
    pub target: String,
    pub relationship: String, // "imports", "calls", "renders", "migrates", "depends_on", "tests"
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

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BrainReconcileReport {
    pub nodes_total: usize,
    pub nodes_added: usize,
    pub nodes_removed: usize,
    pub edges_total: usize,
    pub is_complete: bool,
    pub errors: Vec<String>,
}

pub struct ProjectBrain {
    db: DatabaseEngine,
    graph: ProjectBrainGraph,
}

impl ProjectBrain {
    pub fn new(db: DatabaseEngine) -> Self {
        Self {
            db,
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
        if !self.graph.edges.iter().any(|e| {
            e.source == edge.source
                && e.target == edge.target
                && e.relationship == edge.relationship
        }) {
            self.graph.edges.push(edge);
        }
    }

    pub fn get_graph(&self) -> &ProjectBrainGraph {
        &self.graph
    }

    /// Reconciles the ProjectBrain graph with the workspace capability state.
    /// Safely cleans up stale nodes and dangling edges when files are deleted or renamed,
    /// preserves existing graph on scan failure, and returns an audited reconciliation report.
    pub fn reconcile_workspace(&mut self, root: &Path) -> Result<BrainReconcileReport> {
        let cap = filesystem_engine::WorkspaceCapability::open(root)?;
        let exclude_patterns = vec![
            "target".into(),
            "node_modules".into(),
            "dist".into(),
            "vs".into(),
            "build".into(),
            "DerivedData".into(),
        ];

        let scan_report = cap.scan_workspace_detailed(&exclude_patterns)?;
        let mut active_files: HashSet<String> = HashSet::new();
        let mut source_paths = Vec::new();

        for rel in &scan_report.discovered_files {
            if let Some(ext) = rel.extension().and_then(|e| e.to_str()) {
                if matches!(ext, "rs" | "ts" | "tsx" | "js" | "jsx" | "json" | "toml") {
                    let rel_str = rel.to_string_lossy().replace('\\', "/");
                    active_files.insert(rel_str.clone());
                    source_paths.push(rel_str);
                }
            }
        }

        // 1. Remove stale file nodes whose corresponding workspace files no longer exist
        let initial_node_count = self.graph.nodes.len();
        self.graph.nodes.retain(|n| {
            if let Some(path) = n.metadata.get("path") {
                active_files.contains(path)
            } else {
                true // Preserve non-file semantic nodes
            }
        });
        let nodes_removed = initial_node_count.saturating_sub(self.graph.nodes.len());

        // 2. Remove dangling edges where source or target no longer exists
        let current_node_ids: HashSet<String> =
            self.graph.nodes.iter().map(|n| n.id.clone()).collect();
        self.graph.edges.retain(|e| {
            current_node_ids.contains(&e.source) && current_node_ids.contains(&e.target)
        });

        // 3. Add or update active nodes
        let mut nodes_added = 0;
        for rel_str in &source_paths {
            let label = Path::new(rel_str)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(rel_str)
                .to_string();

            let kind = if rel_str.contains("test")
                || rel_str.ends_with("_test.rs")
                || rel_str.ends_with(".spec.ts")
            {
                "test"
            } else if rel_str.ends_with(".rs") {
                "module"
            } else if rel_str.ends_with(".ts") || rel_str.ends_with(".tsx") {
                "component"
            } else {
                "file"
            };

            let mut metadata = HashMap::new();
            metadata.insert("path".to_string(), rel_str.clone());
            metadata.insert(
                "ext".to_string(),
                Path::new(rel_str)
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("")
                    .to_string(),
            );

            if !self.graph.nodes.iter().any(|n| n.id == *rel_str) {
                self.add_node(BrainNode {
                    id: rel_str.clone(),
                    label,
                    kind: kind.to_string(),
                    metadata,
                });
                nodes_added += 1;
            }
        }

        // 4. Rebuild dependency edges anchored strictly to capability
        for i in 0..source_paths.len() {
            let rel_p = Path::new(&source_paths[i]);
            if let Ok(content) = cap.read_file_to_string(rel_p) {
                for (j, path_j) in source_paths.iter().enumerate() {
                    if i != j {
                        let stem = Path::new(path_j)
                            .file_stem()
                            .and_then(|s| s.to_str())
                            .unwrap_or("");
                        if !stem.is_empty()
                            && stem != "lib"
                            && stem != "main"
                            && stem != "mod"
                            && stem != "index"
                            && content.contains(stem)
                        {
                            self.add_edge(BrainEdge {
                                source: source_paths[i].clone(),
                                target: path_j.clone(),
                                relationship: "imports".to_string(),
                            });
                        }
                    }
                }
            }
        }

        let mut errors = Vec::new();
        for p in &scan_report.permission_errors {
            errors.push(format!("Permission denied: {:?}", p));
        }
        for t in &scan_report.transient_errors {
            errors.push(format!("Transient read error: {:?}", t));
        }

        Ok(BrainReconcileReport {
            nodes_total: self.graph.nodes.len(),
            nodes_added,
            nodes_removed,
            edges_total: self.graph.edges.len(),
            is_complete: scan_report.is_complete,
            errors,
        })
    }

    /// Explicitly removes a file and its associated edges from the project graph.
    pub fn remove_file(&mut self, rel_path: &str) {
        let norm = rel_path.replace('\\', "/");
        self.graph.nodes.retain(|n| n.id != norm);
        self.graph
            .edges
            .retain(|e| e.source != norm && e.target != norm);
    }

    /// Scans the workspace and updates the ProjectBrain graph, returning the total node count.
    pub fn scan_workspace(&mut self, root: &Path) -> usize {
        match self.reconcile_workspace(root) {
            Ok(report) => report.nodes_total,
            Err(e) => {
                tracing::error!(
                    "ProjectBrain failed to reconcile workspace at {:?}: {}",
                    root,
                    e
                );
                self.graph.nodes.len()
            }
        }
    }

    pub fn save_memory_note(&self, note: &ProjectMemoryNote) -> Result<()> {
        self.db.save_memory_note(note)
    }

    pub fn get_memory_notes(&self) -> Result<Vec<ProjectMemoryNote>> {
        self.db.get_memory_notes()
    }

    pub fn delete_memory_note(&self, id: &str) -> Result<bool> {
        self.db.delete_memory_note(id)
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

    #[test]
    fn test_project_memory_integration() {
        let db = DatabaseEngine::open_in_memory().unwrap();
        let brain = ProjectBrain::new(db);

        let note = ProjectMemoryNote {
            id: "mem-arch-01".into(),
            title: "Project Brain Architecture".into(),
            content: "Graphs are built directly from AST and file references.".into(),
            tags: vec!["brain".into(), "architecture".into()],
            created_at: "2026-10-31T11:00:00Z".into(),
            updated_at: "2026-10-31T11:00:00Z".into(),
        };

        brain.save_memory_note(&note).unwrap();
        let notes = brain.get_memory_notes().unwrap();
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].title, "Project Brain Architecture");

        let deleted = brain.delete_memory_note("mem-arch-01").unwrap();
        assert!(deleted);
        assert_eq!(brain.get_memory_notes().unwrap().len(), 0);
    }

    #[test]
    fn test_adversarial_brain_sentinel_isolation() {
        let ws =
            std::env::temp_dir().join(format!("aether_brain_ws_{}", uuid::Uuid::new_v4().simple()));
        let outside_dir = std::env::temp_dir().join(format!(
            "aether_brain_outside_{}",
            uuid::Uuid::new_v4().simple()
        ));
        std::fs::create_dir_all(&ws).unwrap();
        std::fs::create_dir_all(&outside_dir).unwrap();

        // Confidential outside sentinel file
        let outside_file = outside_dir.join("outside_secret.rs");
        std::fs::write(&outside_file, "pub fn confidential_token() {}\n").unwrap();

        // Valid inside workspace files
        let src_dir = ws.join("src");
        std::fs::create_dir_all(&src_dir).unwrap();
        let legit_file = src_dir.join("app.rs");
        std::fs::write(&legit_file, "pub fn run_app() {}\n").unwrap();

        // Adversary creates symlink pointing to outside confidential directory
        let symlink_path = ws.join("adversarial_brain_symlink");
        #[cfg(windows)]
        {
            let _ = std::process::Command::new("cmd")
                .args([
                    "/C",
                    "mklink",
                    "/J",
                    &symlink_path.to_string_lossy(),
                    &outside_dir.to_string_lossy(),
                ])
                .output();
        }
        #[cfg(unix)]
        {
            let _ = std::os::unix::fs::symlink(&outside_dir, &symlink_path);
        }

        let db = DatabaseEngine::open_in_memory().unwrap();
        let mut brain = ProjectBrain::new(db);

        let count = brain.scan_workspace(&ws);
        assert_eq!(count, 1, "Only legitimate internal file should be indexed");

        let graph = brain.get_graph();
        // 1. Ensure outside sentinel is NOT in nodes
        for node in &graph.nodes {
            assert!(
                !node.id.contains("outside"),
                "Outside file leaked into brain nodes: {}",
                node.id
            );
            assert!(
                !node.id.contains("adversarial"),
                "Adversarial symlink entered brain nodes: {}",
                node.id
            );
        }

        // 2. Ensure only app.rs is present
        assert_eq!(graph.nodes.len(), 1);
        assert_eq!(graph.nodes[0].id, "src/app.rs");

        #[cfg(windows)]
        let _ = std::fs::remove_dir(&symlink_path);
        let _ = std::fs::remove_dir_all(&ws);
        let _ = std::fs::remove_dir_all(&outside_dir);
    }

    #[test]
    fn test_reconciliation_cleans_stale_deleted_files() {
        let ws = std::env::temp_dir().join(format!(
            "aether_brain_reconcile_{}",
            uuid::Uuid::new_v4().simple()
        ));
        std::fs::create_dir_all(ws.join("src")).unwrap();

        let file1 = ws.join("src/alpha.rs");
        let file2 = ws.join("src/beta.rs");
        std::fs::write(&file1, "use crate::beta;\npub fn run_alpha() {}\n").unwrap();
        std::fs::write(&file2, "pub fn run_beta() {}\n").unwrap();

        let db = DatabaseEngine::open_in_memory().unwrap();
        let mut brain = ProjectBrain::new(db);

        let report = brain.reconcile_workspace(&ws).unwrap();
        assert_eq!(report.nodes_total, 2);
        assert_eq!(report.nodes_added, 2);
        assert_eq!(report.nodes_removed, 0);
        assert_eq!(brain.get_graph().nodes.len(), 2);
        assert_eq!(brain.get_graph().edges.len(), 1);

        // Delete beta.rs
        std::fs::remove_file(&file2).unwrap();

        // Reconcile again: beta.rs and its connecting edge must be cleanly removed!
        let report2 = brain.reconcile_workspace(&ws).unwrap();
        assert_eq!(report2.nodes_total, 1);
        assert_eq!(report2.nodes_removed, 1);
        assert_eq!(brain.get_graph().nodes.len(), 1);
        assert_eq!(brain.get_graph().nodes[0].id, "src/alpha.rs");
        assert_eq!(brain.get_graph().edges.len(), 0);

        let _ = std::fs::remove_dir_all(ws);
    }
}
