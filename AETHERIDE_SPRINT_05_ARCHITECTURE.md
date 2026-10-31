# AETHERIDE — SPRINT 05 ARCHITECTURAL SPECIFICATION
## Advanced Developer Workbench, Interactive Terminal Studio, Project Brain Visualization, Autonomous Agent Command Center and Live Mobile Preview

**Version:** 1.0.0  
**Author:** CTO & Principal Systems Architect  
**Date:** 2026-10-31  
**Project:** AetherIDE  

---

## 1. System Topology Overview

```
+---------------------------------------------------------------------------------------------------------+
|                                    AetherIDE.exe (Native Windows x64 Host)                              |
|                                                                                                         |
|  +---------------------------------------------------------------------------------------------------+  |
|  | Win32 Top-Level HWND ("AetherIDEWindowClass")                                                      |  |
|  |   - Message Loop (WndProc): WM_SIZE -> controller.SetBounds(), WM_DPICHANGED, WM_DESTROY          |  |
|  |   - Embedded ICoreWebView2Controller -> ICoreWebView2                                             |  |
|  |   - Virtual Origin: https://appassets.aetheride.local/ -> ./workbench/                            |  |
|  |   - Secure Native Message Bridge (args.Source() origin check + typed allowlist)                   |  |
|  |                                                                                                   |  |
|  |   +-------------------------------------------------------------------------------------------+   |  |
|  |   | AETHERIDE WORKBENCH UI (TypeScript / Monaco / xterm.js / Cytoscape Graph)                 |   |  |
|  |   |                                                                                           |   |  |
|  |   | [Top Menu / Activity Bar / Model Selection / Status Indicators]                           |   |  |
|  |   |                                                                                           |   |  |
|  |   | [LEFT SIDEBAR]         [CENTER WORKSPACE]                [RIGHT PANEL]                    |   |  |
|  |   | - File Explorer        - Monaco Editor (Multi-Tabs)      - AI Persona Stepper             |   |  |
|  |   | - Project Brain Graph  - Project Architecture Graph       - Human Approval Center          |   |  |
|  |   | - Mobile Studio        - Mobile Web Device Preview       - Change Impact Radar            |   |  |
|  |   | - 14 Agent Personas    - Living Project Memory Notes     - Context Inspector              |   |  |
|  |   |                                                                                           |   |  |
|  |   | [BOTTOM STUDIO TRAY]                                                                      |   |  |
|  |   | - Terminal Studio (Multi-Tabs, Split Panes, xterm.js + FitAddon + WebGL fallback)          |   |  |
|  |   | - Problems & Diagnostics / Output Logs / Build Activity / Unified Timeline                |   |  |
|  |   +-------------------------------------------------------------------------------------------+   |  |
|  +---------------------------------------------------------------------------------------------------+  |
+---------------------------------------------------|-----------------------------------------------------+
                                                    | Authenticated Native Dispatch (Loopback TCP / Named Pipe)
                                                    v
+---------------------------------------------------------------------------------------------------------+
|                                    aether-daemon.exe (Tokio Runtime)                                    |
|                                                                                                         |
|  +---------------------+  +----------------------+  +---------------------+  +-----------------------+  |
|  | ConPTY Manager      |  | Project Brain Engine |  | Agent Orchestrator  |  | Mobile Build Manager  |  |
|  | - Windows ConPTY    |  | - AST Graph Builder  |  | - 14 Personas       |  | - Expo Metro Server   |  |
|  | - Job Object Guard  |  | - Change Impact      |  | - Persona Stepper   |  | - ADB Discovery       |  |
|  | - VT/ANSI Streaming |  | - Project Memory DB  |  | - Server Approvals  |  | - QR LAN/Tunnel URI   |  |
|  +---------------------+  +----------------------+  +---------------------+  +-----------------------+  |
|                                                                                                         |
|  +---------------------------------------------------------------------------------------------------+  |
|  | SQLite WAL + FTS5 Database Engine | LSP Multiplexer (rust-analyzer) | Security Token Guard        |  |
|  +---------------------------------------------------------------------------------------------------+  |
+---------------------------------------------------------------------------------------------------------+
```

---

## 2. Interactive Terminal Studio Architecture

```
+---------------------------------------------------------------------------------------------------------+
|                                     TERMINAL STUDIO WORKSPACE COMPONENT                                 |
|                                                                                                         |
|  +---------------------------------------------------------------------------------------------------+  |
|  | Tab Bar: [ Terminal 1 (PowerShell) ● ] [ Terminal 2 (CMD) ] [ + Split H ] [ + Split V ] [ Maximize ]|  |
|  +---------------------------------------------------------------------------------------------------+  |
|  | Pane Container (CSS Flexbox / Resizable Panes with Drag Dividers)                                 |  |
|  |                                                                                                   |  |
|  |   +----------------------------------------+   +----------------------------------------+         |  |
|  |   | Pane 1: Session #1 (PowerShell 7)      |   | Pane 2: Session #2 (CMD / Git Bash)    |         |  |
|  |   | - xterm.js Instance                    |   | - xterm.js Instance                    |         |  |
|  |   | - FitAddon (auto-calculates cols/rows)  |   | - FitAddon (auto-calculates cols/rows)  |         |  |
|  |   | - SearchAddon & WebLinksAddon          |   | - SearchAddon & WebLinksAddon          |         |  |
|  |   | - terminal.onData -> TerminalWrite     |   | - terminal.onData -> TerminalWrite     |         |  |
|  |   | - ResizeObserver -> TerminalResize     |   | - ResizeObserver -> TerminalResize     |         |  |
|  |   +----------------------------------------+   +----------------------------------------+         |  |
|  |                                                                                                   |  |
|  | Status Bar: PowerShell 7 | C:\Users\abdal\ide | PID: 18442 | 80x24 | UTF-8 | ConPTY: Active       |  |
|  +---------------------------------------------------------------------------------------------------+  |
+---------------------------------------------------|-----------------------------------------------------+
                                                    |
                         IPC Requests: TerminalSpawn, TerminalWrite, TerminalResize, TerminalPoll, TerminalClose
                                                    v
+---------------------------------------------------------------------------------------------------------+
| ConPTY Windows Backend (terminal-manager crate)                                                         |
| - HPCON PseudoConsole handle, ScopedJobObject process tree termination, input/output async pipes        |
+---------------------------------------------------------------------------------------------------------+
```

---

## 3. Project Brain & Change Impact Radar Architecture

```
+---------------------------------------------------------------------------------------------------------+
|                                       PROJECT BRAIN COMPONENT                                           |
|                                                                                                         |
|  [ Architecture Graph ]   [ Impact Radar ]   [ Living Project Memory ]                                  |
|                                                                                                         |
|  +---------------------------------------------------------------------------------------------------+  |
|  | Interactive Graph Canvas (Cytoscape.js / Canvas Engine, 100% Bundled Locally):                    |  |
|  |                                                                                                   |  |
|  |       [ Crate: application-core ]                                                                 |  |
|  |                  │ imports                                                                        |  |
|  |                  ▼                                                                                |  |
|  |       [ File: src/lib.rs ] ─── calls ───> [ Struct: AetherConfig ]                                |  |
|  |                  ▲                                                                                |  |
|  |                  │ tested by                                                                      |  |
|  |       [ Test: tests/app_test.rs ]                                                                 |  |
|  |                                                                                                   |  |
|  | Click node -> Navigation to exact Monaco line/col (editor.revealRangeInCenter)                      |  |
|  +---------------------------------------------------------------------------------------------------+  |
|  | Change Impact Radar Panel:                                                                        |  |
|  | - Target: src/auth/token.rs | Risk Score: 0.35 (Medium Blast Radius)                             |  |
|  | - Direct Dependents: src/api/routes.rs                                                             |  |
|  | - Transitive Dependents: tests/auth_test.rs                                                        |  |
|  | - Affected Tests: 2 test suites affected                                                          |  |
|  +---------------------------------------------------------------------------------------------------+  |
|  | Layer 2: Living Project Memory (SQLite Backed)                                                    |  |
|  | - Architectural Decisions, Technology Conventions, API Schemas, Notes                              |  |
|  +---------------------------------------------------------------------------------------------------+  |
+---------------------------------------------------------------------------------------------------------+
```

---

## 4. Autonomous Agent Command Center Architecture

```
+---------------------------------------------------------------------------------------------------------+
|                                  AI ENGINEERING COMMAND CENTER                                          |
|                                                                                                         |
|  14 Specialized Personas:                                                                               |
|  [ Architect ] [ Frontend ] [ Backend ] [ Database ] [ Mobile ] [ Systems ] [ Debugging ] [ Testing ]    |
|  [ Security ] [ Code Reviewer ] [ DevOps ] [ Researcher ] [ Documentation ] [ Performance ]             |
|                                                                                                         |
|  Persona Stepper:                                                                                       |
|  (1) Task Created -> (2) Planned -> (3) Tool Requested -> [ (4) WAITING FOR APPROVAL ] -> (5) Executed   |
|                                                                                                         |
|  Human Approval Center Card:                                                                            |
|  +---------------------------------------------------------------------------------------------------+  |
|  | Task #104: Systems Engineer | Step 3: Atomic Patch to src/main.rs                                  |  |
|  | Action: write_file | Permissions: WriteAllowed                                                    |  |
|  |                                                                                                   |  |
|  | [ Unified Git Diff Viewer ]:                                                                      |  |
|  |   - let old_config = false;                                                                       |  |
|  |   + let new_config = true;                                                                        |  |
|  |                                                                                                   |  |
|  | [ Approve Step ]    [ Reject Step ]    [ Cancel Task ]                                            |  |
|  +---------------------------------------------------------------------------------------------------+  |
|  Server-Side Verification: Rust daemon validates session token, task ownership, pending state version     |
+---------------------------------------------------------------------------------------------------------+
```

---

## 5. Mobile Studio Live Preview Architecture

```
+---------------------------------------------------------------------------------------------------------+
|                                        MOBILE STUDIO WORKSPACE                                          |
|                                                                                                         |
|  Controls: [ Project: MyAetherApp ] [ Framework: Expo 52 ] [ Device Preset: iPhone 16 / Pixel 8 ]      |
|            [ Start Server ] [ Fast Refresh ] [ QR Pairing ] [ Device Discovery (ADB) ]                  |
|                                                                                                         |
|  +-----------------------------------------+  +------------------------------------------------------+  |
|  | Mobile Device Preview Canvas            |  | Metro Bundler & Runtime Log Console                  |  |
|  | +-------------------------------------+ |  | [INFO] Starting Metro Bundler on port 8081...         |  |
|  | |                                     | |  | [METRO] Fast Refresh connected.                     |  |
|  | |  [ WEB RUNTIME PREVIEW ]            | |  | [LOGCAT] Android emulator-5554 detected.              |  |
|  | |  Isolated iframe / webview          | |  | [QR] exp://192.168.1.100:8081                        |  |
|  | |  Zero native bridge credentials    | |  |                                                      |  |
|  | |                                     | |  +------------------------------------------------------+  |
|  | +-------------------------------------+ |  | Connected Devices:                                   |  |
|  | Preset: 390x844 (Portrait)              |  | - Pixel 8 Pro (Emulator) - Ready                     |  |
|  +-----------------------------------------+  | - Physical iPhone (Scan QR with Expo Go)             |  |
|                                               +------------------------------------------------------+  |
+---------------------------------------------------------------------------------------------------------+
```
