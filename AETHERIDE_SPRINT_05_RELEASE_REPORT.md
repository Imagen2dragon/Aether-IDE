# AetherIDE — Sprint 05 Release Report
## Production Release Candidate, Binary Packaging, SHA256 Verification and Deployment Guide

**Author:** Release & Packaging Specialist  
**Project:** AetherIDE  
**Sprint:** Sprint 05  
**Version:** v0.1.0-alpha (Sprint 05 Edition)  
**Target Architecture:** Windows 10/11 x86_64  

---

## 1. Release Manifest & Distribution

The release distribution is packaged as a standalone, portable Windows x64 zip archive that requires zero external network dependencies or installer installation:

- **Archive File:** `dist/aether-ide-windows-x64-v0.1.0-alpha.zip`
- **File Size:** ~9.8 MB
- **Package Structure:**
  ```
  aether-ide-windows-x64-v0.1.0-alpha/
  ├── AetherIDE.exe                 (Native Win32 HWND + Microsoft WebView2 Host)
  ├── AetherIDE.cmd                 (Convenience startup script)
  ├── CHECKSUMS-SHA256.txt          (Cryptographic verification file)
  ├── bin/
  │   ├── aether-daemon.exe         (Tokio daemon, ConPTY manager, SQLite WAL engine)
  │   └── aether-cli.exe            (Developer CLI tool)
  ├── extensions/                   (Core syntax and language extensions)
  └── workbench/
      ├── index.html                (Unified Interactive Workbench)
      ├── vs/                       (Locally bundled Monaco Editor)
      ├── xterm/                    (Locally bundled Xterm.js & Fit/Search Addons)
      └── cytoscape/                (Locally bundled Cytoscape.js)
  ```

---

## 2. Cryptographic Checksums (SHA256)

Every binary and the release zip archive have been hashed using SHA256:

| Artifact | SHA256 Hash |
|---|---|
| `aether-ide-windows-x64-v0.1.0-alpha.zip` | `4954648e4641be6610769f05906bcb2b38cf11f6c809af7cf77eb475c6fee1ec` |
| `AetherIDE.exe` | `f4853e63cb3763a17d3e491b72edda4209afd737233010cd4d17211b848919f8` |
| `bin/aether-daemon.exe` | `aecd799ed09f82f03a75c9ec9714878664e01306610d1821b1aac2b6f2da365a` |
| `bin/aether-cli.exe` | `8e6f1de5763553127714c4294756f92e0a360f022cf1ec62d6266e298b8818c0` |

---

## 3. Verification & Deployment Instructions

### 3.1 Verifying Checksums
On Windows PowerShell:
```powershell
Get-FileHash -Path dist\aether-ide-windows-x64-v0.1.0-alpha.zip -Algorithm SHA256
```
Confirm the output matches `4954648e4641be6610769f05906bcb2b38cf11f6c809af7cf77eb475c6fee1ec`.

### 3.2 Running the Portable Application
1. Extract `aether-ide-windows-x64-v0.1.0-alpha.zip` to any local directory (e.g. `C:\AetherIDE`).
2. Run `AetherIDE.exe` or `AetherIDE.cmd`.
3. The native Win32 window mounts the Microsoft WebView2 controller, automatically launches the background `aether-daemon.exe` on authenticated port 44888, and loads the interactive developer workbench.
4. Use `Ctrl+P` for Quick Open, `Ctrl+Shift+P` for Command Palette, `Ctrl+\`` to toggle the ConPTY Terminal dock, or navigate to **Terminal Studio**, **Project Brain**, **AI Engineering**, or **Mobile Studio** from the header navigation bar.
