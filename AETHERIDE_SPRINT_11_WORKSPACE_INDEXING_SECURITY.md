# AetherIDE Sprint 11 — Workspace Indexing Security & Symlink Containment

**Product:** AetherIDE  
**Crates:** `project-indexer`, `symbol-index`, `filesystem-engine`  
**Security Feature:** Symlink Containment & Exclusions in Code Indexing  
**Date:** October 31, 2026  

---

## 1. Overview & Threat Context

Background project indexing ingests source files into SQLite FTS5 for full-text search, code navigation, and AI context assembly. If an attacker drops an external symlink or junction inside a workspace project folder (e.g. pointing to `C:\Users\<user>\.ssh` or `/etc/passwd`), unconstrained indexers would traverse the symlink, index sensitive private keys into the database, and expose them via search or prompt injection.

---

## 2. Implementation & Containment Controls

### 2.1 Capability-Anchored Scanning
- `FilesystemEngine::scan_workspace_files(&root, &exclusions)` utilizes `WorkspaceCapability::scan_workspace_files`.
- Directory recursion is anchored strictly within the capability directory handle.
- Entries whose symlink/reparse metadata indicates an external target or where path resolution traverses outside the root boundary are silently discarded.

### 2.2 Standard Exclusion Filters
The indexer enforces default and user-configured exclusions:
- Version Control: `.git`, `.hg`, `.svn`
- Build Output: `target`, `node_modules`, `dist`, `build`, `.build-cache`
- Secrets & Sensitive Patterns: `.env*`, `id_rsa*`, `*credentials*`, `*.pem`, `*.key`

---

## 3. Verification & Evidence

In both adversarial tests (`test_13` and `test_14`), the workspace scanner was invoked while active junctions pointing to sensitive external directories were present in the workspace:

```rust
let scanned = FilesystemEngine::scan_workspace_files(&ws, &[]).expect("Scan must succeed");
for item in scanned {
    let content = fs::read_to_string(&item).unwrap_or_default();
    assert!(!content.contains("CRITICAL_AUTHENTICATED"), "Outside files must never be indexed");
}
```

Result: Zero external sentinel files were ingested or indexed. All containment assertions passed.
