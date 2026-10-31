# AetherIDE — Sprint 06 Terminal Subsystem Guide
## ConPTY, Unix OpenPTY and Cross-Platform PseudoTerminal Infrastructure

---

## 1. Overview

AetherIDE implements true native interactive pseudo-terminals across desktop platforms using the `TerminalBackend` trait (`crates/terminal-manager/src/backend.rs`):

- **Windows:** Microsoft Windows PseudoConsole (ConPTY) with Job Object process isolation.
- **Linux:** POSIX PTY master-slave allocation (`openpty`) with `/bin/bash` or `/bin/sh`.
- **macOS:** Unix PTY allocation with Apple `/bin/zsh` or `/bin/bash`.

---

## 2. Windows ConPTY Implementation Architecture

1. **Handle Allocation:**
   - Input and output pipes created via Win32 `CreatePipe`.
   - PseudoConsole created via `CreatePseudoConsole(COORD { X: cols, Y: rows }, hInPipe, hOutPipe, 0, &mut hpcon)`.

2. **Process Attribute List:**
   - `InitializeProcThreadAttributeList` with `PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE`.
   - `CreateProcessW` launches the requested shell executable (`powershell.exe`, `cmd.exe`, or custom shell).

3. **Job Object Containment:**
   - Win32 Job Object with `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`.
   - Closing the terminal session terminates all child subprocesses without orphaned background tasks.

---

## 3. Unix OpenPTY Implementation Architecture

- Allocates master and slave pseudoterminal file descriptors using POSIX conventions.
- Configures non-blocking I/O and asynchronous polling thread for zero-latency ANSI streaming.
- Terminal resize operations emit `ioctl(fd, TIOCSWINSZ, &ws)` signals to notify child shell processes of window dimension changes.
