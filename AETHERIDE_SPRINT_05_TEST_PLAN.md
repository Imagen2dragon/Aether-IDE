# AETHERIDE — SPRINT 05 TEST PLAN
## Interactive Terminal Studio, Living Project Brain, Autonomous Agent Stepper and Mobile Live Preview

**Version:** 1.0.0  
**Author:** QA Director & Systems Test Architect  
**Date:** 2026-10-31  
**Project:** AetherIDE  

---

## 1. Test Strategy & Scope

Sprint 05 introduces four major interactive subsystems that operate directly against real backend services. Test verification must ensure:
1. **Interactive Terminal Studio:**
   - Real ConPTY process lifecycle (PowerShell, CMD).
   - Multi-session independence and split-pane resizing.
   - User input transmission and ANSI/VT100 streaming with zero data corruption.
   - Clean Win32 Job Object process-tree termination on session closure.
2. **Living Project Brain & Radar:**
   - Graph generation from actual workspace file ASTs and symbols.
   - Accurate direct and transitive dependency mapping in the Impact Radar.
   - Node click navigation jumping to the exact line/col in Monaco.
   - Persistent Project Memory storage in SQLite.
3. **Autonomous Agent Command Center:**
   - Real-time streaming of agent execution events.
   - Strict server-side validation of approvals (`ApproveAgentStep`, `RejectAgentStep`).
   - Prevention of duplicate/stale approvals and rejection of unauthorized self-approvals.
   - Review of real Git diffs and test evidence before integration.
4. **Mobile Studio Device Preview:**
   - Expo development server lifecycle via `process-manager`.
   - Isolated web preview rendering without native bridge access.
   - Real QR code URI generation from detected network interface.
   - Accurate reporting of Android emulator/device status via ADB.
5. **Non-Regression Baseline:**
   - All 64 existing Rust tests must remain passing with zero failures.
   - Zero Clippy warnings across all workspace targets with `-D warnings`.
   - Zero Rustfmt diffs.

---

## 2. Test Execution Matrix

| Subsystem | Test Case | Target / Expected Behavior | Verification Command / Method |
|---|---|---|---|
| **Terminal Studio** | ConPTY Interactive Spawn | Spawns `powershell.exe` session with valid PID | `cargo test -p terminal-manager` |
| **Terminal Studio** | Multi-Session Independence | Session 1 and Session 2 maintain isolated I/O | Multi-session test in `terminal-manager` |
| **Terminal Studio** | Window / ConPTY Resize | Resizing pane triggers `ResizePseudoConsole` | `test_terminal_resize` |
| **Terminal Studio** | Clean Process Termination | Closing session kills child processes via Job Object | `test_conpty_interactive_cmd_lifecycle` |
| **Project Brain** | AST Graph Construction | Extracts nodes/edges from Rust workspace files | `cargo test -p project-brain` |
| **Project Brain** | Impact Radar Calculation | Reports direct/transitive dependents and risk score | `test_project_brain_and_impact_radar` |
| **Project Brain** | Monaco Source Navigation | Graph node click navigates to source URI and line | UI integration test / Monaco jump |
| **Project Brain** | Living Memory Persistence | Saves and retrieves architectural notes from SQLite | `test_project_memory_crud` |
| **Agent Stepper** | Persona Stepper Event Flow | Transitions from Planned -> WaitingApproval -> Done | `test_agent_runtime_step_execution` |
| **Agent Stepper** | Server-Side Approval Guard | Unapproved steps block execution in Observe mode | `test_observe_mode_blocks_writes` |
| **Agent Stepper** | Rejection and Task Cancel | Rejection aborts tool execution; state marked Cancelled | `test_agent_rejection_and_cancellation` |
| **Agent Stepper** | Git Diff & Proof of Work | Diff and exit code recorded in `ProofOfWorkRecord` | `test_proof_of_work_ledger` |
| **Mobile Studio** | Expo Project Scaffold | Generates valid Expo 52 structure and `package.json` | `test_expo_scaffold_and_config_generation` |
| **Mobile Studio** | Development Server Lifecycle | Starts/stops Metro server and parses port | `test_expo_dev_server_lifecycle` |
| **Mobile Studio** | Isolated Web Preview | Web preview iframe runs without native bridge access | Security origin audit |
| **Mobile Studio** | ADB Device Discovery | Queries ADB for attached devices/emulators | `test_adb_discovery` |
| **Regression** | Full Workspace Build | All 24 crates compile without errors | `cargo build --manifest-path rust/Cargo.toml` |
| **Regression** | Full Workspace Test Suite | All automated tests pass with 0 failures | `cargo test --manifest-path rust/Cargo.toml --workspace --all-targets` |
| **Regression** | Workspace Clippy Check | Zero warnings across all workspace crates | `cargo clippy --workspace --all-targets -- -D warnings` |
