# AetherIDE — Interactive Terminal Studio Guide
## Native Windows ConPTY Architecture, Multi-Session Management and Split Pane Operations

**Author:** Terminal Infrastructure Engineer & Principal Rust Systems Architect  
**Project:** AetherIDE  
**Sprint:** Sprint 05  
**Target Platform:** Windows 10/11 x64  

---

## 1. Subsystem Architecture Overview

The AetherIDE Interactive Terminal Studio provides an enterprise-grade terminal experience embedded directly in the developer workbench. It bridges Microsoft Windows PseudoConsole (ConPTY) with modern web-based ANSI terminal rendering via `@xterm/xterm`.

```
+-----------------------------------------------------------------------+
|                       AetherIDE Workbench (DOM)                       |
|  +--------------------+  +--------------------+  +-----------------+  |
|  |  xterm.js Pane #1  |  |  xterm.js Pane #2  |  | Fit / Search    |  |
|  |  (PowerShell 7)    |  |  (Command Prompt)  |  | WebLinks Addons |  |
|  +---------+----------+  +---------+----------+  +--------+--------+  |
+------------|-----------------------|----------------------|-----------+
             | User Keystrokes       | Polled Output        | Resize
             v                       ^                      v
+------------------------------------+----------------------------------+
|           Win32 Native WebView2 IPC Host (win32_host.rs)               |
|   - TerminalSpawn        - TerminalWrite        - TerminalResize      |
|   - TerminalPollOutput   - TerminalClose                              |
+------------------------------------+----------------------------------+
                                     | Loopback TCP / Named Pipe
                                     v
+-----------------------------------------------------------------------+
|                    Aether Daemon (terminal-manager)                   |
|  +-----------------------------------------------------------------+  |
|  |                 Win32 Job Object Container                      |  |
|  |  +--------------------------+     +--------------------------+  |  |
|  |  |  ConPtySession #1 (HPCON)|     |  ConPtySession #2 (HPCON)|  |  |
|  |  |  powershell.exe          |     |  cmd.exe                 |  |  |
|  |  |  In/Out Pipe Channels    |     |  In/Out Pipe Channels    |  |  |
|  |  +--------------------------+     +--------------------------+  |  |
|  +-----------------------------------------------------------------+  |
+-----------------------------------------------------------------------+
```

---

## 2. Zero-CDN Local Bundling

To guarantee 100% offline functionality, deterministic rendering, and air-gapped security, all terminal client assets are locally bundled in `apps/desktop/workbench/xterm/`:
- `xterm.js` (488 KB): Core ANSI / VT100 / VT220 / xterm terminal emulator.
- `xterm.css` (7.1 KB): Essential terminal styles, selection overlays, and cursor rendering.
- `addon-fit.js` (1.5 KB): Calculates precise character cell dimensions and scales cols/rows to fit container parent DOM nodes.
- `addon-search.js` (78 KB): High-speed regex and incremental text search across scrollback history.
- `addon-web-links.js` (3.1 KB): Automatic URL detection and click handling.

---

## 3. ConPTY Streaming Protocol & Lifecycle

### 3.1 Spawning a Session (`TerminalSpawn`)
A session is created by specifying shell profile and initial dimensions:
```json
{
  "action": "TerminalSpawn",
  "params": {
    "shell": "powershell",
    "cols": 80,
    "rows": 24,
    "cwd": "C:\\Users\\abdal\\OneDrive\\Desktop\\ide"
  }
}
```
The daemon calls `CreatePseudoConsole`, sets up input/output pipes with non-blocking async IO, spawns the target shell process inside a restricted Win32 Job Object, and returns the unique `session_id`.

### 3.2 Live Input and Streaming Output (`TerminalWrite` & `TerminalPollOutput`)
- When the user types, `term.onData(data => callIpc('TerminalWrite', { id, data }))` transmits raw keystrokes and escape sequences directly to the ConPTY input pipe.
- The daemon maintains a sliding output buffer. The frontend polls `TerminalPollOutput({ id, offset })` every 100ms. New bytes are decoded via UTF-8 `TextDecoder` and piped directly into `term.write()`.

### 3.3 Dynamic Resize Synchronization (`TerminalResize`)
A `ResizeObserver` monitors the terminal pane container. When pane dimensions change (e.g., resizing window or splitting panes):
1. `fitAddon.fit()` recalculates `cols` and `rows` based on cell font metrics.
2. `callIpc('TerminalResize', { id, cols, rows })` signals ConPTY via `ResizePseudoConsole(hpcon, size)`.

### 3.4 Clean Process Tree Termination (`TerminalClose`)
When a session is killed:
1. ConPTY pseudo-console handle is closed (`ClosePseudoConsole`).
2. The associated Win32 Job Object is terminated (`TerminateJobObject`), which forcefully and cleanly terminates all spawned descendant processes (e.g., subshells, node, python, cargo), preventing orphaned background processes.

---

## 4. UI Controls & Split Configurations

- **New Terminal (`+ New`):** Spawns a new tab with chosen profile (`powershell`, `cmd`, `bash`).
- **Split Horizontal (`Ctrl+Shift+E`):** Divides the active workspace horizontally side-by-side.
- **Split Vertical (`Ctrl+Shift+O`):** Stacks terminal panes vertically.
- **Toggle Terminal Dock (`Ctrl+\``):** Collapses or expands the docked terminal tray within Code Studio without losing terminal state.
- **Find in Terminal:** Instant text search across scrollback via `SearchAddon`.
- **Kill Terminal (`✕ Kill`):** Safely destroys the session and re-focuses remaining sessions.
