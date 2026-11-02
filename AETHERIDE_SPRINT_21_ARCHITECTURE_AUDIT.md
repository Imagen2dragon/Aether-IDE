# AETHERIDE — SPRINT 21 ARCHITECTURE AUDIT & IMPLEMENTATION MATRIX
## Full AI-Native IDE Activation & React Workbench Engineering

**Branch:** `feat/sprint-21-full-ai-ide-integration`  
**Base:** `main` (commit `14b078c`, merged PR #12 & PR #13)  
**Shell:** Tauri 2  
**Frontend:** React 19 + TypeScript  
**Editor:** Monaco Editor (offline bundled)  
**Backend:** Rust (24 workspace crates)  
**Date:** November 2026  

---

## 1. EXECUTIVE MISSION & AUDIT OBJECTIVE

The purpose of this audit is to catalog every functional capability of AetherIDE across the Rust backend crates and the desktop frontend, identifying what is already built, what Tauri commands exist, what integration gaps remain, and the exact contracts required to replace all simulated or hardcoded paths with authentic backend-driven engineering workflows.

---

## 2. SUBSYSTEM INVENTORY & INTEGRATION MATRIX

### 2.1 Workspace & Filesystem Engine
- **Existing Rust Implementation:**
  - `rust/crates/filesystem-engine`: `FilesystemEngine` provides atomic file writes (`write_file_atomic`), capability sandboxing (`WorkspaceCapability`), directory traversal, recursive size calculation, boundary validation (`is_within_boundary`).
  - `rust/crates/security-core`: Enforces path sanitization, traversal protection, permission tokens.
  - `apps/desktop/src-tauri/src/workspace_bridge.rs`: `WorkspaceSession` maintains multi-root workspaces, canonicalized roots, and capability containment.
- **Existing Frontend Implementation:**
  - Monolithic `index.html` File Explorer tree with basic folder listing, open folder, add folder, open file, save file.
- **Existing Tauri Commands:**
  - `workspace_open_folder`, `workspace_add_folder`, `workspace_get_tree`, `workspace_open_file`, `workspace_save_file`, `workspace_save_file_as`, `workspace_create_file`, `workspace_create_folder`, `workspace_rename_entry`, `workspace_delete_entry`, `workspace_reveal_in_explorer`, `workspace_set_trust`.
- **Missing Connection:**
  - Modular React component tree for Explorer (`ExplorerTree.tsx`, `FileItem.tsx`).
  - Automatic re-indexing on workspace open or folder changes.
- **Required Tests:**
  - Multi-root folder opening, directory traversal, capability violation rejection, atomic file updates.

---

### 2.2 Project Brain (Architecture Graph & AST Scanner)
- **Existing Rust Implementation:**
  - `rust/crates/project-brain`: `ProjectBrain` contains `reconcile_workspace(root: &Path)` and `scan_workspace(root)`. Scans active files (`.rs`, `.ts`, `.tsx`, `.js`, `.jsx`, `.toml`, `.json`), generates `BrainNode` (modules, components, tests, files), creates `BrainEdge` (`imports`, `depends_on`), prunes stale nodes/edges, and outputs `BrainReconcileReport`.
- **Existing Frontend Implementation:**
  - Project Brain view with Cytoscape.js rendering static, hardcoded nodes (`aether-daemon`, `aether-cli`, `kernel-core`).
- **Existing Tauri Commands:**
  - None (previously hardcoded in frontend).
- **Missing Connection:**
  - Tauri command `brain_scan_workspace(path)`: Executes `ProjectBrain::reconcile_workspace`, returning real nodes and edges.
  - Tauri command `brain_get_graph()`: Returns active graph state.
  - Modular React Cytoscape component (`ArchitectureGraph.tsx`) bound to live workspace graph data.
  - Node click inspector with "Open in Editor" jumping to the exact file and line in Monaco.
- **Required Tests:**
  - Real workspace graph generation on actual repositories, node inspection fidelity, navigation to file from node click.

---

### 2.3 Impact Radar (Blast Radius & Dependency Calculation)
- **Existing Rust Implementation:**
  - `rust/crates/project-brain`: `ProjectBrain::calculate_impact_radar(modified_file: &str)` performs BFS/DFS traversal over `BrainEdge` dependency links, identifying direct dependents, transitive dependents, affected test suites, and calculates a normalized risk score (`0.0` to `1.0`).
- **Existing Frontend Implementation:**
  - Hardcoded static risk score (`0.45`) and static file lists in `index.html`.
- **Existing Tauri Commands:**
  - None.
- **Missing Connection:**
  - Tauri command `brain_calculate_impact(file_path)`: Invokes `calculate_impact_radar` with active editor file path.
  - React component `ImpactRadar.tsx`: Real-time blast radius indicator reflecting dependents and affected test suites.
- **Required Tests:**
  - Verified impact calculations on files with known dependents, zero-dependent edge cases, and test associations.

---

### 2.4 Project Memory (Persistent Knowledge Base)
- **Existing Rust Implementation:**
  - `rust/crates/database-engine`: SQLite storage with WAL mode, FTS5 full-text search, and table `project_memory` (`id`, `title`, `content`, `tags`, `created_at`, `updated_at`).
  - `rust/crates/project-brain`: `save_memory_note`, `get_memory_notes`, `delete_memory_note`.
- **Existing Frontend Implementation:**
  - Hardcoded list of 3 notes in `index.html` stored in client-side JavaScript memory, disappearing on restart.
- **Existing Tauri Commands:**
  - None.
- **Missing Connection:**
  - Tauri commands: `memory_save_note(title, content, tags)`, `memory_get_notes()`, `memory_delete_note(id)`.
  - React component `ProjectMemory.tsx`: Create, edit, tag, filter, delete, and search notes. Survives application restarts.
- **Required Tests:**
  - Persistence across process restart, full-text note search, tagging and deletion verification.

---

### 2.5 AI Assistant & Model Gateway
- **Existing Rust Implementation:**
  - `rust/crates/model-gateway`: `ModelGateway` manages cloud and local LLM providers:
    - OpenAI (`OpenAiCompatibleProvider`)
    - Anthropic (`AnthropicProvider`)
    - Google Gemini (`GoogleGeminiProvider`)
    - DeepSeek (`DeepSeekProvider`)
    - xAI Grok (`XAIGrokProvider`)
    - Local Ollama (`LocalOllamaProvider`, port 11434)
    - Local LM Studio (`OpenAiCompatibleProvider`, port 1234)
    - Custom OpenAI-compatible endpoints
  - Provides `list_models()`, `complete(&CompletionRequest)`, `check_status()`, `ProviderStatus` enum.
- **Existing Frontend Implementation:**
  - Mock chat interface using `setTimeout` (1.2s delay) emitting a static response.
- **Existing Tauri Commands:**
  - None.
- **Missing Connection:**
  - Tauri command `ai_list_models()`: Discovers verified models across configured providers.
  - Tauri command `ai_get_providers()`: Returns real configured/unconfigured provider status.
  - Tauri command `ai_stream_prompt(prompt, model, context)`: Streams tokens via Tauri event `ai-stream-chunk` and completes via `ai-generation-complete`.
  - Modular React component `AIAssistant.tsx`: Active file context injector, streaming token renderer, model selector, diff preview with Accept/Reject controls.
- **Required Tests:**
  - Streaming token delivery, error reporting when provider key is missing, context injection validation.

---

### 2.6 AI Agent Orchestrator & Runtime
- **Existing Rust Implementation:**
  - `rust/crates/agent-orchestrator`: `AgentOrchestrator` implements 14 production engineering personas:
    1. Principal Software Architect (`architect`)
    2. Frontend Developer (`frontend`)
    3. Backend Developer (`backend`)
    4. Database Engineer (`database`)
    5. Mobile Developer (`mobile`)
    6. Rust/C++ Systems Engineer (`systems`)
    7. Debugging Specialist (`debugging`)
    8. Testing Specialist (`testing`)
    9. Security Reviewer (`security`)
    10. Code Reviewer (`reviewer`)
    11. DevOps Engineer (`devops`)
    12. Technical Researcher (`researcher`)
    13. Documentation Engineer (`documentation`)
    14. Performance Engineer (`performance`)
  - `spawn_task_with_worktree`: Spawns task inside isolated git worktree with autonomy modes (`Autonomous`, `Supervised`, `ReviewRequired`).
  - `agent-runtime`: Manages execution steps, tool invocations, and checkpoints.
- **Existing Frontend Implementation:**
  - Hardcoded array of personas and canned progress bars in `index.html`.
- **Existing Tauri Commands:**
  - None.
- **Missing Connection:**
  - Tauri command `agent_list_personas()`: Fetches genuine 14 personas from `AgentOrchestrator`.
  - Tauri command `agent_spawn_task(role, objective, autonomy_mode)`: Dispatches real task.
  - Tauri command `agent_get_task(task_id)`: Fetches execution state, steps, modified files.
  - Event streaming `agent-task-updated` and `agent-output`.
  - React component `AIAgents.tsx`: Real persona picker, task submission, live event log, diff review.
- **Required Tests:**
  - Task lifecycle execution, event streaming, capability boundary enforcement.

---

### 2.7 Mobile Studio & Metro Server
- **Existing Rust Implementation:**
  - `rust/crates/mobile-build-manager`: `MobileBuildManager` provides:
    - `scaffold_expo_app`: Generates production-ready Expo Router React Native project (`package.json`, `app.json`, `eas.json`, `app/index.tsx`).
    - `check_eas_status`: Tests EAS CLI presence and authentication.
    - `trigger_eas_build`: Initiates cloud builds.
    - `discover_devices`: Executes `adb devices -l` to find physical and emulator Android devices.
    - `get_expo_dev_server_info`: Computes Metro port and QR URI.
- **Existing Frontend Implementation:**
  - Hardcoded device lists ("Pixel 8 Pro (Simulated)") and fake "Ready" badge for Metro.
- **Existing Tauri Commands:**
  - None.
- **Missing Connection:**
  - Tauri command `mobile_discover_devices()`: Real ADB probing.
  - Tauri command `mobile_start_metro(cwd, port)`: Spawns `npx expo start` via `ProcessManager`, monitors PID, streams logs via `mobile-process-output`.
  - Tauri command `mobile_stop_metro()`: Terminates running Metro process.
  - Tauri command `mobile_check_eas(cwd)`: Returns real EAS CLI status.
  - React component `MobileStudio.tsx`: Real device status, real Metro start/stop, interactive log stream.
- **Required Tests:**
  - Real device discovery via ADB or web fallback, Metro process launch/kill lifecycle.

---

### 2.8 Build & Storage Management
- **Existing Rust Implementation:**
  - `rust/crates/process-manager`: Asynchronous command execution with streaming output and timeouts.
  - `rust/crates/filesystem-engine`: Directory size analysis, atomic file deletions.
  - `rust/crates/checkpoint-engine`: `CheckpointEngine` creates snapshots and restores file trees.
- **Existing Frontend Implementation:**
  - Simulated build progress bar with `setInterval` and hardcoded "842 MB cleaned" alert.
- **Existing Tauri Commands:**
  - None.
- **Missing Connection:**
  - Tauri command `build_detect_systems(cwd)`: Discovers Cargo, npm/yarn/pnpm, Gradle, Python.
  - Tauri command `build_run_task(cwd, task_type)`: Executes real command, streams `build-process-output`, returns exit code and duration.
  - Tauri command `storage_analyze(cwd)`: Calculates exact disk consumption for project files, `target/`, `node_modules/`, `dist/`.
  - Tauri command `storage_cleanup(cwd, targets, dry_run)`: Performs verified deletion and returns actual bytes freed.
  - Tauri command `checkpoint_create(cwd, desc)`, `checkpoint_list(cwd)`, `checkpoint_restore(cwd, id)`.
  - React component `BuildStorage.tsx`: Real build runner, storage analyzer, cleanup with confirmation, checkpoint snapshots.
- **Required Tests:**
  - Accurate disk calculation, dry-run safety, real process execution with exit codes.

---

### 2.9 Git & LSP Integration
- **Existing Rust Implementation:**
  - `rust/crates/git-engine`: `GitEngine::get_status` runs `git rev-parse` and `git status --porcelain`.
  - `rust/crates/lsp-client`: JSON-RPC protocol client, multiplexer, and document synchronization.
- **Existing Frontend Implementation:**
  - Static branch name `main*` and hardcoded 3-item diff in `index.html`.
- **Existing Tauri Commands:**
  - None.
- **Missing Connection:**
  - Tauri commands: `git_get_status(cwd)`, `git_stage_file(cwd, file)`, `git_unstage_file(cwd, file)`, `git_commit(cwd, message)`, `git_get_diff(cwd, file)`.
  - Tauri commands: `lsp_get_status()`, `lsp_get_diagnostics()`.
  - React components `SourceControl.tsx` and `StatusBar.tsx`: Live Git branch, staging controls, diff viewer.
- **Required Tests:**
  - Git status parsing, staging/unstaging, commit creation, diff display.

---

## 3. AUDIT CONCLUSION & EXECUTION ROADMAP

1. **Step 1 — Rust Backend Contract Expansion (`apps/desktop/src-tauri`):**
   - Register shared states: `SharedWorkspace`, `SharedTerminalManager`, `SharedBrain`, `SharedGateway`, `SharedOrchestrator`, `SharedMobile`, `SharedCheckpoint`.
   - Implement typed Tauri commands in `src/commands/`: `brain.rs`, `memory.rs`, `ai.rs`, `agents.rs`, `mobile.rs`, `build.rs`, `storage.rs`, `git.rs`, `lsp.rs`.
2. **Step 2 — React 19 Workbench Component Migration (`apps/desktop/src`):**
   - Refactor `index.html` into structured React components under `apps/desktop/src/` maintaining 100% visual fidelity to the Sprint 19/20 workbench design.
   - Centralize Tauri communication in `src/services/tauri.ts`.
3. **Step 3 — Eliminate All Simulated / Mock Paths:**
   - Remove fake timeouts, hardcoded graphs, and simulated build/memory/AI responses.
4. **Step 4 — Automated Verification & Desktop Packaging:**
   - Unit tests, clippy, build, silent install test, live window screenshot, PR creation.
