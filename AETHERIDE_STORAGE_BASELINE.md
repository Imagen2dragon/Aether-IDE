# AetherIDE — Storage Baseline & Analysis Report
## Initial Measurement, Directory Breakdown and Growth Drivers

**Author:** Storage and Build Infrastructure Engineer  
**Project:** AetherIDE  
**Sprint:** Sprint 06 (Gate 0 Baseline)  
**Measurement Timestamp:** 2026-10-31T12:35:00+03:00  
**Project Root:** `C:\Users\abdal\OneDrive\Desktop\ide`  

---

## 1. Executive Summary

A comprehensive storage audit of the AetherIDE workspace identified a total repository footprint of **12.23 GB** across **35,897 files**. 

The investigation revealed that **96.4%** of the total storage footprint is concentrated in the Rust build target folder (`rust/target/`, **11.517 GB** across 32,288 files). Because the source checkout is currently located inside a Microsoft OneDrive path (`OneDrive\Desktop\ide`), OneDrive is constantly indexing and attempting to synchronize tens of thousands of volatile binary intermediates, PDB debug symbol files, and incremental compilation fragments.

By decoupling the Cargo build cache from the OneDrive working tree, tuning Cargo profile debuginfo levels, and managing stale compilation outputs, AetherIDE can achieve an immediate **>50% to 75% reduction** in local storage usage, bringing the workspace build-cache footprint well under the 6 GB operating target.

---

## 2. Directory Breakdown & Storage Classification

| Directory / Component | Size (MB) | Size (GB) | File Count | Classification | Can Recreate? | Actively Used? | Recommended Retention |
|---|---|---|---|---|---|---|---|
| `rust/target/debug/deps` | 6,442.74 MB | 6.292 GB | 2,602 | Generated Intermediates | Yes (`cargo build`) | Yes (Old sprints) | Relocate outside OneDrive; tune debuginfo level |
| `rust/target/debug/incremental` | 4,280.53 MB | 4.180 GB | 23,327 | Volatile Cache | Yes (`cargo build`) | Yes | Relocate outside OneDrive |
| `rust/target/release` | 764.00 MB | 0.746 GB | 2,054 | Release Binaries & PDBs | Yes (`cargo build --release`) | Yes (Sprint 05 artifacts) | Relocate outside OneDrive |
| `rust/target/debug/build` | 172.64 MB | 0.169 GB | 280 | Build Script Outputs | Yes | Yes | Relocate outside OneDrive |
| `apps/desktop/node_modules` | 396.75 MB | 0.387 GB | 3,098 | Vendored Node Packages | Yes (`npm ci`) | Yes (Monaco/Xterm/Cyto) | Retain canonical npm tree; exclude dist duplication |
| `dist/windows-x64` | 37.75 MB | 0.037 GB | 142 | Release Distribution | Yes (`build-windows`) | Yes (v0.1.0-alpha) | Retain single latest release package |
| `extensions/` | 25.57 MB | 0.025 GB | 274 | Syntax Extensions | No (Source) | Yes | Retain in repository |
| `rust/crates/` (Source) | 0.30 MB | < 0.001 GB | 60 | Canonical Rust Code | No (Source) | Yes | Permanent source repository |
| `rust/cli/` (Source) | 0.05 MB | < 0.001 GB | 6 | Canonical Rust Code | No (Source) | Yes | Permanent source repository |
| `rust/services/` (Source) | 0.04 MB | < 0.001 GB | 3 | Canonical Rust Code | No (Source) | Yes | Permanent source repository |
| `docs/` | 0.06 MB | < 0.001 GB | 22 | Documentation | No (Source) | Yes | Permanent source repository |
| `scripts/` | < 0.01 MB | < 0.001 GB | 2 | Automation Scripts | No (Source) | Yes | Permanent source repository |
| **TOTAL BASELINE** | **12,238.00 MB** | **12.238 GB** | **35,897** | | | | |

---

## 3. Key Observations & Inefficiencies

1. **Full PDB Symbol Generation in Debug Profile (`debug = 2` / full debuginfo):**
   Rust's default `dev` and `test` profiles emit complete Microsoft PDB files for every single crate and transitive dependency (`winapi`, `windows`, `tokio`, `hyper`, `rusqlite`, `ring`, etc.). This causes `deps/` to explode to **6.29 GB**. Reducing `split-debuginfo` and using `debug = 1` (line-tables-only) cuts PDB volume by up to 70% while preserving stack traces and file/line diagnostics.
2. **Accumulation of Obsolete Incremental Artifacts:**
   Incremental compilation has accumulated 23,327 individual hash fragments totaling **4.18 GB** across Sprints 01 through 05.
3. **OneDrive Synchronization Penalty:**
   Having 32,288 files inside `OneDrive\Desktop\ide\rust\target\` triggers continuous background I/O contention with the OneDrive sync client, causing file lock delays, CPU consumption, and network upload saturation.

---

## 4. Remediation Targets (Sprint 06 Gate 0)

- **Target 1:** Establish external build cache at `%LOCALAPPDATA%\AetherIDE\build-cache\`.
- **Target 2:** Clean obsolete in-tree `rust/target/` directory after verifying external build workflow.
- **Target 3:** Optimize Cargo profile configuration in `rust/Cargo.toml` (`debug = 1` for dev/test, `strip = "symbols"` for release).
- **Target 4:** Implement `aether storage audit`, `aether storage status`, and `aether storage clean` in `aether-cli`.
- **Target 5:** Establish active build-cache budget threshold (6 GB operating target, 8 GB warning threshold).
