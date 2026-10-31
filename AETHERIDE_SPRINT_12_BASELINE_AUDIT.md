# AetherIDE Sprint 12 — Baseline Audit Report

**Date:** 2026-10-31  
**Product:** AetherIDE (`v0.1.0-beta.2`)  
**Baseline Commit:** `7fc1e146dd289ba0ef3f460bac43cbdfb029e731` (Merge PR #4 into `main`)  
**Current Development Branch:** `feat/sprint-12-beta2-certification`  
**Toolchain:** `rustc 1.93.1 (01f6ddf75 2026-02-11)`, `cargo 1.93.1`  
**Repository Size:** 565.57 MiB git pack, 0 bytes in-tree build target cache  

---

## 1. Executive Summary & Reproduction Status

Gate 0 baseline reproduction was executed using the external target cache directory `$env:LOCALAPPDATA\AetherIDE\build-cache\aetheride\windows-x64`.

The reproduction verified:
- **Git State:** PR #4 successfully merged into `main` at `7fc1e14`. Branch `feat/sprint-12-beta2-certification` cleanly branched from `main`.
- **Formatting:** `cargo fmt --all -- --check` passed with 0 diffs.
- **Linter:** `cargo clippy --workspace --all-targets -- -D warnings` passed with 0 warnings across all 24 crates.
- **Compilation:** `cargo check --workspace --all-targets` passed cleanly.
- **Unit & Integration Tests:** 132 automated tests passed across the 24 workspace crates (0 failures, 0 ignored, 0 skipped).
- **CI Status:** GitHub Actions PR #4 passed all 5 jobs (Code Quality, Frontend Build, Linux x64 GNU, macOS ARM64 Apple Silicon, Windows x64 MSVC).

---

## 2. Workspace Crate Architecture (24 Crates)

| # | Crate Name | Path | Responsibility | Baseline Tests |
|---|---|---|---|:---:|
| 1 | `application-core` | `rust/crates/application-core` | Core domain types, errors, configuration | 4 |
| 2 | `ipc-protocol` | `rust/crates/ipc-protocol` | JSON-RPC & CBOR IPC envelope framing | 6 |
| 3 | `security-core` | `rust/crates/security-core` | DPAPI/Vault credential store, tokens, RBAC | 10 |
| 4 | `permission-engine` | `rust/crates/permission-engine` | Autonomy modes, policy enforcement | 2 |
| 5 | `database-engine` | `rust/crates/database-engine` | SQLite WAL + FTS5 search & telemetry | 6 |
| 6 | `filesystem-engine` | `rust/crates/filesystem-engine` | `cap-std` v4.0.3 directory capabilities & path sanitization | 19 (5 unit + 14 adv) |
| 7 | `syntax-engine` | `rust/crates/syntax-engine` | Tree-sitter AST & symbol extraction | 3 |
| 8 | `symbol-index` | `rust/crates/symbol-index` | In-memory & FTS5 symbol indexing | 2 |
| 9 | `project-indexer` | `rust/crates/project-indexer` | Project workspace background indexing | 2 |
| 10 | `process-manager` | `rust/crates/process-manager` | Job Object & process isolation | 2 |
| 11 | `git-engine` | `rust/crates/git-engine` | Git status, commits, branch handling | 2 |
| 12 | `terminal-manager` | `rust/crates/terminal-manager` | Windows ConPTY & Unix PTY | 5 (4 unit + 1 integ) |
| 13 | `model-gateway` | `rust/crates/model-gateway` | Multi-provider AI streaming gateway | 3 |
| 14 | `agent-runtime` | `rust/crates/agent-runtime` | Autonomous agent execution loops | 5 |
| 15 | `agent-orchestrator` | `rust/crates/agent-orchestrator` | Multi-agent swarms & tool approval | 6 |
| 16 | `project-brain` | `rust/crates/project-brain` | Knowledge graph & architecture maps | 3 |
| 17 | `checkpoint-engine` | `rust/crates/checkpoint-engine` | File undo/redo snapshots & rollbacks | 4 |
| 18 | `diagnostics` | `rust/crates/diagnostics` | Environment doctor & health diagnostics | 1 |
| 19 | `mobile-build-manager` | `rust/crates/mobile-build-manager` | React Native & Expo build previews | 0 |
| 20 | `lsp-client` | `rust/crates/lsp-client` | Language Server Protocol multiplexer | 3 |
| 21 | `workspace-sync` | `rust/crates/workspace-sync` | Yrs CRDT, AEAD channels, P2P sync | 19 (14 unit + 5 integ) |
| 22 | `aether-daemon` | `rust/services/aether-daemon` | Core IDE background daemon | 20 (19 unit + 1 e2e) |
| 23 | `aether-cli` | `rust/cli/aether-cli` | Developer CLI tool | 0 |
| 24 | `aether-launcher` | `rust/cli/aether-launcher` | Native host WebView2/WebKit launcher | 7 |
| **Total** | **24 Crates** | | | **132 Passed** |

---

## 3. Existing Security Findings (Sprint 12 Scope)

1. **P0 Insecure Non-Windows Credential Protection:**
   - In `rust/crates/security-core/src/lib.rs`, `protect_unix_fallback` uses a SHA-256 digest of the `USER` environment variable with a repeating XOR mask. This is neither encryption nor secure storage. It must be replaced with native macOS Keychain and Linux Secret Service integration.
2. **P0 ProjectIndexer Ambient Filesystem Bypass:**
   - `ProjectIndexer::index_workspace` calls `FilesystemEngine::scan_workspace_files`, converts results to absolute paths, and re-reads via `FilesystemEngine::read_file_string`. This circumvents `cap-std` capability isolation.
3. **P0 ProjectBrain Ambient Traversal Bypass:**
   - `ProjectBrain::scan_workspace` performs its own directory traversal with `std::fs::read_dir`, `Path::is_dir`, and `std::fs::read_to_string`, completely bypassing capability enforcement.
4. **P0 Generic Ambient Helpers in FilesystemEngine:**
   - `FilesystemEngine::read_file_string` and `FilesystemEngine::write_file_atomic` accept arbitrary paths without workspace capability anchoring.
5. **P1 SessionRecord Secret Exposure:**
   - `SessionRecord` stores `session_token` directly and derives `Debug`, `Serialize`, `Deserialize`, posing accidental leakage risks during logging or diagnostics export.
6. **P1 GitHub Actions Maintenance:**
   - CI workflow steps utilize actions with Node.js 20 deprecation warnings.

---

## 4. Quality Gate Status

- [x] Baseline Reproduction: **PASS** (132 tests, 0 warnings, clean fmt)
- [ ] P0 Credential Vault Implementation: **PENDING**
- [ ] P0 End-to-End Capability Filesystem: **PENDING**
- [ ] P1 Rate Limiting Transport Verification: **PENDING**
- [ ] P1 Secret Lifetime & Diagnostics Review: **PENDING**
- [ ] P1 CI Dependency Updates: **PENDING**
- [ ] Sprint 12 Engineering Reports: **PENDING**
