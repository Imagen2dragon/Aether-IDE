# AetherIDE Sprint 14 — Controlled Beta Pilot Feedback & Triage Plan

## 1. Objectives & Pilot Scope

The AetherIDE Controlled Developer Beta Pilot is designed to evaluate real-world product reliability, developer ergonomics, local AI inference quality, and platform stability across diverse hardware environments.

**Key Evaluation Areas:**
1. **Workbench Reliability:** Editor performance, syntax highlighting, multi-file navigation, and terminal responsiveness.
2. **AI Provider Quality:** Latency, tool-calling accuracy, streaming stability, and local offline inference with Ollama and LM Studio.
3. **Agent Orchestration:** Multi-agent collaboration, plan formulation, capability-sandboxed code edits, and checkpoint rollback safety.
4. **Security & Stability:** Zero crashes, zero credential leaks, and strict filesystem containment across complex codebases.

---

## 2. Feedback Channels & Protocols

Pilot developers have three distinct channels for communicating observations and defects:

### Channel A: GitHub Issues (Public or Private Pilot Repository)
For bug reports, unexpected crashes, UI glitches, or feature enhancements:
- **Repository Issues URL:** `https://github.com/Imagen2dragon/Aether-IDE/issues`
- **Issue Templates:** Dedicated GitHub Issue templates for `Bug Report`, `AI Provider Performance`, and `Feature Request`.

### Channel B: Anonymized Diagnostic Bundles
When encountering an internal daemon error, indexer crash, or communication failure:
```bash
aether-cli doctor --export-diagnostics
```
- Generates a timestamped zip bundle: `aether-diagnostics-<timestamp>.zip`.
- All credentials, file paths containing usernames, source code contents, and prompt texts are strictly redacted before export.
- Developers can attach this bundle directly to GitHub issues.

### Channel C: Weekly Structured Pilot Survey
At the end of each pilot week, participants submit a short 5-minute structured evaluation:
- System configuration (OS, CPU, GPU, RAM)
- Primary AI provider used (Local Ollama vs. Cloud API)
- Stability rating (1 to 5)
- Editor responsiveness rating (1 to 5)
- Qualitative highlights and blocker reports

---

## 3. Defect Classification & Triage SLA

Issues reported during the beta pilot are triaged according to the following severity matrix:

| Severity | Description | Examples | Target Triage SLA | Target Resolution |
| :--- | :--- | :--- | :--- | :--- |
| **P0 — Critical Blocker** | Crash on startup, data loss, security vulnerability, credential leak, sandbox escape | Daemon fails to bind, file corruption on save, DPAPI failure | **< 4 Hours** | Hotfix within 24 hours (`v0.1.0-beta.2.1`) |
| **P1 — High Severity** | Core feature non-functional, indexing loop, AI stream disconnect, terminal lockup | Local Ollama streaming failure, PTY freezing under Windows ConPTY | **< 12 Hours** | Scheduled patch within 48 hours |
| **P2 — Medium Severity** | UI rendering glitch, non-critical error log, syntax highlight imperfection | Monaco minimap artifact, non-blocking warning in diagnostic log | **< 24 Hours** | Addressed in `v0.1.0-beta.3` |
| **P3 — Low / Suggestion** | Cosmetic improvement, workflow ergonomics suggestion, documentation clarity | Keyboard shortcut customization, theme color adjustments | **< 48 Hours** | Product backlog triage |

---

## 4. Privacy & Diagnostic Sanitization Safeguards

Pilot developers must have full confidence that their intellectual property and proprietary source code remain private:

1. **Zero Raw Code Collection:** Diagnostic bundles inspect process states, memory allocation, and error stack traces. They **never** collect, transmit, or cache code snippets or file contents.
2. **Path Sanitization:** Usernames and home directory paths are replaced with generic placeholders (e.g., `C:\Users\<REDACTED>\...` or `/home/<REDACTED>/...`).
3. **Secret Redaction:** Any string resembling an API key (`sk-ant-...`, `sk-proj-...`, `AIzaSy...`) is replaced with `[REDACTED_SECRET]`.
4. **Explicit Consent:** Diagnostic bundles are never automatically transmitted. They are saved locally on disk and can only be shared if the user manually uploads them.

---

## 5. Pilot Exit Criteria

The controlled developer pilot concludes when the following criteria are satisfied:

- **Stability:** Minimum 50 hours of active editing across pilot group with zero unhandled daemon panics or crashes.
- **Security:** Zero reported sandbox escape attempts or credential vault compromise incidents.
- **Provider Reliability:** > 99% successful completion rate for local model code completions and agent tool calls.
- **P0 / P1 Debt:** Zero unresolved P0 or P1 defects in the issue tracker.

Upon meeting these criteria, AetherIDE will proceed to `v0.1.0-beta.3` and subsequent public release candidate certification.
