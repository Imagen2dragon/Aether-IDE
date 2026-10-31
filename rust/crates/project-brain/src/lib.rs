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

    pub fn scan_workspace(&mut self, root: &Path) -> usize {
        let mut count = 0;
        let mut file_paths = Vec::new();

        self.collect_source_files(root, root, &mut file_paths);

        for rel_str in &file_paths {
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

            self.add_node(BrainNode {
                id: rel_str.clone(),
                label,
                kind: kind.to_string(),
                metadata,
            });
            count += 1;
        }

        // Establish inferred dependencies across files
        for i in 0..file_paths.len() {
            let full_source = root.join(&file_paths[i]);
            if let Ok(content) = std::fs::read_to_string(&full_source) {
                for (j, path_j) in file_paths.iter().enumerate() {
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
                                source: file_paths[i].clone(),
                                target: path_j.clone(),
                                relationship: "imports".to_string(),
                            });
                        }
                    }
                }
            }
        }

        count
    }

    fn collect_source_files(&self, base_root: &Path, current_dir: &Path, acc: &mut Vec<String>) {
        let entries = match std::fs::read_dir(current_dir) {
            Ok(e) => e,
            Err(_) => return,
        };

        for entry in entries.flatten() {
            let path = entry.path();
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

            if name.starts_with('.')
                || name == "target"
                || name == "node_modules"
                || name == "dist"
                || name == "vs"
                || name == "build"
                || name == "DerivedData"
            {
                continue;
            }

            if path.is_dir() {
                self.collect_source_files(base_root, &path, acc);
            } else if path.is_file() {
                if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                    if matches!(ext, "rs" | "ts" | "tsx" | "js" | "jsx" | "json" | "toml") {
                        if let Ok(rel) = path.strip_prefix(base_root) {
                            let rel_str = rel.to_string_lossy().replace('\\', "/");
                            acc.push(rel_str);
                        }
                    }
                }
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
}
