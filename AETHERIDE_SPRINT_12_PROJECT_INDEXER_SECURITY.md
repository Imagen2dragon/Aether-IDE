# AETHERIDE — SPRINT 12 REPORT: PROJECT INDEXER CAPABILITY ENFORCEMENT

**Product:** AetherIDE (`v0.1.0-beta.2`)  
**Sprint:** Sprint 12 — Cross-Platform Credential Vault & Release Certification  
**Author:** Principal Rust Systems Architect  
**Date:** 2026-10-31  
**Status:** Certified  

---

## 1. Ambient Filesystem Bypass in `ProjectIndexer`

Prior to Sprint 12, `ProjectIndexer::index_workspace` performed file enumeration by calling `FilesystemEngine::scan_workspace_files`, which returned a vector of absolute paths (`Vec<PathBuf>`).
The indexing loop then read each file via:
```rust
FilesystemEngine::read_file_string(file)
```
This was an **ambient filesystem bypass**:
1. `FilesystemEngine::read_file_string` was an unanchored wrapper around `std::fs::read_to_string`.
2. It opened files by global pathname rather than through the directory capability handle (`Dir`).
3. If an attacker swapped a workspace path or symlink between enumeration and read, the indexer would read ambient files outside the workspace.

---

## 2. Capability-Anchored Redesign

In Sprint 12, `ProjectIndexer` was refactored to hold and operate exclusively through `WorkspaceCapability`:

```rust
// SEC-CAP-01: Anchor indexing strictly to WorkspaceCapability
let cap = WorkspaceCapability::open(root_path)?;
let rel_files = cap.scan_workspace_relative_files(exclude_patterns)?;
let mut total_symbols = 0;
let mut scanned = 0;

for rel_file in &rel_files {
    let ext = rel_file.extension().and_then(|e| e.to_str()).unwrap_or("");
    let lang = SupportedLanguage::from_extension(ext);
    if lang != SupportedLanguage::Generic {
        if let Ok(content) = cap.read_file_to_string(rel_file) {
            let symbols = SyntaxEngine::extract_symbols(&content, lang);
            if !symbols.is_empty() {
                let rel_path = rel_file.to_string_lossy();
                if let Ok(count) = self.index.insert_file_symbols(&rel_path, &symbols) {
                    total_symbols += count;
                }
            }
        }
    }
    scanned += 1;
    ...
```

### Architectural Guarantees:
1. `cap.scan_workspace_relative_files` returns strictly normalized, workspace-relative paths.
2. `cap.read_file_to_string` resolves paths strictly through the opened capability handle, refusing to traverse symlinks or directory escapes.
3. Symbol index keys store relative paths (`src/main.rs`) rather than absolute machine paths (`C:\Users\...`), ensuring portability and security.

---

## 3. Automated Adversarial Verification

We implemented `test_adversarial_indexer_sentinel_isolation` in `rust/crates/project-indexer/src/lib.rs`.

```
test tests::test_adversarial_indexer_sentinel_isolation ... ok
```

### Attack Scenario:
- Adversary plants a symlink/junction inside the workspace pointing to an external confidential directory containing `outside_secret.rs` with `pub fn confidential_sentinel_outside_key()`.
- Legitimate file `src/legit.rs` exists inside the workspace.
- The indexer runs against the compromised workspace.

### Results:
1. `db.search_symbols_fts("confidential_sentinel_outside_key")` -> Returned **0 matches**.
2. `db.search_symbols_fts("OutsideSecretVault")` -> Returned **0 matches**.
3. `db.search_symbols_fts("legitimate_workspace_function")` -> Returned **1 match** (verified indexing functionality preserved).
