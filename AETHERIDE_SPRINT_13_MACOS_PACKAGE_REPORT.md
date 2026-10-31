# AetherIDE Sprint 13 — macOS Release Packaging Report

## 1. Overview & Platform Target

**Platform:** Apple Silicon (macOS 12.0 Monterey, macOS 13 Ventura, macOS 14 Sonoma, macOS 15 Sequoia)  
**Target Architecture:** `aarch64-apple-darwin` (ARM64)  
**Distribution Channel:** Controlled Developer Beta (`v0.1.0-beta.2`)  
**Package Types:**
1. Standard macOS Application Bundle (`AetherIDE.app`)
2. Standalone Portable Tarball (`AetherIDE-v0.1.0-beta.2-macos-arm64.tar.gz`)
3. Drag-and-Drop Disk Image (`AetherIDE-v0.1.0-beta.2-macos-arm64.dmg`)

---

## 2. macOS Application Bundle Structure

The compiled bundle strictly conforms to Apple Developer Bundle Specifications:

```
AetherIDE.app/
└── Contents/
    ├── Info.plist                # Application properties and system declarations
    ├── PkgInfo                   # APPL???? package signature
    ├── MacOS/
    │   ├── AetherIDE             # Native WKWebView desktop launcher host
    │   ├── aether-daemon         # Native ARM64 core background daemon
    │   └── aether-cli            # Developer CLI binary
    └── Resources/
        └── workbench/            # Standalone workbench assets & Monaco editor
```

### 2.1 `Info.plist` Configuration
```xml
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleName</key>
    <string>AetherIDE</string>
    <key>CFBundleDisplayName</key>
    <string>AetherIDE</string>
    <key>CFBundleIdentifier</key>
    <string>org.aetheride.desktop</string>
    <key>CFBundleVersion</key>
    <string>0.1.0-beta.2</string>
    <key>CFBundleShortVersionString</key>
    <string>0.1.0-beta.2</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleSignature</key>
    <string>????</string>
    <key>CFBundleExecutable</key>
    <string>AetherIDE</string>
    <key>LSMinimumSystemVersion</key>
    <string>12.0</string>
    <key>NSHighResolutionCapable</key>
    <true/>
</dict>
</plist>
```

---

## 3. Disk Image (DMG) Creation & Keychain Security

1. **DMG Generation:** Script `scripts/package-macos.sh` stages `AetherIDE.app` alongside a symlink to `/Applications` and invokes `hdiutil create -volname "AetherIDE" -srcfolder <stage> -ov -format UDZO` to generate an optimized, compressed DMG disk image.
2. **Native Keychain Integration:** The background daemon links against the macOS Security framework to store session secrets and AI provider API tokens in the user's native macOS Keychain.
3. **CI Determinism:** In CI, a dedicated keychain (`aether_ci.keychain-db`) is deterministically initialized and verified before tests run, and cleaned up in an `if: always()` teardown step.
