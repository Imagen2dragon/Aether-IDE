# AetherIDE Sprint 16 — Controlled Developer Pilot Onboarding Guide

**Product:** AetherIDE  
**Target Release:** `v0.1.0-beta.2`  
**Audience:** Pilot Cohort Developers (Windows, Linux, macOS)  
**Distribution:** Private Controlled Evaluation  

---

## 1. Welcome to the AetherIDE Beta Pilot!

Thank you for participating in the controlled developer pilot of **AetherIDE `v0.1.0-beta.2`**! AetherIDE is an AI-native integrated development environment engineered in Rust, designed for high performance, strict privacy, capability-based filesystem security, and developer control.

This guide will help you install, configure, and evaluate AetherIDE in under five minutes.

---

## 2. System Requirements

| Platform | Minimum OS | Runtime Dependencies |
| :--- | :--- | :--- |
| **Windows** | Windows 10 (version 1903+) or Windows 11 x64 | Microsoft Edge WebView2 Runtime (pre-installed on Win 10/11) |
| **Linux** | Ubuntu 22.04 LTS / Fedora 38 / Debian 12 (glibc 2.35+) | `libgtk-3-0`, `libwebkit2gtk-4.1-0` (or `4.0`), `libsecret-1-0` |
| **macOS** | macOS 12 Monterey / 13 Ventura / 14 Sonoma / 15 Sequoia | Apple Silicon (M1, M2, M3, M4) |

---

## 3. Installation & First Launch

AetherIDE is delivered as a zero-installer, portable standalone package.

### 3.1 Windows Installation
1. Extract `AetherIDE-v0.1.0-beta.2-windows-x64.zip` into any folder (e.g., `C:\Tools\AetherIDE` or `%LOCALAPPDATA%\AetherIDE`).
2. Run health check from PowerShell:
   ```powershell
   cd C:\Tools\AetherIDE
   .\AetherIDE.exe doctor
   ```
3. Launch `AetherIDE.exe` by double-clicking or running `.\AetherIDE.exe`.

### 3.2 Linux Installation
1. Extract package:
   ```bash
   tar -xzvf AetherIDE-v0.1.0-beta.2-linux-x64.tar.gz -C ~/aetheride
   cd ~/aetheride
   ```
2. Verify system dependencies:
   ```bash
   ./AetherIDE doctor
   ```
3. Run the desktop application:
   ```bash
   ./AetherIDE
   ```

### 3.3 macOS Installation
1. Extract package:
   ```bash
   tar -xzvf AetherIDE-v0.1.0-beta.2-macos-arm64.tar.gz
   ```
2. Clear quarantine attribute (required for beta builds without commercial Apple Developer ID notarization):
   ```bash
   xattr -cr AetherIDE.app
   ```
3. Drag `AetherIDE.app` to `/Applications` or launch directly:
   ```bash
   open AetherIDE.app
   ```

---

## 4. Initial Configuration

### 4.1 Opening a Workspace
- Click **File > Open Folder...** (`Ctrl+O` / `Cmd+O`) to select your target codebase.
- AetherIDE will initialize a local `.aether/` index in your project root. All index databases are strictly local to that directory.

### 4.2 Configuring AI Providers
AetherIDE supports both local offline models and cloud inference:
- **Local (Ollama / LM Studio):**
  - Start Ollama (`ollama serve`) or LM Studio.
  - AetherIDE automatically discovers running local instances at `http://localhost:11434`.
  - No credentials or internet connection required!
- **Cloud Providers (OpenAI, Anthropic, Gemini, OpenRouter):**
  - Click **Settings > AI Providers**.
  - Enter your API key.
  - The key is immediately saved into your OS native vault (Windows DPAPI, macOS Keychain, Linux Secret Service) and never stored in plain text.

---

## 5. Core Workflows to Test

During the pilot, we encourage you to test the following real-world development workflows:

1. **Smart Code Navigation:** Use `Ctrl+P` (`Cmd+P`) for fast symbol and file searching powered by the Project Brain SQLite FTS5 engine.
2. **AI Task Planning & Coding:**
   - Press `Ctrl+Shift+A` (`Cmd+Shift+A`) to open the Agent Panel.
   - Enter a natural language request (e.g., *"Add input validation to this function"* or *"Write unit tests for this module"*).
   - Watch the Planner agent outline steps, followed by the Coder proposing an interactive side-by-side diff.
3. **Diff Review & Rollback:**
   - Review proposed code changes in the Monaco diff viewer.
   - Click **Accept** to apply changes, or **Reject** to revert immediately.
4. **Integrated ConPTY/xterm Terminal:**
   - Press `` Ctrl+` `` to open the terminal.
   - Run tests, builds, and Git commands with full ANSI streaming support.

---

## 6. How to Report Feedback

If you encounter issues, unexpected behavior, or have feature ideas:
- Use the built-in diagnostic report: **Help > Export Diagnostics** (sanitizes all credentials and paths).
- Submit feedback via our Pilot Feedback Form or email our engineering triage team at `pilot-triage@aetheride.dev`.
- For urgent defects, consult the [Pilot Feedback & SLA Plan](file:///c:/Users/abdal/OneDrive/Desktop/ide/AETHERIDE_SPRINT_16_PILOT_FEEDBACK_PLAN.md).

---

## 7. Clean Removal

To remove AetherIDE from your workstation:
1. Delete the extracted AetherIDE application directory.
2. Optional: Remove workspace indexes by deleting the `.aether/` folder within any opened projects.
3. No registry entries, global daemons, or hidden background services are left behind.
