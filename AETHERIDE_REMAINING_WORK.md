# AETHERIDE — POST-BETA GENERAL AVAILABILITY ROADMAP
## Prioritized Post-Commercial Beta Backlog & GA Milestones

**Document Type:** Strategic Engineering Backlog  
**Date:** October 31, 2026  
**Project:** AetherIDE  
**Status:** ACTIVE BACKLOG (Post-Sprint 08 Beta Release Validation)  

---

## 1. Sprint 07 & 08 Commercial Beta Completed Foundations

All foundational P0/P1 capabilities from Sprints 01 through 08 are fully implemented, independently reproduced, and verified:
- [x] Monaco Editor workbench integration with LSP syntax, completion, and diagnostics.
- [x] Native HWND-embedded Microsoft WebView2 host (`aether-launcher`) with COM messaging bridge.
- [x] Windows ConPTY terminal host with `ScopedJobObject` containment.
- [x] 14 autonomous engineering personas coordinated via `SwarmCoordinator`.
- [x] Worktree sandboxing (`.aether/worktrees/`) and native three-way merge engine.
- [x] Yrs/Yjs CRDT multi-user collaboration and cursor awareness tracking.
- [x] Backend-enforced `RoleRegistry` eliminating role-spoofing privilege elevation.
- [x] Constant-time session token authentication and hardened filesystem containment (`resolve_contained_path` with ADS/UNC/device filters).
- [x] Stateful `TokenStore` with 24h TTL, token rotation grace periods, and brute-force lockout.
- [x] Daemon RBAC gating commands, terminals, and agents behind `can_write()` and `can_admin()`.
- [x] Ed25519 digital update signatures and OS/Arch platform binding.
- [x] Low Resource Mode runtime optimization (`AetherConfig::enable_low_resource_mode()`).
- [x] SQLite WAL online backups via native `sqlite3_backup` API.
- [x] Staged auto-updater with anti-downgrade rollback defense and atomic `.rollback_bak` recovery.
- [x] Cross-platform release packaging (Windows Inno Setup/MSIX, Linux AppImage/deb, macOS DMG).
- [x] Storage hygiene: zero in-tree target bloat, 1.109 GB repository footprint in OneDrive (< 6.00 GB limit).
- [x] 82 automated test suites passing across all 24 crates with zero Clippy warnings.

---

## 2. General Availability (GA) Prioritized Backlog

### Phase 1 — Hardware Security & Enterprise Trust (P0)
1. **EV Code Signing Certificate Pipeline:**
   - Procure and integrate Extended Validation (EV) hardware token / Azure Key Vault signing for Windows `AetherIDE.exe` and `aether-daemon.exe` to establish instant SmartScreen reputation.
   - Apple Developer ID Application certificate signing and automated notarization via `xcrun notarytool` for macOS universal binaries.
2. **Enterprise Single Sign-On (SSO) & SCIM:**
   - Support SAML 2.0 and OIDC authentication for enterprise workspace synchronization.
   - Centralized RBAC synchronization via SCIM directory integrations.

### Phase 2 — Extended Language Server Ecosystem (P1)
3. **One-Click LSP Bundle Installer:**
   - Pre-package or provide automated sandbox downloaders for `rust-analyzer`, `clangd`, `pyright`, `gopls`, and `zls` within the desktop package manager.
4. **Wasm Edge Extensions:**
   - Sandboxed WebAssembly extension host for third-party tools, themes, and grammar analyzers.

### Phase 3 — Mobile Cloud Build Execution (P2)
5. **EAS Cloud Build Production Triggers:**
   - Connect live EAS API tokens for automated remote build submission from the desktop Mobile Studio pane directly to the Apple App Store and Google Play Store.
