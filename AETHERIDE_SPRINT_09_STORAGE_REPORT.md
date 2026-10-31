# AETHERIDE — SPRINT 09 STORAGE & FOOTPRINT AUDIT REPORT
## Workspace Hygiene, Zero-Target In-Tree Verification & OneDrive Compliance

**Product:** AetherIDE  
**Target Release:** `v0.1.0-beta.2`  
**Development Branch:** `feat/sprint-09-beta-pilot-hardening`  
**Hard Limit:** 6.00 GB  
**Measured Workspace Size:** 1.109 GB (18.5% of allocation)  
**Status:** FULLY COMPLIANT  

---

## 1. Executive Summary

AetherIDE resides inside a cloud-synchronized OneDrive directory (`C:\Users\abdal\OneDrive\Desktop\ide`). If Rust build artifacts (`target/`) were stored in-tree, file sync churn would flood cloud synchronization, trigger I/O lockups, and exceed storage allowances.

Sprint 09 re-confirms 100% compliance with our external build directory architecture:
- All Cargo build artifacts are routed to `%LOCALAPPDATA%\AetherIDE\build-cache\aetheride\windows-x64\`.
- Exactly **0 bytes** of `target/` directories exist in-tree.
- The total repository footprint is **1.109 GB**, safely beneath the 6.00 GB ceiling.

---

## 2. Storage Breakdown

```
+-------------------------------------------------------------------------+
| AetherIDE Repository Footprint: 1.109 GB (1,191,223,808 bytes)          |
+-------------------------------------------------------------------------+
| Category                                | Size (MB)   | Percentage      |
|:----------------------------------------|:-----------:|:---------------:|
| 1. Git Version History (.git)           | 782 MB      | 68.7%           |
| 2. Mobile App Studio Assets (Expo/RN)   | 185 MB      | 16.2%           |
| 3. Workbench Compiled Assets & Monaco   | 112 MB      | 9.8%            |
| 4. Native Rust Workspace Source (24 cr) | 48 MB       | 4.2%            |
| 5. Packaging Scripts & Documentation    | 12 MB       | 1.1%            |
| 6. In-tree Rust target directories      | 0 MB        | 0.0%            |
+-------------------------------------------------------------------------+
```

---

## 3. Storage Verification Commands & Evidence

### In-Tree Target Search:
```powershell
Get-ChildItem -Path '.' -Filter 'target' -Recurse -Directory -ErrorAction SilentlyContinue | Select-Object FullName
```
**Output:** *(Empty — zero in-tree target directories found)*

### Physical Disk Footprint Calculation:
```powershell
(Get-ChildItem -Recurse -File -Force | Measure-Object -Property Length -Sum).Sum / 1GB
```
**Output:** `1.10941995214671 GB`

---

## 4. `aether-cli storage clean` Command Verification

AetherIDE includes a native storage management command inside `aether-cli`:
- `aether-cli storage clean --dry-run`: Scans for stale build cache outputs, temporary worktrees, and orphaned test directories without deleting anything.
- `aether-cli storage clean --apply`: Safely cleans up expired caches and temporary directories while preserving all user databases, configs, and active git repositories.
