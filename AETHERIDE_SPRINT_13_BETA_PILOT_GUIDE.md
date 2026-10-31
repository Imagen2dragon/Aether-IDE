# AetherIDE — Controlled Developer Beta Pilot Guide
## Version: v0.1.0-beta.2

Welcome to the AetherIDE Controlled Developer Beta! This guide provides everything you need to install, configure, and evaluate AetherIDE in your development environment.

---

## 1. System Requirements

| Platform | Minimum OS Version | CPU Architecture | Recommended Memory | Storage |
| :--- | :--- | :--- | :--- | :--- |
| **Windows** | Windows 10 (21H2+) / Windows 11 | x86_64 (64-bit) | 8 GB RAM (16 GB for large codebases) | 2 GB free disk space |
| **Linux** | Ubuntu 20.04+, Debian 11+, Fedora 36+ | x86_64 (64-bit glibc 2.31+) | 8 GB RAM | 2 GB free disk space |
| **macOS** | macOS 12.0 Monterey or newer | Apple Silicon (M1 / M2 / M3 / M4) | 8 GB Unified Memory | 2 GB free disk space |

---

## 2. Installation & Quick Start

### 2.1 Windows Installation
**Option A: Standalone Portable Package (Recommended for Pilot)**
1. Download `AetherIDE-v0.1.0-beta.2-windows-x64.zip`.
2. Extract the archive to any convenient directory (e.g. `C:\Tools\AetherIDE`).
3. Run `AetherIDE.cmd` (or `AetherIDE.exe`).
4. (Optional) Add the `bin\` folder to your PATH to use `aether-cli` directly in PowerShell or Command Prompt.

**Option B: Inno Setup Installer**
1. Run `AetherIDE-v0.1.0-beta.2-windows-x64-setup.exe`.
2. Follow the setup wizard; it installs to `%LocalAppData%\Programs\AetherIDE` without requiring administrator privileges.
3. Check "Add AetherIDE bin directory to user PATH" for command-line access.

### 2.2 Linux Installation
**Option A: Portable Tarball**
1. Download `AetherIDE-v0.1.0-beta.2-linux-x64.tar.gz`.
2. Extract: `tar -xzf AetherIDE-v0.1.0-beta.2-linux-x64.tar.gz`.
3. Launch: `./portable/aetheride.sh`.

**Option B: Debian / Ubuntu Package**
1. Download `AetherIDE-v0.1.0-beta.2-linux-x64.deb`.
2. Install: `sudo dpkg -i AetherIDE-v0.1.0-beta.2-linux-x64.deb`.
3. Launch via terminal `aetheride` or application launcher.

### 2.3 macOS Installation
1. Download `AetherIDE-v0.1.0-beta.2-macos-arm64.dmg` (or tarball).
2. Open the DMG and drag `AetherIDE.app` to `/Applications`.
3. Launch `AetherIDE` from Spotlight or Launchpad.

---

## 3. First-Run Configuration

### 3.1 AI Provider Setup
AetherIDE supports cloud and local model providers. Your credentials are protected using your OS's native secure credential store (Windows DPAPI, macOS Keychain, Linux Secret Service):
1. Navigate to **Settings -> AI Providers** in the workbench.
2. Select your preferred provider:
   - **Anthropic Claude:** Paste your API key (`sk-ant-...`).
   - **OpenAI:** Paste your API key (`sk-...`).
   - **Google Gemini:** Paste your API key (`AIza...`).
   - **Local Ollama:** Select "Local Ollama" and specify endpoint (defaults to `http://localhost:11434`).
3. Click **Save & Test Connection**. The key is securely encrypted and validated against the provider test endpoint.

### 3.2 Opening a Workspace
1. Click **File -> Open Workspace Folder**.
2. Select your local project directory.
3. AetherIDE will automatically scan your project using capability-bounded sandboxing, build the symbol index, and render the initial Project Brain dependency radar.

---

## 4. Key Workflows to Test

1. **Project Brain Radar:** Open the Brain pane to inspect your codebase architecture visualized as an interactive dependency graph.
2. **Integrated Terminal:** Open terminal panels (`Ctrl+` `) and verify ConPTY/Unix PTY responsiveness.
3. **Autonomous Agent Task:** Open the AI Agent panel, type a task description (e.g., "Add unit test for parser module"), and review the step-by-step plan before execution.
4. **Checkpoint Rollback:** If an agent step makes unwanted changes, click **Rollback Checkpoint** in the status bar to restore the workspace instantly.
5. **Real-Time Collaboration:** Click **Collaborate -> Start Session** to generate a peer pairing invite link or QR code.

---

## 5. Submitting Pilot Feedback & Diagnostics

We value your feedback during the beta pilot!
- **Feedback & Bug Reports:** Open an issue at [https://github.com/Imagen2dragon/Aether-IDE/issues](https://github.com/Imagen2dragon/Aether-IDE/issues).
- **Exporting Diagnostics:** Run `aether-cli diagnostics export` or click **Help -> Export Diagnostic Bundle** in the workbench. All diagnostic bundles are automatically sanitized to remove secrets and file paths.
