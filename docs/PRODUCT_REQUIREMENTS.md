# AetherIDE — Product Requirements Document (PRD)

## 1. Scope & Objective
This document outlines the functional and non-functional requirements for **AetherIDE**, a commercial-grade, Rust-first AI-native software development platform and autonomous engineering ecosystem.

---

## 2. System Architecture Requirements

### 2.1 Rust Core Subsystems
- **R-SYS-01:** High-concurrency daemon (`aether-daemon`) built in Rust with Tokio async runtime.
- **R-SYS-02:** Local IPC communication via Windows Named Pipes (`\\.\pipe\aether-daemon-ipc`) and local WebSocket/HTTP on loopback with cryptographic handshake tokens.
- **R-SYS-03:** Local SQLite persistence with WAL mode and SQLite FTS5 for full-text code search, symbol index, memory storage, and ledger logging.
- **R-SYS-04:** Native process manager with platform isolation (Windows Job Objects on Windows, cgroups on Linux), CPU/memory limits, cancellation tokens, and real-time stdout/stderr streaming.
- **R-SYS-05:** Native file watcher built on Rust `notify` crate with debouncing, ignore patterns (`.gitignore`, `.aetherignore`), and incremental AST cache updates.

### 2.2 Editor Workbench
- **R-EDT-01:** Code - OSS integration preserving standard editor APIs, LSP, DAP, themes, keyboard shortcuts, and Open VSX extension compatibility.
- **R-EDT-02:** First-class language intelligence for Rust, C/C++, TypeScript, JavaScript, Python, Go, Java, C#, Swift, Kotlin, Dart, SQL, HTML, CSS, JSON, and YAML.
- **R-EDT-03:** Tree-sitter incremental syntax parsing in Rust for resilient, high-speed structural navigation and AST manipulation.
- **R-EDT-04:** Built-in multi-shell terminal supporting PowerShell, CMD, Git Bash, and WSL with AI inline command generation and streaming execution.
- **R-EDT-05:** Native Git orchestration supporting commit, branch, merge, rebase, cherry-pick, stash, conflict resolution, blame, and Git worktrees for parallel agent sandboxing.

---

## 3. Universal AI Model Gateway

### 3.1 Supported Providers
- **R-MDL-01:** Native adapters for Anthropic Claude (Claude 3.5 Sonnet, Claude 3 Opus, Haiku), OpenAI (GPT-4o, o1, o3-mini), Google Gemini (Gemini 1.5 Pro, 2.0 Flash, 3.8 Flash), xAI Grok, DeepSeek (V3, R1), OpenRouter, Ollama, LM Studio, and generic OpenAI-compatible local/remote endpoints.
- **R-MDL-02:** Real-time token streaming with server-sent events (SSE) parser and backpressure handling.
- **R-MDL-03:** Dynamic model capability discovery (tool calling, vision, context length limits, reasoning budget).
- **R-MDL-04:** Intelligent Model Router directing queries based on task type:
  - Fast completions -> low-latency models (Haiku, Gemini Flash, Ollama).
  - Deep architecture & refactoring -> reasoning models (o1, R1, Claude Sonnet).
  - Visual inspection -> vision models (GPT-4o, Gemini Pro).
- **R-MDL-05:** Model Arena allowing side-by-side comparison of multi-model code generation, test plans, and architectural proposals with scoring.

---

## 4. Autonomous AI Engineering Platform

### 4.1 Multi-Agent Engine
- **R-AGT-01:** 14 preconfigured engineering roles: Architect, Frontend, Backend, Database, Mobile, Systems (Rust/C++), Debugging, Testing, Security, Code Reviewer, DevOps, Research, Documentation, Performance.
- **R-AGT-02:** 3 Autonomy Levels:
  1. *Observe:* Read-only inspection and proposal generation without filesystem writes.
  2. *Assist:* Staged file edits and command execution requiring user click approval.
  3. *Autopilot:* Autonomous multi-step execution within pre-authorized permission bounds.
- **R-AGT-03:** Parallel execution using isolated Git worktrees (`.aether/worktrees/agent-<id>`) preventing concurrent file conflicts.
- **R-AGT-04:** Proof of Work Ledger tracking prompt -> plan -> files inspected -> diff -> execution logs -> test output -> verification status.

### 4.2 Living Project Brain & Impact Radar
- **R-BRN-01:** Incremental AST & symbol graph mapping modules, functions, classes, API routes, database schemas, and inter-file imports.
- **R-BRN-02:** Change Impact Radar calculating upstream and downstream blast radius for proposed edits before changes are applied.
- **R-BRN-03:** Bug Reproduction Lab capturing error traces, console logs, and environment state into isolated reproducible regression test suites.
- **R-BRN-04:** Development Environment Doctor diagnosing missing toolchains (Rust, Node, Python, Android SDK, Git, MSVC) and providing safe 1-click remediation scripts.

---

## 5. Mobile App Studio

### 5.1 Workspace & Frameworks
- **R-MOB-01:** Dedicated Mobile App navigation pane with Application Manager, Project Creator, AI App Builder, Visual Screen Designer, and Device Previews.
- **R-MOB-02:** Primary framework: React Native + Expo + TypeScript with auto-configured navigation (Expo Router), state management, and modern component styling.
- **R-MOB-03:** Secondary framework templates: Flutter/Dart, Native Android (Kotlin), Native iOS (SwiftUI).
- **R-MOB-04:** Device capabilities module: Camera, Biometrics, SecureStore, Notifications, Geolocation, Network state.
- **R-MOB-05:** Visual Screen Designer providing drag-and-drop mobile component layout, responsive device frame previews (iPhone 16, Pixel 9, iPad), and bi-directional TypeScript code synchronization.
- **R-MOB-06:** Cloud Build Center integrating EAS (Expo Application Services) with support for Android APK/AAB build generation and iOS IPA build orchestration.

---

## 6. iOS/iPadOS Companion Client
- **R-IOS-01:** Native SwiftUI application for iPhone and iPad.
- **R-IOS-02:** Reusable Rust core integrated via UniFFI / C-FFI for local crypto, token verification, and fast protocol parsing.
- **R-IOS-03:** Secure pairing with Windows Desktop IDE via QR code handshake, ECDSA-signed challenge, and TLS/WSS relay.
- **R-IOS-04:** Remote agent monitoring, diff approval, terminal command triggering, and EAS build status tracking from mobile.

---

## 7. Web Workspace & Collaboration
- **R-WEB-01:** Web IDE client connecting to local or cloud-hosted Aether daemon.
- **R-WEB-02:** Multi-user session sharing, live cursor presence, shared agent execution, and code review threads.

---

## 8. Security, Privacy & Reliability
- **R-SEC-01:** Permission Engine classifying operations into Read, Write, Execute, Network, Destructive, and Credential-Sensitive tiers.
- **R-SEC-02:** Native OS credential storage integration (Windows Credential Manager / DPAPI) ensuring API keys are never stored in plaintext.
- **R-SEC-03:** Automatic secret redaction filtering API keys, bearer tokens, and private keys from logs, LLM prompts, and UI diffs.
- **R-SEC-04:** Offline-First Mode allowing complete editing, local AST indexing, Git, terminal, and local Ollama inference without internet access.
