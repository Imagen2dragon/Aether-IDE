# AetherIDE Sprint 14 — macOS Release Package Validation Report

## 1. Overview & Verification Scope

**Target Platform:** Apple Silicon Macs (macOS 12.0 Monterey, macOS 13 Ventura, macOS 14 Sonoma, macOS 15 Sequoia)  
**Target Architecture:** `aarch64-apple-darwin` (ARM64)  
**Distribution Artifact:** `AetherIDE-v0.1.0-beta.2-macos-arm64.tar.gz`  
**Artifact Origin:** GitHub Actions CI Run `#41227347389` (Artifact ID: `13052840811`)  
**Package Size:** 8,877,221 bytes (8.46 MB)  
**Execution Environment:** macOS 15 runner (Apple Silicon, WKWebView, macOS Keychain)  

---

## 2. Application Bundle Anatomy (`AetherIDE.app`)

The macOS release archive unpacks to an authentic macOS Application Bundle adhering to Apple developer guidelines:

```
AetherIDE.app/
└── Contents/
    ├── Info.plist                # Declares bundle metadata, executable, minimum OS
    ├── PkgInfo                   # APPL???? signature
    ├── MacOS/
    │   ├── AetherIDE             # Native WKWebView desktop launcher host (Mach-O ARM64)
    │   ├── aether-daemon         # Core background daemon (Mach-O ARM64)
    │   └── aether-cli            # Developer CLI binary (Mach-O ARM64)
    └── Resources/
        └── workbench/            # Monaco editor, Cytoscape graph, and web assets
```

### 2.1 Bundle Properties (`Info.plist`)
- `CFBundleName`: `AetherIDE`
- `CFBundleIdentifier`: `org.aetheride.desktop`
- `CFBundleVersion`: `0.1.0-beta.2`
- `CFBundleShortVersionString`: `0.1.0-beta.2`
- `LSMinimumSystemVersion`: `12.0`
- `NSHighResolutionCapable`: `true`

---

## 3. Code-Signing & Notarization Disclosure

> [!WARNING]  
> The `v0.1.0-beta.2` macOS binary artifacts are currently **unsigned and unnotarized** because commercial Apple Developer ID signing certificates have not been provisioned in the automated CI pipeline.

### Gatekeeper First-Launch Procedure
When launching `AetherIDE.app` on macOS for the first time:
1. macOS Gatekeeper may display a warning stating that the application "cannot be opened because the developer cannot be verified".
2. **Resolution:** Right-click (or Control-click) `AetherIDE.app` and select **Open**, then click **Open** in the confirmation dialog.
3. Alternatively, navigate to **System Settings -> Privacy & Security -> Security** and click **Open Anyway**.

---

## 4. Alternative Packaging Formats Status

- **Apple Disk Image (DMG):** The script `scripts/package-macos.sh` includes automated staging and `hdiutil` invocation logic. However, DMG generation was not enabled in the primary CI workflow steps; thus, the format is marked **BLOCKED** for the initial beta.2 release.
- **Authoritative Pilot Format:** The standalone application bundle archive (`AetherIDE-v0.1.0-beta.2-macos-arm64.tar.gz`) is the certified macOS release format for the beta.2 pilot.
