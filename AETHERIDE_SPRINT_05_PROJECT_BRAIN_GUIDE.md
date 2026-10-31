# AetherIDE — Living Project Brain & Impact Radar Guide
## Semantic Architecture Graph, AST Blast Radius Calculation and Layer 2 SQLite Project Memory

**Author:** AI Infrastructure Architect & Principal Software Architect  
**Project:** AetherIDE  
**Sprint:** Sprint 05  

---

## 1. System Overview

The Living Project Brain transforms static source code into a living, queryable semantic graph. It provides engineers with immediate structural awareness of their codebase, automates impact analysis before making code changes, and persists architectural decisions across development sessions.

The subsystem consists of three complementary components:
1. **Interactive Architecture Graph:** Powered by locally bundled Cytoscape.js (v3.31.0).
2. **Change Impact Radar:** Calculates direct and transitive blast radius, risk scoring, and impacted test suites.
3. **Layer 2 Living Project Memory:** Persistent SQLite database tables for recording design rationale, conventions, and architectural constraints.

---

## 2. Interactive Semantic Graph (Cytoscape.js)

The graph visualizes the entire AetherIDE workspace:
- **Crate Nodes (Purple):** Top-level Cargo workspace members (`aether-daemon`, `terminal-manager`, `project-brain`, etc.).
- **Module Nodes (Cyan):** Source files and submodules containing exported types and logic (`conpty.rs`, `crypto.rs`, `main.rs`).
- **Test Nodes (Amber):** Unit tests and integration test suites mapped directly to the modules they verify.
- **Dependency Edges (Lines with Arrows):** Directed relationships indicating imports, function invocations, and crate dependencies.

### Graph Interaction & Monaco Navigation:
- **Layout Algorithms:** Switchable at runtime between `Concentric`, `Breadthfirst`, `Force Cose`, and `Circle`.
- **Node Selection Inspector:** Clicking any node reveals its metadata, symbol definitions, inbound and outbound dependency counts.
- **Direct Monaco Jump:** Clicking "Open in Monaco (Ln X)" smoothly transitions the workbench into Code Studio, opens the file in Monaco Editor, and calls `editor.revealLineInCenter(line)` with the cursor positioned at the exact symbol declaration.

---

## 3. Change Impact Radar & Blast Radius

Before modifying a critical file (such as `conpty.rs` or `lib.rs`), an engineer can select or input the target path in the Impact Radar panel and click **Calculate Blast Radius**.

The backend AST scanner and dependency graph engine analyze the project graph and compute:
1. **Risk Score Badge:**
   - **LOW (< 0.35):** Changes affect isolated leaf crates or test files. Minimal regression risk.
   - **MEDIUM (0.35 - 0.70):** Changes affect core utilities with downstream consumers.
   - **HIGH (> 0.70):** Changes affect foundational system crates (such as `ipc-protocol`, `security-core`, `application-core`) with widespread transitive blast radius.
2. **Direct Dependents:** Immediate crates or files that import or call the changed target.
3. **Transitive Blast Radius:** Complete transitive closure of downstream components requiring re-compilation or validation.
4. **Impacted Test Suites:** Specific unit tests, integration tests, and doc-tests that must be re-run to verify the change.

---

## 4. Layer 2 Living Project Memory (SQLite WAL)

To prevent architectural drift and preserve institutional knowledge across sessions, AetherIDE persists architectural decision records directly in SQLite:
- **Database Table:** `project_memory_notes` (with columns: `id`, `title`, `category`, `content`, `tags`, `created_at`).
- **Categories:** `Architecture`, `Security`, `Convention`, `TechDebt`.
- **IPC Actions:**
  - `SaveProjectMemoryNote`: Inserts or updates an architectural decision note.
  - `GetProjectMemoryNotes`: Retrieves chronological notes for display in the Brain sidebar.
  - `DeleteProjectMemoryNote`: Removes obsolete notes.

This ensures that critical security rules (such as SEC-01 constant-time token comparison) and architectural invariants remain documented and discoverable by both human engineers and autonomous AI agents.
