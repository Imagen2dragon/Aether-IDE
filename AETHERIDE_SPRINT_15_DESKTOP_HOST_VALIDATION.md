# AetherIDE Sprint 15 — Desktop Host Architecture & Runtime Validation Report

## 1. Executive Summary

Sprint 15 independently audited and validated the native desktop host implementation of AetherIDE across Windows, Linux, and macOS. The packaging architecture strictly guarantees that every distribution launches its dedicated platform-native desktop host rather than falling back to an unmanaged web browser or external editor.

---

## 2. Desktop Host Architecture Matrix

```
                        ┌────────────────────────────────────┐
                        │      AetherIDE Desktop Host        │
                        │   (rust/cli/aether-launcher)       │
                        └─────────────────┬──────────────────┘
                                          │
            ┌─────────────────────────────┼─────────────────────────────┐
            ▼                             ▼                             ▼
   ┌─────────────────┐           ┌─────────────────┐           ┌─────────────────┐
   │  Windows Host   │           │   Linux Host    │           │   macOS Host    │
   │ Win32 + WebView2│           │ GTK3 + WebKitGTK│           │ Cocoa + WKWeb   │
   └────────┬────────┘           └────────┬────────┘           └────────┬────────┘
            │                             │                             │
            └─────────────────────────────┼─────────────────────────────┘
                                          ▼
                        ┌────────────────────────────────────┐
                        │     Desktop IPC Message Bridge     │
                        │    (Bidirectional JSON Protocol)   │
                        └─────────────────┬──────────────────┘
                                          ▼
                        ┌────────────────────────────────────┐
                        │        aether-daemon (IPC)         │
                        │   (127.0.0.1:44888 / Unix Sock)     │
                        └────────────────────────────────────┘
```

---

## 3. Platform Implementations & Source Verification

### 3.1 Windows x64: Win32 + Microsoft WebView2
- **Source Module:** `rust/cli/aether-launcher/src/win32_host.rs`
- **Native Host Type:** `Win32WebView2Host`
- **Execution Path:**
  1. `SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2)` guarantees crisp typography on 4K/high-DPI displays.
  2. `CoInitializeEx(None, COINIT_APARTMENTTHREADED)` initializes COM.
  3. `RegisterClassExW` registers the custom window class `AetherIDEWindowClass`.
  4. `CreateWindowExW` instantiates the top-level Win32 HWND.
  5. `CreateCoreWebView2EnvironmentWithOptions` creates the `ICoreWebView2Controller`.
  6. The webview is pinned to the client RECT with auto-resizing on `WM_SIZE`.
  7. `WebMessageReceivedEventHandler` intercepts messages from Monaco/JavaScript and routes them to `aether-daemon`.

### 3.2 Linux x64: GTK 3 + WebKitGTK
- **Source Module:** `rust/cli/aether-launcher/src/linux_host.rs`
- **Native Host Type:** `LinuxGtkHost`
- **Execution Path:**
  1. `gtk::init()` initializes the GTK event subsystem.
  2. Top-level `gtk::Window` created with dark-theme decorations.
  3. `webkit2gtk::WebView` embedded within a GTK container.
  4. `UserContentManager` registers message handlers to bridge web requests to local Unix Domain Socket or TCP port `44888`.

### 3.3 macOS ARM64: Cocoa + WKWebView
- **Source Module:** `rust/cli/aether-launcher/src/macos_host.rs`
- **Native Host Type:** `MacWkWebViewHost`
- **Execution Path:**
  1. `NSApplication` shared instance created.
  2. `NSWindow` created with titlebar customization and native traffic-light buttons.
  3. `WKWebView` mounted via `WKUserContentController`.
  4. Bridges IPC communication directly to `aether-daemon` over loopback TCP.

---

## 4. First-Run & Fault Recovery Validation

The desktop host includes graceful degradation when hardware or credential services are constrained:

| Fault Condition | Expected Host Behavior | Validation Result |
| :--- | :--- | :--- |
| **Missing WebView2 on Windows** | Launches Edge WebView2 evergreen installer or falls back to app host | **VERIFIED** |
| **Locked Credential Vault** | Prompts for re-authentication; falls back to ephemeral session token | **VERIFIED** |
| **Daemon Crashed on Startup** | Spawns clean daemon instance with exponential retry (up to 3s) | **VERIFIED** |
| **Path Traversal Attack in Webview** | Blocked at IPC bridge handler via capability root validation | **VERIFIED** |
| **Unresponsive Client Message** | Enforces 5-second IPC timeout, preventing host UI freezing | **VERIFIED** |
