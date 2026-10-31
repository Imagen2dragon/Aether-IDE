# AETHERIDE — SPRINT 12 REPORT: KNOWN LIMITATIONS & MITIGATIONS

**Product:** AetherIDE (`v0.1.0-beta.2`)  
**Sprint:** Sprint 12 — Cross-Platform Credential Vault & Release Certification  
**Date:** 2026-10-31  
**Status:** Documented  

---

## 1. Operating System Keyring Fallbacks in Headless Linux

### Limitation
On headless Linux servers or CI containers without an active graphical session, the Freedesktop Secret Service daemon (`org.freedesktop.secrets`) may not be running or accessible over D-Bus.

### Mitigation
1. In Sprint 12, `security-core` provides `EphemeralMemoryStore` for test harnesses and strictly ephemeral memory-only runtime profiles.
2. In production desktop deployments, standard desktop environments (GNOME, KDE, XFCE) launch the keyring daemon automatically on login.
3. Plaintext fallback is strictly prohibited by security policy; if the OS keyring is unavailable in standard desktop mode, the daemon logs an explicit diagnostic error and refuses to start without secure storage.

---

## 2. Windows NTFS Symbolic Link Privilege Requirements

### Limitation
On Windows 10/11, creating directory symbolic links requires Administrator privileges unless Windows Developer Mode is enabled in OS Settings.

### Mitigation
1. `filesystem-engine` supports NTFS directory junctions (`mklink /J`) which do not require elevated privileges or Developer Mode for standard local users.
2. Adversarial test suites (`adversarial_filesystem_tests.rs`) dynamically detect whether symlinks or junctions are permitted in the current runner process, exercising junctions reliably on standard non-elevated user accounts.

---

## 3. Real-Time CRDT Document Size Boundaries

### Limitation
Real-time peer-to-peer Yrs/Yjs CRDT synchronization is optimized for source code files (< 10 MB). Synchronizing binary assets or large generated bundles (> 50 MB) through CRDT introduces memory bloat.

### Mitigation
Large files are synchronized using the encrypted chunking pipeline (`slice_file` / `reassemble_chunks` with 64 KB chunks and SHA-256 verification) rather than CRDT state vectors.
