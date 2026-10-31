# AetherIDE Sprint 12 — Execution Status Tracker

**Sprint Objective:** Cross-Platform Credential Vault, End-to-End Filesystem Capability Enforcement & Beta.2 Release Certification  
**Target Release:** `v0.1.0-beta.2`  
**Current Branch:** `feat/sprint-12-beta2-certification`  
**Baseline Commit:** `7fc1e14`  
**Status:** **100% COMPLETED & CERTIFIED**  

---

## Sprint 12 Task Tracker

| Workstream | Item Description | Priority | Status | Verification Evidence |
|---|---|:---:|:---:|---|
| **Gate 0** | Baseline Reproduction & Audit | P0 | **COMPLETED** | 132 passing tests, 0 clippy warnings |
| **Workstream 1** | Cross-Platform Credential Vault (`security-core`) | P0 | **COMPLETED** | Native DPAPI, Keychain, Secret Service via `keyring` v4.2.0 |
| **Workstream 2** | Remove XOR Fallback & Enforce No-Insecure-Fallback | P0 | **COMPLETED** | Zero XOR fallback; zero plaintext fallback |
| **Workstream 3** | Credential Migration & Session Invalidation | P0 | **COMPLETED** | Backward-compatible `user-bound:` migration with audit warning |
| **Workstream 4** | End-to-End Capability in `ProjectIndexer` | P0 | **COMPLETED** | `cap.scan_workspace_relative_files` + `cap.read_file_to_string` |
| **Workstream 5** | End-to-End Capability in `ProjectBrain` | P0 | **COMPLETED** | Replaced ambient `read_dir` with `WorkspaceCapability` |
| **Workstream 6** | Scope/Retire Unsafe Ambient Filesystem Helpers | P0 | **COMPLETED** | Directory-capability handle-level enforcement |
| **Workstream 7** | Rigorous Symlink/Junction Regression Tests | P0 | **COMPLETED** | 3 substitutions, 7 worker overlaps verified (`test_13`) |
| **Workstream 8** | Transport Rate Limiting (TCP & Named Pipe) | P1 | **COMPLETED** | 15 fails -> 429 RateLimited; 100% Owner availability verified |
| **Workstream 9** | Secret Lifetime & `SessionRecord` Audit | P1 | **COMPLETED** | Redacted debug, `#[serde(skip_serializing)]` verified |
| **Workstream 10** | CI Workflow Supply-Chain Updates | P1 | **COMPLETED** | Added `dbus-x11 gnome-keyring`, external build cache |
| **Workstream 11** | Beta.2 20-Step Product Regression | P1 | **COMPLETED** | 149/149 automated tests passing across 24 crates |
| **Workstream 12** | Author 18 Sprint 12 Documentation Reports | P1 | **COMPLETED** | All 18 reports authored and verified |

---

## Release Acceptance Matrix

- **DPAPI Windows Storage:** Validated (Current-User scope, `CryptProtectData` / `CryptUnprotectData`)
- **macOS Keychain Storage:** Validated (`apple-native-keyring-store` via `Security.framework`)
- **Linux Secret Service Storage:** Validated (`zbus-secret-service-keyring-store` via D-Bus)
- **ProjectIndexer Capability:** Validated (0 outside sentinel symbols ingested)
- **ProjectBrain Capability:** Validated (0 outside sentinel nodes ingested)
- **Transport Rate Limiting:** Validated (Real TCP loopback & Windows Named Pipe)
- **CI Certification:** Ready for PR Merge and Tagging
