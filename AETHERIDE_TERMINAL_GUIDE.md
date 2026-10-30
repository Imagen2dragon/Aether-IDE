# AetherIDE — Native Windows ConPTY Terminal Engine Guide

**Subsystem:** `rust/crates/terminal-manager/`  
**Integration:** `rust/services/aether-daemon/`  
**Underlying Engine:** Win32 Pseudoconsole API (`CreatePseudoConsole`)  
**Isolation:** Windows Job Objects (`ScopedJobObject` via `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`)

---

## 1. Architecture & Design Principles

AetherIDE provides a true native Windows terminal emulator engine using Microsoft's official Windows Pseudoconsole (ConPTY) architecture. Rather than relying on simple redirected standard I/O pipes or third-party terminal wrappers, AetherIDE directly interfaces with the Windows console subsystem.

### Key Capabilities:
- **True Win32 ConPTY:** Instantiates a pseudo-console session (`HPCON`) using `CreatePseudoConsole`, supporting rich VT escape sequences, cursor addressing, color styling, and interactive programs (such as `vim`, `htop`, `git interactive rebase`).
- **Process Tree Isolation:** All spawned processes are bound to a Windows Job Object configured with `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`. When a terminal session is closed, all child processes spawned within it are guaranteed to be terminated by the OS kernel, preventing orphaned background tasks.
- **Asynchronous Pipe Worker:** Dedicated worker threads read the anonymous output pipe into non-blocking Tokio broadcast channels, buffering the terminal scrollback history and streaming live VT events without blocking the async event loop.
- **Dynamic Window Resizing:** Dispatches `ResizePseudoConsole` whenever the workbench terminal dimensions change.
- **Shell Agnostic:** Full out-of-the-box support for PowerShell (`powershell.exe`), PowerShell Core (`pwsh.exe`), Command Prompt (`cmd.exe`), and Git Bash (`bash.exe`).

---

## 2. Shell Profiles & Discovery

| Shell | Executable Profile | Default Arguments |
|---|---|---|
| **PowerShell** | `powershell.exe` | `-NoLogo` |
| **PowerShell 7+** | `pwsh.exe` | `-NoLogo` |
| **Command Prompt** | `cmd.exe` | Default interactive |
| **Git Bash** | `C:\Program Files\Git\bin\bash.exe` | `--login -i` |

Custom executables or scripts can be spawned by passing the command line string to `TerminalSpawn`.

---

## 3. Daemon IPC API Reference

The terminal subsystem communicates over Named Pipe (`\\.\pipe\aether-daemon-ipc`) and local TCP loopback (`127.0.0.1:44888`):

### 3.1 `TerminalSpawn`
Spawns a new native ConPTY terminal session.
```json
{
  "id": "req-term-1",
  "session_token": "<token>",
  "type": "Request",
  "data": {
    "action": "TerminalSpawn",
    "params": {
      "shell": "powershell",
      "cwd": "C:\\Users\\dev\\project",
      "cols": 120,
      "rows": 30
    }
  }
}
```

### 3.2 `TerminalWrite`
Writes user keystrokes, input text, or VT sequences into the terminal session.
```json
{
  "id": "req-term-2",
  "session_token": "<token>",
  "type": "Request",
  "data": {
    "action": "TerminalWrite",
    "params": {
      "session_id": 1,
      "data": "dir\r\n"
    }
  }
}
```

### 3.3 `TerminalResize`
Informs ConPTY that the frontend terminal window was resized.
```json
{
  "id": "req-term-3",
  "session_token": "<token>",
  "type": "Request",
  "data": {
    "action": "TerminalResize",
    "params": {
      "session_id": 1,
      "cols": 140,
      "rows": 40
    }
  }
}
```

### 3.4 `TerminalClose`
Closes the ConPTY handle and tears down the process tree via Job Object.
```json
{
  "id": "req-term-4",
  "session_token": "<token>",
  "type": "Request",
  "data": {
    "action": "TerminalClose",
    "params": {
      "session_id": 1
    }
  }
}
```

---

## 4. Test Verification

The ConPTY engine is verified with automated tests:
- Unit & struct layout tests: `cargo test -p terminal-manager --lib`
- End-to-end interactive cmd execution & ANSI streaming: `cargo test -p terminal-manager --test conpty_test`
- Daemon IPC integration: `cargo test -p aether-daemon --test test_authenticated_terminal_spawn_and_close`
