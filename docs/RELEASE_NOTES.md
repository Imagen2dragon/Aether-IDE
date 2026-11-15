# AetherIDE — Release Notes

## v0.1.0-beta.3 — Expanded Obsidian Pro Workbench & Reliability Update (November 13, 2026)

### Highlights:
- **Expanded Obsidian Pro Workbench Experience:**
  - Full-featured Obsidian Pro desktop developer workbench with dark high-contrast styling, responsive split editor panes, file explorer, integrated ConPTY/PTY terminal panels, and command palette.
  - Multi-tab Monaco editor with syntax highlighting, AST navigation, symbol definitions, and live output stream tabs.
  - AI Assistant side panel with autonomous multi-persona workflows, inline change previews, and diff acceptance controls.
- **Filesystem Engine Reliability & Continuity Hardening:**
  - Strict handle-bound directory pinning and atomic publication preventing concurrent namespace substitution.
  - Fail-closed cleanup paths: unpinned pathname deletions eliminated; ownership continuity established through retained OS handles before destructive actions.
  - Windows NTFS access-control hardening: verified private owner SID checks (`TokenUser`, authorized Administrators, SYSTEM) and comprehensive dangerous-DACL inspection rejecting permissive or inherited non-owner write access.
- **Cross-Platform Release Packaging & Distribution:**
  - **Universal Download Portal:** Permanent public download endpoint at `https://github.com/Imagen2dragon/Aether-IDE/releases/latest`.
  - **Windows 10/11 x64:** Non-administrative per-user installer (`AetherIDE-Setup-0.1.0-beta.3-x64.exe`) and portable standalone archive (`AetherIDE-v0.1.0-beta.3-windows-x64.zip`).
  - **Linux x64 (glibc 2.35+):** Standalone portable distribution (`AetherIDE-v0.1.0-beta.3-linux-x64.tar.gz`).
  - **macOS ARM64 (Apple Silicon):** Standalone portable distribution (`AetherIDE-v0.1.0-beta.3-macos-arm64.tar.gz`).
  - Cryptographic SHA-256 digests generated and verified for all release artifacts.
- **Official Public Download URLs:**
  - **Latest Universal Portal:** [AetherIDE Latest Release](https://github.com/Imagen2dragon/Aether-IDE/releases/latest)
  - **Windows Setup Installer:** [AetherIDE-Setup-0.1.0-beta.3-x64.exe](https://github.com/Imagen2dragon/Aether-IDE/releases/download/v0.1.0-beta.3/AetherIDE-Setup-0.1.0-beta.3-x64.exe)
  - **Windows Installer SHA-256:** [AetherIDE-Setup-0.1.0-beta.3-x64.exe.sha256](https://github.com/Imagen2dragon/Aether-IDE/releases/download/v0.1.0-beta.3/AetherIDE-Setup-0.1.0-beta.3-x64.exe.sha256)
  - **Windows Portable ZIP:** [AetherIDE-v0.1.0-beta.3-windows-x64.zip](https://github.com/Imagen2dragon/Aether-IDE/releases/download/v0.1.0-beta.3/AetherIDE-v0.1.0-beta.3-windows-x64.zip)
  - **Windows Portable SHA-256:** [AetherIDE-v0.1.0-beta.3-windows-x64.zip.sha256](https://github.com/Imagen2dragon/Aether-IDE/releases/download/v0.1.0-beta.3/AetherIDE-v0.1.0-beta.3-windows-x64.zip.sha256)
  - **Linux x64 Tarball:** [AetherIDE-v0.1.0-beta.3-linux-x64.tar.gz](https://github.com/Imagen2dragon/Aether-IDE/releases/download/v0.1.0-beta.3/AetherIDE-v0.1.0-beta.3-linux-x64.tar.gz)
  - **Linux x64 SHA-256:** [AetherIDE-v0.1.0-beta.3-linux-x64.tar.gz.sha256](https://github.com/Imagen2dragon/Aether-IDE/releases/download/v0.1.0-beta.3/AetherIDE-v0.1.0-beta.3-linux-x64.tar.gz.sha256)
  - **macOS ARM64 Tarball:** [AetherIDE-v0.1.0-beta.3-macos-arm64.tar.gz](https://github.com/Imagen2dragon/Aether-IDE/releases/download/v0.1.0-beta.3/AetherIDE-v0.1.0-beta.3-macos-arm64.tar.gz)
  - **macOS ARM64 SHA-256:** [AetherIDE-v0.1.0-beta.3-macos-arm64.tar.gz.sha256](https://github.com/Imagen2dragon/Aether-IDE/releases/download/v0.1.0-beta.3/AetherIDE-v0.1.0-beta.3-macos-arm64.tar.gz.sha256)
- **Security & Code-Signing Disclosure:**
  - As an open-source beta release, binaries are compiled deterministically and published with authoritative SHA-256 checksums. They are not yet signed with a commercial Extended Validation (EV) code-signing certificate.
  - Windows SmartScreen or "Unknown Publisher" dialogs may appear during installation. Users can independently verify the published SHA-256 checksum and click **More info** -> **Run anyway**.
- **System Requirements:**
  - Windows: 64-bit Windows 10 (version 1809+) or Windows 11.
  - Linux: x86_64 Linux distribution with glibc 2.35+ (Ubuntu 22.04+, Debian 12+, Fedora 38+).
  - macOS: Apple Silicon (M1/M2/M3/M4) running macOS 13 (Ventura) or newer.
- **Known Limitations:**
  - EV code signing and macOS Gatekeeper notarization are scheduled for the General Availability (GA) milestone.

---

## v0.1.0-beta.2 — Production Release, Atomic Checkpoint Hardening & Verified Installer (November 7, 2026)

### Highlights:
- **Atomic Checkpoint Rollback & Windows Kernel Mutual Exclusion:**
  - Hardened Windows NTFS object-identity verification binding cleanup staging to destination hard links without race conditions.
  - Reciprocal dual-handle locking in `remove_file_windows_exclusive`: staging handle (`GENERIC_READ | DELETE`, `FILE_SHARE_READ`) and destination handle (`GENERIC_READ`, `FILE_SHARE_READ`) held concurrently.
  - Windows kernel actively rejects concurrent unlinking and modifications with `ERROR_SHARING_VIOLATION` (OS error 32).
  - Reparse point traversal protection via `FILE_FLAG_OPEN_REPARSE_POINT`.
  - Comprehensive adversarial regression test matrix (CHK-209, CHK-210, CHK-210B with scoped RAII fault reset, and CHK-220 sharing matrix) with 211 passing tests in `checkpoint-engine`.
  - POSIX fail-closed retained staging behavior (`PublishedWithRetainedStaging`) preserved.
- **Cross-Platform Release Packaging:**
  - **Windows 10/11 x64:** Official Inno Setup installer (`AetherIDE-Setup-0.1.0-beta.2-x64.exe`) and portable zip (`AetherIDE-v0.1.0-beta.2-windows-x64.zip`).
  - **Linux x64 (glibc 2.35+):** Standalone portable distribution (`AetherIDE-v0.1.0-beta.2-linux-x64.tar.gz`).
  - **macOS ARM64 (Apple Silicon):** Standalone portable distribution (`AetherIDE-v0.1.0-beta.2-macos-arm64.tar.gz`).
  - Cryptographic SHA-256 digests generated and verified for all release artifacts.
- **Official Public Download URLs:**
  - **Windows Setup Installer:** [AetherIDE-Setup-0.1.0-beta.2-x64.exe](https://github.com/Imagen2dragon/Aether-IDE/releases/download/v0.1.0-beta.2/AetherIDE-Setup-0.1.0-beta.2-x64.exe)
  - **Windows Installer SHA-256:** [AetherIDE-Setup-0.1.0-beta.2-x64.exe.sha256](https://github.com/Imagen2dragon/Aether-IDE/releases/download/v0.1.0-beta.2/AetherIDE-Setup-0.1.0-beta.2-x64.exe.sha256)
  - **Windows Portable ZIP:** [AetherIDE-v0.1.0-beta.2-windows-x64.zip](https://github.com/Imagen2dragon/Aether-IDE/releases/download/v0.1.0-beta.2/AetherIDE-v0.1.0-beta.2-windows-x64.zip)
  - **Windows Portable SHA-256:** [AetherIDE-v0.1.0-beta.2-windows-x64.zip.sha256](https://github.com/Imagen2dragon/Aether-IDE/releases/download/v0.1.0-beta.2/AetherIDE-v0.1.0-beta.2-windows-x64.zip.sha256)
  - **Linux x64 Tarball:** [AetherIDE-v0.1.0-beta.2-linux-x64.tar.gz](https://github.com/Imagen2dragon/Aether-IDE/releases/download/v0.1.0-beta.2/AetherIDE-v0.1.0-beta.2-linux-x64.tar.gz)
  - **Linux x64 SHA-256:** [AetherIDE-v0.1.0-beta.2-linux-x64.tar.gz.sha256](https://github.com/Imagen2dragon/Aether-IDE/releases/download/v0.1.0-beta.2/AetherIDE-v0.1.0-beta.2-linux-x64.tar.gz.sha256)
  - **macOS ARM64 Tarball:** [AetherIDE-v0.1.0-beta.2-macos-arm64.tar.gz](https://github.com/Imagen2dragon/Aether-IDE/releases/download/v0.1.0-beta.2/AetherIDE-v0.1.0-beta.2-macos-arm64.tar.gz)
  - **macOS ARM64 SHA-256:** [AetherIDE-v0.1.0-beta.2-macos-arm64.tar.gz.sha256](https://github.com/Imagen2dragon/Aether-IDE/releases/download/v0.1.0-beta.2/AetherIDE-v0.1.0-beta.2-macos-arm64.tar.gz.sha256)
- **Security & Code-Signing Disclosure:**
  - As an open-source beta release, binaries are compiled deterministically and published with authoritative SHA-256 checksums. They are not yet signed with a commercial Extended Validation (EV) code-signing certificate.
  - Windows SmartScreen or "Unknown Publisher" dialogs may appear during installation. Users can independently verify the published SHA-256 checksum and click **More info** -> **Run anyway**.
- **System Requirements:**
  - Windows: 64-bit Windows 10 (version 1809+) or Windows 11.
  - Linux: x86_64 Linux distribution with glibc 2.35+ (Ubuntu 22.04+, Debian 12+, Fedora 38+).
  - macOS: Apple Silicon (M1/M2/M3/M4) running macOS 13 (Ventura) or newer.
- **Known Limitations:**
  - EV code signing and macOS Gatekeeper notarization are scheduled for the General Availability (GA) milestone.

---

## v0.1.0-beta.1 (Sprint 08 Release Certification & Stabilization) — October 31, 2026

### Highlights:
- **P0 Security Hardening & Closure:**
  - Stateful session token management (`TokenStore`) with 24-hour TTL, token rotation with a 5-minute grace period, and brute-force rate-limiting lockout after 5 failed authentication attempts.
  - Constant-time SHA-256 session token verification eliminating timing side-channels across TCP loopback and Windows Named Pipes.
  - Hardened multi-vector path containment (`resolve_contained_path`) blocking NTFS Alternate Data Streams (`:`), Windows silent truncation evasion (trailing dots/spaces), UNC paths (`\\`, `//`), extended device namespaces (`\??\`), directory traversals, and reserved DOS device names (`CON`, `PRN`, `AUX`, `NUL`).
  - Comprehensive Daemon RBAC gating `RunCommand`, `TerminalSpawn`, `TerminalWrite`, `CreateAgentTask`, and step execution behind `can_write()`, and role management behind `can_admin()`.
  - Upgraded auto-updater manifest verification with authoritative root Ed25519 digital signatures and operating system / architecture platform binding.
- **Full Product End-to-End Reliability:**
  - Verified 82 automated tests across all 24 crates with zero Clippy warnings (`-D warnings`) and clean formatting.
  - Non-blocking live SQLite WAL backup via native `sqlite3_backup` API under concurrent read/write transactions.
  - Complete disposable project lifecycle test verifying Git initialization, symbol indexing, LSP diagnostics, interactive ConPTY terminal streaming, autonomous agent worktree execution, proof-of-work logging, checkpoint creation, and encrypted device synchronization.
- **Cross-Platform Enterprise Packaging:**
  - Windows 10/11: Inno Setup executable (`.exe`) and signed MSIX package (`.msix`).
  - Linux: Portable AppImage and Debian package (`.deb`).
  - macOS: Universal `.app` bundle and DMG disk image (`.dmg`).
- **Low Resource Mode & Storage Hygiene:**
  - Introduced `AetherConfig::enable_low_resource_mode()` restricting concurrency and memory pressure for constrained environments.
  - Storage discipline strictly maintained: in-tree repository size 1.109 GB on OneDrive (well under 6.00 GB limit) with zero in-tree Cargo target bloat.

---

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
