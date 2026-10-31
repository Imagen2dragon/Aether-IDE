# AetherIDE — Sprint 06 Architecture Specification
## Cross-Platform Systems, Storage-Aware Compilation, CRDT Collaboration and Swarm Coordination

---

## 1. System Topology Overview

Sprint 06 advances the AetherIDE architecture across three core system planes:

```
+---------------------------------------------------------------------------------------+
|                               AETHERIDE RUNTIME ENVIRONMENT                           |
+---------------------------------------------------------------------------------------+
| [UI Workbench]                                                                        |
|  - Monaco Editor (vs/loader.js)        - Xterm.js ANSI Terminal Studio                |
|  - Cytoscape.js Project Brain Graph    - Build & Storage Center Dashboard             |
+---------------------------------------------------------------------------------------+
| [Desktop Native Host Plane] (DesktopHost Trait)                                       |
|  - Windows: Win32 HWND + Microsoft WebView2 Controller                                |
|  - Linux: Native Linux Windowing + WebKitGTK WebKitWebView                            |
|  - macOS: Cocoa NSWindow + Apple WebKit WKWebView                                     |
+---------------------------------------------------------------------------------------+
| [Storage Optimization Plane]                                                          |
|  - In-Tree OneDrive Footprint: 2.49 GB (Cleaned & Zero-Locked)                         |
|  - External Build Cache: %LOCALAPPDATA%\AetherIDE\build-cache\                         |
|  - Profile Strategy: opt-level=0, debug=1 (line-tables-only), strip="symbols"          |
+---------------------------------------------------------------------------------------+
| [Core Systems Plane - 24 Crates]                                                      |
|  - terminal-manager: TerminalBackend trait -> ConPtySession / UnixPtySession          |
|  - workspace-sync: Yrs Yjs V1 Doc -> AwarenessTracker -> Ed25519 DeviceIdentity       |
|  - agent-orchestrator: SwarmTaskGraph DAG -> SwarmCoordinator -> ThreeWayMergeEngine  |
|  - application-core: UpdateVerifier -> SHA256 Integrity -> Semver Rollback Guard     |
+---------------------------------------------------------------------------------------+
```

---

## 2. Abstraction Layers

### 2.1 TerminalBackend Trait (`crates/terminal-manager/src/backend.rs`)

```rust
pub trait TerminalBackend: Send + Sync {
    fn write_input(&self, data: &[u8]) -> Result<usize>;
    fn read_output(&self, offset: usize) -> (Vec<u8>, usize, bool);
    fn resize(&self, cols: u16, rows: u16) -> Result<()>;
    fn close(&self) -> Result<()>;
    fn pid(&self) -> u32;
    fn is_alive(&self) -> bool;
    fn backend_name(&self) -> &'static str;
}
```

- **Windows Implementation:** `Mutex<ConPtySession>` wrapping Win32 `CreatePseudoConsole`, HPCON handles, and Win32 Job Object isolation.
- **Unix/macOS Implementation:** `UnixPtySession` wrapping POSIX PTY allocation, file descriptor pipes, and non-blocking IO.

### 2.2 DesktopHost Trait (`cli/aether-launcher/src/host.rs`)

```rust
pub trait DesktopHost {
    fn run(&self) -> Result<(), Box<dyn std::error::Error>>;
    fn platform_name(&self) -> &'static str;
}
```

- **Windows:** `Win32WebView2Host` with embedded Win32 message loop, COM apartment threading, and bidirectional web-message bridge.
- **Linux:** `LinuxGtkHost` initializing GTK3 event loops and WebKitGTK webview instances.
- **macOS:** `MacWkWebViewHost` initializing Cocoa NSApplication and WKWebView frames.

---

## 3. Real-Time CRDT Architecture (`crates/workspace-sync/src/crdt.rs`)

- **Underlying Engine:** Yrs (Yjs V1 compatible).
- **Wire Protocol:**
  - State Vectors encoded via `StateVector::encode_v1()` (Base64 transport).
  - Updates encoded via `Transaction::encode_state_as_update_v1()` (Base64 transport).
  - Delta updates computed via `Transaction::encode_diff_v1(remote_sv)`.
- **Convergence Guarantee:** Commutative, associative, and idempotent mathematical properties guarantee deterministic multi-peer document convergence without centralized locks.
- **Presence & Awareness:** `AwarenessTracker` maintains peer IDs, display names, user colors, file URIs, line/column coordinates, and selection ranges, pruning inactive peers after 30 seconds of inactivity.

---

## 4. Distributed Autonomous Agent Swarms (`crates/agent-orchestrator/src/swarm.rs`)

- **Task Dependency DAG (`SwarmTaskGraph`):**
  - Directed Acyclic Graph tracking nodes, dependencies, and execution states.
  - Cycle detection using Kahn's topological sorting algorithm guarantees deadlocks are prevented before scheduling.
- **Worktree Isolation:**
  - Each agent task executes in a dedicated Git worktree (`.aether/worktrees/agent-<persona>-<task_id>`).
  - Completely prevents write collisions on identical files between concurrent personas.
- **Three-Way Merge Engine (`crates/agent-orchestrator/src/merge.rs`):**
  - Line-by-line comparison of Base vs Ours (Workspace) vs Theirs (Agent Worktree).
  - Clean hunks merged automatically.
  - Conflicting hunks isolated with visual Git conflict markers (`<<<<<<< OURS`, `||||||| BASE`, `=======`, `>>>>>>> THEIRS`).
  - Supports `AcceptOurs`, `AcceptTheirs`, `AcceptBoth`, and `Custom` resolution strategies.
