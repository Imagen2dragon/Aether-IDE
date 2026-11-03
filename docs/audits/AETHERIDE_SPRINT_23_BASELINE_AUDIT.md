# AetherIDE — Sprint 23 Baseline Audit
**Date:** 2026-11-04  
**Sprint:** Sprint 23 — Release Candidate Hardening, Agent Security, Checkpoint Integrity & Genuine AI Streaming  
**Base Commit:** `757d9651267d61c41343612133ed16d5122bab43` (`main`)  
**Target Branch:** `feat/sprint-23-release-candidate-hardening`  

---

## 1. Executive Summary

Following the completion and merge of Sprint 22 (`757d965`), a rigorous source-level security and reliability audit was conducted across the AetherIDE Tauri desktop application host, Rust systems crates, and React 19 workbench.

The audit verified that while Sprint 22 established foundational security barriers (such as storage candidate whitelisting and central workspace bridging), critical vulnerabilities remain at the final execution boundaries:
1. **Agent Tool Dispatch:** `agent-runtime` operates with naive string matching for path traversal and accepts model-supplied `cwd` paths into shell execution without workspace capability bounds.
2. **Build Execution:** Windows batch shims (`npm`, `npx`, `yarn`, `pnpm`) still invoke `cmd.exe /C`, and argument tokenization relies on `split_whitespace()`.
3. **Checkpoint Restoration:** Restores use `String::from_utf8_lossy()`, corrupting binary files, and conflict detection in `restore_checkpoint_into` is not enforced (conflicts are silently overwritten).
4. **AI Token Streaming:** The `ModelProvider` trait provides a default pseudo-streaming fallback that calls `complete()` and emits the entire buffer at once. Several providers (Anthropic, Gemini, DeepSeek, xAI) rely on this fake streaming.
5. **Cancellation:** Stream cancellation does not abort the underlying network connection handle.

---

## 2. Comprehensive Baseline Audit Register

| Issue ID | Specific Behavior Investigated | Source File & Function | Actual Status | Impact | Correction Required | Verification Strategy |
|:---|:---|:---|:---:|:---|:---|:---|
| **AUDIT-01** | Agent path traversal & escaping workspace root | `rust/crates/agent-runtime/src/lib.rs`<br>`ToolDispatcher::resolve_safe_path` | **Vulnerable** | Model-supplied paths can escape workspace boundaries via Windows absolute paths or symlinks/junctions. | Anchor all file operations to `WorkspaceCapability` with canonical boundary verification and symlink refusal. | SEC-001, SEC-002, SEC-003 regression unit tests. |
| **AUDIT-02** | Agent arbitrary process working directory (`cwd`) | `rust/crates/agent-runtime/src/lib.rs`<br>`ToolDispatcher::dispatch("run_command")` | **Vulnerable** | Model can pass arbitrary host directories (e.g. `C:\Windows`) as `cwd` to `run_command`. | Require workspace-relative subdirectories only; validate strictly against authorized root; reject model `cwd` override. | SEC-004 test verifying rejection of unauthorized `cwd`. |
| **AUDIT-03** | Agent command execution invokes raw shell | `rust/crates/process-manager/src/lib.rs`<br>`ProcessManager::run_command_streaming` | **Vulnerable** | Uses `cmd.exe /C` on Windows and `sh -c` on Unix, opening shell injection risks. | Implement direct executable execution without implicit shell fallback; restrict agent tools to authorized binaries. | SEC-001 & process manager regression tests. |
| **AUDIT-04** | Agent tasks lack immutable execution context | `rust/crates/agent-runtime/src/lib.rs`<br>`AgentTask`, `ToolDispatcher` | **Vulnerable** | Tools independently resolve roots rather than inheriting session ID, root IDs, capabilities, and trust state. | Introduce `AgentExecutionContext` passed to every tool dispatch call. | SEC-005 & SEC-006 multi-root and concurrency tests. |
| **AUDIT-05** | Build runner whitespace splitting | `apps/desktop/src-tauri/src/commands/build.rs`<br>`build_run_task` | **Vulnerable** | `command.split_whitespace()` breaks arguments with spaces, quotes, and Unicode. | Implement typed `BuildCommand { tool, args, workspace_root_id, working_directory }` with literal argument vectors. | BLD-003 argument preservation test. |
| **AUDIT-06** | Windows package managers invoke `cmd.exe /C` | `apps/desktop/src-tauri/src/commands/build.rs`<br>`build_run_task` lines 201-207 | **Vulnerable** | Invoking `cmd.exe /C` allows command injection via `&`, `\|`, `;`, etc. | Implement direct Node-based executable adapters for npm/npx/yarn/pnpm and direct java wrapper for Gradle. | BLD-001, BLD-004 command injection adversarial tests. |
| **AUDIT-07** | Checkpoint restore corrupts binary files | `rust/crates/checkpoint-engine/src/lib.rs`<br>`restore_checkpoint_into` line 384 | **Vulnerable** | Uses `String::from_utf8_lossy(&blob_bytes)`, permanently corrupting PNG, ZIP, binary files. | Implement byte-preserving atomic write APIs (`write_file_bytes_atomic`). | CHK-001 (PNG) and CHK-002 (invalid UTF-8) tests. |
| **AUDIT-08** | Checkpoint conflicts ignored on restore | `rust/crates/checkpoint-engine/src/lib.rs`<br>`restore_checkpoint_into` line 331 | **Vulnerable** | Unused `_force` flag; conflicts detected in preview are silently overwritten on disk. | Enforce "Abort on unresolved conflict" default; require explicit user override list and one-use restore token. | CHK-003 and CHK-004 conflict enforcement tests. |
| **AUDIT-09** | Fake streaming fallback in `ModelProvider` trait | `rust/crates/model-gateway/src/lib.rs`<br>`ModelProvider::stream_completion` | **Vulnerable** | Default implementation blocks on `complete()` then emits entire string as single fake chunk. | Remove misleading default fallback; implement native streaming for all advertised providers or return explicit `UnsupportedStreaming`. | AI-001 and AI-004 real chunk streaming tests. |
| **AUDIT-10** | Missing native streaming in Anthropic & Gemini | `rust/crates/model-gateway/src/lib.rs`<br>`AnthropicProvider`, `GoogleGeminiProvider` | **Vulnerable** | Providers rely on default trait method, blocking until full inference completes. | Implement Anthropic SSE streaming parser and Gemini chunk streaming. | AI-001 streaming tests with mock SSE endpoints. |
| **AUDIT-11** | Stream cancellation does not abort network requests | `rust/crates/model-gateway/src/lib.rs`<br>`execute_stream` & `apps/desktop/src-tauri/src/commands/ai.rs` | **Partially Fixed** | `ai_cancel_stream` sets atomic boolean, but does not abort in-flight HTTP request stream. | Propagate cancellation to `reqwest` stream abort handle / tokio abort handle. | AI-002 cancellation test proving request abort. |
| **AUDIT-12** | Assistant UI missing streaming state granularity | `apps/desktop/src/features/ai-assistant/AssistantPanel.tsx` | **Partially Fixed** | UI lacks explicit states for `connecting`, `waiting_first_token`, `unsupported_streaming`, `cancelled`. | Update UI state machine to reflect true provider streaming states. | Frontend state verification. |

---

## 3. Implementation Order & Target Architecture

1. **Phase 1 (P0): Agent Workspace Isolation:**
   - Define `AgentExecutionContext` in `agent-runtime`.
   - Update `ToolDispatcher` to require `AgentExecutionContext` and enforce capabilities: `authorize_read`, `authorize_write`, `authorize_delete`, `authorize_process`.
   - Update `ProcessManager` to eliminate unvetted shell execution.
2. **Phase 2 (P0): Build Command Execution:**
   - Introduce `BuildCommand` with typed `tool` and literal `args: Vec<String>`.
   - Create safe executable resolution adapters for Windows package managers (`node.exe` + CLI entrypoints, direct executables).
   - Strict process tree termination and cancellation.
3. **Phase 3 (P0): Byte-Perfect & Conflict-Safe Checkpoints:**
   - Add `write_file_bytes_atomic` to `WorkspaceCapability` and `FilesystemEngine`.
   - Update `CheckpointEngine::restore_checkpoint_into` to use raw bytes.
   - Implement `RestoreToken` with pre-restore hash verification, conflict detection, and default fail-closed abort.
4. **Phase 4 (P1): Genuine AI Streaming & Cancellation:**
   - Remove misleading `stream_completion` default implementation from `ModelProvider`.
   - Implement native SSE streaming for Anthropic, Gemini, DeepSeek, and xAI.
   - Wire HTTP connection abort into `ai_cancel_stream`.
   - Update `AssistantPanel.tsx` streaming state machine.
5. **Phase 5 (P1): Adversarial Regression Testing:**
   - Author tests for all 28 scenarios in `AETHERIDE_SPRINT_23_ADVERSARIAL_TEST_MATRIX.md`.
6. **Phase 6 (P1): Release Candidate Validation & Packaging:**
   - Full test suite, installer generation, end-to-end execution, release artifacts and documentation.
