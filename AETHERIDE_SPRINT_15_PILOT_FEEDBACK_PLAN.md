# AetherIDE Sprint 15 — Controlled External Developer Pilot Feedback & Triage Plan

## 1. Pilot Scope & Governance

The AetherIDE Controlled External Developer Pilot is targeted at a focused cohort of 3 to 5 independent developers across Windows 10/11 x64, Linux x64, and macOS Apple Silicon.

**Governance Policy:** In accordance with Sprint 15 engineering rules, **all pilot invitations and cohort onboarding materials remain staged locally and will not be issued or transmitted without explicit user authorization.**

---

## 2. Participant Target Profile

1. **Independent Perspective:** Developers who have not contributed to or reviewed the AetherIDE source repository.
2. **Platform Balance:**
   - 1–2 Windows x64 developers (Visual Studio / VS Code users).
   - 1–2 Linux x64 developers (Ubuntu / Fedora / Arch workstations).
   - 1 macOS Apple Silicon developer (M1/M2/M3 MacBook or Mac Studio).
3. **Diverse Workloads:** Software engineering projects spanning Rust, TypeScript, Python, and C/C++.

---

## 3. Evaluation Scenarios & Feedback Metrics

Pilot participants will be requested to evaluate twelve core workflow dimensions:

| Scenario Code | Evaluation Area | Success Threshold |
| :--- | :--- | :--- |
| **SC-01** | Download & Checksum Verification | Exact SHA-256 match without friction |
| **SC-02** | Extraction & Clean First Launch | Workbench window renders within < 1,000 ms |
| **SC-03** | Workspace Loading | Project tree and AST symbols populated |
| **SC-04** | Monaco Editor Responsiveness | Typing, syntax coloring, split-pane editing |
| **SC-05** | LSP Integration | Autocompletion, hover, and diagnostics |
| **SC-06** | Terminal Execution | Shell commands execute without PTY hang |
| **SC-07** | Git SCM Integration | Staging, diff inspection, and atomic commit |
| **SC-08** | Offline Local Model (Ollama/LM Studio)| Seamless detection and local inference |
| **SC-09** | Cloud Model (if configured) | Streaming completion and rate-limit handling |
| **SC-10** | Autonomous Agent Pair Programming | Planner formulation, Coder execution, diff review |
| **SC-11** | Checkpoint Safety & Rollback | Safe rollback of unwanted changes |
| **SC-12** | Overall Stability & Memory | Zero daemon crashes or runaway RSS |

---

## 4. Defect Classification & SLA Matrix

| Severity Tier | Definition | Response SLA | Target Resolution |
| :--- | :--- | :--- | :--- |
| **P0 — Critical Blocker** | Crash on launch, data corruption, security vulnerability, credential leak | **< 4 Hours** | Patch within 24 hours (`v0.1.0-beta.2.1`) |
| **P1 — High Severity** | Core subsystem failure (LSP freeze, terminal failure, model stream disconnect) | **< 12 Hours** | Resolution within 48 hours |
| **P2 — Medium Severity** | UI artifact, non-critical warning log, minor formatting discrepancy | **< 24 Hours** | Addressed in `v0.1.0-beta.3` |
| **P3 — Low / Suggestion** | Ergonomic tweak, keyboard shortcut suggestion, theme preference | **< 48 Hours** | Product backlog prioritization |

---

## 5. Privacy & Diagnostic Collection Protocols

- Diagnostic collection is completely voluntary and user-triggered (`aether-cli doctor --export-diagnostics`).
- Diagnostics are saved locally on disk as an inspectable zip archive.
- User source code, prompts, credentials, and private file paths are strictly redacted prior to bundle creation.
