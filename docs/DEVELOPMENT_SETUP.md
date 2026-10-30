# AetherIDE — Development Setup Guide

## 1. Prerequisites
Ensure your development environment meets the following baseline toolchain versions:
- **Operating System:** Windows 10/11 x64 (Build 19045 or higher)
- **Rust Toolchain:** Stable 1.80+ (`rustup toolchain install stable`)
  - Target: `x86_64-pc-windows-msvc`
  - Components: `rustfmt`, `clippy`
- **Node.js:** v20.x or v22.x LTS with npm 10+
- **Git:** 2.40+ (`git version`)
- **Python:** 3.10+ (for test automation scripts)
- **C++ Build Tools:** Visual Studio 2022 Build Tools with "Desktop development with C++"

---

## 2. Quickstart Step-by-Step

### Step 1: Clone and Inspect Workspace
```powershell
git clone <repository_url> AetherIDE
cd AetherIDE
```

### Step 2: Run Development Environment Doctor
Use the built-in diagnostic tool to verify all required SDKs:
```powershell
cd rust
cargo run --bin aether-cli -- doctor
```

### Step 3: Compile Rust Native Core & Daemon
```powershell
cd rust
cargo build --workspace
```

### Step 4: Launch the Aether Native Daemon
```powershell
cd rust
cargo run --bin aether-daemon
```
The daemon will initialize SQLite WAL storage at `%LOCALAPPDATA%\AetherIDE\aether.db` and bind to:
- Windows Named Pipe: `\\.\pipe\aether-daemon-ipc`
- Local IPC loopback: `127.0.0.1:44888`

### Step 5: Build and Run Workbench Extension
In a separate terminal:
```powershell
cd extensions\aether-core
npm install
npm run compile
```

### Step 6: Launch Desktop Development Shell
```powershell
.\scripts\build_windows_release.ps1
.\dist\windows-x64\AetherIDE.cmd
```
