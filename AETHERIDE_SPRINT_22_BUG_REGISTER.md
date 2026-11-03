# AetherIDE — Sprint 22 Production Hardening Bug Register
**Date:** 2026-11-03  
**Sprint:** Sprint 22 — Production Hardening, Filesystem Security, AI Reliability & End-to-End Desktop Validation  
**Target Release:** `v0.1.0-beta.2`  
**Framework:** Tauri 2 (Desktop Shell) + Rust (Systems Core) + React 19 (Workbench) + Monaco Editor  

---

## Executive Summary

During Sprint 22, an exhaustive security, reliability, and lifecycle audit of the AetherIDE Tauri desktop application and Rust workspace engine was conducted. A total of 11 critical and high-priority architectural vulnerabilities were identified, audited, and resolved.

This document serves as the formal record of all identified bugs, their root causes, potential security impacts, and the concrete hardening implementations deployed.

---

## Defect & Vulnerability Register

### BUG-01 (CRITICAL — P0): Arbitrary Path Deletion in Storage Management
- **Location:** `apps/desktop/src-tauri/src/commands/storage.rs`
- **Description:** The frontend previously passed arbitrary path strings directly to the backend for deletion. If malicious or manipulated input was supplied, arbitrary directories on the host operating system could be deleted without authorization. Furthermore, freed bytes were calculated optimistically prior to deletion, reporting false success even if deletion failed.
- **Root Cause:** Absence of a server-side candidate registry and lack of path whitelisting on deletion requests.
- **Hardening Resolution:**
  1. Implemented `APPROVED_CLEANUP_WHITELIST` restricting cleanable items strictly to known generated artifacts (`target`, `node_modules`, `dist`, `build`, `.gradle`, `.turbo`, `.cache`, `.pytest_cache`, `__pycache__`, `.next`, `.aether/worktrees`).
  2. Implemented `StorageCleanupCandidate` with ephemeral UUID tokens (`cand-<uuid>`) registered in `SharedStorageCandidates`.
  3. Symlink rejection: Symlinks and reparse points are explicitly refused during discovery and recursive traversal to prevent junction point escapes.
  4. Verified deletion calculation: Freed byte counters only increment after the filesystem engine confirms the target directory was successfully removed from disk.

### BUG-02 (CRITICAL — P0): Absolute Path Traversal & Unverified Checkpoint Restoration
- **Location:** `rust/crates/checkpoint-engine/src/lib.rs`, `apps/desktop/src-tauri/src/commands/checkpoint.rs`
- **Description:** Checkpoint manifests allowed absolute paths or `../` relative traversal entries. Restoring a corrupted or tampered checkpoint could overwrite critical system files outside the workspace root. Additionally, restored files were not verified against blob hashes.
- **Root Cause:** Lack of strict path sanitization on manifest records and absence of content-addressed blob storage with post-restore verification.
- **Hardening Resolution:**
  1. Enforced strictly relative paths on all `CheckpointFileRecord` entries using `filesystem_engine::sanitize_relative_path`. Any path escaping the workspace boundary returns an immediate `PermissionDenied` error.
  2. Migrated checkpoint storage to content-addressed blobs (`blobs/<sha256>`).
  3. Implemented `restore_preview` providing dry-run conflict detection against disk files before writing.
  4. Implemented atomic file restoration via `WorkspaceCapability::write_file_atomic` followed by post-restore SHA-256 verification against the manifest hash.

### BUG-03 (CRITICAL — P0): AI Whole-Editor Silent Overwrite & Lost Edits
- **Location:** `apps/desktop/src-tauri/src/commands/ai.rs`, `apps/desktop/src/features/ai-assistant/AssistantPanel.tsx`
- **Description:** Previous AI code assistance commands performed direct full-buffer text replacements in the Monaco editor. If a user was typing or external processes modified the file concurrently, changes were overwritten without review, diff preview, or recovery backup.
- **Root Cause:** Direct unverified editor replacement without atomic diff proposal modeling or base-hash conflict checks.
- **Hardening Resolution:**
  1. Introduced `StructuredProposal` containing `proposal_id`, `relative_path`, `operation`, `base_hash`, `proposed_content`, and `unified_diff`.
  2. Implemented `ai_preview_proposal` computing the target file's current disk SHA-256 hash and detecting conflicts when `base_hash != current_disk_hash`.
  3. Added interactive Diff Review Modal in `AssistantPanel.tsx` showing side-by-side / unified diffs with explicit Accept/Reject actions and conflict warnings.
  4. Implemented `ai_apply_proposal` which creates an automated pre-edit safety checkpoint prior to writing and validates the file atomically.

### BUG-04 (HIGH — P1): Model Gateway UI-Blocking Pseudo-Streaming
- **Location:** `rust/crates/model-gateway/src/lib.rs`, `apps/desktop/src-tauri/src/commands/ai.rs`
- **Description:** Model completion requests lacked genuine chunked token streaming and cancellation. Long inference requests locked worker threads and could not be cancelled by the user.
- **Root Cause:** `ModelProvider` trait only supported synchronous batch completion without stream delta channels.
- **Hardening Resolution:**
  1. Defined `StreamDelta` enum (`Started`, `Chunk`, `Completed`, `Error`) and added `stream_completion` to the `ModelProvider` trait.
  2. Implemented streaming SSE parser for OpenAI-compatible endpoints and streaming chunk reader for Ollama.
  3. Added `ActiveAiStreams` in Tauri state with atomic `Arc<AtomicBool>` cancellation tokens and exposed `ai_cancel_stream`.
  4. Wired frontend "Stop" button in `AssistantPanel.tsx` to halt streaming immediately.

### BUG-05 (CRITICAL — P0): Multi-Root Workspace Ambiguity & Path Traversal
- **Location:** `apps/desktop/src-tauri/src/workspace_bridge.rs`
- **Description:** Commands resolving paths lacked multi-root isolation, allowing requests targeting one project root to inadvertently read, write, or delete files in adjacent or system directories.
- **Root Cause:** Absence of a centralized capability-backed workspace access manager in the desktop host.
- **Hardening Resolution:**
  1. Implemented `WorkspaceAccessManager` (`WorkspaceSession`) managing `AuthorizedWorkspaceRoot` instances with explicit trust flags and capability boundaries.
  2. Implemented `resolve_in_workspace` supporting multi-root prefixes (`root-0/...`, `root-1/...`), absolute path containment checks, and directory canonicalization.
  3. Added `validate_process_directory` ensuring command execution directories reside within authorized roots.

### BUG-06 (HIGH — P1): Agent False `Running` State & Workspace Scope Absence
- **Location:** `apps/desktop/src-tauri/src/commands/agents.rs`
- **Description:** Agent tasks immediately reported `AgentTaskStatus::Running` upon creation before scheduling, misleading users. Agent execution lacked explicit root anchoring and pre-task safety snapshots.
- **Root Cause:** Premature status assignment and absence of workspace capability checks during agent initialization.
- **Hardening Resolution:**
  1. New agent tasks are explicitly initialized in `AgentTaskStatus::Pending` and only transition to `Running` upon active step dispatch.
  2. Added working directory validation against authorized roots.
  3. Automatic creation of a pre-task safety checkpoint (`Pre-agent task snapshot ({role})`) prior to executing autonomous steps.

### BUG-07 (HIGH — P1): Build Command Shell Injection & Raw Process Spawning
- **Location:** `apps/desktop/src-tauri/src/commands/build.rs`
- **Description:** Arbitrary command strings supplied by the client were previously passed to shell execution without command validation, allowing potential command injection.
- **Root Cause:** Direct invocation of shell binaries without executable whitelisting or tokenization.
- **Hardening Resolution:**
  1. Created `APPROVED_BUILD_TOOLS` whitelist (`cargo`, `npm`, `npx`, `yarn`, `pnpm`, `bun`, `python`, `gradle`, `make`, etc.).
  2. Tokenized commands into structured program and argument vectors, spawning executables directly without intermediate shell interpreters.
  3. Enforced working directory containment within trusted workspace roots.
  4. Added `build_cancel_task` with full Windows process tree termination (`taskkill /F /T /PID`).

### BUG-08 (HIGH — P1): Mobile Studio Port Collisions & Zombie Metro Processes
- **Location:** `apps/desktop/src-tauri/src/commands/mobile.rs`
- **Description:** Metro bundler and EAS processes could conflict with already-bound TCP ports (e.g. 8081). Terminating the Metro dev server failed to kill child Node processes, leaving orphaned background daemons.
- **Root Cause:** Absence of TCP port pre-flight checks and single-PID process termination.
- **Hardening Resolution:**
  1. Harmonized scaffolding parameters with serde camelCase/snake_case aliases (`targetDir`, `bundleId`).
  2. Implemented TCP port availability checking (`std::net::TcpListener::bind("127.0.0.1:port")`) before launching Metro.
  3. Replaced basic process termination with recursive process tree killing (`taskkill /F /T /PID` on Windows).

### BUG-09 (HIGH — P1): Project Brain Multi-Root Scanning Blindspot
- **Location:** `apps/desktop/src-tauri/src/commands/brain.rs`
- **Description:** In multi-root workspaces, Project Brain only indexed the primary workspace directory, leaving secondary roots unmapped in code graphs and symbol searches.
- **Root Cause:** Hardcoded single-path indexing in the desktop command handler.
- **Hardening Resolution:**
  1. Updated `brain_scan_workspace` to query `ws.get_roots()` and iterate across all authorized project roots.
  2. Aggregated file records, AST symbols, and graph metrics across all roots into a unified workspace knowledge map.

### BUG-10 (HIGH — P1): Git & LSP Path Boundary Verification Deficiencies
- **Location:** `apps/desktop/src-tauri/src/commands/git.rs`, `apps/desktop/src-tauri/src/commands/lsp.rs`
- **Description:** Git and LSP operations accepted client paths without validating containment within active roots, risking interactions with unauthorized directories.
- **Root Cause:** Direct path passing to external engines without workspace boundary validation.
- **Hardening Resolution:**
  1. Git commands validate repository roots and target file paths against `WorkspaceSession::resolve_in_workspace`.
  2. LSP client verifies that URI schemas and file paths reside strictly within authorized roots before opening, updating, or saving language server documents.

### BUG-11 (MEDIUM — P2): Doctor Diagnostics Information Disclosure
- **Location:** `apps/desktop/src-tauri/src/commands/system.rs`
- **Description:** System diagnostics risked emitting sensitive environment details or API tokens in diagnostic reports.
- **Root Cause:** Unfiltered environment serialization in the doctor command.
- **Hardening Resolution:**
  1. Sanitized all doctor reports to emit structured, redacted summaries (workspace roots count, filesystem health, active process PIDs, provider availability) without exposing environment variables or keys.

---

## Verification Matrix

| Defect ID | Severity | Category | Unit / Integration Test | Status |
|:---|:---:|:---|:---|:---:|
| **BUG-01** | P0 | Filesystem / Storage | `commands::storage::tests::test_storage_candidate_whitelist` | **RESOLVED & VERIFIED** |
| **BUG-02** | P0 | Checkpoint Security | `checkpoint_engine::tests::test_checkpoint_path_traversal_and_tamper_defense` | **RESOLVED & VERIFIED** |
| **BUG-03** | P0 | AI Reliability | `commands::ai::tests::test_hash_conflict_detection` | **RESOLVED & VERIFIED** |
| **BUG-04** | P1 | AI Gateway | `model_gateway::tests` (Stream parsing & cancellation) | **RESOLVED & VERIFIED** |
| **BUG-05** | P0 | Workspace Bridge | `workspace_bridge::tests::test_workspace_session_single_root`, `test_workspace_session_multi_root` | **RESOLVED & VERIFIED** |
| **BUG-06** | P1 | Agent Runtime | `agent_orchestrator::tests` & `commands::agents` pre-task snapshots | **RESOLVED & VERIFIED** |
| **BUG-07** | P1 | Process Execution | `commands::build::tests::test_approved_build_tools_whitelist` | **RESOLVED & VERIFIED** |
| **BUG-08** | P1 | Mobile Studio | `commands::mobile` port binding & process tree kill | **RESOLVED & VERIFIED** |
| **BUG-09** | P1 | Project Brain | `commands::brain` multi-root aggregation | **RESOLVED & VERIFIED** |
| **BUG-10** | P1 | Git & LSP | `commands::git` & `commands::lsp` boundary validation | **RESOLVED & VERIFIED** |
| **BUG-11** | P2 | Diagnostics | `commands::system::app_doctor` redacted output verification | **RESOLVED & VERIFIED** |
