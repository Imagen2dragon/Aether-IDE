# AETHERIDE — SPRINT 12 REPORT: PROJECT BRAIN CAPABILITY ENFORCEMENT

**Product:** AetherIDE (`v0.1.0-beta.2`)  
**Sprint:** Sprint 12 — Cross-Platform Credential Vault & Release Certification  
**Author:** Principal Rust Systems Architect  
**Date:** 2026-10-31  
**Status:** Certified  

---

## 1. Ambient Filesystem Bypass in `ProjectBrain`

In Sprint 11, `ProjectBrain::scan_workspace` contained its own independent directory traversal method `collect_source_files`:
```rust
fn collect_source_files(&self, base_root: &Path, current_dir: &Path, acc: &mut Vec<String>) {
    let entries = match std::fs::read_dir(current_dir) { ... };
    ...
    if path.is_dir() {
        self.collect_source_files(base_root, &path, acc);
    } else if path.is_file() {
        ...
    }
}
```
And then in `scan_workspace`:
```rust
let full_source = root.join(&file_paths[i]);
if let Ok(content) = std::fs::read_to_string(&full_source) { ... }
```

### Vulnerability Analysis:
1. `std::fs::read_dir` used ambient operating system directory iteration without capability sandboxing.
2. `path.is_dir()` and `path.is_file()` automatically follow symbolic links and Windows NTFS junctions.
3. An adversary could plant a junction inside the workspace pointing to `/etc` or sensitive developer directories, causing Project Brain to map external confidential files into the knowledge graph and AST dependency matrix.

---

## 2. Capability-Anchored Redesign

In Sprint 12, `collect_source_files` and `std::fs::read_to_string` were completely removed.
`ProjectBrain::scan_workspace` now operates strictly via `WorkspaceCapability`:

```rust
pub fn scan_workspace(&mut self, root: &Path) -> usize {
    let mut count = 0;

    let cap = match filesystem_engine::WorkspaceCapability::open(root) {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("ProjectBrain failed to open workspace capability: {}", e);
            return 0;
        }
    };

    let exclude_patterns = vec![
        "target".into(),
        "node_modules".into(),
        "dist".into(),
        "vs".into(),
        "build".into(),
        "DerivedData".into(),
    ];

    let rel_files = match cap.scan_workspace_relative_files(&exclude_patterns) {
        Ok(f) => f,
        Err(e) => {
            tracing::error!("ProjectBrain failed scanning files: {}", e);
            return 0;
        }
    };
    ...
    // Establish inferred dependencies across files strictly anchored to capability
    for i in 0..file_paths.len() {
        let rel_p = Path::new(&file_paths[i]);
        if let Ok(content) = cap.read_file_to_string(rel_p) {
            ...
```

---

## 3. Automated Adversarial Verification

We implemented `test_adversarial_brain_sentinel_isolation` in `rust/crates/project-brain/src/lib.rs`.

```
test tests::test_adversarial_brain_sentinel_isolation ... ok
```

### Invariants Proven:
1. When an adversarial symlink/junction points to an outside directory containing `outside_secret.rs`, Project Brain completely ignores the symlink.
2. The number of indexed nodes equals exactly 1 (`src/app.rs`).
3. Zero graph nodes or dependency edges reference outside paths or secret tokens.
