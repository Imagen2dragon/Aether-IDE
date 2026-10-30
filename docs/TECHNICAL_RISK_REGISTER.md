# AetherIDE — Technical Risk Register & Mitigation Strategy

## 1. Overview
This register tracks technical risks, concurrency pitfalls, cross-platform constraints, and architectural mitigations throughout the development of AetherIDE.

---

## 2. Risk Matrix

| Risk ID | Risk Category | Description | Severity | Probability | Mitigation Strategy |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **TR-01** | Systems / IPC | Windows Named Pipe deadlocks or high latency under sustained high-throughput streaming | High | Low | Use asynchronous Tokio named pipe streams with bounded chunk buffers (64KB max packet) and explicit read/write timeouts. |
| **TR-02** | Concurrency | Agents executing parallel modifications on overlapping files causing Git merge conflicts | High | Med | Enforce strict Git worktree isolation per task; calculate Change Impact Radar overlapping file locks before granting agent parallel execution. |
| **TR-03** | Security | Malicious prompt injection in untrusted repositories commanding agent to run destructive code | Critical | Med | System prompt fencing; strict binary whitelisting for process executions; P4 destructive actions require mandatory user interaction. |
| **TR-04** | Performance | Large repositories (100K+ files) exhausting memory during Tree-sitter AST parsing | High | Low | Incremental parsing with lazy loading; memory-mapped file reads (`memmap2`); bounded worker threadpool with RSS memory monitor. |
| **TR-05** | Storage | SQLite database corruption during unexpected process termination or OS power loss | Med | Low | Enforce SQLite WAL (Write-Ahead Logging) mode, `PRAGMA synchronous = NORMAL`, and automated integrity check on startup. |
| **TR-06** | Cloud / Mobile | Building iOS IPAs on Windows without local macOS/Xcode environment | High | High (by design) | Architecture relies on EAS (Expo Application Services) and remote cloud build runners for iOS IPA compilation; Windows orchestrates via authenticated REST APIs. |
| **TR-07** | UI / Node.js | Memory bloat and unresponsive UI when handling huge terminal or indexing streams in Electron | Med | Med | Decouple streaming in Rust daemon; batch UI update notifications via IPC with 30fps throttle; virtualize terminal/DOM lists. |
| **TR-08** | Secret Exposure | API keys leaked into agent logs or test outputs | Critical | Low | Rust `security-core` secret scanner filters all outgoing payloads, replacing identified tokens with secure SHA256 hashes. |
