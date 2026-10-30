# AETHERIDE — SPRINT 04 RELEASE REPORT
## Windows x64 Portable Distribution, Release Manifest, and Artifact Verification

**Release Version:** `v0.1.0-alpha` (Sprint 04 Complete)  
**Target Platform:** Windows 10 & 11 x64  
**Date:** 2026-10-31  
**Release Engineer:** Release Engineering Director & Systems Auditor  

---

## 1. Release Artifact Summary

The release package is an end-to-end self-contained distribution of AetherIDE for Windows x64. It embeds the native Rust Win32/WebView2 launcher (`AetherIDE.exe`), the native background daemon (`aether-daemon.exe`), the command-line interface (`aether-cli.exe`), and the fully bundled offline Monaco Editor workbench (`workbench/`).

### Primary Release Archive
- **Path:** `dist/aether-ide-windows-x64-v0.1.0-alpha.zip`
- **Size:** 9,213,784 bytes (~9.2 MB)
- **SHA-256 Hash:** `a55da6175bb7f45139d13cdfeb08815532da5a416c5de0fe5a7c6851d88c97d3`
- **Checksum Verification File:** `dist/aether-ide-windows-x64-v0.1.0-alpha.zip.sha256`

---

## 2. Release Directory Manifest

```
dist/windows-x64/
├── AetherIDE.exe                # Native Win32 + WebView2 Embedded Desktop Host [19ada0ef94471046...]
├── AetherIDE.cmd                # Convenience command wrapper [32c98026bf60e28f...]
├── CHECKSUMS-SHA256.txt         # Cryptographic SHA256 manifest
├── bin/
│   ├── aether-daemon.exe        # Background Tokio daemon, SQLite, ConPTY, LSP [52233ab8ce635fe7...]
│   └── aether-cli.exe           # CLI diagnostic & management tool [bbc7bed31f20a074...]
├── workbench/
│   ├── index.html               # Monaco IDE workbench with integrated status & tabs [6802c8007c5eaae1...]
│   ├── package.json             # Workbench package metadata
│   ├── dist/
│   │   ├── main.js              # Optional Electron host entrypoint
│   │   └── preload.js           # Electron preload bridge
│   └── vs/                      # Complete offline Monaco Editor distribution
│       ├── loader.js            # RequireJS AMD loader
│       ├── editor/              # Monaco CSS and JS core
│       ├── basic-languages/     # 50+ language syntaxes (Rust, Python, TS, C++, etc.)
│       ├── language/            # Advanced TS/JSON/CSS/HTML language workers
│       └── base/                # Codicon fonts and base worker scripts
└── extensions/
    └── aether-core/             # Integrated extension bundle
```

---

## 3. Cryptographic Verification Table

| File Name | File Size | SHA-256 Checksum |
|---|---|---|
| `AetherIDE.exe` | 3,112,960 bytes | `19ada0ef944710465f36bd6fc3bedd7ea4f66ec205b3ad3df09a2b88e79b9593` |
| `AetherIDE.cmd` | 134 bytes | `32c98026bf60e28f9bf4b7f5194375b05ed7a29ad93d36b4f138edae56a8f9c9` |
| `bin/aether-daemon.exe` | 12,854,272 bytes | `52233ab8ce635fe7500cc9a12fe09690d43d9aecafe58da936555d953b43baa4` |
| `bin/aether-cli.exe` | 3,456,512 bytes | `bbc7bed31f20a074999f9ec46295f7c87a87eb2bc7b2f92d854ca48f7f80776d` |
| `workbench/index.html` | 34,840 bytes | `6802c8007c5eaae152f8d5745a0bd77ba749ed38a71e7b1dc186be11080842d9` |
| `aether-ide-windows-x64-v0.1.0-alpha.zip` | 9,213,784 bytes | `a55da6175bb7f45139d13cdfeb08815532da5a416c5de0fe5a7c6851d88c97d3` |

---

## 4. Verification Instructions for End Users

To verify the integrity and run the distribution on any 64-bit Windows machine:

1. **Verify Archive Integrity:**
   ```powershell
   (Get-FileHash -Path dist\aether-ide-windows-x64-v0.1.0-alpha.zip -Algorithm SHA256).Hash -eq (Get-Content dist\aether-ide-windows-x64-v0.1.0-alpha.zip.sha256).Split(' ')[0]
   ```
2. **Extract Archive:**
   ```powershell
   Expand-Archive -Path dist\aether-ide-windows-x64-v0.1.0-alpha.zip -DestinationPath C:\AetherIDE
   ```
3. **Launch Studio:**
   ```powershell
   C:\AetherIDE\AetherIDE.exe
   ```
   The launcher will automatically verify or spawn the background Tokio daemon, register the Win32 window, embed the Microsoft WebView2 controller, map `https://appassets.aetheride.local/`, and render the Monaco Editor workbench with full offline language support.
