# AetherIDE Sprint 17 — Windows Installer Lifecycle Validation Report

**Product:** AetherIDE  
**Release Target:** `v0.1.0-beta.2`  
**Artifact Tested:** `dist/AetherIDE-Setup-0.1.0-beta.2-x64.exe` (SHA-256: `B2DEE0AE658A1860ABC7F1FA0FBEDF9F4B838B84B73D108BD82D8E55FEB99173`)  
**Test Machine:** Windows 11 x64 (Build 26200), WebView2 Evergreen 156.0.4314.50  
**Test Location:** `%LOCALAPPDATA%\AetherIDE\installer-test-sprint17`  
**Status:** ALL ACCEPTANCE TESTS PASSED (100%)  

---

## 1. Test Scenarios & Acceptance Matrix

In accordance with Section 9 of the Sprint 17 Master Engineering Directive, the installer was subjected to seven mandatory lifecycle tests:

| Test ID | Test Scenario | Command Executed | Expected Outcome | Actual Result | Verdict |
| :---: | :--- | :--- | :--- | :--- | :---: |
| **WIN-01** | Compile Setup EXE | `.\scripts\package-windows.ps1` | `AetherIDE-Setup-0.1.0-beta.2-x64.exe` generated | Built 8,374,833 byte installer | **PASS** |
| **WIN-02** | Fresh Installation | `Setup.exe /VERYSILENT /DIR="..."` | Extracted all binaries and assets | Complete payload extracted into test directory | **PASS** |
| **WIN-03** | Application Launch | `AetherIDE.exe doctor` | Application runs cleanly from installed path | Toolchain verified healthy, exit 0 | **PASS** |
| **WIN-04** | WebView2 Detection | Inno Setup `IsWebView2Installed` | Detects Evergreen 156.0 | Successfully confirmed active runtime | **PASS** |
| **WIN-05** | Upgrade Workflow | Re-run setup over existing install | Files replaced, settings preserved | Successfully updated without errors | **PASS** |
| **WIN-06** | Silent Uninstallation | `unins000.exe /VERYSILENT` | App directory purged, user DB kept | Directory deleted, `aether.db` preserved | **PASS** |
| **WIN-07** | CLI PATH Cleanup | Uninstaller PATH hook | User PATH restored to initial state | Semicolon delimiters cleaned | **PASS** |

---

## 2. Detailed Test Execution Evidence

### 2.1 Fresh Installation (WIN-02)
Executed from PowerShell:
```powershell
$testDir = "$env:LOCALAPPDATA\AetherIDE\installer-test-sprint17"
Start-Process -FilePath "dist\AetherIDE-Setup-0.1.0-beta.2-x64.exe" -ArgumentList "/VERYSILENT /SUPPRESSMSGBOXES /NORESTART /DIR=""$testDir""" -Wait
```
**Verification:**
- Directory created: `%LOCALAPPDATA%\AetherIDE\installer-test-sprint17`
- Payload verified: `AetherIDE.exe` (2.99 MB), `bin\aether-daemon.exe` (11.1 MB), `bin\aether-cli.exe` (3.25 MB), `unins000.exe` (4.45 MB), `workbench\` directory with Monaco and assets.
- Total installed footprint: **35.4 MB** (well below the 100 MB budget).

### 2.2 Installed Application Health Verification (WIN-03)
Executed:
```powershell
& "$env:LOCALAPPDATA\AetherIDE\installer-test-sprint17\AetherIDE.exe" doctor
```
**Output Log:**
```text
🔍 Running Aether Development Environment Doctor...

✅ Rust Compiler             | Verified operational: rustc 1.93.1 (01f6ddf75 2026-02-11)
✅ Cargo Package Manager     | Verified operational: cargo 1.93.1 (083ac5135 2025-12-15)
✅ Git SCM                   | Verified operational: git version 2.48.1.windows.1
✅ Node.js Runtime           | Verified operational: v24.11.1
✅ Python Interpreter        | Verified operational: Python 3.12.10

Overall Toolchain Status: HEALTHY
```
Exit code: 0.

### 2.3 Upgrade & File Replacement (WIN-05)
Re-ran installer over the populated installation path.
- Inno Setup replaced outdated binaries with no locked-file errors.
- Verified file timestamps updated correctly.

### 2.4 Clean Uninstallation & Data Protection (WIN-06, SEC-02)
Executed:
```powershell
Start-Process -FilePath "$env:LOCALAPPDATA\AetherIDE\installer-test-sprint17\unins000.exe" -ArgumentList "/VERYSILENT /SUPPRESSMSGBOXES /NORESTART" -Wait
Start-Sleep -Seconds 2
Test-Path "$env:LOCALAPPDATA\AetherIDE\installer-test-sprint17"
```
**Result:**
- `Test-Path` returned `False`: The application installation directory was purged completely.
- User database check: `Test-Path "$env:LOCALAPPDATA\AetherIDE\aether.db"` returned `True`.
- Saved credentials and user workspace caches remained intact.

---

## 3. Verdict

The installer satisfies every operational, functional, and safety criteria. `AetherIDE-Setup-0.1.0-beta.2-x64.exe` is certified ready for distribution.
