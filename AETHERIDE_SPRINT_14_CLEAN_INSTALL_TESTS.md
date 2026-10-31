# AetherIDE Sprint 14 — Clean-Environment Installation & Smoke Testing Report

## 1. Overview & Testing Methodology

This report documents the installation and smoke testing of the downloaded AetherIDE `v0.1.0-beta.2` release packages in clean, isolated target environments. In accordance with Sprint 14 directives, tests were performed against the actual downloaded release archives, not raw cargo development builds.

---

## 2. Windows 11 x64 Clean-Environment Test

### 2.1 Extraction & Setup Procedure
1. **Archive Path:** `%LOCALAPPDATA%\AetherIDE\downloaded-artifacts\windows-extracted\AetherIDE-v0.1.0-beta.2-windows-x64.zip`
2. **SHA-256 Validation:**
   - Expected: `4C0C4D479E7B3FBD55B9FBE771A2ED7BB8F275565E1A686E41851E25A2F01CBD`
   - Observed: `4C0C4D479E7B3FBD55B9FBE771A2ED7BB8F275565E1A686E41851E25A2F01CBD` (PASS)
3. **Extraction Target:** `%LOCALAPPDATA%\AetherIDE\clean-install-test\windows-x64`
4. **Result:** All binaries (`AetherIDE.exe`, `bin\aether-cli.exe`, `bin\aether-daemon.exe`) and web assets unpacked without corruption.

### 2.2 CLI Execution Test
```powershell
& ".\bin\aether-cli.exe" --version
# Result: PASS (Output: aether-cli 0.1.0-beta.2)
```

### 2.3 Daemon Bootstrap & Zero-Trust Identity
The background daemon was launched from the isolated extraction root:
- Created user token database at `%LOCALAPPDATA%\AetherIDE\sessions\`.
- Published DPAPI-encrypted bootstrap token to `%LOCALAPPDATA%\AetherIDE\desktop_bootstrap.json`.
- Bound TCP loopback listener to `127.0.0.1:44888`.
- Bound Named Pipe to `\\.\pipe\aether-daemon-ipc`.
- Verified unauthenticated requests are rejected with HTTP 401.

### 2.4 Process Termination & Hygiene
Terminating the daemon process resulted in clean shutdown:
- Named pipe and TCP sockets unmapped.
- Session secrets wiped via volatile zeroization.
- Zero orphaned helper processes left running.

---

## 3. Linux Clean-Environment Validation

- **Archive:** `AetherIDE-v0.1.0-beta.2-linux-x64.tar.gz`
- **Extracted Structure:** Self-contained `portable/` tree.
- **Launcher:** `portable/aetheride.sh` configures `PATH` and starts `AetherIDE`.
- **System Prerequisites:** Verified against standard Ubuntu 20.04/22.04 LTS dependencies (`libgtk-3-0`, `libwebkit2gtk-4.1-0`, `dbus-x11`, `gnome-keyring`).
- **Secret Service:** Daemon connects via D-Bus to the active user keyring.

---

## 4. macOS Clean-Environment Validation

- **Archive:** `AetherIDE-v0.1.0-beta.2-macos-arm64.tar.gz`
- **Extracted Structure:** Standard `AetherIDE.app` bundle.
- **Execution Target:** Apple Silicon (M-series processor).
- **Gatekeeper Handling:** User instructed to right-click -> Open on first launch to clear Gatekeeper quarantine attribute.
- **Keychain Integration:** Uses macOS Security framework for persistent, encrypted credential storage.
