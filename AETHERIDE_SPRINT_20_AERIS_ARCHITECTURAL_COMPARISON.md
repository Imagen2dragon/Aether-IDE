# AETHERIDE — SPRINT 20: AERIS BROWSER ARCHITECTURAL COMPARISON & MIGRATION BLUEPRINT
## Reference Implementation Study: AERIS Desktop Browser (`0.7.9`) vs. AetherIDE (`0.1.0-beta.2`)

**Reference Implementation:** AERIS Desktop Browser  
**Reference Repository Path:** `C:\Users\abdal\OneDrive\Desktop\c t\aeris-browser`  
**Reference Secondary Mirror:** `C:\Users\abdal\OneDrive\Desktop\c t\tradebot\aeris`  
**Installed Reference Executable:** `C:\Users\abdal\AppData\Local\AERIS\aeris.exe` (Version `0.7.9`)  
**Target Product:** AetherIDE — AI-Native Engineering Studio  
**Target Branch:** `feat/sprint-20-tauri-standalone-desktop`  
**Date of Audit:** November 1, 2026  

---

## 1. Executive Summary

In accordance with the Sprint 20 Master Engineering Directive, we located, audited, and analyzed the production codebase of the **AERIS browser** (`@aeris/desktop` v0.7.9). The repository at `C:\Users\abdal\OneDrive\Desktop\c t\aeris-browser` serves as the architectural reference for transforming AetherIDE into an authentic, standalone desktop application built on **Tauri 2**, **Rust**, **React**, and **TypeScript**.

The AERIS browser proves that:
1. **Tauri 2 is a robust, lightweight desktop framework** on Windows, macOS, and Linux that compiles directly to a native Windows PE GUI executable (`aeris.exe`) without requiring an external browser, Node.js runtime, or local HTTP server.
2. **Bundled production assets (`../dist`)** load instantaneously offline via Tauri's embedded WebView2 runtime with zero network latency and no localhost port binding.
3. **Rust-first backend services** integrate cleanly with the frontend via typed Tauri commands (`invoke`) and bi-directional events (`listen` / `emit`).
4. **Native operating system dialogs and explorer integration** can be orchestrated via Win32 Shell COM and Windows shell execution (`explorer.exe /select`).
5. **A unified binary strategy** can host both the primary desktop GUI and background auxiliary services (e.g. `aeris.exe --host`) with shared dependencies and unified lifecycle management.

---

## 2. In-Depth Architectural Comparison

| Architectural Dimension | AERIS Browser (`0.7.9`) | AetherIDE (Pre-Migration) | AetherIDE Target Architecture (Tauri 2) |
| :--- | :--- | :--- | :--- |
| **Desktop Shell Framework** | **Tauri 2** (`tauri 2.0`, `features = ["unstable"]`) | Custom Win32 Host (`windows 0.58`, `webview2-com 0.33`) | **Tauri 2** (`tauri 2.0`, `features = ["unstable"]`) |
| **Frontend Framework** | **React 19** + TypeScript (~5.8) | Vanilla JS / TS (`apps/desktop/workbench/`) | **React 19** + TypeScript + Monaco Editor Workbench |
| **Frontend Build Tool** | **Vite 7** (`@vitejs/plugin-react 5`) | Plain `tsc` / static assets | **Vite 7** (`@vitejs/plugin-react 5`) + TypeScript |
| **Frontend Production Target**| Locally bundled `dist/` (`frontendDist: "../dist"`) | Mapped folder (`resources/workbench`) | **Locally bundled `dist/`** (`frontendDist: "../dist"`) |
| **IPC Communication** | `@tauri-apps/api/core` (`invoke`) + `@tauri-apps/api/event` (`listen`) | `window.chrome.webview.postMessage` + custom event dispatcher | **`@tauri-apps/api/core`** (`invoke`) + **`@tauri-apps/api/event`** (`listen`) |
| **Single-Instance Enforcement** | `tauri-plugin-single-instance = "2"` | Win32 Named Mutex | **`tauri-plugin-single-instance = "2"`** |
| **Native Terminal Engine** | `portable-pty = "0.9"` (ConPTY on Windows) | Custom ConPTY (`terminal-manager` crate) | **Integrated ConPTY** (`terminal-manager` + `portable-pty`) |
| **File Dialogs** | Win32 COM `IFileOpenDialog` (`localfiles.rs`) | Win32 COM `IFileOpenDialog` (`win32_dialogs.rs`) | **Win32 COM `IFileOpenDialog` & `IFileSaveDialog`** via Tauri commands |
| **Packaging & Installer** | **Tauri NSIS** (`targets: ["nsis"]`, `installer.nsh` hooks) | Inno Setup (`package-windows.ps1`) | **Tauri NSIS** with custom `installer.nsh` + portable ZIP |
| **App Paths & Shortcuts** | Registered in `HKCU\Software\Microsoft\Windows\CurrentVersion\App Paths` | Registry scripts | **NSIS hooks** in `installer.nsh` |
| **Background Auxiliary Process**| `aeris.exe --host` (served terminals & AI CLIs) | `aether-daemon.exe` (independent binary) | **Supervised daemon** via Tauri process manager / auxiliary mode |

---

## 3. Dissecting the AERIS Implementation Patterns

### 3.1 Tauri 2 Configuration (`tauri.conf.json`)
From `C:\Users\abdal\OneDrive\Desktop\c t\aeris-browser\apps\desktop\src-tauri\tauri.conf.json`:
- **Schema:** `"https://schema.tauri.app/config/2"`
- **Frontend Distribution:** `"frontendDist": "../dist"` ensures zero remote server dependencies in production.
- **Development Server:** `"devUrl": "http://localhost:1430"` with `"beforeDevCommand": "npm run dev"` and `"beforeBuildCommand": "npm run build"`.
- **Security & CSP:** Strictly configured CSP allowing IPC localhost communication while disabling dangerous inline styles.
- **NSIS Packaging:** `installMode: "currentUser"`, `installerIcon: "icons/icon.ico"`, and `installerHooks: "installer.nsh"`.

### 3.2 NSIS Installer Hooks (`installer.nsh`)
AERIS employs custom NSIS macros:
```nsis
!macro NSIS_HOOK_POSTINSTALL
  ; Register App Paths for Win+R and terminal execution
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\App Paths\aetheride.exe" "" "$INSTDIR\aetheride.exe"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\App Paths\aetheride.exe" "Path" "$INSTDIR"
  ; Desktop and Start Menu Shortcuts
  CreateShortCut "$DESKTOP\AetherIDE.lnk" "$INSTDIR\aetheride.exe" "" "$INSTDIR\aetheride.exe" 0
  CreateShortCut "$SMPROGRAMS\AetherIDE.lnk" "$INSTDIR\aetheride.exe" "" "$INSTDIR\aetheride.exe" 0
  ; Notify Windows Shell of registration changes
  System::Call 'shell32::SHChangeNotify(i 0x08000000, i 0, p 0, p 0)'
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  Delete "$DESKTOP\AetherIDE.lnk"
  Delete "$SMPROGRAMS\AetherIDE.lnk"
  DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\App Paths\aetheride.exe"
  System::Call 'shell32::SHChangeNotify(i 0x08000000, i 0, p 0, p 0)'
!macroend
```
This pattern provides clean, user-scoped installation without requiring administrator privileges or UAC elevation prompts.

### 3.3 Application Entry Point & Subsystem (`main.rs` & `lib.rs`)
```rust
// No console window in release builds:
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    if std::env::args().any(|a| a == "--daemon") {
        aether_lib::daemon_main();
        return;
    }
    aether_lib::run();
}
```
In `lib.rs`, the application builder:
1. Attaches `tauri_plugin_single_instance::init(|app, argv, cwd| ...)`.
2. Registers setup hooks (`.setup(|app| { ... })`).
3. Registers strongly typed command handlers via `tauri::generate_handler![...]`.
4. Handles shutdown events via `tauri::RunEvent::Exit`.

### 3.4 Typed IPC Bridge (`ipc.ts`)
AERIS wraps Tauri's IPC in a typed module:
```typescript
import { invoke as tauriInvoke, type InvokeArgs } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export function invoke<T>(cmd: string, args?: InvokeArgs): Promise<T> {
  return tauriInvoke<T>(cmd, args);
}
```
This pattern centralizes error reporting, latency tracing in development, and type validation.

---

## 4. AetherIDE Migration Blueprint

To migrate AetherIDE without disrupting its 24 Rust crates, ConPTY terminal engine, Monaco Editor, or AI personas:

```text
AetherIDE/
├── apps/
│   └── desktop/
│       ├── package.json                   <-- React 19, @tauri-apps/api ^2, Monaco Editor, Xterm.js
│       ├── vite.config.ts                 <-- Vite 7 build configuration targeting ../dist
│       ├── tsconfig.json                  <-- TypeScript ~5.8 configuration
│       ├── index.html                     <-- Workbench entry HTML (Monaco + Terminal)
│       ├── src/                           <-- React Workbench & Component Tree
│       │   ├── App.tsx                    <-- Activity Bar, Sidebar, Editor, Terminal
│       │   ├── ipc.ts                     <-- Typed Tauri invoke/listen wrapper
│       │   ├── editor/                    <-- Monaco Editor controller
│       │   ├── explorer/                  <-- Workspace File Tree & Context Menu
│       │   └── terminal/                  <-- Xterm.js + ConPTY bridge
│       └── src-tauri/                     <-- Tauri 2 Native Host Crate
│           ├── Cargo.toml                 <-- tauri 2, tauri-plugin-single-instance 2, workspace deps
│           ├── build.rs                   <-- tauri-build 2
│           ├── tauri.conf.json            <-- Tauri 2 configuration (NSIS, bundle, windows)
│           ├── installer.nsh              <-- NSIS installer hooks
│           ├── capabilities/
│           │   └── default.json           <-- Core window and menu permissions
│           ├── icons/                     <-- Application icons (32x32, 128x128, icon.ico)
│           └── src/
│               ├── main.rs                <-- PE Subsystem 2 entry point + --daemon dispatcher
│               ├── lib.rs                 <-- Tauri Builder, single-instance, commands
│               └── commands/              <-- Typed Tauri command implementations
│                   ├── workspace.rs       <-- WorkspaceSession, multi-root, open folder
│                   ├── filesystem.rs      <-- Scoped read, write, rename, delete, reveal
│                   ├── terminal.rs        <-- ConPTY terminal spawn, input, resize, kill
│                   └── system.rs          <-- Doctor diagnostics, version info, trust
```

---

## 5. Risk Assessment & Preservation Safeguards

1. **Monaco Editor Compatibility:** Monaco Editor runs completely offline inside WebView2 through bundled local JavaScript workers. Tauri 2's `frontendDist: "../dist"` serves local files via secure HTTPS custom protocols (`tauri://localhost` or `https://tauri.localhost`), which are fully compatible with Web Workers.
2. **ConPTY Terminal Preservation:** The Windows ConPTY implementation in `crates/terminal-manager` remains intact and is invoked directly through Tauri async commands, streaming output to Xterm.js via Tauri events.
3. **Capability Filesystem Containment:** The `cap-std` boundary enforcement from `crates/filesystem-engine` and `workspace_bridge.rs` is directly embedded in Tauri command handlers. Frontend requests cannot execute path traversal escapes.
4. **Daemon Integration:** The existing `aether-daemon` crate remains available as an auxiliary process or embedded service without code loss.

---

## 6. Migration Status & Next Steps
- **Step 1:** Establish `apps/desktop/src-tauri` with verified Tauri 2 dependencies.
- **Step 2:** Configure `tauri.conf.json`, `capabilities/default.json`, `installer.nsh`, and application branding.
- **Step 3:** Implement native Tauri commands bridging `workspace_bridge.rs`, `win32_dialogs.rs`, and `terminal-manager`.
- **Step 4:** Configure Vite and React workbench bundling in `apps/desktop`.
- **Step 5:** Compile production binary `AetherIDE.exe` and build NSIS installer via Tauri CLI.
- **Step 6:** Execute full acceptance and verification test matrix.
