# AETHERIDE — NATIVE HWND WEBVIEW2 HOSTING ARCHITECTURE
## Win32 Windowing, Microsoft WebView2 Controller Lifecycle, Virtual Origin Mapping, and Message Bridge

**Sprint:** 04  
**Document Version:** 1.0.0  
**Target Platform:** Windows 10 & 11 x64  
**Component:** `rust/cli/aether-launcher/` (`AetherIDE.exe`)  
**Core Technologies:** Rust, Win32 API (`windows` v0.58), Microsoft WebView2 COM (`webview2-com` v0.33)

---

## 1. Architectural Mission

Prior to Sprint 04, the AetherIDE Windows launcher spawned `msedge.exe --app=...` as an external standalone process. While functional, external app-mode suffered from several drawbacks:
- It was visible as a separate Microsoft Edge instance rather than an embedded application.
- Window resizing, DPI changes, and process lifecycle were decoupled from the host launcher.
- Inter-process communication relied entirely on unsecured network or file-polling hooks.

In Sprint 04, `aether-launcher` was re-engineered into a **True Native HWND-Embedded WebView2 Host**. The executable directly registers a Win32 window class, creates a top-level `HWND`, initializes Microsoft WebView2 via COM FFI, embeds the WebView2 rendering controller into the window client area, and brokers typed IPC between the Monaco frontend and the background Tokio daemon.

---

## 2. Component Structure

```
rust/cli/aether-launcher/
├── Cargo.toml                  # Workspace dependencies + windows v0.58 + webview2-com v0.33
└── src/
    ├── main.rs                 # CLI entrypoint, daemon supervisor, fallback router
    ├── win32_host.rs           # Win32 HWND creation, WndProc, DPI, WebView2 COM lifecycle, & bridge
    └── webview2_runtime.rs     # Evergreen runtime registry detection and app-mode fallback
```

---

## 3. Native Win32 Window Implementation

### 3.1 Window Class Registration (`RegisterClassExW`)

The launcher registers a custom window class named `AetherIDEWindowClass`:

```rust
let class_name = w!("AetherIDEWindowClass");
let wc = WNDCLASSEXW {
    cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
    style: CS_HREDRAW | CS_VREDRAW,
    lpfnWndProc: Some(wndproc),
    cbClsExtra: 0,
    cbWndExtra: 0,
    hInstance: hinstance,
    hIcon: HICON::default(),
    hCursor: LoadCursorW(HMODULE::default(), IDC_ARROW)?,
    hbrBackground: HBRUSH::default(),
    lpszMenuName: PCWSTR::null(),
    lpszClassName: class_name,
    hIconSm: HICON::default(),
};
RegisterClassExW(&wc);
```

### 3.2 High-DPI Awareness (`SetProcessDpiAwarenessContext`)

To guarantee crisp rendering on high-resolution displays (4K, Surface monitors), the process calls:
```rust
SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
```
When a window moves between monitors with differing scaling factors, Windows delivers `WM_DPICHANGED`. The `wndproc` captures this message, extracts the suggested target `RECT` from `lparam`, and invokes `SetWindowPos` to adjust bounds seamlessly.

### 3.3 Window Procedure (`WndProc`) Message Handling

The window procedure handles key window messages:
- **`WM_SIZE`:** Queries the new client dimensions via `GetClientRect` and updates the embedded WebView2 controller bounds via `controller.SetBounds(rect)`.
- **`WM_DPICHANGED`:** Synchronizes window scale factor and adjusts position.
- **`WM_CLOSE`:** Calls `DestroyWindow(hwnd)`.
- **`WM_DESTROY`:** Calls `PostQuitMessage(0)` to exit the message pump and initiate clean COM uninitialization.

---

## 4. Microsoft WebView2 COM Lifecycle Management

The controller lifecycle runs on a Single-Threaded Apartment (STA) thread initialized with `CoInitializeEx(None, COINIT_APARTMENTTHREADED)`:

### Step 1: Environment Creation
```rust
CreateCoreWebView2EnvironmentWithOptions(
    PCWSTR::null(),
    &user_data_hstring, // %LOCALAPPDATA%\AetherIDE\webview_profile
    None,
    &env_handler,
)?;
```
`webview2_com::wait_with_pump` processes Windows messages while awaiting asynchronous environment initialization, preventing UI deadlocks.

### Step 2: Controller Parenting
```rust
env.CreateCoreWebView2Controller(hwnd, &ctrl_handler)?;
let controller = wait_with_pump(rx_ctrl)?;
controller.SetIsVisible(true)?;
controller.SetBounds(rect)?;
```
The controller takes the parent `HWND` and immediately binds its rendering surface to the Win32 client area.

### Step 3: Virtual Host Name Mapping
Rather than serving the application over an unsecured `http://localhost` port or using restrictive `file:///` URIs, the launcher maps a virtual origin via `ICoreWebView2_3`:

```rust
let host_name = w!("appassets.aetheride.local");
let folder_hstring = HSTRING::from(&workbench_dir.to_string_lossy());
webview3.SetVirtualHostNameToFolderMapping(
    host_name,
    &folder_hstring,
    COREWEBVIEW2_HOST_RESOURCE_ACCESS_KIND_ALLOW,
);
```
The WebView2 engine is then instructed to navigate to:
```
https://appassets.aetheride.local/index.html
```
This configuration provides:
- Secure context (`isSecureContext = true`) enabling full Web Worker, Service Worker, and Clipboard APIs.
- Clean origin isolation preventing untrusted external web pages from accessing local IDE assets.

---

## 5. Fallback Hierarchy

To maintain maximum reliability across all Windows environments, the launcher implements a three-tier fallback hierarchy:

```
[ AetherIDE.exe Launch ]
         │
         ▼
[ Tier 1: True Native HWND WebView2 Host ] ──(Success)──> [ Running in Native HWND ]
         │ (Fails e.g. headless/no display)
         ▼
[ Tier 2: Standalone WebView2 App Mode ]   ──(Success)──> [ Running in Edge App Window ]
         │ (Fails if WebView2 missing)
         ▼
[ Tier 3: Default Browser Fallback ]       ─────────────> [ Running in Default Browser ]
```
