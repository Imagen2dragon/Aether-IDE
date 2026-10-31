# AetherIDE

> **Production-Grade, Rust-First AI-Native IDE and Autonomous Software Engineering Ecosystem**

[![Rust Version](https://img.shields.io/badge/rust-1.80%2B-blue.svg)](https://www.rust-lang.org)
[![Platform](https://img.shields.io/badge/platform-Windows%20%7C%20iOS%20%7C%20Web-brightgreen.svg)]()
[![License](https://img.shields.io/badge/license-MIT-purple.svg)]()

---

## 🌟 Executive Overview
**AetherIDE** is a unified, high-performance software engineering platform designed from the ground up to synthesize the best aspects of Visual Studio Code, Cursor, Claude Code, and modern autonomous coding systems into a single cohesive development environment.

Unlike simple UI wrappers or chatbot sidebars, AetherIDE is powered by a multithreaded native **Rust systems core** that executes heavy operations—incremental AST parsing, full-text symbol search, Git worktrees, terminal streaming, process isolation, and multi-agent coordination—at sub-millisecond speeds with zero memory corruption vulnerabilities.

---

## 🚀 Key Architectural Pillars

### 1. 🦀 Rust-First Core (`rust/`)
- **24 Modular Crates:** Including `application-core`, `ipc-protocol`, `filesystem-engine`, `database-engine`, `symbol-index`, `process-manager`, `model-gateway`, `agent-orchestrator`, `terminal-manager`, `workspace-sync`, and `diagnostics`.
- **Storage-Aware External Build Cache:** Compilation cache offloaded to `%LOCALAPPDATA%\AetherIDE\build-cache\` with zero OneDrive lockups, reclaiming 11.81 GB (net 2.49 GB footprint).
- **Cross-Platform Native Subsystems:** `TerminalBackend` trait supporting Windows ConPTY and Unix/macOS POSIX OpenPTY; `DesktopHost` trait supporting Win32 WebView2, Linux WebKitGTK, and macOS WKWebView.
- **Real-Time CRDT Collaboration:** Yrs (Yjs V1 compatible) deterministic synchronization with ephemeral cursor tracking and Ed25519 Role-Based Access Control.
- **Autonomous AI Swarms & Three-Way Merging:** DAG task scheduling with Kahn's cycle detection, isolated Git worktrees, and visual 3-way conflict resolution.
- **High-Speed IPC:** Windows Named Pipes (`\\.\pipe\aether-daemon-ipc`) and authenticated loopback TCP.
- **Embedded Storage:** SQLite with WAL mode, memory-mapped I/O, and FTS5 full-text code search.

### 2. 🤖 14-Persona Autonomous AI Engineering Team (`crates/agent-orchestrator`)
- **Specialized Roles:** Principal Architect, Frontend Developer, Backend Developer, Database Engineer, Mobile Developer, Rust/C++ Systems Engineer, Debugging Specialist, Testing Specialist, Security Reviewer, Code Reviewer, DevOps Engineer, Technical Researcher, Documentation Engineer, Performance Engineer.
- **3 Autonomy Modes:**
  - *Observe Mode:* Read-only analysis and proposal formulation.
  - *Assist Mode:* User approval required for all staged changes.
  - *Autopilot Mode:* Autonomous execution within sandboxed Git worktrees.
- **Proof of Work Ledger:** Verifiable cryptographic audit trail of every prompt, plan, diff, command output, and test pass/fail assertion.

### 3. 🧠 Living Project Brain & Change Impact Radar (`crates/project-brain`)
- Real-time dependency graph mapping modules, endpoints, schemas, and tests.
- Blast radius calculation warning developers of downstream breakage before edits are committed.

### 4. 📱 Mobile App Studio (`mobile-studio/` & `crates/mobile-build-manager`)
- Dedicated IDE workspace for building cross-platform mobile apps.
- Default stack: **React Native + Expo SDK 52 + TypeScript + Expo Router**.
- Visual Screen Designer with responsive device frames (iPhone 16 Pro, Google Pixel 9, iPad).
- Automated cloud builds via EAS generating Android APK/AAB and iOS IPA packages.

### 5. 📱 iOS & iPadOS Companion Client (`apps/ios-companion/`)
- Native SwiftUI companion application with shared Rust UniFFI native bridge.
- Secure QR-code pairing, remote agent control, change approval, and terminal streaming.

### 6. 🌐 Web Workspace & Collaboration (`apps/web-workspace/`)
- Browser-based cloud workspace connecting to local or remote Aether daemons.

---

## 📂 Repository Topology

```
ide/
├── apps/
│   ├── desktop/              # Windows desktop IDE application (Electron / Code - OSS integration)
│   ├── ios-companion/        # SwiftUI + Rust UniFFI native iOS companion
│   └── web-workspace/        # React + TypeScript Web IDE client
├── rust/                     # Complete Rust Workspace (21 Crates)
│   ├── Cargo.toml
│   ├── crates/
│   │   ├── application-core/
│   │   ├── ipc-protocol/
│   │   ├── security-core/
│   │   ├── permission-engine/
│   │   ├── database-engine/
│   │   ├── filesystem-engine/
│   │   ├── syntax-engine/
│   │   ├── symbol-index/
│   │   ├── project-indexer/
│   │   ├── process-manager/
│   │   ├── git-engine/
│   │   ├── terminal-manager/
│   │   ├── model-gateway/
│   │   ├── agent-runtime/
│   │   ├── agent-orchestrator/
│   │   ├── project-brain/
│   │   ├── checkpoint-engine/
│   │   ├── diagnostics/
│   │   └── mobile-build-manager/
│   ├── services/
│   │   └── aether-daemon/    # Native background service binary
│   └── cli/
│       └── aether-cli/       # Developer CLI binary
├── extensions/
│   └── aether-core/          # Core TypeScript IDE extension
├── scripts/                  # Automated build, test, and packaging scripts
├── docs/                     # Specifications, architecture, guides, audit records
└── IMPLEMENTATION_STATUS.md
```

---

## ⚡ Quickstart

### 1. Diagnose Development Environment
```powershell
cd rust
cargo run --bin aether-cli -- doctor
```

### 2. Build and Launch Aether Daemon
```powershell
cd rust
cargo run --bin aether-daemon
```

### 3. Test CLI Operations
```powershell
# Ping daemon
cargo run --bin aether-cli -- ping

# Index current codebase
cargo run --bin aether-cli -- index .

# Search symbol index
cargo run --bin aether-cli -- search ProjectIndexer

# Dispatch an autonomous agent
cargo run --bin aether-cli -- agent architect "Audit codebase structure"

# Scaffold a React Native + Expo mobile application
cargo run --bin aether-cli -- mobile create AetherStore
```

### 4. Build Windows Release Package
```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\build_windows_release.ps1
powershell -ExecutionPolicy Bypass -File .\scripts\package_portable_zip.ps1
```

---

## 📄 Documentation Directory
- [Product Vision](docs/PRODUCT_VISION.md)
- [Product Requirements (PRD)](docs/PRODUCT_REQUIREMENTS.md)
- [System Architecture](docs/ARCHITECTURE.md)
- [Implementation Plan](docs/IMPLEMENTATION_PLAN.md)
- [Security & Threat Model](docs/SECURITY.md)
- [Performance & Benchmarks](docs/PERFORMANCE_PLAN.md)
- [Acceptance Criteria](docs/ACCEPTANCE_CRITERIA.md)
- [Technical Risk Register](docs/TECHNICAL_RISK_REGISTER.md)
- [Development Setup](docs/DEVELOPMENT_SETUP.md)
- [Windows Build Guide](docs/WINDOWS_BUILD.md)
- [iOS Companion Guide](docs/IOS_BUILD.md)
- [Mobile App Studio Guide](docs/MOBILE_APP_GUIDE.md)
- [AI Model Gateway Guide](docs/AI_MODEL_GUIDE.md)
- [Autonomous Agents Guide](docs/AGENT_GUIDE.md)
- [Model Context Protocol (MCP)](docs/MCP_GUIDE.md)
- [Extension Development](docs/EXTENSION_DEVELOPMENT.md)
- [Deployment Guide](docs/DEPLOYMENT.md)
- [Troubleshooting](docs/TROUBLESHOOTING.md)
- [Release Notes](docs/RELEASE_NOTES.md)
- [Known Limitations](docs/KNOWN_LIMITATIONS.md)

---

## 🛡️ License
Licensed under the [MIT License](LICENSE).
