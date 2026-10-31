# AetherIDE v0.1.0-beta.2 — Developer Pilot Onboarding Guide

Welcome to the AetherIDE controlled developer beta pilot! This guide provides everything you need to set up, verify, launch, and evaluate AetherIDE `v0.1.0-beta.2` on your machine.

---

## 1. System Requirements & Prerequisites

| Operating System | Minimum Architecture | Required Runtime Dependencies | Recommended Specs |
| :--- | :--- | :--- | :--- |
| **Windows** | Windows 10 (1903+) or Windows 11 x64 | Microsoft Visual C++ 2015–2022 Redistributable (x64) | 4+ Cores, 8 GB RAM, SSD |
| **Linux** | Ubuntu 22.04 LTS, Fedora 38+, Arch Linux x64 | `glibc` ≥ 2.35, `libwebkit2gtk-4.1` (or 4.0), `libsecret-1-0` | 4+ Cores, 8 GB RAM, SSD |
| **macOS** | macOS Monterey (12.0) or higher, Apple Silicon (M1/M2/M3/M4) | None (self-contained native ARM64 application) | Apple Silicon, 8 GB RAM |

*Optional for AI Assistance:*
- **Ollama** (`http://127.0.0.1:11434`) or **LM Studio** (`http://127.0.0.1:1234`) running locally.
- Or API credentials for Anthropic, OpenAI, or Google Gemini.

---

## 2. Package Acquisition & Verification

Download the appropriate portable package and its SHA-256 checksum file from the official release distribution:

| Platform | Distribution Package | Expected SHA-256 Checksum |
| :--- | :--- | :--- |
| **Windows x64** | `AetherIDE-v0.1.0-beta.2-windows-x64.zip` | `4C0C4D479E7B3FBD55B9FBE771A2ED7BB8F275565E1A686E41851E25A2F01CBD` |
| **Linux x64** | `AetherIDE-v0.1.0-beta.2-linux-x64.tar.gz` | `8C7FE33205779FECC6655B954196DBCF7C774122CC79B936710147DE8F055C32` |
| **macOS ARM64** | `AetherIDE-v0.1.0-beta.2-macos-arm64.tar.gz` | `A477A333C40A6FDC4C72F4588BFCCA79C078A7AB224E597177FED3FED82D4E10` |

### Integrity Verification Commands:

#### Windows (PowerShell):
```powershell
Get-FileHash -Path .\AetherIDE-v0.1.0-beta.2-windows-x64.zip -Algorithm SHA256
```

#### Linux / macOS:
```bash
sha256sum AetherIDE-v0.1.0-beta.2-linux-x64.tar.gz
# or on macOS:
shasum -a 256 AetherIDE-v0.1.0-beta.2-macos-arm64.tar.gz
```

---

## 3. Installation & First Launch

AetherIDE `v0.1.0-beta.2` is distributed as a zero-install portable application. It does not tamper with your registry or global system files.

### 3.1 Windows x64
1. Extract `AetherIDE-v0.1.0-beta.2-windows-x64.zip` to your preferred directory (e.g., `C:\Tools\AetherIDE` or `%LOCALAPPDATA%\AetherIDE\app`).
2. Add the `bin` directory to your user `PATH` (optional, for CLI usage).
3. Launch `AetherIDE.exe` to start the workbench.
   - Alternatively, test the CLI: `bin\aether-cli.exe --version`
   - Or start the headless daemon: `bin\aether-daemon.exe`

### 3.2 Linux x64
1. Extract the tarball:
   ```bash
   tar -xzf AetherIDE-v0.1.0-beta.2-linux-x64.tar.gz
   cd portable
   ```
2. Ensure executable permissions:
   ```bash
   chmod +x AetherIDE bin/aether-daemon bin/aether-cli aetheride.sh
   ```
3. Run the launch script:
   ```bash
   ./aetheride.sh
   ```

### 3.3 macOS ARM64 (Apple Silicon)
1. Extract the tarball:
   ```bash
   tar -xzf AetherIDE-v0.1.0-beta.2-macos-arm64.tar.gz
   ```
2. Move `AetherIDE.app` to `/Applications` or keep it in your developer folder:
   ```bash
   cp -R AetherIDE.app /Applications/
   ```
3. **Important Note on macOS Gatekeeper:**
   Because beta.2 is distributed as a pre-release developer build without Apple Developer ID notarization, Gatekeeper may display a warning: `"AetherIDE.app cannot be opened because it is from an unidentified developer."`
   - Right-click `AetherIDE.app` in Finder and select **Open**, then click **Open** in the dialog.
   - Or run the following terminal command once to clear the quarantine flag:
     ```bash
     xattr -cr /Applications/AetherIDE.app
     ```

---

## 4. Connecting AI Models

AetherIDE supports both fully local, offline inference and cloud AI providers.

### Local Offline Models (Zero Cloud Latency & Total Privacy):
- **Ollama:** Start Ollama with `ollama run qwen2.5-coder:7b` or `ollama run llama3.1:8b`.
  AetherIDE automatically auto-detects `http://127.0.0.1:11434`.
- **LM Studio:** Launch LM Studio, load your preferred GGUF model, and start the local server on port 1234.

### Cloud AI Providers:
Navigate to **Settings → AI Providers** or use the CLI:
```bash
aether-cli config set-key anthropic <YOUR_KEY>
aether-cli config set-key openai <YOUR_KEY>
aether-cli config set-key gemini <YOUR_KEY>
```
All keys are encrypted at rest using your platform's hardware-backed credential vault (Windows DPAPI, macOS Keychain, or Linux Secret Service).

---

## 5. Security Architecture Notice

- **Capability-Based Filesystem:** All workspace indexing, file edits, and agent actions are constrained to explicitly opened workspace directories via OS directory capabilities (`cap-std`). Path traversal (`../`) and external filesystem access are blocked at the OS capability level.
- **Zero-Trust Identity:** The backend daemon does not trust incoming peer IDs. Every request requires cryptographic session tokens bound to OS-enforced user session identities.
- **Zero Telemetry:** AetherIDE contains zero background telemetry, user activity tracking, or phone-home analytics.

---

## 6. Submitting Feedback & Bug Reports

As a beta pilot developer, your feedback directly shapes `v0.1.0-beta.3` and the upcoming General Availability release!
- See the [Pilot Feedback Plan](file:///c:/Users/abdal/OneDrive/Desktop/ide/AETHERIDE_SPRINT_14_PILOT_FEEDBACK_PLAN.md) for feedback protocols.
- To export an anonymized diagnostic bundle, run:
  ```bash
  aether-cli doctor --export-diagnostics
  ```
- File issues on the GitHub repository: `https://github.com/Imagen2dragon/Aether-IDE/issues`
