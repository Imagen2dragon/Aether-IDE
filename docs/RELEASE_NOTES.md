# AetherIDE — Release Notes

## v0.1.0-beta.1 (Sprint 07 Commercial Beta Release Candidate) — October 31, 2026

### Highlights:
- **Independent Security Hardening:**
  - Constant-time SHA-256 session token verification eliminating timing side-channels in authentication.
  - Strict filesystem path containment (`resolve_contained_path`) preventing directory traversal, Windows drive letter escapes, and device namespace squatting (`CON`, `PRN`, `AUX`, `NUL`).
  - Backend-enforced `RoleRegistry` disallowing client-side role forgery in collaborative CRDT editing.
- **Distributed CRDT Collaboration:**
  - Real-time multi-client document synchronization powered by Yrs (Yjs V1 compatible).
  - Minimal delta diff exchange protocol with sub-millisecond calculation latency.
  - Ephemeral collaborator cursor and selection presence tracking with sliding-window heartbeat eviction.
- **Distributed AI Agent Swarms:**
  - 14 autonomous engineering personas coordinated via `SwarmCoordinator` with Kahn's cycle prevention.
  - Isolated Git worktree sandboxing (`.aether/worktrees/`) preventing contamination of the main branch.
  - Native Three-Way Merge Engine supporting automatic clean merges and interactive conflict resolution (`AcceptOurs`, `AcceptTheirs`, `AcceptBoth`).
- **Production Operations & Online Backups:**
  - Non-blocking online SQLite WAL backup via native `sqlite3_backup` C-API (`rusqlite::backup::Backup`).
  - Safe point-in-time recovery runbooks with zero database lockups or torn pages.
- **Staged Auto-Updater & Rollback Protection:**
  - 5-stage secure updater verifying semver anti-downgrade rules, SHA-256 checksums, and Ed25519 digital signatures.
  - Atomic payload deployment with automatic `.rollback_bak` recovery upon installation failure.
- **Cross-Platform Release Packaging:**
  - Windows 10/11: Inno Setup executable (`.exe`) and signed MSIX package (`.msix`).
  - Linux: Portable AppImage and Debian package (`.deb`).
  - macOS: Universal `.app` bundle and DMG disk image.
- **Storage Optimization:**
  - Externalized compilation cache to `%LOCALAPPDATA%\AetherIDE\build-cache\`, maintaining 0.449 GB in-tree repo size and zero OneDrive target bloat.

---

## v0.1.0-alpha

## Highlights
- **Rust-First Core Engine:** 21 modular Rust crates delivering ultra-fast, memory-safe systems engineering, AST symbol extraction, and process sandboxing.
- **Universal AI Model Gateway:** Native provider integrations for Anthropic Claude, OpenAI, Google Gemini, xAI Grok, DeepSeek, and local Ollama.
- **Autonomous Multi-Agent Team:** 14 preconfigured engineering personas operating in Observe, Assist, and Autopilot modes.
- **Mobile App Studio:** Complete studio for scaffolding React Native + Expo + TypeScript apps, visual device canvas, and EAS Cloud build integration.
- **Companion Ecosystem:** SwiftUI companion application for iPhone and iPad with remote terminal and agent monitoring.
- **Living Project Brain:** Real-time AST dependency graph and Change Impact Radar for blast-radius calculation.
- **Proof of Work Ledger:** Cryptographically verifiable SQLite journal logging all diffs, execution logs, and test evidence.
