# AetherIDE Sprint 11 — Release Manifest & Software Inventory

**Product:** AetherIDE  
**Target Release:** `v0.1.0-beta.2`  
**Git Branch:** `feat/sprint-11-beta2-release-blocker-closure`  
**Rust Toolchain:** `stable` (MSRV 1.80.0+)  
**Date:** October 31, 2026  

---

## 1. Rust Workspace Inventory (24 Crates)

| # | Crate Name | Path | Responsibility |
|---|---|---|---|
| 1 | `application-core` | `rust/crates/application-core` | Core domain types, shared configurations, error models |
| 2 | `security-core` | `rust/crates/security-core` | DPAPI/AEAD credential protection, session store, rate limits |
| 3 | `filesystem-engine` | `rust/crates/filesystem-engine` | Capability-based directory operations (`cap-std`), atomic IO |
| 4 | `database-engine` | `rust/crates/database-engine` | SQLite WAL connection pool, FTS5 migrations, schema management |
| 5 | `syntax-engine` | `rust/crates/syntax-engine` | Tree-sitter AST parsing, syntax highlighting, token analysis |
| 6 | `permission-engine` | `rust/crates/permission-engine` | File and system capability privilege arbitration |
| 7 | `process-manager` | `rust/crates/process-manager` | Subprocess supervision, job groups, process tree lifecycle |
| 8 | `terminal-manager` | `rust/crates/terminal-manager` | Windows ConPTY / Unix PTY multiplexer and streams |
| 9 | `git-engine` | `rust/crates/git-engine` | Git status, diff, log, staging, and commit operations |
| 10 | `mobile-build-manager`| `rust/crates/mobile-build-manager` | Expo CLI integration, React Native & device bridges |
| 11 | `diagnostics` | `rust/crates/diagnostics` | Telemetry, structured tracing, and diagnostic metrics |
| 12 | `ipc-protocol` | `rust/crates/ipc-protocol` | Typed IPC requests, responses, and serialization envelopes |
| 13 | `symbol-index` | `rust/crates/symbol-index` | Fast in-memory symbol lookup and definition mapping |
| 14 | `checkpoint-engine` | `rust/crates/checkpoint-engine` | Workspace snapshots, undo history, and rollback states |
| 15 | `project-brain` | `rust/crates/project-brain` | Developer context graph, memory notes, and architectural summaries |
| 16 | `lsp-client` | `rust/crates/lsp-client` | Language Server Protocol multiplexer (Rust, TS, Python) |
| 17 | `workspace-sync` | `rust/crates/workspace-sync` | Yrs/Yjs CRDT collaboration, awareness, encrypted sync |
| 18 | `project-indexer` | `rust/crates/project-indexer` | Background full-text workspace indexer |
| 19 | `model-gateway` | `rust/crates/model-gateway` | Multi-provider LLM gateway (Anthropic, OpenAI, Gemini, Ollama) |
| 20 | `agent-runtime` | `rust/crates/agent-runtime` | Autonomous agent execution environment and tools |
| 21 | `agent-orchestrator` | `rust/crates/agent-orchestrator` | Swarm planning, coordination, and consensus |
| 22 | `aether-cli` | `rust/cli/aether-cli` | Developer CLI tool for daemon control and diagnostics |
| 23 | `aether-launcher` | `rust/cli/aether-launcher` | Desktop application bootstrapper and process supervisor |
| 24 | `aether-daemon` | `rust/services/aether-daemon` | Core background services, IPC server, collaborative daemon |

---

## 2. Desktop Host & Frontend Packages

| Component | Path | Technology |
|---|---|---|
| Desktop Host | `rust/src/main.rs` | Rust + Win32 + WebView2 / WebKitGTK / WKWebView |
| Desktop Workbench | `apps/desktop` | TypeScript, Monaco Editor, xterm.js, Cytoscape.js |
| Mobile Companion | `apps/mobile` | React Native + Expo + iOS SwiftUI Companion |

---

## 3. Binary Artifacts (Release Profile)
- `aetheride.exe` (Desktop Application Host)
- `aether-daemon.exe` (Background System Daemon)
- `aether-launcher.exe` (Secure Application Launcher)
- `aether.exe` (Developer CLI Utility)
