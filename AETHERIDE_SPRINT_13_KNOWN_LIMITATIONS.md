# AetherIDE Sprint 13 — Known Limitations & Beta Pilot Considerations

## 1. Release Status & Scope

AetherIDE `v0.1.0-beta.2` is a controlled developer beta release. While core security boundaries, capability filesystem operations, daemon authentication, and IDE workflows have passed rigorous testing, the following operational characteristics and limitations should be noted by pilot participants:

---

## 2. Platform-Specific Considerations

### 2.1 Windows Environment
- **Installer Toolchain:** The Inno Setup script (`scripts/installer/aetheride_installer.iss`) requires `ISCC.exe` to compile the Windows `.exe` setup wizard. When ISCC is not installed on the build machine, the standalone portable ZIP package (`AetherIDE-v0.1.0-beta.2-windows-x64.zip`) serves as the primary distribution vehicle.
- **SmartScreen Prompt:** As an initial beta release without an EV code-signing certificate, Windows SmartScreen may display an informational dialog upon first run. Clicking "More info" -> "Run anyway" allows execution.

### 2.2 Linux Environment
- **Secret Service Daemon:** On headless server instances or minimal container environments without a running D-Bus session or Secret Service provider (`gnome-keyring` or `ksecretservice`), credential store operations will report that the service is unavailable. Desktop environments (GNOME, KDE, XFCE) provide this automatically.
- **AppImage Toolchain:** Generating single-file `.AppImage` bundles requires `appimagetool` on the host machine. The portable `.tar.gz` and `.deb` packages provide full functionality without this dependency.

### 2.3 macOS Environment
- **Gatekeeper Notarization:** Beta releases distributed outside the Mac App Store or without an active Apple Developer ID certificate will require users to right-click -> Open or approve in **System Settings -> Privacy & Security** upon initial launch.
- **Architecture:** Builds are optimized specifically for Apple Silicon (`aarch64-apple-darwin`). Intel x86_64 Macs are not supported in beta.2.

---

## 3. Subsystem Performance Characteristics

### 3.1 Project Indexing Scale
- Workspaces containing up to 25,000 files index in under 5 seconds. Extremely large repositories (>50,000 source files) may experience brief initial indexing latency. Subsequent edits are handled incrementally via `index_file_incremental` without re-scanning the entire workspace.

### 3.2 Keyring Timeout Semantics
- If an operating system modal dialog prompts the user for keychain access (e.g. unlocking a login keyring) and remains unanswered for more than 3,000 ms, `OsKeyringStore` returns an explicit timeout error with outcome-unknown semantics rather than hanging the application.
