# AETHERIDE — SPRINT 09 INSTALLER VALIDATION REPORT
## Multi-Platform Distribution Packaging, Clean Install, Upgrade & Uninstall (E2E-11)

**Product:** AetherIDE  
**Target Release:** `v0.1.0-beta.2`  
**Development Branch:** `feat/sprint-09-beta-pilot-hardening`  
**Packaging Directory:** `packaging/`  
**Status:** VALIDATED ACROSS ALL THREE TARGET PLATFORMS  

---

## 1. Executive Summary

Sprint 09 validates **E2E-11 (Full Installer Lifecycle)** across Windows, Linux, and macOS. AetherIDE provides production-grade installers that respect operating system standards, clean uninstallation, side-by-side user data isolation, and desktop shortcut provisioning.

---

## 2. Platform Installer Configurations

### 2.1 Windows 10/11 x64
- **Inno Setup Script (`packaging/windows/aether-setup.iss`):**
  - Installs to `C:\Program Files\AetherIDE` (machine-wide) or `%LOCALAPPDATA%\Programs\AetherIDE` (per-user non-elevated).
  - Bundles:
    - `AetherIDE.exe` (Win32 + WebView2 launcher)
    - `aether-daemon.exe` (native Rust background service)
    - `aether-cli.exe` (CLI management tool)
    - `workbench/` (compiled Monaco editor assets, TypeScript bundles)
  - Registers file associations for `.aether`, `.rs`, `.ts`, `.tsx`, `.py`, `.json`.
  - Creates Start Menu shortcuts and Desktop icon.
  - Generates uninstaller that purges binaries while preserving user databases in `%LOCALAPPDATA%\AetherIDE\aether.db`.
- **MSIX Modern Package (`packaging/windows/AppxManifest.xml`, `package-msix.ps1`):**
  - Clean Windows Package Manager (`winget`) distribution.
  - Declarative capabilities: `runFullTrust`.
  - Declarative uninstall with zero leftover registry keys or orphaned files.

### 2.2 Linux x64
- **AppImage (`packaging/linux/build-appimage.sh`):**
  - Portable, distribution-agnostic binary bundle for Ubuntu 20.04+, Debian 11+, Fedora 36+, Arch Linux.
  - Embeds GTK3 runtime dependencies and WebKitGTK bindings.
  - Integrates desktop entry (`aetheride.desktop`) and AppStream metadata.
- **Debian Package (`packaging/linux/build-deb.sh`):**
  - Generates `.deb` package targeting Debian/Ubuntu APT repositories.
  - Configures dependencies: `libc6 (>= 2.31), libgtk-3-0, libwebkit2gtk-4.0-37`.
  - Installs binaries into `/usr/bin/` and workbench assets into `/usr/share/aetheride/`.

### 2.3 macOS ARM64 / Universal
- **Application Bundle (`packaging/macos/build-app.sh`):**
  - Assembles standard macOS app structure `AetherIDE.app`:
    - `AetherIDE.app/Contents/MacOS/AetherIDE`
    - `AetherIDE.app/Contents/MacOS/aether-daemon`
    - `AetherIDE.app/Contents/Resources/workbench/`
    - `AetherIDE.app/Contents/Info.plist`
- **Apple Disk Image (`packaging/macos/build-dmg.sh`):**
  - Drag-and-drop installer DMG with `/Applications` symlink.
  - Prepared for Apple Notarization (`xcrun notarytool`) and hardened runtime entitlements.

---

## 3. Installer Lifecycle Matrix

| Action | Windows (Inno / MSIX) | Linux (AppImage / DEB) | macOS (DMG / App) | Verification |
|:---|:---|:---|:---|:---:|
| **Clean Install** | Creates directory, extracts binaries, registers PATH | Extracts or installs to `/usr/share/` | Drags `AetherIDE.app` to `/Applications` | **PASSED** |
| **First Launch** | Bootstraps `%LOCALAPPDATA%\AetherIDE\`, generates SQLite WAL | Bootstraps `~/.aether/`, generates SQLite WAL | Bootstraps `~/Library/Application Support/AetherIDE/` | **PASSED** |
| **In-Place Upgrade** | Overwrites binaries, runs DB migration without data loss | Replaces AppImage or `dpkg -i` upgrade | Replaces `AetherIDE.app` | **PASSED** |
| **Clean Uninstall** | Uninstaller removes all binaries and registry keys | `apt purge` or deleting AppImage file | Moves `AetherIDE.app` to Trash | **PASSED** |
| **User Data Safety** | User project brain, memory notes, and keys preserved | User configuration in `~/.aether/` preserved | User data in `Application Support` preserved | **PASSED** |
