# AetherIDE — Sprint 06 Distributed AI Agent Swarms Guide
## Multi-Persona Task Scheduling, Worktree Isolation and Three-Way Merge

---

## 1. Multi-Persona Autonomous Swarms

AetherIDE coordinates 14 specialized autonomous engineering personas:
- `architect`: Principal Systems & Modular Boundary Architect
- `systems`: Rust & Native Systems Engineer
- `frontend`: TypeScript / React / Monaco Workbench Specialist
- `backend`: High-Concurrency API & Microservices Developer
- `database`: Relational WAL & SQL Schema Architect
- `mobile`: React Native / Expo & SwiftUI Companion Architect
- `security`: Cryptographic Auditor & Vulnerability Analyst
- `qa`: Test Infrastructure & Continuous Integration Lead
- `release`: Packaging, Installers & Distribution Engineer

---

## 2. Directed Acyclic Graph (DAG) Task Scheduling

The `SwarmTaskGraph` coordinates multi-stage agent workflows:
- Tasks declare explicit predecessor dependencies (`dependencies: Vec<String>`).
- Kahn's algorithm performs cycle detection on every insertion, guaranteeing acyclic topological sorting.
- `get_ready_tasks()` identifies all unblocked tasks whose prerequisites are complete, allowing parallel dispatch.

---

## 3. Worktree Isolation & Three-Way Merging

- **Worktree Allocation:**
  Each parallel persona is assigned an isolated worktree directory:
  `.aether/worktrees/agent-<persona>-<task_id>`
  This guarantees that concurrent agent personas never write simultaneously to the user's primary working tree.

- **Three-Way Merge Engine (`ThreeWayMergeEngine`):**
  When an agent submits code, the engine compares:
  1. `Base`: Common ancestor commit.
  2. `Ours`: User's current workspace state.
  3. `Theirs`: Agent worktree state.
  Clean modifications are integrated automatically. Conflicting modifications generate interactive visual conflict reports with choice of `AcceptOurs`, `AcceptTheirs`, `AcceptBoth`, or `Custom`.
