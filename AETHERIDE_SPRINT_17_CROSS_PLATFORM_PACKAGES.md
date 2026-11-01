# AetherIDE Sprint 17 — Cross-Platform Packaging Architecture Report

**Product:** AetherIDE  
**Release Target:** `v0.1.0-beta.2`  
**Supported Operating Systems:** Windows 10/11 x64, Linux x64, macOS ARM64  
**Status:** VALIDATED & DECOUPLED ACROSS PLATFORMS  

---

## 1. Executive Summary

This report outlines the platform-specific packaging architectures of AetherIDE `v0.1.0-beta.2`. While Sprint 17 prioritized creating a genuine Windows setup executable (`.exe`), the engineering directives strictly prohibit forcing Windows installer abstractions onto Linux or macOS. Each desktop operating system receives an idiomatic, native distribution format matching its windowing system and credential vault.

---

## 2. Platform Distribution Matrix

| Platform | Primary Target Format | Secondary Format | Host Architecture | Native Credential Vault |
| :--- | :--- | :--- | :--- | :--- |
| **Windows x64** | `AetherIDE-Setup-0.1.0-beta.2-x64.exe` (Inno Setup) | `AetherIDE-v0.1.0-beta.2-windows-x64.zip` (Portable) | Win32 + WebView2 Evergreen | Windows DPAPI (`CryptProtectData`) |
| **Linux x64** | `AetherIDE-v0.1.0-beta.2-linux-x64.tar.gz` (Portable) | None | GTK3 + WebKitGTK (`libwebkit2gtk-4.1`) | FreeDesktop Secret Service via DBus |
| **macOS ARM64** | `AetherIDE-v0.1.0-beta.2-macos-arm64.tar.gz` (App Bundle) | None | Apple Silicon Cocoa + WKWebView | Apple Keychain (`Security.framework`) |

---

## 3. Platform Package Details

### 3.1 Windows Packages
- **Setup EXE (`AetherIDE-Setup-0.1.0-beta.2-x64.exe`):** 8.37 MB. Self-contained Inno Setup installer registering Start Menu and Desktop shortcuts, dependency detection, and uninstaller.
- **Portable ZIP (`AetherIDE-v0.1.0-beta.2-windows-x64.zip`):** 10.3 MB. Zero-installer standalone archive for portable USB drives or CI test runners.

### 3.2 Linux Package (`AetherIDE-v0.1.0-beta.2-linux-x64.tar.gz`)
- **Size:** 12.5 MB (12,538,961 bytes).
- **Structure:** Contains `AetherIDE` launcher, `bin/aether-daemon`, `bin/aether-cli`, and `workbench/`.
- **System Prerequisites:** glibc 2.35+, GTK3, WebKit2GTK, and DBus Secret Service.

### 3.3 macOS Package (`AetherIDE-v0.1.0-beta.2-macos-arm64.tar.gz`)
- **Size:** 8.88 MB (8,889,706 bytes).
- **Structure:** Standard macOS Application Bundle (`AetherIDE.app`).
- **Gatekeeper Notice:** Requires `xattr -cr AetherIDE.app` on first run due to unnotarized beta status.

---

## 4. Verification Verdict

All three operating systems maintain clean, independent packaging pipelines that preserve native desktop hosts and credential vaults.
