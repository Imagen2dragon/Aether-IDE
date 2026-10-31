# AETHERIDE — SPRINT 09 KNOWN LIMITATIONS & PLATFORM CONSTRAINTS
## Transparent Beta Boundary Documentation & General Availability Roadmap

**Product:** AetherIDE  
**Target Release:** `v0.1.0-beta.2`  
**Classification:** Public Engineering Disclosure  

---

## 1. Purpose & Scope

In accordance with our engineering principles of radical transparency and evidence-based software engineering, this document details known limitations, architectural boundaries, and platform-specific behaviors in `v0.1.0-beta.2`.

---

## 2. Platform-Specific Differences

### 2.1 Windows (Primary Desktop Platform)
- **Host Engine:** True Win32 HWND + Microsoft WebView2 embedded controller.
- **Terminal:** Windows ConPTY host with `ScopedJobObject` containment.
- **Limitation:** Windows SmartScreen warning may appear during initial installation of the beta installer until our EV Code Signing Certificate pipeline completes production reputation accumulation. (Workaround: Click "More info" -> "Run anyway").
- **WebView2 Evergreen Runtime:** Requires Microsoft Edge WebView2 runtime. Automatically provided on Windows 11; auto-downloaded on legacy Windows 10 installations.

### 2.2 Linux x64
- **Host Engine:** GTK3 windowing + WebKitGTK 4.0 embedded controller.
- **Terminal:** OpenPTY POSIX pseudo-terminal.
- **Limitation:** Wayland fractional scaling may cause minor font subpixel blurring on certain high-DPI displays when running under XWayland. (Workaround: Set `GDK_BACKEND=x11` or configure integer scaling).

### 2.3 macOS ARM64 / Universal
- **Host Engine:** Cocoa native windowing + WKWebView embedded controller.
- **Terminal:** POSIX pseudo-terminal.
- **Limitation:** Apple Gatekeeper quarantine attribute (`com.apple.quarantine`) on unsigned beta DMGs requires right-clicking `AetherIDE.app` -> "Open" on first launch until Apple Developer ID notarization is automated in GA Phase 1.

---

## 3. Subsystem Limitations

### 3.1 Autonomous Agent LLM Streaming
- Agent steps execute asynchronously with structured database event streaming. While full step diffs and thoughts stream in near-real-time, token-by-token character streaming inside the Agent Studio UI is batched every 250ms to prevent WebView2 IPC message queue saturation.

### 3.2 Mobile Cloud EAS Builds
- The Mobile Studio provides full local Expo development server management, QR code pairing, and mobile preview framing. Remote EAS cloud build submission requires providing your own personal Expo access token; enterprise pooled build credits will be introduced in GA.

### 3.3 Large File Indexing
- Files larger than 10 MB are excluded from deep AST syntax tree parsing and full-text search indexing to protect RAM budgets. They remain fully readable and editable in Monaco Editor as plain text.

---

## 4. General Availability (GA) Resolution Path

All listed limitations are scheduled for closure across our GA Roadmap:
1. **EV Hardware Certificate Signing:** Instant SmartScreen reputation on Windows and automated Apple Developer ID notarization on macOS.
2. **WebKitGTK 4.1 / WebKit2GTK-5.0 Upgrade:** Native Wayland fractional scaling support on Linux.
3. **Wasm Extension Sandbox:** Sandboxed plugin execution for third-party extensions.
