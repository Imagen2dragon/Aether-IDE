# AetherIDE — Sprint 06 Enterprise Deployment & Packaging Guide

---

## 1. Enterprise Packaging Formats

AetherIDE provides production-grade release packaging for all major operating systems:

### Windows Enterprise Packaging
1. **Inno Setup Standard Installer (`packaging/windows/aether-setup.iss`):**
   - Solid LZMA2 compression.
   - Per-user or per-machine installation into `%LOCALAPPDATA%\Programs\AetherIDE` or `C:\Program Files\AetherIDE`.
   - Desktop and Start Menu shortcut creation.
   - Automatic PATH variable registration for `aether-cli`.
   - Clean uninstaller removing registry keys and local binaries.

2. **MSIX Modern Package (`packaging/windows/AppxManifest.xml`, `package-msix.ps1`):**
   - Windows 10/11 sandbox compatibility.
   - Declarative `AppxManifest.xml` with `runFullTrust` capability.
   - Code signing support with enterprise PKI certificates.
   - Zero-impact install and clean uninstall via Windows Package Manager (`winget`).

### Linux Enterprise Packaging
1. **AppImage Universal Binary (`packaging/linux/build-appimage.sh`):**
   - Single-file distribution executable on any modern x86_64 Linux distribution.
   - Bundles all necessary dynamic libraries and XDG desktop integrations.

2. **Debian / Ubuntu Package (`packaging/linux/build-deb.sh`):**
   - Standard `.deb` package compatible with `apt` and `dpkg`.
   - Installs binaries to `/opt/aetheride/` with symlinks in `/usr/bin/`.

### macOS Enterprise Packaging
1. **Application Bundle (`packaging/macos/build-app.sh`):**
   - Standard `AetherIDE.app` bundle structure with `Info.plist` and resource bundles.

2. **Apple DMG Disk Image (`packaging/macos/build-dmg.sh`):**
   - Drag-and-drop disk image installer with symlink to `/Applications`.
