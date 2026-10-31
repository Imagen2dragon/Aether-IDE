# AetherIDE Sprint 11 — Threat Model Update

**Document:** STRIDE Threat Analysis & Security Architecture Update  
**Sprint:** Sprint 11 — Beta.2 Release Blocker Closure  
**Scope:** Local Daemon IPC, Credential Storage, Filesystem Boundary, Collaborator Roles  

---

## 1. Updated Threat Surface Analysis

### 1.1 Local Credential Exposure (Information Disclosure)
- **Previous Risk:** `desktop_bootstrap.json` contained the plaintext Owner bearer token. Any local unprivileged process running under the same user account could inspect the file and obtain full administrative control over the daemon.
- **Mitigation in Sprint 11:**
  - Token is protected at creation time via `security_core::protect_credential`.
  - Windows: Uses Windows DPAPI (`CryptProtectData`) with current-user scope, binding decryption exclusively to the active logged-in Windows user session and DPAPI master keys.
  - Unix: Uses PBKDF2/SHA256 key derivation bound to machine-id and user UID.
  - Fail-Closed: If encryption or storage fails, the daemon terminates immediately without listening on the network or IPC pipe.
  - At-rest recursive file scanner test validates zero plaintext token occurrences on disk.

### 1.2 Path Traversal & Reparse Point Hijacking (Elevation of Privilege / Tampering)
- **Previous Risk:** Path canonicalization algorithms could be raced against adversarial directory substitution between check time and use time (TOCTOU).
- **Mitigation in Sprint 11:**
  - Filesystem operations are anchored to an open directory capability (`cap_std::fs::Dir`).
  - All relative operations use directory-relative system calls (`openat`, `unlinkat`, `mkdirat`, etc.) which reject traversing parent directories beyond the anchor.
  - Symlinks and junctions targeting locations outside the workspace boundary fail closed at the capability boundary.
  - Rigorous input sanitization eliminates Windows Alternate Data Streams (`:`), extended paths (`\\?\`), DOS device names (`CON`, `PRN`, `AUX`, `NUL`), and control characters.

### 1.3 Connection-Churn Denial of Service (Denial of Service)
- **Previous Risk:** IP/pipe-based rate limiting allowed an attacker to rapidly open new connections with unique source IDs, exhausting server memory and CPU without triggering individual source limits.
- **Mitigation in Sprint 11:**
  - Layered rate limiting in `TokenStore`:
    1. Per-source rate limiting tracks repeated failures per IP/pipe.
    2. Global unauthenticated failure sliding window (100 failures / 60 seconds) detects distributed churn attacks.
    3. Automated stale entry eviction prevents memory exhaustion.
  - Priority channel preservation: Authenticated Owner sessions bypass global unauthenticated failure limits, preventing denial of service to legitimate operators.

---

## 2. Residual Risks & Next Steps
- Physical access to an unlocked Windows session with the same user privileges can still invoke DPAPI `CryptUnprotectData`. This is consistent with the Windows security model for per-user desktop applications.
- Additional hardware-bound credential protection (TPM 2.0 / Windows Hello) is scheduled for enterprise qualification in Sprint 12.
