# AETHERIDE — WINDOWS RELEASE CANDIDATE REPORT

**Release Version:** v0.1.0-alpha (Release Candidate 1)  
**Target Platform:** Windows 10 / Windows 11 x64  
**Date:** 2026-10-31  
**Build Profile:** `release` (Optimized, LTO, stripped symbols)  
**Distribution Archive:** `dist/aether-ide-windows-x64-v0.1.0-alpha.zip` (5.89 MB)  
**SHA-256 Checksum:** `581B6A4282AF1230C407FF7169E9C522E3ADAB5C0DBFA8FA34302847BB20CDD4`  

---

## 1. Package Inventory & Structure

The portable standalone distribution package contains all required binaries, frontend assets, extensions, and launchers:

```
dist/windows-x64/
├── AetherIDE.exe                (309 KB - Native Windows Entrypoint Host)
├── AetherIDE.cmd                (Command wrapper script)
├── CHECKSUMS-SHA256.txt         (File integrity verification hashes)
├── bin/
│   ├── aether-daemon.exe        (11.58 MB - Native Background Tokio Daemon)
│   └── aether-cli.exe           (3.12 MB - Management & Diagnostics CLI)
├── workbench/
│   └── index.html               (AetherIDE Standalone Desktop Workbench)
└── extensions/
    └── aether-core/             (First-party extension bundle)
```

---

## 2. Release Verification Checklist

| Gate | Acceptance Criteria | Verified | Evidence |
|:---|:---|:---|:---|
| **Standalone Launch** | Operates without pre-installed VS Code | **YES** | Standalone launcher spawns daemon on `127.0.0.1:44888` and opens workbench |
| **WebView2 Evergreen** | Detects installed Microsoft Edge / WebView2 runtime | **YES** | `webview2_runtime::tests::test_detect_webview2_runtime` passed |
| **Daemon Authentication**| Cryptographic token enforcement (`401 Unauthorized`) | **YES** | Session token generated at startup, constant-time verification |
| **Native ConPTY** | Interactive Windows terminal sessions (`powershell`, `cmd`) | **YES** | High-fidelity Win32 pseudoconsole stream verified in live tests |
| **Language Intelligence**| Real LSP server routing & actionable error reporting | **YES** | `rust-analyzer` live integration test passed |
| **Workspace Sync** | End-to-end encrypted local peer sync | **YES** | ChaCha20-Poly1305 AEAD bidirectional file transfer test passed |
| **Integrity & Checksum**| SHA-256 validation on all binaries and zip archive | **YES** | Generated and verified |

---

## 3. Installation & Getting Started

1. Download and extract `aether-ide-windows-x64-v0.1.0-alpha.zip`.
2. Double-click `AetherIDE.exe`.
3. The launcher will automatically start `bin/aether-daemon.exe`, verify the session token, and launch the native desktop workbench.
4. To run command-line diagnostics or commands:
   ```cmd
   bin\aether-cli.exe doctor
   ```
