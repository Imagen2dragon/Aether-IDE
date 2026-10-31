# AetherIDE Sprint 17 — GitHub Release Integration & Asset Staging Report

**Product:** AetherIDE  
**Proposed Release Tag:** `v0.1.0-beta.2`  
**Prerelease Flag:** `true` (Beta qualification)  
**Status:** DRAFT STAGED — NOT PUBLISHED (HELD FOR USER APPROVAL)  
**Governance Constraint:** In strict compliance with Sprint 17 instructions, no public release has been published and no Git tag has been pushed to `origin`.

---

## 1. Release Manifest & Target Asset Matrix

The following table defines the official payload for the `v0.1.0-beta.2` GitHub Release:

| Asset Filename | Platform / Target | Size (Bytes) | SHA-256 Checksum |
| :--- | :--- | :---: | :--- |
| **`AetherIDE-Setup-0.1.0-beta.2-x64.exe`** | Windows 10/11 x64 (Installer) | 8,374,833 | `B2DEE0AE658A1860ABC7F1FA0FBEDF9F4B838B84B73D108BD82D8E55FEB99173` |
| **`AetherIDE-v0.1.0-beta.2-windows-x64.zip`** | Windows 10/11 x64 (Portable) | 10,323,820 | `456F38D410F00B15A3C7D62135BE18A45CC2A2CB3487C957D84A01C0012D9A4C` |
| **`AetherIDE-v0.1.0-beta.2-linux-x64.tar.gz`** | Linux x64 (glibc 2.35+) | 12,538,961 | `615F6403092792020837D6D8AC0F82E2D10D5CFC7019AAB79F3D1DA9F9D67FFC` |
| **`AetherIDE-v0.1.0-beta.2-macos-arm64.tar.gz`** | macOS Apple Silicon (arm64) | 8,889,706 | `64C2A6A6D702528FE0DB4C0C9CDB62D4B883296997B1DA99DE38C60FB353F236` |
| **`RELEASE_MANIFEST.json`** | All Platforms (Metadata) | ~1.2 KB | Generated manifest mapping digests |
| **`SHA256SUMS.txt`** | All Platforms (Checksums) | ~320 bytes | Text digest manifest |

---

## 2. Release Notes Body (Staged Markdown)

```markdown
# AetherIDE v0.1.0-beta.2 — Native Cross-Platform AI-Assisted IDE

AetherIDE is a high-performance, AI-native integrated development environment engineered in Rust, designed for fast startup, low RAM consumption, capability-based filesystem security, and zero background telemetry.

### What's New in Beta.2:
- **Windows Setup Installer (.exe):** Introducing `AetherIDE-Setup-0.1.0-beta.2-x64.exe`—a lightweight (8.37 MB), per-user Inno Setup installer with Start Menu and desktop shortcut management, dependency verification, and clean uninstallation.
- **Dynamic Lazy Loading:** Monaco code studio loads instantaneously. Auxillary modules (xterm.js terminal, Cytoscape graph, agent personas) load on-demand when their respective views are activated.
- **Native Win32 Splash Screen:** Instantaneous (< 15 ms) dark splash frame provides immediate visual feedback before WebView2 navigation completes.
- **Adaptive Low-Resource Mode (LRM):** Reduces memory footprint to 67.7 MB, optimized for constrained dual-core workstations with 4 GB RAM.
- **Multi-Platform Native Desktop Hosts:** Windows (Win32 + Microsoft Edge WebView2), Linux (GTK3 + WebKitGTK), and macOS (Apple Silicon Cocoa + WKWebView).
- **Public Download Portal:** Staged static download website with direct platform links and cryptographic verification tables.

### Download & Verification:
Verify the SHA-256 checksum of your download before executing:

```powershell
# Windows
Get-FileHash -Algorithm SHA256 .\AetherIDE-Setup-0.1.0-beta.2-x64.exe
# Expected: B2DEE0AE658A1860ABC7F1FA0FBEDF9F4B838B84B73D108BD82D8E55FEB99173
```
```

---

## 3. Governance Verification

The release metadata is fully staged and ready for instant publication via `gh release create` once the user grants explicit permission.
