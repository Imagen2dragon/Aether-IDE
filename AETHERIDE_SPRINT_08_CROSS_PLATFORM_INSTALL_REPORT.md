# AETHERIDE — SPRINT 08 CROSS-PLATFORM PACKAGING & INSTALLATION REPORT
## Windows Inno Setup & MSIX, Linux AppImage & Debian, macOS .app & DMG Disk Images

**Date:** 2026-10-31  
**Sprint:** Sprint 08 — Beta Release Validation  
**Evaluator:** Principal Release Engineer & Desktop Packaging Specialist  
**Branch:** `feat/sprint-08-release-validation`  
**Targets:** Windows 10/11 x64, Linux x64, macOS ARM64  
**Status:** **READY — SCRIPTED & CERTIFIED**

---

## 1. CROSS-PLATFORM RELEASE MATRIX

AetherIDE packages native desktop deliverables tailored to the security and installation paradigms of each operating system:

| Platform | Target Architecture | Primary Installer | Alternative Installer | Windowing / Web Engine |
|:---|:---|:---|:---|:---|
| **Windows 10/11** | `x86_64-pc-windows-msvc` | Inno Setup Executable (`.exe`) | MSIX Package (`.msix`) | Win32 + WebView2 |
| **Linux** | `x86_64-unknown-linux-gnu` | Standalone AppImage (`.AppImage`) | Debian Package (`.deb`) | X11/Wayland + WebKitGTK |
| **macOS** | `aarch64-apple-darwin` | Compressed Disk Image (`.dmg`) | Application Bundle (`.app`) | AppKit + WKWebView |

---

## 2. PACKAGING SCRIPTS & SPECIFICATIONS

### 2.1 Windows 10/11 x64
- **Inno Setup Script (`packaging/windows/aether-setup.iss`):**
  - Ultra64 LZMA2 solid compression.
  - User-level per-user install (`PrivilegesRequired=lowest`) avoiding UAC elevation prompts.
  - Optional task to add `aether-cli` directly to the user's `PATH`.
  - Bundles `AetherIDE.exe`, `bin/aether-daemon.exe`, `bin/aether-cli.exe`, and static Monaco workbench assets.
- **MSIX Packaging Pipeline (`packaging/windows/package-msix.ps1`):**
  - Staging layout with `AppxManifest.xml`, full trust capability (`runFullTrust`), and app icon assets.
  - Automatic `MakeAppx.exe` discovery and optional Authenticode signing via `SignTool.exe`.

### 2.2 Linux x64 (Ubuntu / Debian / Fedora / Arch)
- **Debian Packaging (`packaging/linux/build-deb.sh`):**
  - Generates standard `control` file specifying dependencies (`libc6 >= 2.31`, `libwebkit2gtk-4.0-37 | libwebkit2gtk-4.1-0`, `libgtk-3-0`).
  - Installs to `/opt/aetheride`, symlinks `/usr/bin/aetheride` and `/usr/bin/aether`, and registers desktop entry in `/usr/share/applications/`.
- **AppImage Pipeline (`packaging/linux/build-appimage.sh`):**
  - Creates self-contained `AppDir` with custom `AppRun` launcher and embedded runtime libraries.
  - Executes portably across any modern Linux distribution without installation.

### 2.3 macOS ARM64 (Apple Silicon M1/M2/M3/M4)
- **macOS Application Bundle (`packaging/macos/build-app.sh`):**
  - Standard `.app` directory hierarchy: `Contents/MacOS/AetherIDE`, `Contents/Info.plist`, and `Contents/Resources/`.
  - Configures high-resolution retina rendering (`NSHighResolutionCapable`), automatic graphics switching, and minimum OS 12.0 (Monterey).
- **macOS DMG Disk Image (`packaging/macos/build-dmg.sh`):**
  - Staged drag-and-drop disk image with symbolic link to `/Applications`.
  - Uses `hdiutil` with `UDZO` zlib compression.

---

## 3. VERIFICATION

All packaging scripts reside in `packaging/`, use version `0.1.0-beta.1`, output to `dist/installers`, and strictly respect the storage policy keeping build artifacts outside the repository tree.
