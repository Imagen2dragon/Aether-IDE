# AetherIDE — Sprint 06 Build and Compilation Guide

---

## 1. Prerequisites

### Windows x64
- Windows 10/11 x64
- Rust 1.85+ stable with `x86_64-pc-windows-msvc` target
- Microsoft Visual C++ Build Tools (MSVC v143+)
- Microsoft Edge WebView2 Runtime (Evergreen)

### Linux x64
- Ubuntu 22.04+ / Debian 12+
- `libwebkit2gtk-4.0-dev` or `libwebkit2gtk-4.1-dev`
- `libgtk-3-dev`, `libsoup2.4-dev`
- `appimagetool` (for AppImage packaging) or `dpkg-deb` (for Debian packaging)

### macOS ARM64
- macOS 12+ (Monterey, Ventura, Sonoma, Sequoia)
- Apple Silicon (M1/M2/M3/M4)
- Xcode Command Line Tools (`xcode-select --install`)

---

## 2. Storage-Aware External Build Cache

AetherIDE strictly enforces external build caches outside OneDrive to prevent sync engine thrashing, file locks, and disk bloat.

### Recommended Build Command
Always use the automated build scripts:

```powershell
# Windows
.\scripts\build.ps1 dev      # Fast development build with line tables (debug=1)
.\scripts\build.ps1 release  # Optimized production release with stripped symbols
```

```bash
# Linux / macOS
./scripts/build.sh dev
./scripts/build.sh release
```

### Manual Cargo Invocation
If calling `cargo` manually, you **MUST** specify `CARGO_TARGET_DIR` outside of OneDrive:

```powershell
$env:CARGO_TARGET_DIR = "$env:LOCALAPPDATA\AetherIDE\build-cache\windows-x64"
cargo build --workspace
```

---

## 3. Storage Diagnostics and Cleanup

AetherIDE provides a built-in CLI tool to monitor and maintain disk budgets:

```bash
# Audit storage consumption across repository and caches
aether storage audit

# Check current budget status against 6.00 GB target / 8.00 GB warning
aether storage status

# Perform dry-run sweep of stale compilation caches
aether storage clean --dry-run

# Apply purge to reclaim disk space
aether storage clean --apply
```
