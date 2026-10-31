# AETHERIDE — SPRINT 09 CONTROLLED BETA PILOT GUIDE
## Participant Onboarding, Diagnostic Protocol, Feedback Loops & Operational Procedures

**Product:** AetherIDE  
**Release Target:** `v0.1.0-beta.2`  
**Audience:** External Beta Pilot Program Participants & Engineering Leads  
**Classification:** Beta Distribution Package  

---

## 1. Welcome to the AetherIDE Beta Pilot

Thank you for participating in the controlled external beta of AetherIDE. AetherIDE is an AI-native software engineering platform built from the ground up in Rust, featuring native desktop windowing (Microsoft WebView2, WebKitGTK, WKWebView), Monaco Editor, Windows ConPTY terminal hosting, and autonomous multi-agent swarm engineering teams.

---

## 2. System Prerequisites

### Supported Operating Systems:
- **Windows:** Windows 10 (version 1903+) or Windows 11 x64.
  - Microsoft Edge WebView2 Runtime (pre-installed on Windows 11; auto-installed if missing).
- **Linux:** Ubuntu 20.04+, Debian 11+, Fedora 36+, or Arch Linux (x86_64).
  - WebKitGTK 4.0 runtime libraries (`libwebkit2gtk-4.0-37`).
- **macOS:** macOS Monterey (12.0+) or later (Apple Silicon ARM64 or Intel x86_64).

### Recommended Hardware:
- **Processor:** 4+ cores (Intel Core i5/i7/i9, AMD Ryzen 5/7/9, or Apple M1/M2/M3/M4).
- **RAM:** Minimum 8 GB (16 GB recommended for multi-agent workflows).
- **Disk:** 2 GB available SSD storage.

---

## 3. Quickstart: 3-Minute Zero-Friction Setup

### Step 1: Install AetherIDE
- **Windows:** Run `AetherIDE-v0.1.0-beta.2-x64-Setup.exe` and follow prompts.
- **Linux:** Make executable and run `./AetherIDE-v0.1.0-beta.2-x86_64.AppImage`.
- **macOS:** Open `AetherIDE-v0.1.0-beta.2-arm64.dmg` and drag `AetherIDE.app` to `Applications`.

### Step 2: System Health Check
Open your terminal and run:
```bash
aether-cli doctor
```
This inspects local toolchains, Git presence, WebView2 runtime, and database health.

### Step 3: Launch AetherIDE
Launch AetherIDE from your desktop shortcut or applications menu.
The desktop window will initialize, start the local `aether-daemon`, establish the zero-trust desktop bootstrap handshake, and present the Monaco IDE Workbench.

---

## 4. Key Workflows to Explore

### Workflow A: Multi-Tab Code Editing with Monaco & LSP
- Open any source repository via `File -> Open Folder`.
- Open `.rs`, `.ts`, `.py`, or `.json` files.
- Experience instant syntax highlighting, AST folding, hover tooltips, and symbol autocomplete.

### Workflow B: Interactive ConPTY Terminal
- Open the bottom drawer or press `Ctrl+\``.
- Launch interactive PowerShell, CMD, or Bash shells.
- Run builds, git operations, or interactive CLIs with native PTY throughput.

### Workflow C: Autonomous Multi-Agent Swarms
- Open the **Agent Command Center** tab in the sidebar.
- Choose from 14 autonomous engineering personas (e.g. `architect`, `backend`, `security`, `debugging`).
- Spawn tasks or create multi-agent DAG pipelines.
- Watch tasks execute with isolated worktrees and structured three-way merges.

### Workflow D: Real-Time CRDT Collaboration
- Share your session or workspace pairing URI (`aether://pair?...`).
- Connect another client or mobile companion.
- Edit code simultaneously with zero merge lockups and real-time cursor presence.

---

## 5. Diagnostic Protocol & Feedback Export

If you encounter unexpected behavior, run the privacy-preserving diagnostics export:

```bash
aether-cli doctor --export
```

This generates `aether-diagnostics-report.json` containing:
- Operating system version, CPU architecture, memory metrics.
- Daemon connection status, database latency, and storage footprint.
- **Strict Privacy Guarantee:** All API keys, tokens, file paths containing usernames, and source code contents are automatically scrubbed via `SecretRedactor` before generation.

### Reporting Issues:
- **GitHub Issue Tracker:** Submit issues to `https://github.com/Imagen2dragon/Aether-IDE/issues` with the sanitized diagnostics report attached.
- **Beta Feedback Channel:** Directly message your designated technical pilot coordinator.
