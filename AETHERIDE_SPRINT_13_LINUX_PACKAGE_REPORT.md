# AetherIDE Sprint 13 — Linux Release Packaging Report

## 1. Overview & Platform Target

**Platform:** Linux x86_64 (glibc 2.31+, Ubuntu 20.04+, Debian 11+, Fedora 36+, Arch Linux)  
**Distribution Channel:** Controlled Developer Beta (`v0.1.0-beta.2`)  
**Package Types:**
1. Standalone Portable Tarball (`AetherIDE-v0.1.0-beta.2-linux-x64.tar.gz`)
2. AppImage AppDir Specification (`dist/linux-x64/AppDir`)
3. Debian Package Layout (`dist/linux-x64/deb` -> `AetherIDE-v0.1.0-beta.2-linux-x64.deb`)

All native Linux binaries (`AetherIDE`, `aether-daemon`, `aether-cli`) are compiled with standard glibc linking and WebKitGTK / GTK3 native desktop bindings.

---

## 2. Package Formats & Directory Layouts

### 2.1 Portable Tarball (`AetherIDE-v0.1.0-beta.2-linux-x64.tar.gz`)
Self-contained archive suitable for any modern Linux distribution without package manager installation:
```
portable/
├── AetherIDE                 # WebKitGTK / native desktop host
├── aetheride.sh              # Environment wrapper & PATH launcher
├── bin/
│   ├── aether-daemon         # Core background daemon
│   └── aether-cli            # Developer CLI tool
├── workbench/                # HTML5 workbench, Monaco, Cytoscape, xterm
├── VERSION.txt               # 0.1.0-beta.2
├── LICENSE                   # MIT License
└── README.md
```

### 2.2 AppImage Specification (`AppDir`)
Complies with AppImageKit directory structure:
```
AppDir/
├── AppRun                    # Primary entry point setting PATH and resource roots
├── aetheride.desktop         # Standard XDG desktop entry
├── usr/
│   ├── bin/
│   │   ├── aetheride         # Symlink/binary to main host
│   │   ├── aether-daemon     # Background daemon
│   │   └── aether-cli        # CLI tool
│   ├── lib/
│   │   └── aetheride/        # Bundled workbench assets
│   └── share/icons/hicolor/
```
Built into a self-mounting squashfs AppImage when `appimagetool` is available.

### 2.3 Debian Package (`.deb`)
Configured for automated installation on Debian, Ubuntu, Linux Mint, and Pop!_OS:
- **Control Metadata:**
  ```text
  Package: aetheride
  Version: 0.1.0-beta.2
  Section: devel
  Priority: optional
  Architecture: amd64
  Maintainer: AetherIDE Engineering Organization <dev@aetheride.org>
  Depends: libgtk-3-0, libwebkit2gtk-4.1-0 | libwebkit2gtk-4.0-37, libsoup2.4-1 | libsoup-3.0-0
  ```
- **FHS Placement:**
  - `/usr/bin/aetheride`, `/usr/bin/aether-daemon`, `/usr/bin/aether-cli`
  - `/usr/lib/aetheride/workbench/`
  - `/usr/share/applications/aetheride.desktop`

---

## 3. Automation Script (`scripts/package-linux.sh`)

The script is executable across all POSIX shells and integrated directly into `.github/workflows/ci.yml`. It handles:
- Automated release binary compilation (if not pre-compiled).
- Asset copying, permission granting (`chmod +x`), and directory layout assembly.
- Tarball compression and SHA-256 generation.
- AppImage packaging when `appimagetool` is installed.
- `.deb` binary package creation using `dpkg-deb --build`.
