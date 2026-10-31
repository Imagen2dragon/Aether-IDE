# AetherIDE — Autonomous Agent Command Center Guide
## 14 Specialized Personas, Visual Persona Stepper and Human Authorization Guardrails

**Author:** AI Agent Architect & Security Auditor  
**Project:** AetherIDE  
**Sprint:** Sprint 05  

---

## 1. Mission & System Overview

AetherIDE incorporates 14 autonomous engineering personas designed to collaborate with human developers. To prevent uncontrolled file modifications, rogue commands, or unintended network access, Sprint 05 introduces the **Autonomous Agent Command Center** with strict human-in-the-loop authorization guardrails.

---

## 2. 14 Specialized Engineering Personas

The system features 14 specialized engineering personas registered in `agent-orchestrator`:
1. **Principal Software Architect:** System boundaries, interface design, structural governance.
2. **Rust Systems Engineer:** Low-level Win32 memory safety, ConPTY, async Tokio performance.
3. **Frontend & UI/UX Specialist:** Monaco Editor, Xterm.js styling, Cytoscape graph canvas.
4. **Mobile Engineering Lead:** React Native, Expo SDK 52, ADB emulator orchestration.
5. **Security & Cryptography Auditor:** ChaCha20-Poly1305, constant-time verification, allowlists.
6. **Database & Indexing Engineer:** SQLite WAL mode, FTS5 inverted index, schema migrations.
7. **DevOps & Infrastructure Lead:** Process supervision, Job Object sandboxing, Windows service host.
8. **QA & Verification Director:** Test suite integrity, regression matrices, boundary verification.
9. **Performance & Profiling Engineer:** Zero-copy streaming, sub-millisecond IPC latency, memory limits.
10. **API & Model Gateway Integrator:** Multi-provider LLM failover, rate limiting, token economics.
11. **Technical Documentation Engineer:** Architecture runbooks, API references, sprint reports.
12. **Release & Packaging Specialist:** Windows portable packaging, SHA256 integrity, release builds.
13. **Concurrency & Async Specialist:** Tokio channels, mutex contention elimination, deadlock prevention.
14. **Network & Sync Systems Architect:** Loopback TCP, UDP mDNS discovery, chunked AEAD transport.

---

## 3. Visual Persona Stepper

Every dispatched task moves through a structured, observable pipeline visualized by the Persona Stepper:

```
[1. Task Ingestion] -> [2. Architectural Plan] -> [3. Tool Execution] -> [4. Human Approval] -> [5. Proof of Work] -> [6. Verified Done]
        ✓                      ✓                        ●                      !                     ○                   ○
```

- **Stage 1 (Task Ingestion):** The agent decomposes the user's objective and constraints.
- **Stage 2 (Architectural Plan):** The agent formulates a step-by-step modification plan.
- **Stage 3 (Tool Execution):** Tool requests (e.g., reading files, running read-only diagnostics) execute within the security sandbox.
- **Stage 4 (Human Approval):** When a sensitive write or command is requested, execution halts and pulses amber awaiting human sign-off.
- **Stage 5 (Proof of Work):** Validates post-execution test results, diff evidence, and exit codes.
- **Stage 6 (Verified Done):** Confirms success and merges changes into working state.

---

## 4. Human-in-the-Loop Approval Center

Whenever an autonomous agent proposes a state mutation (such as writing a file or executing a build command):
1. **Approval Card Display:** Highlights the Task ID, Persona Name, Requested Action, and Target File.
2. **Unified Git Diff Viewer:** Displays precise line-by-line diffs (`+` additions in green, `-` deletions in red) so the engineer can review every proposed line before applying it.
3. **Test Evidence Review:** Displays the pre-validation test output and proof-of-work records.
4. **Decision Actions:**
   - **`Approve Step (Enter)`:** Emits `ApproveAgentStep` via IPC, granting authorization for the atomic write.
   - **`Reject Step (Esc)`:** Emits `RejectAgentStep` with user feedback; the agent replans its approach based on the human guidance.
   - **`Cancel Task`:** Emits `CancelAgentTask`, immediately terminating the agent task and aborting pending changes.

---

## 5. Security & Verification Guardrails

- **Zero Self-Approval:** Autonomous agents cannot approve their own steps. The IPC protocol enforces that approvals must originate from the verified user session bridge.
- **Observe Mode Enforcement:** In Observe/Supervised mode, write tools are completely disabled at the kernel/Rust level unless accompanied by a verified approval token.
- **Audit Logging:** Every approval, rejection, and cancellation is immutably recorded in the database ledger (`proof_of_work_ledger`).
