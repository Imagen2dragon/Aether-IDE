# AetherIDE v0.1.0-beta.2 — Developer Pilot Onboarding Guide

Welcome to the AetherIDE controlled external developer pilot! AetherIDE is an AI-native software engineering studio engineered from the ground up in Rust and Monaco Editor with capability-based security and local-first AI intelligence.

---

## 1. System Requirements & Prerequisites

| Operating System | Minimum OS Version | Architecture | Required System Runtimes |
| :--- | :--- | :--- | :--- |
| **Windows** | Windows 10 (Build 18362+) or Windows 11 | x86_64 | Microsoft Edge WebView2 Evergreen, MSVC x64 Redistributable |
| **Linux** | Ubuntu 22.04 LTS, Fedora 38+, Arch | x86_64 | `glibc` ≥ 2.35, `libwebkit2gtk-4.1-0`, `libsecret-1-0` |
| **macOS** | macOS Monterey (12.0) or later | Apple Silicon (ARM64) | None (native Cocoa/WKWebView built-in) |

---

## 2. Package Acquisition & Checksum Verification

Download the appropriate archive and calculate its SHA-256 hash before opening:

| Platform | Package Name | Expected SHA-256 Digest |
| :--- | :--- | :--- |
| **Windows x64** | `AetherIDE-v0.1.0-beta.2-windows-x64.zip` | `E7636A5B6F3931172292CF5D8E1911FBEB30981E0ED65DCA529A2C47536CD8F0` |
| **Linux x64** | `AetherIDE-v0.1.0-beta.2-linux-x64.tar.gz` | `4C0D497EF91E35B219CA2A3DF0A759DEF86F9613F6A9A128FD345EDD6CF130A7` |
| **macOS ARM64** | `AetherIDE-v0.1.0-beta.2-macos-arm64.tar.gz` | `C0D275DE287A86DBD485D19EC7328E6BE49E4B674DFD8437F96B77E139D06F67` |

### Integrity Verification Commands:
- **Windows (PowerShell):** `Get-FileHash -Path .\AetherIDE-v0.1.0-beta.2-windows-x64.zip -Algorithm SHA256`
- **Linux:** `sha256sum AetherIDE-v0.1.0-beta.2-linux-x64.tar.gz`
- **macOS:** `shasum -a 256 AetherIDE-v0.1.0-beta.2-macos-arm64.tar.gz`

---

## 3. Extraction & First Launch

AetherIDE is distributed as a zero-install, portable application that makes no unsolicited changes to system registries or global folders.

### 3.1 Windows
1. Extract `AetherIDE-v0.1.0-beta.2-windows-x64.zip` to a folder of your choice (e.g., `C:\Tools\AetherIDE`).
2. Double-click `AetherIDE.exe` to launch the studio.
3. (Optional) Run `.\AetherIDE.exe doctor` from PowerShell to verify your developer toolchains.

### 3.2 Linux
1. Extract the tarball: `tar -xzf AetherIDE-v0.1.0-beta.2-linux-x64.tar.gz`
2. Enter the directory and launch:
   ```bash
   cd portable
   ./aetheride.sh
   ```

### 3.3 macOS (Apple Silicon)
1. Extract the tarball: `tar -xzf AetherIDE-v0.1.0-beta.2-macos-arm64.tar.gz`
2. Move `AetherIDE.app` to `/Applications`.
3. **Note on Gatekeeper:** Because this is an unnotarized beta build, macOS will display an unidentified developer warning.
   - Right-click `AetherIDE.app` in Finder and choose **Open**, then confirm **Open**.
   - Or clear the quarantine attribute in Terminal: `xattr -cr /Applications/AetherIDE.app`

---

## 4. Configuring AI Assistance

### Offline Local Models (Recommended for Complete Privacy):
- Start Ollama (`http://127.0.0.1:11434`) or LM Studio (`http://127.0.0.1:1234`).
- AetherIDE auto-detects active local models without any configuration.

### Cloud API Keys:
Configure credentials via the UI under **Settings → AI Providers** or using the CLI:
```bash
aether-cli config set-key anthropic <YOUR_KEY>
aether-cli config set-key openai <YOUR_KEY>
aether-cli config set-key gemini <YOUR_KEY>
```
All keys are securely encrypted at rest using your operating system's hardware-backed vault (Windows DPAPI, macOS Keychain, or Linux Secret Service).

---

## 5. Security Architecture & Privacy Guarantee

- **Capability-Based Filesystem:** All workspace indexing, file edits, and agent actions are constrained to explicitly opened workspace directories via OS directory capabilities (`cap-std`). Path traversal (`../`) and external filesystem access are blocked at the OS capability level.
- **Zero-Trust Identity:** The backend daemon does not trust incoming peer IDs. Every request requires cryptographic session tokens bound to OS-enforced user session identities.
- **Zero Telemetry:** AetherIDE contains zero background telemetry, user activity tracking, or phone-home analytics.

---

## 6. Feedback & Uninstallation

- Export sanitized diagnostic bundles via `aether-cli doctor --export-diagnostics`.
- File feedback on GitHub: `https://github.com/Imagen2dragon/Aether-IDE/issues`.
- To completely uninstall AetherIDE, simply delete the extracted application folder and user settings directory (`%LOCALAPPDATA%\AetherIDE\` on Windows, `~/.config/aetheride/` on Linux, `~/Library/Application Support/AetherIDE/` on macOS).
