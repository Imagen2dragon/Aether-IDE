# AETHERIDE — SPRINT 12 REPORT: FILESYSTEM RACE EVIDENCE

**Product:** AetherIDE (`v0.1.0-beta.2`)  
**Sprint:** Sprint 12 — Cross-Platform Credential Vault & Release Certification  
**Author:** Distributed Systems Engineer & Security Principal  
**Date:** 2026-10-31  
**Status:** Certified  

---

## 1. Challenge: Proving TOCTOU Protection Under Active Concurrency

Time-of-Check to Time-of-Use (TOCTOU) symlink substitution is an attack where an adversary alternates between a benign directory and an adversarial symlink pointing to an external confidential directory, attempting to trick a background thread into operating on external files.

In Sprint 11, review showed that `test_13_concurrent_adversarial_symlink_race` asserted that substitutions occurred, but did not assert that worker operations actually overlapped while the adversarial symlink was actively present.

---

## 2. Hardened Race Test (`test_13`)

In Sprint 12, we enhanced `test_13_concurrent_adversarial_symlink_race` with explicit atomic overlap tracking and synchronization:

```rust
// PROVEN OVERLAP COORDINATION COUNTERS
let substitution_count = Arc::new(AtomicU64::new(0));
let sub_clone = substitution_count.clone();

let overlapped_operations = Arc::new(AtomicU64::new(0));
let overlap_clone = overlapped_operations.clone();
```

1. **Initial Substitution:** An adversarial junction is planted prior to worker start to guarantee immediate attack presence.
2. **Attacker Loop:** Repeatedly replaces the directory with a junction pointing to an external confidential directory containing `sentinel.txt` (`CRITICAL_SENTINEL_ROOT_KEY_DO_NOT_TAMPER`) and `canary.txt`.
3. **Worker Loop:** Polls rapidly (every 8ms), checks if `is_symlink_or_reparse(&target)` is true, increments `overlapped_operations`, and performs:
   - Contained file writes
   - Contained file reads
   - Contained directory creations
   - Contained renames
   - Contained deletes

---

## 3. Empirical Test Output and Metrics

Execution of `cargo test --test adversarial_filesystem_tests -- test_13 --nocapture`:

```
 [Race Test Metrics] Successful substitutions: 3, Overlapped worker ops: 7
test test_13_concurrent_adversarial_symlink_race ... ok
```

### Invariants Verified and Proven:
1. `total_subs > 0` (3 successful substitutions occurred).
2. `total_overlaps > 0` (7 worker operations executed while the adversarial junction was confirmed actively in place).
3. `sentinel.txt` content in the outside directory remained 100% identical and unmodified.
4. `canary.txt` in the outside directory was NOT deleted.
5. Zero unauthorized files were written to the outside directory.
6. Workspace indexer scans completely excluded the external sentinel data.
