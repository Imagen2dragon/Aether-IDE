# AetherIDE — Sprint 06 Cross-Platform Portability Guide

---

## 1. Supported Platforms & System Matrix

| Operating System | Architecture | Windowing & Host | Terminal Subsystem | Release Format |
|---|---|---|---|---|
| **Windows 10/11** | x86_64 | Win32 HWND + Microsoft WebView2 | ConPTY (Win32 PseudoConsole) | Inno Setup (.exe), MSIX, .zip |
| **Linux (Ubuntu/Debian)** | x86_64 | GTK3 + WebKitGTK 4.0/4.1 | POSIX OpenPTY | AppImage, Debian (.deb), .tar.gz |
| **macOS (Apple Silicon)** | aarch64 | Cocoa + Apple WebKit WKWebView | Unix OpenPTY | .app Bundle, DMG, .tar.gz |

---

## 2. Implementation Architecture

### 2.1 Terminal Abstraction (`TerminalBackend`)
The terminal subsystem polymorphic interface in `terminal-manager` dynamically delegates to platform primitives:
- On Windows: Uses `ConPtySession` via Win32 `CreatePseudoConsole`, with Windows Job Objects isolating shell subprocesses (`powershell.exe`, `cmd.exe`).
- On Unix/macOS: Uses `UnixPtySession` via POSIX `openpty()` and termios control structures, executing `/bin/zsh` or `/bin/bash`.

### 2.2 Host Abstraction (`DesktopHost`)
The desktop entrypoint `AetherIDE` launcher implements the `DesktopHost` trait:
- Windows: Embedded Microsoft WebView2 evergreen runtime inside native Win32 window with Per-Monitor V2 DPI awareness.
- Linux: Native WebKitGTK webview host.
- macOS: Native WKWebView Cocoa host.

---

## 3. Continuous Integration Matrix

Cross-platform builds and automated regression suites are verified on every push and pull request via `.github/workflows/ci.yml`:
- **Code Quality:** `cargo fmt --check`
- **Windows Build:** `windows-latest` -> MSVC toolchain, Clippy (`-D warnings`), test suite, release binary build.
- **Linux Build:** `ubuntu-latest` -> WebKitGTK dependencies, GNU toolchain, Clippy, test suite.
- **macOS Build:** `macos-14` -> Apple Silicon M-series runner, Clippy, test suite.
