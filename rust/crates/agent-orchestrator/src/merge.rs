use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MergeConflictHunk {
    pub line_number: usize,
    pub base_content: Vec<String>,
    pub ours_content: Vec<String>,
    pub theirs_content: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MergeResolutionAction {
    AcceptOurs,
    AcceptTheirs,
    AcceptBoth,
    Custom(String),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MergeReport {
    pub is_clean: bool,
    pub merged_content: Option<String>,
    pub conflict_preview: Option<String>,
    pub conflicts: Vec<MergeConflictHunk>,
    pub total_clean_lines: usize,
}

pub struct ThreeWayMergeEngine;

impl ThreeWayMergeEngine {
    /// Executes a three-way merge across base, ours (current workspace), and theirs (agent worktree).
    pub fn merge(base: &str, ours: &str, theirs: &str) -> MergeReport {
        let base_lines: Vec<&str> = base.lines().collect();
        let ours_lines: Vec<&str> = ours.lines().collect();
        let theirs_lines: Vec<&str> = theirs.lines().collect();

        let max_len = base_lines
            .len()
            .max(ours_lines.len())
            .max(theirs_lines.len());

        let mut merged: Vec<String> = Vec::new();
        let mut conflicts: Vec<MergeConflictHunk> = Vec::new();
        let mut conflict_preview: Vec<String> = Vec::new();
        let mut clean_lines_count = 0;

        let mut idx = 0;
        while idx < max_len {
            let b = base_lines.get(idx).copied();
            let o = ours_lines.get(idx).copied();
            let t = theirs_lines.get(idx).copied();

            match (b, o, t) {
                // Identical lines across all three
                (Some(bv), Some(ov), Some(tv)) if bv == ov && ov == tv => {
                    merged.push(ov.to_string());
                    conflict_preview.push(ov.to_string());
                    clean_lines_count += 1;
                }
                // Only theirs changed (clean merge of agent's edit)
                (Some(bv), Some(ov), Some(tv)) if bv == ov && ov != tv => {
                    merged.push(tv.to_string());
                    conflict_preview.push(tv.to_string());
                    clean_lines_count += 1;
                }
                // Only ours changed (clean keep of workspace edit)
                (Some(bv), Some(ov), Some(tv)) if bv != ov && bv == tv => {
                    merged.push(ov.to_string());
                    conflict_preview.push(ov.to_string());
                    clean_lines_count += 1;
                }
                // Both changed identically
                (Some(_), Some(ov), Some(tv)) if ov == tv => {
                    merged.push(ov.to_string());
                    conflict_preview.push(ov.to_string());
                    clean_lines_count += 1;
                }
                // Addition at end from theirs
                (None, None, Some(tv)) => {
                    merged.push(tv.to_string());
                    conflict_preview.push(tv.to_string());
                    clean_lines_count += 1;
                }
                // Addition at end from ours
                (None, Some(ov), None) => {
                    merged.push(ov.to_string());
                    conflict_preview.push(ov.to_string());
                    clean_lines_count += 1;
                }
                // Genuine Conflict
                _ => {
                    let base_chunk = b.map(|s| vec![s.to_string()]).unwrap_or_default();
                    let ours_chunk = o.map(|s| vec![s.to_string()]).unwrap_or_default();
                    let theirs_chunk = t.map(|s| vec![s.to_string()]).unwrap_or_default();

                    let hunk = MergeConflictHunk {
                        line_number: idx + 1,
                        base_content: base_chunk,
                        ours_content: ours_chunk.clone(),
                        theirs_content: theirs_chunk.clone(),
                    };
                    conflicts.push(hunk);

                    // Render standard visual git conflict markers
                    conflict_preview.push("<<<<<<< OURS (Workspace)".to_string());
                    for line in &ours_chunk {
                        conflict_preview.push(line.clone());
                    }
                    conflict_preview.push("||||||| BASE (Original)".to_string());
                    if let Some(bv) = b {
                        conflict_preview.push(bv.to_string());
                    }
                    conflict_preview.push("=======".to_string());
                    for line in &theirs_chunk {
                        conflict_preview.push(line.clone());
                    }
                    conflict_preview.push(">>>>>>> THEIRS (Agent Swarm Worktree)".to_string());
                }
            }
            idx += 1;
        }

        let is_clean = conflicts.is_empty();
        MergeReport {
            is_clean,
            merged_content: if is_clean {
                Some(merged.join("\n"))
            } else {
                None
            },
            conflict_preview: Some(conflict_preview.join("\n")),
            conflicts,
            total_clean_lines: clean_lines_count,
        }
    }

    /// Resolves conflicts using chosen resolutions for each conflict hunk.
    pub fn apply_resolutions(
        base: &str,
        ours: &str,
        theirs: &str,
        resolutions: &[MergeResolutionAction],
    ) -> String {
        let report = Self::merge(base, ours, theirs);
        if report.is_clean {
            return report.merged_content.unwrap_or_default();
        }

        let mut lines = Vec::new();
        let base_lines: Vec<&str> = base.lines().collect();
        let ours_lines: Vec<&str> = ours.lines().collect();
        let theirs_lines: Vec<&str> = theirs.lines().collect();
        let max_len = base_lines
            .len()
            .max(ours_lines.len())
            .max(theirs_lines.len());

        let mut conflict_idx = 0;
        let mut idx = 0;
        while idx < max_len {
            let b = base_lines.get(idx).copied();
            let o = ours_lines.get(idx).copied();
            let t = theirs_lines.get(idx).copied();

            let is_conflict = match (b, o, t) {
                (Some(bv), Some(ov), Some(tv)) if bv == ov && ov == tv => false,
                (Some(bv), Some(ov), Some(tv)) if bv == ov && ov != tv => false,
                (Some(bv), Some(ov), Some(tv)) if bv != ov && bv == tv => false,
                (Some(_), Some(ov), Some(tv)) if ov == tv => false,
                (None, None, Some(_)) => false,
                (None, Some(_), None) => false,
                _ => true,
            };

            if !is_conflict {
                if let Some(line) = t.or(o).or(b) {
                    lines.push(line.to_string());
                }
            } else {
                let action = resolutions
                    .get(conflict_idx)
                    .unwrap_or(&MergeResolutionAction::AcceptOurs);
                match action {
                    MergeResolutionAction::AcceptOurs => {
                        if let Some(ov) = o {
                            lines.push(ov.to_string());
                        }
                    }
                    MergeResolutionAction::AcceptTheirs => {
                        if let Some(tv) = t {
                            lines.push(tv.to_string());
                        }
                    }
                    MergeResolutionAction::AcceptBoth => {
                        if let Some(ov) = o {
                            lines.push(ov.to_string());
                        }
                        if let Some(tv) = t {
                            lines.push(tv.to_string());
                        }
                    }
                    MergeResolutionAction::Custom(custom) => {
                        lines.push(custom.clone());
                    }
                }
                conflict_idx += 1;
            }
            idx += 1;
        }

        lines.join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clean_three_way_merge() {
        let base = "line 1\nline 2\nline 3";
        let ours = "line 1\nline 2\nline 3";
        let theirs = "line 1\nline 2 MODIFIED BY AGENT\nline 3";

        let report = ThreeWayMergeEngine::merge(base, ours, theirs);
        assert!(report.is_clean);
        assert_eq!(report.conflicts.len(), 0);
        let merged = report.merged_content.unwrap();
        assert_eq!(merged, "line 1\nline 2 MODIFIED BY AGENT\nline 3");
    }

    #[test]
    fn test_conflicting_three_way_merge_and_resolution() {
        let base = "line 1\nshared line\nline 3";
        let ours = "line 1\nworkspace change\nline 3";
        let theirs = "line 1\nagent change\nline 3";

        let report = ThreeWayMergeEngine::merge(base, ours, theirs);
        assert!(!report.is_clean);
        assert_eq!(report.conflicts.len(), 1);
        assert_eq!(report.conflicts[0].line_number, 2);

        let preview = report.conflict_preview.unwrap();
        assert!(preview.contains("<<<<<<< OURS"));
        assert!(preview.contains("workspace change"));
        assert!(preview.contains("agent change"));
        assert!(preview.contains(">>>>>>> THEIRS"));

        // Resolution: AcceptTheirs
        let resolved_theirs = ThreeWayMergeEngine::apply_resolutions(
            base,
            ours,
            theirs,
            &[MergeResolutionAction::AcceptTheirs],
        );
        assert_eq!(resolved_theirs, "line 1\nagent change\nline 3");

        // Resolution: AcceptOurs
        let resolved_ours = ThreeWayMergeEngine::apply_resolutions(
            base,
            ours,
            theirs,
            &[MergeResolutionAction::AcceptOurs],
        );
        assert_eq!(resolved_ours, "line 1\nworkspace change\nline 3");
    }
}
