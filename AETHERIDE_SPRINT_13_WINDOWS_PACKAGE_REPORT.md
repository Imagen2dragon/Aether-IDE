# AetherIDE Sprint 13 — Windows Release Packaging Report

## 1. Overview & Platform Target

**Platform:** Windows 10 / Windows 11 (x86_64)  
**Distribution Channel:** Controlled Developer Beta (`v0.1.0-beta.2`)  
**Package Types:**
1. Standalone Portable Distribution (`AetherIDE-v0.1.0-beta.2-windows-x64.zip`)
2. Inno Setup Production Installer (`scripts/installer/aetheride_installer.iss`)

All binaries are compiled against the stable MSVC target (`x86_64-pc-windows-msvc`) with full optimizations and link-time optimizations (LTO) enabled in the release profile.

---

## 2. Package Anatomy & Directory Layout

The staged Windows distribution adheres to standard desktop software layout conventions:

```
dist/windows-x64/
├── AetherIDE.exe                 # Standalone native desktop application host
├── AetherIDE.cmd                 # PATH-aware batch launcher
├── bin/
│   ├── aether-daemon.exe         # High-performance background core server
│   └── aether-cli.exe            # Developer terminal CLI interface
├── workbench/
│   ├── index.html                # Native desktop HTML5 workbench shell
│   ├── dist/
│   │   ├── main.js               # Host process orchestration bundle
│   │   └── preload.js            # Secure IPC bridge
│   ├── cytoscape/                # Project Brain interactive radar assets
│   ├── vs/                       # Monaco Editor core & language workers
│   └── xterm/                    # GPU-accelerated terminal assets
├── RELEASE_INFO.json             # Machine-readable release metadata
├── VERSION.txt                   # Version identifier (0.1.0-beta.2)
├── LICENSE                       # MIT Open Source License
└── README.md                     # Getting started and usage guide
```

---

## 3. Production Inno Setup Installer Script

The installer script (`scripts/installer/aetheride_installer.iss`) configures:
- **Clean Installation Directory:** Defaults to `{autopf}\AetherIDE` (per-user non-elevated or system-wide).
- **Environment PATH Integration:** Automatically appends `{app}\bin` to user PATH without requiring administrative privileges, enabling instant terminal access to `aether-cli` and `aether-daemon`.
- **Start Menu & Desktop Shortcuts:** Generates customizable program groups and optional desktop shortcuts.
- **Elevation Handling:** Configured with `PrivilegesRequired=lowest` and `PrivilegesRequiredOverridesAllowed=dialog` to allow non-admin developer installations without UAC elevation roadblocks.
- **Uninstallation Support:** Cleanly registers uninstaller in Windows Add/Remove Programs.

---

## 4. Verification & Integrity

The standalone ZIP package was generated and verified:
- **Package Archive:** `dist/AetherIDE-v0.1.0-beta.2-windows-x64.zip`
- **Integrity Digest:** Calculated via SHA-256 and stored alongside the distribution file in `dist/AetherIDE-v0.1.0-beta.2-windows-x64.zip.sha256` and included in the master `SHA256SUMS.txt`.
