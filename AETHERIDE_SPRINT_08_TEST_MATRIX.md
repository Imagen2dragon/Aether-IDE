# AETHERIDE — SPRINT 08 COMPREHENSIVE TEST MATRIX & SCENARIO MAPPING
**Document Version:** 1.0.0  
**Date:** October 31, 2026  
**Status:** Active Execution Matrix  
**Sprint:** Sprint 08 Beta Release Validation  
**Repository:** `AetherIDE`  

---

## 1. End-to-End Acceptance Scenarios Mapping

| ID | Scenario Description | Subsystem(s) | Expected Outcome | Baseline Status | Sprint 08 Target |
|:---:|:---|:---|:---|:---:|:---:|
| **E2E-01** | Install and first launch | Packaging, `aether-launcher`, WebView2 | Installed app launches, loads workbench without external CDN | IMPLEMENTED — UNVERIFIED | **PASS** |
| **E2E-02** | Open and edit project | `win32_host`, `filesystem-engine`, Monaco | Multi-file opening, dirty state, atomic save, restart persistence | PASS | **PASS** |
| **E2E-03** | Language tooling | `lsp-client`, `syntax-engine` | Real LSP completion, diagnostics, hover, go-to-definition | PASS | **PASS** |
| **E2E-04** | Terminal studio | `terminal-manager`, ConPTY | Interactive shell session, output streaming, ScopedJobObject cleanup | PASS | **PASS** |
| **E2E-05** | Git operations | `git-engine` | Real repository detection, status, diffing, commit creation | PASS | **PASS** |
| **E2E-06** | AI model provider | `model-gateway` | Provider streaming, error handling, token metadata, secret redaction | PASS | **PASS** |
| **E2E-07** | Agent task execution | `agent-orchestrator`, `agent-runtime` | Multi-step task planning, tool execution, proof-of-work recording | PASS | **PASS** |
| **E2E-08** | Multi-agent swarm work | `agent-orchestrator::swarm` | Isolated worktrees, cycle prevention, 3-way merge conflict handling | PASS | **PASS** |
| **E2E-09** | Collaboration | `workspace-sync` | Two independent clients, state vector exchange, Yrs convergence | PASS | **PASS** |
| **E2E-10** | Checkpoint restore | `checkpoint-engine` | State restoration preserving unrelated files | PASS | **PASS** |
| **E2E-11** | Database online backup | `database-engine` | `sqlite3_backup` under concurrent writes, full restoration test | PASS | **PASS** |
| **E2E-12** | Update and rollback | `application-core::update` | Valid update succeeds; failed update triggers automatic rollback | PASS | **PASS** |
| **E2E-13** | Installer lifecycle | `packaging/` | Clean install, upgrade, and removal across Windows, Linux, macOS | IMPLEMENTED — UNVERIFIED | **PASS** |
| **E2E-14** | Storage budget audit | `diagnostics::storage` | In-tree size < 6.00 GB, zero in-tree target bloat in OneDrive | PASS | **PASS** |

---

## 2. Security Test Scenarios Mapping

| Test ID | Vulnerability / Target | Hardening Mechanism | Verification Method | Target Status |
|:---:|:---|:---|:---|:---:|
| **SEC-01** | Session token timing leak | SHA-256 fixed-width digest + bitwise XOR | `TokenManager::verify_token` unit tests | **PASS** |
| **SEC-02** | Brute force / stale tokens | TokenStore with expiration, rotation & rate-limiting | `test_token_store_lifecycle_and_rate_limiting` | **PASS** |
| **SEC-03** | Path traversal & symlink escape | Canonicalization + component inspection | `resolve_contained_path` regression tests | **PASS** |
| **SEC-04** | Alternate Data Streams (ADS) | Reject components containing `:` | `test_resolve_contained_path_advanced_attacks` | **PASS** |
| **SEC-05** | Trailing dots/spaces on Windows | Reject components ending with `.` or ` ` | `test_resolve_contained_path_advanced_attacks` | **PASS** |
| **SEC-06** | UNC network path traversal | Reject `\\` and `//` prefixes | `test_resolve_contained_path_advanced_attacks` | **PASS** |
| **SEC-07** | Daemon RBAC role forgery | Ignore client `role`, look up `RoleRegistry` | `test_crdt_payload_role_spoofing_rejected` | **PASS** |
| **SEC-08** | Unauthorized daemon commands | Enforce `can_write()` / `can_admin()` across all IPC actions | `test_rbac_daemon_permission_matrix` | **PASS** |
| **SEC-09** | Update signature forgery | Ed25519 digital signature check vs root key | `test_manifest_signature_verification` | **PASS** |
| **SEC-10** | Cross-platform update confusion | Bind `target_os` and `target_arch` in manifest | `test_update_manifest_target_platform_binding` | **PASS** |
