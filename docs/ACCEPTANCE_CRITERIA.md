# AetherIDE — Acceptance Criteria & Definition of Done

## 1. Release Quality Gates
Each phase and subsystem must satisfy rigorous, verifiable criteria before it is marked as `Verified`. No mock APIs, phantom buttons, or speculative pass states are permitted.

---

## 2. Subsystem Acceptance Criteria

### 2.1 Rust Core Foundation (`Phase 1`)
- [ ] Cargo workspace compiles cleanly with zero clippy warnings (`cargo clippy --workspace --all-targets -- -D warnings`).
- [ ] Native named pipe server launches on Windows and successfully responds to ping/pong and method calls with `< 1ms` latency.
- [ ] SQLite database initializes in WAL mode, applies migrations cleanly, and indexes 1,000+ symbols into FTS5.
- [ ] Native process manager spawns processes, captures stdout/stderr streams, enforces timeouts, and handles graceful cancellation.
- [ ] Unit tests pass with `100%` success rate (`cargo test --workspace`).

### 2.2 Windows Desktop Foundation (`Phase 2`)
- [ ] Desktop IDE application launches without errors on Windows 10/11 x64.
- [ ] Connects seamlessly to the local `aether-daemon` over Named Pipe IPC.
- [ ] User can open workspaces, create files, edit files, and save files with changes instantly synced.
- [ ] Integrated terminal runs PowerShell, CMD, and Git Bash with full interactive streaming.
- [ ] Git view displays active changes, branch names, staging, and commit functionality.

### 2.3 Advanced IDE Capabilities (`Phase 3`)
- [ ] Incremental Tree-sitter parsing updates syntax nodes on keystrokes.
- [ ] Environment Doctor audits Rust, Node.js, Python, Git, and Windows MSVC, presenting actionable diagnostic status.
- [ ] Observability dashboard displays real-time CPU, RAM, and background task load.
- [ ] API & Database Studio performs HTTP REST requests and inspects SQLite database schemas.

### 2.4 Universal AI Model Gateway (`Phase 4`)
- [ ] Adapter interface supports Anthropic Claude, OpenAI, Google Gemini, xAI Grok, DeepSeek, and local Ollama.
- [ ] Streaming completions stream tokens smoothly without buffering or blocking the event loop.
- [ ] Intelligent Router accurately classifies queries and routes to fast, reasoning, or vision models.
- [ ] Model Arena runs side-by-side completions and displays latency and token cost metrics.

### 2.5 Autonomous AI Engineering Platform (`Phase 5`)
- [ ] 14 agent personas are registered with explicit system instructions, tool manifests, and capabilities.
- [ ] Observe, Assist, and Autopilot modes enforce strict permission boundaries (P0 to P5).
- [ ] Parallel agent runs inside an isolated Git worktree without modifying the primary working tree until approved.
- [ ] Proof of Work Ledger records full audit trail (inputs, generated diffs, command executions, test results).
- [ ] Living Project Brain visualizes file dependencies and Change Impact Radar computes blast radius.

### 2.6 Mobile App Studio & Cloud Builds (`Phase 6 & 7`)
- [ ] Mobile App Studio navigation provides Application Manager, Project Creator, Visual Designer, and Build Center.
- [ ] React Native + Expo + TypeScript project generator creates fully functional Expo projects with Expo Router.
- [ ] Visual Screen Designer provides drag-and-drop preview frames (iPhone, Android, Tablet) with bi-directional code sync.
- [ ] EAS cloud build manager generates build configurations (`eas.json`), validates project credentials, and polls build pipelines.

### 2.7 iOS Companion & Web Workspace (`Phase 8 & 9`)
- [ ] SwiftUI companion app compiles with shared Rust native core.
- [ ] Secure pairing protocol verifies connection tokens between phone and desktop.
- [ ] Remote agent monitor displays real-time task status, diffs, and approval prompts.

### 2.8 Security, Packaging & Verification (`Phase 10`)
- [ ] Secret scanner successfully detects and redacts test API keys and private keys.
- [ ] Windows portable package (`aether-ide-win64.zip`) and launcher installer are built with SHA256 checksums.
- [ ] Complete documentation suite generated and validated.
