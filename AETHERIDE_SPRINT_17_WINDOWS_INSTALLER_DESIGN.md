# AetherIDE Sprint 17 — Windows Installer Architecture & Design Specification

**Product:** AetherIDE  
**Release Target:** `v0.1.0-beta.2`  
**Deliverable:** `AetherIDE-Setup-0.1.0-beta.2-x64.exe`  
**Technology:** Inno Setup 6.7.3 (LZMA2 Solid Compression)  
**Target OS:** Windows 10 (version 1903+) / Windows 11 x64  
**Installation Scope:** Non-Administrative Per-User Installation  

---

## 1. Executive Overview

Prior to Sprint 17, AetherIDE was distributed on Windows exclusively as a portable `.zip` archive. While portable archives are convenient for developer pilot environments, general developer adoption requires a genuine Windows setup executable that registers application associations, manages desktop and Start Menu shortcuts, integrates CLI tools with user environment variables, detects runtime dependencies, and provides a safe, clean uninstallation and upgrade path.

This document specifies the architecture, security parameters, file layout, and lifecycle behaviors of `AetherIDE-Setup-0.1.0-beta.2-x64.exe`.

---

## 2. Distinction Between Executables

To avoid confusion between setup packages and application binaries:

| Binary Name | Role | Location | Permissions Required |
| :--- | :--- | :--- | :--- |
| **`AetherIDE-Setup-0.1.0-beta.2-x64.exe`** | Standalone Inno Setup Installer | Download / Distribution media | Standard User (`PrivilegesRequired=lowest`) |
| **`AetherIDE.exe`** | Primary Native Desktop Host | `{app}\AetherIDE.exe` | Standard User |
| **`aether-daemon.exe`** | Core Headless Daemon Engine | `{app}\bin\aether-daemon.exe` | Standard User |
| **`aether-cli.exe`** | Command Line Tool & Diagnostics | `{app}\bin\aether-cli.exe` | Standard User |
| **`unins000.exe`** | Windows Registered Uninstaller | `{app}\unins000.exe` | Standard User |

---

## 3. Directory Layout & Storage Hierarchy

```text
%LOCALAPPDATA%\Programs\AetherIDE/      <-- {app} (Application Installation Root)
├── AetherIDE.exe                       <-- Native Win32 + WebView2 launcher
├── AetherIDE.cmd                       <-- Command-line helper
├── LICENSE                             <-- License agreement
├── README.md                           <-- Release documentation
├── RELEASE_INFO.json                   <-- Build metadata descriptor
├── VERSION.txt                         <-- Pinned version: 0.1.0-beta.2
├── unins000.exe                        <-- Inno Setup uninstaller binary
├── unins000.dat                        <-- Installation manifest data
├── bin/
│   ├── aether-daemon.exe               <-- Core background daemon
│   └── aether-cli.exe                  <-- CLI utility
└── workbench/
    ├── index.html                      <-- Optimized workbench UI shell
    ├── package.json                    <-- Workbench package metadata
    ├── vs/                             <-- Local Monaco Editor & workers
    ├── xterm/                          <-- Local xterm.js terminal assets
    ├── cytoscape/                      <-- Local Cytoscape.js graph engine
    └── dist/                           <-- Compiled TypeScript modules
```

### Strict User Data Isolation
User data is **never** written inside `{app}`:
- **Project Index & Brain:** Placed strictly in `<project_root>\.aether\brain.db`.
- **Protected Credentials & Session Tokens:** Placed strictly in `%LOCALAPPDATA%\AetherIDE\desktop_bootstrap.json` and `%LOCALAPPDATA%\AetherIDE\credentials` via Windows DPAPI (`CryptProtectData`).
- **WebView2 Profile Cache:** Placed in `%LOCALAPPDATA%\AetherIDE\webview_profile`.

Because `{app}` contains only application binaries, upgrading or uninstalling AetherIDE can never corrupt or purge developer workspaces or saved credentials.

---

## 4. Key Installer Capabilities

### 4.1 Non-Administrative Installation
- Sets `PrivilegesRequired=lowest` and `DefaultDirName={localappdata}\Programs\AetherIDE`.
- Installs smoothly on corporate or locked-down machines without requiring UAC elevation prompts or system-level administrative tokens.

### 4.2 Start Menu & Desktop Shortcuts
- **Start Menu:** Creates `{autoprograms}\AetherIDE\AetherIDE.lnk` and `{autoprograms}\AetherIDE\Uninstall AetherIDE.lnk`.
- **Desktop (Optional):** User-selectable task `desktopicon` creating `{autodesktop}\AetherIDE.lnk`.

### 4.3 Command Line PATH Integration
- Task `addtopath` adds `{app}\bin` to `HKCU\Environment\Path`.
- Inno Setup Pascal code detects whether `{app}\bin` is already present to prevent duplicate PATH entries.
- The uninstaller hook (`CurUninstallStepChanged(usPostUninstall)`) safely parses and removes `{app}\bin` from the user's PATH string without altering other entries.

### 4.4 Process Lock Detection & Safe Upgrades
- Configured with `CloseApplications=yes` and `RestartApplications=no`.
- If an earlier version of `AetherIDE.exe` or `aether-daemon.exe` is running when Setup starts, the installer gracefully prompts the user to close the application before overwriting binary files.

---

## 5. Microsoft Edge WebView2 Dependency Management

The native Win32 host requires the Microsoft Edge WebView2 Evergreen Runtime:
1. **Detection:** Checks registry keys `HKLM` and `HKCU` under `SOFTWARE\Microsoft\EdgeUpdate\Clients\{F3017226-9E47-49F8-8CE6-8575C7A28A89}` and file paths for `msedge.exe` / `msedgewebview2.exe`.
2. **Missing Runtime Flow:** If WebView2 is absent, `InitializeSetup` displays a dialog offering to open the official Microsoft bootstrapper (`https://go.microsoft.com/fwlink/p/?LinkId=2124703`).
3. **Resilience:** The installer never bundles untrusted third-party Chromium mirrors, and permits setup to proceed so offline users can install WebView2 independently.

---

## 6. Design Verification Verdict

The architecture satisfies all requirements of Gate 1: lightweight footprint (< 10 MB installer), per-user safety, complete shortcut management, clean uninstallation, and zero interference with developer source code.
