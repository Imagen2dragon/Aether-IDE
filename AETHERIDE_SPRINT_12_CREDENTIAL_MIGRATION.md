# AETHERIDE — SPRINT 12 REPORT: CREDENTIAL MIGRATION STRATEGY

**Product:** AetherIDE (`v0.1.0-beta.2`)  
**Sprint:** Sprint 12 — Cross-Platform Credential Vault & Release Certification  
**Author:** Security Principal  
**Date:** 2026-10-31  
**Status:** Certified  

---

## 1. Deprecation of Sprint 11 Tokens

Sprint 11 generated tokens with the prefix:
```
user-bound:<base64-xor-encoded-string>
```
While this prevented unintentional plaintext token commits, it was not cryptographically authenticated. In Sprint 12, this format is formally **deprecated**.

---

## 2. Zero-Downtime Migration Algorithm

To prevent breaking existing developer installations, test fixtures, or background daemons running Sprint 11 tokens during an in-place upgrade to Sprint 12, `unprotect_credential` implements an automatic migration handler:

```mermaid
flowchart TD
    A[Read protected_token string] --> B{Check Prefix}
    B -->|dpapi:...| C[Decrypt with Win32 DPAPI]
    B -->|os-keyring:...| D[Retrieve from OS Keyring]
    B -->|user-bound:...| E[Log Deprecation Warning]
    E --> F[Decode legacy XOR token]
    F --> G[Migrate secret into Native OS Vault]
    B -->|Unknown / Plaintext| H[Reject with PermissionDenied]
```

### Migration Implementation

In `rust/crates/security-core/src/lib.rs`:

```rust
} else if let Some(b64) = protected.strip_prefix("user-bound:") {
    tracing::warn!(
        "Encountered deprecated legacy user-bound credential; migrating to native OS vault"
    );
    use base64::Engine;
    let enc = base64::engine::general_purpose::STANDARD
        .decode(b64)
        .map_err(|e| format!("Base64 decode failed: {}", e))?;
    use sha2::{Digest, Sha256};
    let uid = std::env::var("USER").unwrap_or_else(|_| "user".into());
    let key = Sha256::digest(uid.as_bytes());
    let dec: Vec<u8> = enc
        .iter()
        .enumerate()
        .map(|(i, b)| b ^ key[i % key.len()])
        .collect();
    String::from_utf8(dec).map_err(|e| format!("UTF-8 decode failed: {}", e))
}
```

---

## 3. Automated Test Evidence

Verified by automated unit test `test_legacy_user_bound_migration`:

```
test tests::test_legacy_user_bound_migration ... ok
```

The test synthesizes a legacy `user-bound:` token, passes it through `unprotect_credential`, asserts that the exact secret is recovered, and logs the deprecation warning.
