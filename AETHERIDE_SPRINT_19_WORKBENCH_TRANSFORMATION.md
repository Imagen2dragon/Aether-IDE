# AetherIDE Sprint 19 — Workbench Transformation Report

**Product:** AetherIDE — AI-Native Engineering Studio  
**Branch:** `feat/sprint-19-native-desktop-ide-workbench`  
**Date:** 2026-11-01  

---

## 1. Primary Component Breakdown

### 1.1 Vertical Activity Bar (`.activity-bar`)
- **Width:** 48 px.
- **Top Actions:**
  - `Explorer` (`Ctrl+Shift+E`): Toggles Primary Sidebar to File Tree & Open Editors.
  - `Search` (`Ctrl+Shift+F`): Toggles Primary Sidebar to Workspace-wide Find & Replace.
  - `Source Control` (`Ctrl+Shift+G`): Toggles Git branch, status, staged files, commit message.
  - `Project Brain` (`Ctrl+Shift+B`): Toggles Symbol graph index and Blast Radius Impact Radar.
  - `AI Agents` (`Ctrl+Shift+A`): Toggles Autonomous personas (Architect, Sentinel, QA, Perf).
  - `Mobile Studio` (`Ctrl+Shift+M`): Toggles Mobile device emulator & platform runner.
  - `Build & Storage` (`Ctrl+Shift+S`): Toggles Checkpoint engine and SQLite storage stats.
- **Bottom Actions:**
  - `AI Assistant` (`Ctrl+Alt+B`): Toggles the right-side contextual AI sidebar.
  - `Diagnostics Doctor`: Focuses the Bottom Panel Diagnostics tool.
  - `About & Settings`: Opens the branded About and configuration modal.
- **Interaction Model:** Clicking an inactive activity activates its view and opens the sidebar. Clicking the active activity collapses the sidebar to maximize editing space.

### 1.2 Primary Sidebar (`#primary-sidebar`)
- **Width:** Resizable (default 260 px, min 180 px, max 500 px).
- **Splitter:** Resizer handle (`#sidebar-resizer`) with visual highlight on hover/drag.
- **Header:** Shows current active view name in uppercase and contextual actions (e.g. New File, Open Folder, Refresh).
- **File Explorer:** Nested directory tree with syntax icons, active file highlight, and single-click file opening.

### 1.3 Persistent Central Editor (`.editor-container`)
- **Location:** Dominates 65–85% of available screen area.
- **Tabs Bar:** Open file buffers, dirty dot indicators, individual close buttons, and tab action buttons (Split Editor Right, Toggle Minimap, Close All Tabs).
- **Breadcrumbs Bar:** Path hierarchy (`aetheride > rust > cli > aether-launcher > src > win32_host.rs`).
- **Monaco Host:** Full syntax highlighting, line numbers, bracket matching, multi-cursor, and LSP completions.
- **Empty State:** Minimalist card with essential keyboard shortcuts, shown only when all file tabs are closed.

### 1.4 Integrated Bottom Tool Panel (`#bottom-panel`)
- **Height:** Resizable (default 240 px, min 100 px, max 600 px).
- **Splitter:** Horizontal drag handle (`#panel-resizer`).
- **Tabs:**
  - `TERMINAL`: Embeds ConPTY xterm.js instance with multi-session tabs, new session (`+`), horizontal split, clear, and kill.
  - `PROBLEMS`: Displays workspace errors and warnings with badge counts and clickable items.
  - `OUTPUT`: Multi-channel log console (`Aether Daemon`, `Rust Analyzer`, `TypeScript Compiler`, `Git Engine`).
  - `DEBUG CONSOLE`: Interactive expression evaluation.
  - `DOCTOR DIAGNOSTICS`: Toolchain health and verification report.
- **Controls:** Maximize / restore panel (`⤢`), close panel (`×`), toggle shortcut (`Ctrl+\``).

### 1.5 Secondary AI Sidebar (`#secondary-sidebar`)
- **Location:** Right edge of window, resizable (default 330 px).
- **Model Selector:** Claude 3.5 Sonnet, GPT-4o, Ollama Local (8B), Gemini 1.5 Pro.
- **Context Chip:** Automatically binds active file context (`Active Context: win32_host.rs`).
- **Interactive Chat:** Formatted AI responses, quick action chips ("Explain Code", "Generate Tests", "Security Audit").
- **Diff Review:** Reviewable changes with accept/reject capability.

### 1.6 Bottom Status Bar (`footer.workbench-statusbar`)
- **Height:** 24 px.
- **Left Group:** Git branch (`main*`), Problems (`⊗ 0  ⚠ 0`), LSP server status (`⚡ rust-analyzer`), active terminal count.
- **Right Group:** Resource Mode indicator (`⚡ Standard Mode` / `🌱 Low-Resource Mode`), Line/Column (`Ln 1, Col 1`), Spaces: 4, UTF-8, CRLF, Language Mode (`Rust`), Daemon health status dot.

---

## 2. Layout State Persistence

All user adjustments (primary sidebar width, bottom panel height, secondary sidebar width, panel visibility, active activity) are automatically persisted to `localStorage` under key `aether_workbench_layout` on drag completion and restored on launch.

A **Reset Workbench Layout** command in the View menu allows developers to immediately restore factory layout defaults with a single click.
