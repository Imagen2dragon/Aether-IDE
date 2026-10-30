# AetherIDE — Windows Build & Packaging Directive

## 1. Release Targets
- **Platform:** Windows 10/11 x64
- **Architecture:** `x86_64-pc-windows-msvc`
- **Deliverables:**
  1. Portable Package: `aether-ide-windows-x64-v0.1.0-alpha.zip`
  2. Release Directory: `dist/windows-x64/`
  3. Launcher: `AetherIDE.cmd`
  4. Cryptographic Proof: `CHECKSUMS-SHA256.txt`

---

## 2. Automated Production Build Pipeline
Run the PowerShell build script:
```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\build_windows_release.ps1
```

### Build Pipeline Stages:
1. **Cargo Release Build:**
   - Compiles `aether-daemon.exe` with LTO (Link-Time Optimization) and stripped debug symbols.
   - Compiles `aether-cli.exe`.
2. **Extension Packaging:**
   - Compiles TypeScript into `dist/` with source maps.
   - Bundles views and webview assets into `dist/windows-x64/extensions/aether-core`.
3. **Launcher Generation:**
   - Generates non-blocking background launcher `AetherIDE.cmd` that orchestrates daemon spin-up followed by workbench initialization.
4. **Checksum Generation:**
   - Computes SHA256 hashes for all binary and configuration artifacts.

---

## 3. Portable Distribution Packaging
To generate the redistributable ZIP file:
```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\package_portable_zip.ps1
```
The output file will be written to `dist/aether-ide-windows-x64-v0.1.0-alpha.zip` with companion `.sha256` validation file.
