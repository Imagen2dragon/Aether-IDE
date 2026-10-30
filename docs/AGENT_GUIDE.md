# AetherIDE — Autonomous Agent Engineering Guide

## 1. The 14 Specialized Personas
AetherIDE provides an autonomous engineering team with specialized system instructions:
1. **Principal Software Architect:** High-level system design, module boundaries, scalability.
2. **Frontend Developer:** TypeScript, React, responsive CSS, component hierarchies.
3. **Backend Developer:** High-performance APIs, database queries, error handling, microservices.
4. **Database Engineer:** Schema design, SQL optimization, migrations, foreign key integrity.
5. **Mobile Developer:** React Native + Expo, Flutter, iOS SwiftUI, Android Compose.
6. **Rust/C++ Systems Engineer:** Low-level safety, zero-cost abstractions, memory management, FFI.
7. **Debugging Specialist:** Reproduction labs, root-cause isolation, surgical bug fixes.
8. **Testing Specialist:** Unit tests, property tests, integration suites, regression benchmarks.
9. **Security Reviewer:** Threat modeling, secret scanning, injection mitigation, permission auditing.
10. **Code Reviewer:** PR evaluation, idiomatic style, maintainability, architectural adherence.
11. **DevOps Engineer:** CI/CD workflows, Docker containerization, build orchestration.
12. **Technical Researcher:** Library benchmarking, RFC synthesis, official documentation search.
13. **Documentation Engineer:** Architectural docs, API specs, diagrams, tutorials.
14. **Performance Engineer:** Profiling, memory RSS tracking, cache optimization, hot loop analysis.

---

## 2. Autonomy Levels
- **Observe Mode:** Agent inspects code and produces proposals; filesystem writes are strictly disallowed.
- **Assist Mode (Default):** Agent drafts diffs and stages commands; each destructive or write action requires a user confirmation click.
- **Autopilot Mode:** Agent autonomously executes a planned sequence within whitelisted tool and compilation bounds; destructive operations (P4/P5) still trigger prompts.

---

## 3. Proof of Work Ledger
Every action taken by an agent is immutably recorded in the local SQLite journal:
- Exact user instruction hash
- Step-by-step reasoning plan
- Tool calls with sanitized inputs/outputs
- Unified file diffs
- Execution exit codes and test runner outputs
- Verification state: `Verified`, `Failed`, or `Requires Approval`
