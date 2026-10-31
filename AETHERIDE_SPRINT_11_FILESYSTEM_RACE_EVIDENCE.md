# AetherIDE Sprint 11 — Filesystem Race & Adversarial Substitution Evidence

**Product:** AetherIDE  
**Crate:** `filesystem-engine`  
**Test Suite:** `adversarial_filesystem_tests.rs`  
**Focus:** Concurrent TOCTOU Race & Deterministic Substitution Matrix  
**Status:** ALL TESTS VERIFIED & PASSING  
**Date:** October 31, 2026  

---

## 1. Executive Summary

Sprint 10 identified that concurrent symlink race tests had not demonstrated verified overlapping attacks due to unprivileged Windows account constraints and file lock contention. In Sprint 11, `adversarial_filesystem_tests.rs` was re-engineered to test both deterministic substitution matrices and high-concurrency race conditions using native NTFS directory junctions and symlinks.

---

## 2. Test 13: High-Concurrency Adversarial Symlink/Junction Race

### 2.1 Test Architecture
- **Attacker Thread:** Repeatedly swaps `ws/race_target` between a benign directory and an external junction pointing to an outside sentinel directory containing `sentinel.txt` (`CRITICAL_SENTINEL_ROOT_KEY_DO_NOT_TAMPER`) and `canary.txt` (`OUTSIDE_CANARY_DO_NOT_DELETE`).
- **Worker Thread:** Concurrently performs contained writes, reads, directory creations, renames, and deletions anchored to the workspace capability.
- **Overlap Counter:** Real-time reparse point / symlink detection confirms operations executed while an adversarial junction was actively mounted.

### 2.2 Execution Results
```text
     Running tests\adversarial_filesystem_tests.rs
running 1 test
 [Race Test Metrics] Successful substitutions: 6, Overlapped worker ops: 6
test test_13_concurrent_adversarial_symlink_race ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 13 filtered out; finished in 1.14s
```

### 2.3 Verified Invariants Post-Race
1. **Outside Sentinel Integrity:** Exact string `CRITICAL_SENTINEL_ROOT_KEY_DO_NOT_TAMPER` unchanged.
2. **Outside Canary Integrity:** `canary.txt` was never deleted or renamed.
3. **Outside Directory Purity:** Exactly 2 files exist in outside directory (`sentinel.txt`, `canary.txt`); zero unauthorized files created.
4. **Zero Information Leak:** Workspace indexer scan never ingested any sentinel content.

---

## 3. Test 14: Deterministic Adversarial Substitution Matrix

### 3.1 Test Matrix & Results
With an active junction targeting the outside directory:

| Operation | Target Path | Result | Invariant Verified |
|---|---|---|---|
| Contained Read | `sub_symlink/sentinel.txt` | **REJECTED (Err)** | External file read blocked |
| Contained Write | `sub_symlink/evil_payload.txt` | **REJECTED (Err)** | External file write blocked |
| Create Directory | `sub_symlink/injected_folder` | **REJECTED (Err)** | External folder creation blocked |
| Delete File | `sub_symlink/canary.txt` | **FAILED/NO-OP** | External canary preserved |
| Rename File | `sub_symlink/config.json` | **REJECTED (Err)** | External rename blocked |
| Workspace Scan | Full workspace traversal | **SKIPPED** | External files ignored |
| Byte/Hash Invariant | All outside sentinels | **100% IDENTICAL** | Exact initial state preserved |

```text
running 1 test
 [Matrix Test Metrics] Verified all 7 adversarial substitution containment invariants
test test_14_deterministic_adversarial_substitution_matrix ... ok
```
