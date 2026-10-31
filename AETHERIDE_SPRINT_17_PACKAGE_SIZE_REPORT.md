# AetherIDE Sprint 17 — Package Size & Storage Footprint Report

**Product:** AetherIDE  
**Release Target:** `v0.1.0-beta.2`  
**Distribution Targets:** Windows x64 (Installer + Portable), Linux x64, macOS ARM64  
**Status:** ALL PACKAGE & FOOTPRINT TARGETS SURPASSED  

---

## 1. Executive Summary

This report documents the audited package sizes, installed footprints, and repository storage metrics for AetherIDE `v0.1.0-beta.2`. The engineering directives set stringent targets to keep AetherIDE fast to download, quick to decompress, and respectful of local disk space on constrained computers.

All generated release artifacts comfortably beat established budgets:
- The flagship Windows Setup EXE is **8.37 MB** (budget: ≤ 15 MB).
- The Windows portable archive is **10.3 MB** (budget: ≤ 12 MB).
- The installed base application footprint is **35.4 MB** (budget: ≤ 100 MB).

---

## 2. Release Artifact Inventory & Target Comparison

| Release Package | Target Budget | Actual Size (Bytes) | Actual Size (MB) | Margin Under Budget | Result |
| :--- | :---: | :---: | :---: | :---: | :---: |
| **`AetherIDE-Setup-0.1.0-beta.2-x64.exe`** | ≤ 15.0 MB | **8,374,833** | **8.37 MB** | -44.2% | **PASS** |
| **`AetherIDE-v0.1.0-beta.2-windows-x64.zip`** | ≤ 12.0 MB | **10,323,820** | **10.32 MB** | -14.0% | **PASS** |
| **`AetherIDE-v0.1.0-beta.2-linux-x64.tar.gz`** | ≤ 20.0 MB | **12,538,961** | **12.54 MB** | -37.3% | **PASS** |
| **`AetherIDE-v0.1.0-beta.2-macos-arm64.tar.gz`** | ≤ 15.0 MB | **8,889,706** | **8.89 MB** | -40.7% | **PASS** |

---

## 3. Installed Application Footprint Breakdown

Measurement taken after a fresh silent installation into `%LOCALAPPDATA%\AetherIDE\installer-test-sprint17`:

| Component Directory | Files Included | Installed Size | Share of Total |
| :--- | :--- | :---: | :---: |
| **`bin/`** | `aether-daemon.exe`, `aether-cli.exe` | 14.41 MB | 40.7% |
| **`workbench/vs/`** | Monaco Editor, language modes, workers | 12.41 MB | 35.1% |
| **Root Executables** | `AetherIDE.exe`, `unins000.exe` | 7.44 MB | 21.0% |
| **`workbench/xterm/`** | xterm.js, CSS, addons | 0.58 MB | 1.6% |
| **`workbench/cytoscape/`** | Cytoscape.js engine | 0.44 MB | 1.2% |
| **Documentation & Config** | `LICENSE`, `README.md`, `VERSION.txt`, etc. | 0.12 MB | 0.4% |
| **TOTAL INSTALLED FOOTPRINT** | **All Application Files** | **35.40 MB** | **100.0%** |

*Target Budget:* ≤ 100 MB.  
*Actual Footprint:* **35.4 MB** (64.6% below maximum allowance).

---

## 4. Repository & Build Storage Cleanliness

- **Cargo Target Isolation:** Build outputs are strictly isolated outside the Git working tree at `C:\Users\abdal\.cargo-target-aether`. Zero build `.o`, `.d`, or `.exe` files reside in OneDrive or the Git root.
- **Repository On-Disk Size:** `0.448 GB` (Ceiling: 1.0 GB).
- **Zero Local LLM Bloat:** No multi-gigabyte neural network weights are bundled inside the repository or packages.

---

## 5. Verification Verdict

All size-related acceptance gates are **100% COMPLIANT**.
