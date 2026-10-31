# AetherIDE — Sprint 07 Commercial Beta Test Plan

**Project:** AetherIDE  
**Sprint:** 07 — Commercial Beta Preparation  
**Scope:** Full-System Verification, Security Boundary Fuzzing, Multi-Client Collaboration, Swarm Execution, Online Backup, and Packaging  

---

## 1. Test Matrix Overview

The Sprint 07 verification suite consists of 6 structured testing gates:

1. **Gate A: Security & Authentication Hardening:**
   - Constant-time session token validation.
   - Unauthorized access rejection on all Named Pipe and loopback TCP endpoints.
   - Path traversal rejection across canonicalization boundaries (`../`, encoded dots, symlinks).
   - Privilege self-elevation rejection in multi-user collaboration payloads.

2. **Gate B: Two-Endpoint Multi-Client Collaboration:**
   - Multi-client simulation with two distinct endpoints connecting to `workspace-sync`.
   - Real-time text insertion, deletion, and format mutation convergence under network jitter.
   - Remote cursor awareness tracking and ephemeral presence updates.
   - Offline edit buffering and automatic reconnection reconciliation.

3. **Gate C: Swarm Multi-Agent Execution & Three-Way Merging:**
   - Task DAG construction with Kahn's cycle detection.
   - Multi-persona concurrent execution within isolated Git worktrees.
   - Three-way merge conflict detection and resolution application.
   - Agent task crash and timeout recovery.

4. **Gate D: Database Online Backup & Crash Resilience:**
   - SQLite online backup API (`sqlite3_backup`) verification without WAL locking or process interruption.
   - Backup restoration and data integrity verification.
   - Daemon unexpected termination and state recovery.

5. **Gate E: Release Packaging & Staged Update Verification:**
   - Inno Setup installer compilation and installation verification.
   - MSIX package layout and manifest verification.
   - Linux AppImage / Debian package verification.
   - macOS .app bundle and DMG script verification.
   - Auto-updater SHA256 integrity and semver rollback guard verification.

6. **Gate F: Storage & Performance Regression Monitoring:**
   - Live repository size verification (< 6.00 GB target).
   - Zero in-tree `rust/target` bloat enforcement in OneDrive.
   - Desktop startup and IPC latency benchmarking.
