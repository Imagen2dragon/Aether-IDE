# ADR-002: Native Rust Microsoft WebView2 Desktop Host Architecture

**Status:** ACCEPTED  
**Date:** 2026-10-30  
**Authors:** Aether Systems Architecture Team  
**Scope:** Desktop Host Infrastructure, Windows Packaging & Workbench Shell

---

## 1. Context and Problem Statement

AetherIDE is designed as a production-grade, Rust-first software engineering ecosystem. In Sprint 01, standalone desktop packaging used a dual-mode launcher supporting bundled Electron and an Edge app-mode fallback. 

While Electron offers comprehensive web API coverage, it introduces significant resource penalties for developer desktop tools:
1. **Binary Footprint:** Electron binaries bundle Chromium and Node.js runtimes, inflating distribution packages by 150MB–220MB compressed.
2. **Memory Overhead:** Idle Electron applications consume 250MB–450MB of RAM across multiple rendering and GPU helper processes before project loading.
3. **Redundant Node Runtime:** The AetherIDE architecture delegates all systems operations (file I/O, LSP, Git, ConPTY, Process management, AI inference) to the native Rust `aether-daemon`. Running an embedded Node.js process inside Electron creates architectural duplication.

---

## 2. Decision: Microsoft WebView2 Native Rust Host

We adopt **Microsoft WebView2** embedded in a native Rust host (`AetherIDE.exe`) as the primary desktop presentation engine for Windows 10/11.

### Architectural Model:

```
+----------------------------------------------------------------------------------+
|                            AetherIDE.exe (Native Rust Host)                      |
|                                                                                  |
|   +--------------------------------------------------------------------------+   |
|   | Win32 Window: WS_OVERLAPPEDWINDOW, Dark Mode Titlebar, High-DPI Aware    |   |
|   +--------------------------------------------------------------------------+   |
|   | WebView2 Controller & CoreWebView2 Runtime Engine                        |   |
|   | - SetVirtualHostNameToFolderMapping("aether.workbench", local_dist_path) |   |
|   | - Navigation: https://aether.workbench/index.html                        |   |
|   | - postMessage Bridge: window.chrome.webview.postMessage <-> Named Pipe   |   |
|   +--------------------------------------------------------------------------+   |
+----------------------------------------------------------------------------------+
                                        | Named Pipe (\\.\pipe\aether-daemon-ipc)
                                        v
                       +-----------------------------------+
                       |    aether-daemon (Tokio Core)     |
                       | - LSP Multiplexer (rust-analyzer) |
                       | - ConPTY Terminal Engine          |
                       | - Model Gateway & Agents          |
                       +-----------------------------------+
```

---

## 3. Comparison Matrix

| Evaluation Dimension | Electron Standalone | Native Rust + WebView2 Host | Edge App-Mode Fallback |
|---|---|---|---|
| **Distributable Size** | ~180 MB | **< 15 MB** | < 15 MB |
| **Idle Memory (RAM)** | 320 MB – 480 MB | **65 MB – 110 MB** | 90 MB – 140 MB |
| **Cold Startup Time** | 1.8s – 2.8s | **0.4s – 0.7s** | 0.8s – 1.4s |
| **OS Runtime Availability** | Bundled | **Pre-installed on Win 10/11** | Pre-installed |
| **Window Frame Customization** | CSS / Frameless | **Native Win32 / DWM** | Limited browser frame |
| **Security Sandbox** | Context Isolation | **Virtual Host Origin Isolation**| Standard browser |

---

## 4. Runtime Detection Strategy

Microsoft WebView2 Evergreen Runtime is built into Windows 11 and Windows 10 (versions 1809+). To handle environments where Edge Evergreen has been stripped or corrupted:

1. **Registry Verification:**
   Query standard Evergreen GUID `{F3017226-F501-4750-9A06-C237243B108}` in:
   - `HKLM\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\`
   - `HKCU\Software\Microsoft\EdgeUpdate\Clients\`
2. **File System Fallback Detection:**
   Inspect Microsoft Edge Application directory:
   - `C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe`
   - `C:\Program Files\Microsoft\Edge\Application\msedge.exe`
3. **Boot Resolution:**
   - If WebView2 runtime or Edge is present: Launch native standalone window with Virtual Host mapping.
   - If neither is detected: Launch system default browser and display notification with official Evergreen bootstrapper link.

---

## 5. Security & Isolation Architecture

1. **Virtual Host Origin Isolation:**
   Workbench files are served over virtual HTTPS: `https://aether.workbench/`. This isolates webview state from the local `file:///` security zone and prevents path traversal attacks.
2. **Restricted postMessage Bridge:**
   Frontend-to-daemon communication requires valid `session_token` authentication on every IPC message.
3. **Data Directory Segregation:**
   WebView2 user profile cache is stored under `%LOCALAPPDATA%\AetherIDE\webview_profile`, completely separated from system Edge browser data.
