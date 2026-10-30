# AetherIDE — Master Implementation Plan

## Phase Structure & Execution Order

```
[Phase 0: Environment Audit & Architecture] -> (COMPLETED)
       │
[Phase 1: Rust Core Foundation]
       ├── Cargo Workspace & Common Crates
       ├── IPC Protocol (Named Pipe & WebSocket)
       ├── Storage Engine (SQLite WAL + FTS5)
       ├── Native Filesystem & File Watcher
       ├── Syntax Engine & Project Indexer Slice
       └── Native Daemon Service (aether-daemon)
       │
[Phase 2: Windows IDE Foundation]
       ├── Desktop Shell & Workbench Structure
       ├── Aether IDE Extension Integration
       ├── Terminal, File Explorer, Git Integration
       └── Daemon Connection & Live File Editing
       │
[Phase 3: Advanced IDE Capabilities]
       ├── Tree-sitter Code Intelligence
       ├── Environment Doctor (SDK & Toolchain Health)
       ├── Developer Profiler & Application Observability
       └── API & Database Studio
       │
[Phase 4: Universal AI Model Gateway]
       ├── Provider Adapters (Claude, OpenAI, Gemini, Grok, DeepSeek, Ollama)
       ├── Dynamic Capabilities & Streaming Engine
       ├── Intelligent Model Router
       └── Model Arena
       │
[Phase 5: Autonomous Multi-Agent Engineering Platform]
       ├── 14 Agent Role Specifications & Prompts
       ├── Agent Runtime & Execution Loop
       ├── Git Worktree Isolation
       ├── Living Project Brain & Impact Radar
       ├── Proof of Work Ledger
       └── Bug Reproduction Lab
       │
[Phase 6: Mobile App Studio]
       ├── Mobile App Studio Pane & Project Creator
       ├── React Native + Expo + TypeScript Pipeline
       ├── Visual Screen Designer & Preview
       ├── Device Capabilities Matrix
       └── Backend Builder
       │
[Phase 7: Mobile Cloud Builds & Release]
       ├── EAS Build Cloud Integration
       ├── Android Release Flow (Debug/Release APK, AAB)
       └── iOS Release Flow (IPA, TestFlight automation)
       │
[Phase 8: iOS Companion Application]
       ├── SwiftUI Companion Client
       ├── Rust UniFFI Native Bridge
       ├── Secure QR Code Pairing Protocol
       └── Remote Agent Monitor & Terminal
       │
[Phase 9: Web Workspace & Collaboration]
       ├── Web IDE Client & Dashboard
       └── Cloud Relay & Multi-Session Collaboration
       │
[Phase 10: Security, Packaging & Production Hardening]
       ├── Windows Installer (EXE, Portable ZIP, Checksums)
       ├── Verification Test Suites & Benchmarks
       └── Complete Production Documentation Suite
```

---

## Detailed Task Breakdown

### Phase 1: Rust Core Foundation
1. Initialize Cargo workspace at `rust/` with crates:
   - `application-core`
   - `ipc-protocol`
   - `filesystem-engine`
   - `file-watcher`
   - `syntax-engine`
   - `project-indexer`
   - `symbol-index`
   - `database-engine`
   - `process-manager`
   - `git-engine`
   - `terminal-manager`
   - `permission-engine`
   - `security-core`
   - `model-gateway`
   - `agent-runtime`
   - `agent-orchestrator`
   - `project-brain`
   - `mobile-build-manager`
   - `checkpoint-engine`
   - `diagnostics`
   - `aether-daemon` (service binary)
   - `aether-cli` (command line interface)
2. Implement Windows Named Pipe IPC server and cross-platform WebSocket server.
3. Implement SQLite WAL database manager with migrations and FTS5 search index.
4. Implement native process runner with timeout and cancellation support.
5. Write unit tests for IPC, database, and process manager.

### Phase 2: Windows IDE Foundation
1. Setup desktop application architecture in `apps/desktop`.
2. Implement custom Aether theme, branding, activity bar, and status bar.
3. Build the core TypeScript extension `extensions/aether-core` to communicate with `aether-daemon` over IPC.
4. Provide seamless file editing, terminal, and git integration verified with real tests.

### Phase 3: Advanced IDE Capabilities
1. Integrate Tree-sitter for real-time incremental syntax parsing.
2. Build Environment Doctor (`aether-doctor`) to audit Rust, Node.js, Python, Git, Android SDK, and MSVC toolchains.
3. Implement Application Observability & Profiler metrics stream (CPU, RAM, disk, build duration).
4. Implement API & Database Studio for REST/GraphQL testing and SQLite/PostgreSQL schema inspection.

### Phase 4: AI Model Gateway
1. Implement `ModelProvider` trait and concrete clients:
   - Anthropic (Claude 3.5 Sonnet, Haiku)
   - OpenAI (GPT-4o, o1, o3-mini)
   - Google Gemini (Gemini 2.0 Flash, 1.5 Pro)
   - xAI Grok (Grok-2)
   - DeepSeek (V3, R1)
   - Ollama / LM Studio (Local endpoints)
2. Build SSE streaming reader with token rate calculation.
3. Build Intelligent Router evaluating prompt complexity and budget.
4. Implement Model Arena side-by-side benchmark runner.

### Phase 5: Autonomous Multi-Agent Engineering
1. Create 14 agent personas with strict role prompts and tool manifests.
2. Implement Autonomy Levels (Observe, Assist, Autopilot).
3. Implement Git worktree isolation for concurrent tasks.
4. Build Living Project Brain AST/dependency graph generator.
5. Implement Change Impact Radar blast radius calculator.
6. Implement Proof of Work Ledger recording diffs, execution logs, and test evidence.

### Phase 6 & 7: Mobile App Studio & Cloud Builds
1. Build Mobile App Studio studio interface in `mobile-studio/`.
2. Implement React Native + Expo + TypeScript project generator with Expo Router.
3. Build Visual Screen Designer component palette and responsive device canvas.
4. Integrate EAS (Expo Application Services) build runner with APK/AAB and IPA profiles.

### Phase 8 & 9: iOS Companion & Web Workspace
1. Create SwiftUI companion project in `apps/ios-companion` with Rust UniFFI bridge.
2. Implement zero-trust pairing handshake via signed tokens.
3. Create Web workspace interface in `apps/web-workspace`.

### Phase 10: Hardening, Packaging & Verification
1. Run Rust test suites and clippy checks across all crates.
2. Package Windows portable bundle and installer scripts.
3. Generate comprehensive documentation in `docs/` and root README.
