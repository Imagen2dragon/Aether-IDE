# AETHERIDE — COMMERCIAL BETA COMPREHENSIVE TEST PLAN & VALIDATION RUNBOOK
**Document Version:** 1.0.0  
**Status:** Certified / Approved for External Beta Distribution  
**Author:** QA Director & Beta Release Principal  
**Sprint:** Sprint 07 Commercial Beta  
**Repository:** `AetherIDE`

---

## 1. Executive Summary

This document establishes the official test plan, user journey test cases, and quality acceptance gates for the external Commercial Beta distribution of AetherIDE.

The testing methodology covers both automated CI/CD test gates and manual exploratory journeys designed to validate system stability, developer ergonomics, security isolation, and cross-platform synchronization under real-world engineering workflows.

---

## 2. Beta Target Personas & Hardware Profiles

### 2.1 Developer Personas
1. **Rust Systems Engineer:** Compiles complex multi-crate Rust workspaces, uses rust-analyzer LSP, runs cargo tests via integrated ConPTY terminal, profiles memory.
2. **Full-Stack Web/TypeScript Engineer:** Edits large React/TypeScript projects, tests multi-file refactoring in Monaco, manages background dev servers.
3. **Mobile & Systems Architect:** Synchronizes workspace with iOS companion, uses autonomous swarms to design architecture and generate API bindings.

### 2.2 Tested Hardware Matrix
- **Tier 1 (Flagship Windows):** Windows 11 Pro x64, 16-Core CPU, 64 GB RAM, NVMe PCIe Gen4 SSD.
- **Tier 2 (Mainstream Windows):** Windows 11 Home x64, 8-Core CPU, 16 GB RAM, NVMe SSD.
- **Tier 3 (Budget / Legacy Windows):** Windows 10 x64 (Build 19045), 4-Core CPU, 8 GB RAM, SATA SSD (tested under Low Resource Mode).
- **Tier 4 (Linux & macOS Reference):** Ubuntu 22.04 LTS x64 (AppImage/deb), macOS Sequoia (Apple Silicon ARM64).

---

## 3. Core Beta User Journey Test Cases

### TC-01: First-Run Onboarding & Zero-Config Setup
- **Objective:** Verify frictionless installation and self-healing environment diagnostics.
- **Steps:**
  1. Run `AetherIDE-Setup-x64.exe` (per-user installation).
  2. Launch AetherIDE from desktop shortcut or start menu.
  3. Verify Welcome screen opens with clean styling, no missing asset icons, and zero console errors.
  4. Trigger `EnvironmentDoctor::diagnose()` to scan local toolchains (`rustc`, `cargo`, `git`, `node`).
  5. Enter API credentials for AI providers (OpenAI, Anthropic, Gemini, or local Ollama).
- **Pass Criteria:** Onboarding completes in $< 60\text{ seconds}$; keys stored safely via `security-core` without disk exposure.

---

### TC-02: Monaco Editor Core Editing & LSP Intelligence
- **Objective:** Validate Monaco editor responsiveness, language server diagnostics, and code navigation.
- **Steps:**
  1. Open a Rust workspace folder (`File -> Open Folder`).
  2. Verify project file tree populates with correct file/folder icons.
  3. Open a Rust file (`src/main.rs`).
  4. Type code to trigger auto-completion, hover documentation, and syntax error squiggles via LSP Multiplexer.
  5. Execute `Go to Definition` and `Find References`.
- **Pass Criteria:** Keystroke latency $< 10\text{ ms}$; diagnostics populate within 500ms; zero editor freezes.

---

### TC-03: Windows ConPTY Terminal Studio
- **Objective:** Verify genuine native Windows pseudoconsole integration.
- **Steps:**
  1. Open integrated terminal panel (`Ctrl + ~`).
  2. Verify active shell is `powershell.exe` or `cmd.exe` hosted via ConPTY.
  3. Execute long-running build commands (`cargo build`, `npm run dev`).
  4. Test terminal resize events, ANSI 256-color and 24-bit TrueColor rendering, and `Ctrl+C` interrupt handling.
  5. Create multiple concurrent terminal tabs and close them cleanly.
- **Pass Criteria:** ConPTY runs in `ScopedJobObject` without orphan process leaks; clean ANSI output.

---

### TC-04: Distributed AI Agent Swarm Execution
- **Objective:** Verify multi-agent autonomous task planning, worktree isolation, and code integration.
- **Steps:**
  1. Open AI Command Center and submit a task: *"Design and implement an encrypted key-value cache"*.
  2. Observe Swarm Coordinator generate a task DAG (`architect` $\to$ `systems` $\to$ `qa`).
  3. Verify worktrees are created in `.aether/worktrees/`.
  4. Inspect generated diffs in the Three-Way Merge UI.
  5. Accept changes or resolve simulated conflicts.
- **Pass Criteria:** Swarm DAG executes without circular deadlocks; worktrees merge cleanly into main workspace.

---

### TC-05: Real-Time CRDT Collaboration & Device Pairing
- **Objective:** Validate multi-user editing, role-based authorization, and presence tracking.
- **Steps:**
  1. Start a collaborative session and generate a Pairing QR / URI.
  2. Connect a secondary endpoint (or simulated peer client).
  3. Edit the same file simultaneously from both endpoints.
  4. Observe real-time cursor positions and nametags in Monaco.
  5. Attempt an unauthorized write from a demoted `Viewer` peer.
- **Pass Criteria:** Both endpoints achieve identical deterministic content; unauthorized write rejected.

---

### TC-06: Disaster Recovery & Automated Staged Updates
- **Objective:** Validate online database backup and updater integrity.
- **Steps:**
  1. Trigger `DatabaseEngine::backup_online` while running background writes.
  2. Restore backup into an empty workspace and verify FTS5 symbol index.
  3. Stage a verified update payload (`v0.2.0`) and apply it.
  4. Simulate an update failure and verify automatic rollback recovery.
- **Pass Criteria:** Zero database corruption; updater reverts cleanly on simulated failure.

---

## 4. Beta Acceptance Gates

| Gate ID | Name | Requirement | Status |
|:---:|:---|:---|:---:|
| **G1** | Zero Fatal Crashes | Zero unhandled panics or segfaults during 4-hour continuous session | **PASS** |
| **G2** | Build Hygiene | Zero in-tree `rust/target` contamination, storage $< 6.00\text{ GB}$ | **PASS (0.449 GB)** |
| **G3** | Compiler Cleanliness | `cargo test --workspace` 100% pass, 0 clippy warnings, clean fmt | **PASS** |
| **G4** | Security Certification | Constant-time auth, containment verification, RBAC enforced | **PASS** |
| **G5** | Packaging Readiness | Windows Inno Setup & MSIX, Linux AppImage/deb, macOS DMG | **PASS** |

---

## 5. Conclusion
AetherIDE is comprehensively validated and approved for immediate distribution to external commercial beta testers.
