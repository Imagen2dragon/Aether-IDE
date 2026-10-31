# AETHERIDE — SPRINT 07 SECURE UPDATER & RELEASE PIPELINE SPECIFICATION
**Document Version:** 1.0.0  
**Status:** Certified / Security Approved  
**Author:** Security Principal & Release Engineering Director  
**Sprint:** Sprint 07 Commercial Beta  
**Repository:** `AetherIDE`

---

## 1. Executive Summary

This specification establishes the architecture, cryptographic controls, and atomic application lifecycle of AetherIDE's Staged Auto-Updater engine (`application-core::update`).

Client auto-updaters represent one of the most critical remote execution vectors in desktop software. A compromised or fragile update mechanism can expose users to supply-chain attacks, downgrade exploits, or corrupted binaries that render the application unbootable.

In Sprint 07, AetherIDE implemented a hardened 5-stage update pipeline incorporating strict semantic version rollback defenses, SHA-256 hash enforcement, Ed25519 digital signature verification, staged payload isolation, and atomic binary swapping with automatic rollback recovery.

---

## 2. Five-Stage Update Lifecycle

```
[1. Manifest Fetch] ---> [2. Version & Rollback Check] ---> [3. Checksum & Sig Verification]
                                                                      |
[5. Atomic Swap & Cleanup] <--- [4. Staging & Backup (.rollback_bak)] <-+
```

### 2.1 Stage 1: Manifest Acquisition
The application periodically queries the official releases endpoint for `AetherUpdateManifest`:
```rust
pub struct AetherUpdateManifest {
    pub version: String,             // e.g. "0.2.0"
    pub channel: String,             // "stable", "beta", "nightly"
    pub release_notes: String,
    pub download_url: String,        // HTTPS only
    pub sha256_checksum: String,     // 64 hex characters
    pub signature_base64: String,    // Ed25519 cryptographic signature
    pub min_os_version: Option<String>,
    pub published_at: DateTime<Utc>,
}
```

### 2.2 Stage 2: Anti-Downgrade & Rollback Defense
`UpdateVerifier::verify_manifest` parses the installed and target versions using strict semantic versioning:
$$V_{\text{target}} > V_{\text{current}}$$
- If an attacker or compromised server attempts to distribute an older or identical release (e.g. downgrading from `0.2.0` to `0.1.0` to exploit known vulnerabilities), the updater immediately aborts with `AetherError::PermissionDenied`.

### 2.3 Stage 3: Cryptographic Payload Integrity
Before any execution or staging:
1. **SHA-256 Checksum:** `UpdateVerifier::verify_payload_checksum` streams the downloaded binary through SHA-256 and compares it against `manifest.sha256_checksum`.
2. **Signature Validation:** The manifest must contain a valid Base64-encoded Ed25519 digital signature matching the release engineering root public key. Missing or malformed signatures abort the pipeline.

### 2.4 Stage 4: Staged Isolation
`StagedUpdateApplicator::stage_update` writes the verified binary into a dedicated local staging directory:
$$\%LOCALAPPDATA\%\backslash\text{AetherIDE}\backslash\text{staging}\backslash\text{aether-update-}V\text{.bin}$$
The running executable is never touched while active.

### 2.5 Stage 5: Atomic Swap & Rollback Safety
`StagedUpdateApplicator::apply_staged_update`:
1. Transitions status to `StagedUpdateStatus::Applying`.
2. Creates an atomic backup of the currently installed executable (`AetherIDE.exe.rollback_bak`).
3. Replaces `AetherIDE.exe` with the staged payload.
4. If an I/O error occurs mid-copy, the applicator catches the error, immediately copies `AetherIDE.exe.rollback_bak` back to `AetherIDE.exe`, cleans up, and transitions to `StagedUpdateStatus::RolledBack`.
5. Upon successful swap, `AetherIDE.exe.rollback_bak` is deleted and status transitions to `StagedUpdateStatus::Completed`.

---

## 3. Automated Verification & Test Results

The updater pipeline is verified through automated unit tests in `application-core/src/update.rs`:

| Test Function | Validation Objective | Result |
|:---|:---|:---:|
| `test_semver_parsing` | Validates `0.1.0`, `v1.2.34-alpha`, rejects invalid strings | **PASS** |
| `test_update_manifest_verification` | Verifies upgrade allowed, downgrades and reinstallations rejected | **PASS** |
| `test_payload_checksum_verification` | Confirms SHA-256 verification and catches single-bit corruptions | **PASS** |
| `test_staged_update_lifecycle_and_rollback` | End-to-end staging, deployment, content verification, and rollback cleanup | **PASS** |

---

## 4. Operational Release Signing Protocol

All commercial beta production builds are signed using an isolated release pipeline:
1. Binary compilation via CI on clean container images.
2. Binary hashing via SHA-256.
3. Ed25519 digital signature generated using an HSM or isolated offline release key.
4. Manifest generated and published over TLS with HSTS enforcement.

---

## 5. Conclusion
AetherIDE's update mechanism adheres to the highest industry standards for binary distribution security, protecting users against malicious tampering, network corruption, and broken installations.
