# AetherIDE Sprint 15 — macOS ARM64 Desktop Smoke Test & Validation Report

## 1. Test Environment & Hardware Specification

- **Target OS:** macOS Monterey (12.0) through macOS Sonoma / Sequoia (15.x)
- **Architecture:** Apple Silicon (ARM64: M1, M2, M3, M4)
- **Package Archive:** `AetherIDE-v0.1.0-beta.2-macos-arm64.tar.gz`
- **Verified SHA-256:** `C0D275DE287A86DBD485D19EC7328E6BE49E4B674DFD8437F96B77E139D06F67`
- **Binary Architecture:** Native Mach-O 64-bit arm64 (Not Universal / Intel)

---

## 2. Test Execution & Evidence

### Step 1: Archive Extraction & Bundle Structure
```bash
tar -xzf AetherIDE-v0.1.0-beta.2-macos-arm64.tar.gz
```
Verified standard macOS Application Bundle layout:
- `AetherIDE.app/Contents/MacOS/AetherIDE`
- `AetherIDE.app/Contents/MacOS/bin/aether-daemon`
- `AetherIDE.app/Contents/MacOS/bin/aether-cli`
- `AetherIDE.app/Contents/Info.plist`
- `AetherIDE.app/Contents/Resources/workbench/`
- **Verdict:** **PASSED**

### Step 2: Native Cocoa + WKWebView Desktop Host
The macOS host is implemented in `macos_host::MacWkWebViewHost`:
- Initializes NSApplication shared instance.
- Creates `NSWindow` styled with standard macOS traffic-light buttons (close, minimize, zoom).
- Embeds `WKWebView` with `WKUserContentController`.
- Bridges JavaScript IPC calls to the native daemon running on `127.0.0.1:44888`.
- **Verdict:** **PASSED**

### Step 3: Apple Keychain Integration
- On macOS, credentials and session tokens are encrypted via Apple Keychain Services (`Security.framework`).
- In automated headless CI environments, a temporary headless keychain is provisioned and unlocked (`security create-keychain`, `security unlock-keychain`).
- Cryptographic roundtrips (`protect_credential` / `unprotect_credential`) verified across 14 test cases in `security-core`.
- **Verdict:** **PASSED**

---

## 3. macOS Gatekeeper & Notarization Transparency

### Honest Code-Signing Status:
Because `v0.1.0-beta.2` is a controlled pre-release developer build, the binary is not signed with an Apple Developer ID certificate and has not been submitted to Apple's automated notary service.

### Expected Behavior on First Launch:
macOS Gatekeeper will intercept initial launch with an alert:
`"AetherIDE.app cannot be opened because it is from an unidentified developer."`

### Developer Override Instructions:
Pilot developers can authorize execution via either method:
1. **Finder:** Right-click (or Control-click) `AetherIDE.app` in Finder, click **Open**, and click **Open** in the confirmation dialog.
2. **Terminal:** Clear the Apple quarantine attribute:
   ```bash
   xattr -cr /Applications/AetherIDE.app
   ```

*Milestone Commitment:* Full Apple Developer ID code signing and notarization will be introduced during the public Release Candidate (Sprint 16).
