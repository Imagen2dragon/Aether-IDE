# AetherIDE Sprint 18 — Baseline Audit & Gate 0 Reconciliation Report

**Product:** AetherIDE  
**Target Release:** `v0.1.0-beta.2`  
**Previous Baseline Commit (`main`):** `7099ae7bb7fb9850d79bac36847b4f7bc5613981`  
**PR #10 Merge Commit (`main`):** `d8ccfed2f57c1a977693761d0d201341d2dd9bf6`  
**Development Branch:** `feat/sprint-18-branding-windows-identity-download-activation`  
**Repository:** `https://github.com/Imagen2dragon/Aether-IDE.git`  
**Date:** 2026-11-01  

---

## 1. Executive Summary

In accordance with Gate 0 of the Sprint 18 Master Engineering Directive, this report independently audits the state of the repository, previous PR #10, CI build status, packaging scripts, website, runtime dependencies, and application assets before introducing Sprint 18 enhancements.

All baseline findings have been verified on disk and against the remote GitHub repository.

---

## 2. PR #10 Verification & Merge

1. **State Prior to Audit:** PR #10 (`feat(release): Sprint 17 Lightweight Windows Installer, Public Download Portal & Performance Optimizations`) was open, mergeable, and had zero conflicts.
2. **Review Comments:** None pending.
3. **Continuous Integration:** 10/10 matrix jobs passed across both push and pull-request events on GitHub Actions:
   - `Code Formatting & Quality`: PASS (21s / 23s)
   - `TypeScript & Frontend Workbench Verification`: PASS (27s / 33s)
   - `Linux x64 (GNU) Build & Package`: PASS (7m30s / 3m53s)
   - `Windows x64 (MSVC) Build & Package`: PASS (7m0s / 8m8s)
   - `macOS ARM64 (Apple Silicon) Build & Package`: PASS (4m33s / 3m3s)
4. **Merge Execution:** PR #10 was cleanly merged into `origin/main` at commit `d8ccfed2f57c1a977693761d0d201341d2dd9bf6`.
5. **Sprint 18 Branching:** Branch `feat/sprint-18-branding-windows-identity-download-activation` was successfully created and checked out from `d8ccfed2f57c1a977693761d0d201341d2dd9bf6`.

---

## 3. Existing Asset & Configuration Audit

| Target Path | Present on Disk | Inspection Notes |
| :--- | :---: | :--- |
| `scripts/installer/aetheride_installer.iss` | YES | Inno Setup script present. Uses outdated/incorrect registry GUID for WebView2 Evergreen Runtime. |
| `packaging/windows/aether-setup.iss` | YES | Alternate installer script present; unified packaging uses `scripts/installer/aetheride_installer.iss`. |
| `scripts/package-windows.ps1` | YES | Packaging pipeline compiles release binaries, stages payload, and invokes ISCC. |
| `scripts/package-linux.sh` | YES | Linux tar.gz packaging pipeline. |
| `scripts/package-macos.sh` | YES | macOS tar.gz packaging pipeline. |
| `website/index.html` | YES | Static download portal created in Sprint 17. |
| `website/downloads/index.html` | YES | Downloads section redirect. |
| `website/assets/styles.css` | YES | Pure CSS (5.6 KB) without external dependencies. |
| `website/README.md` | YES | GitHub Pages deployment guidance. |
| `rust/cli/aether-launcher/src/win32_host.rs` | YES | Win32 launcher host. Missing embedded icon (`hIcon`/`hIconSm` default), missing `SetCurrentProcessExplicitAppUserModelID`. |
| `apps/desktop/workbench/index.html` | YES | Monaco workbench with dynamic script lazy loading. Lacks master brand logo and unified menu bar. |
| `rust/Cargo.toml` | YES | 24 workspace crates. |

---

## 4. Discovered Critical Defects & Sprint 18 Objectives

1. **WebView2 Runtime Detection GUID Mismatch (Gate 4 blocker):**
   - Previous script queried `{F3017226-9E47-49F8-8CE6-8575C7A28A89}` and had fallback to Edge browser binary.
   - Microsoft official WebView2 Evergreen Runtime GUID is strictly `{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}`.
   - Must inspect `HKLM\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}\pv`, `HKCU\Software\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}\pv`, and 64-bit hive.
   - Must verify non-empty version `!= 0.0.0.0` and eliminate Edge browser fallback.
2. **Windows Application Identity & Shell Integration (Gate 2 blocker):**
   - `AetherIDE.exe` lacked an embedded Windows `.ico` resource and `VERSIONINFO` metadata in the binary itself.
   - Window class used `HICON::default()`, leading to generic Windows taskbar icon.
   - `SetCurrentProcessExplicitAppUserModelID` was not invoked, preventing proper Windows taskbar grouping and pinned shortcut recognition.
3. **Brand Identity & Icon Family (Gate 1 blocker):**
   - No dedicated `branding/` hierarchy existed.
   - Setup executable did not feature custom installer branding (`SetupIconFile`, `WizardImageFile`, `WizardSmallImageFile`).
4. **Download Portal Activation (Gate 7 deliverable):**
   - GitHub Releases currently holds no published assets for `v0.1.0-beta.2`.
   - The user has explicitly authorized building the installer locally and publishing the release assets to GitHub so that download links are live and verifiable.

---

## 5. Baseline Toolchain Status

- **Cargo Format:** `cargo fmt --manifest-path rust/Cargo.toml --all -- --check` -> **0 diffs** (PASSED)
- **Cargo Clippy:** `cargo clippy --manifest-path rust/Cargo.toml --workspace --all-targets -- -D warnings` -> **0 warnings** (PASSED)
- **TypeScript:** `npm --prefix apps/desktop run build` -> **0 errors** (PASSED)
- **Rust Tests:** 152 passed, 0 failed, 0 ignored in Sprint 17.

**Gate 0 Status:** COMPLETED. Ready to proceed to Gate 1 (Brand Identity) and Gate 2 (Windows Application Identity).
