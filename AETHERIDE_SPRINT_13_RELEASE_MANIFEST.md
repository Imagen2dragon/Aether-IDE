# AetherIDE Sprint 13 — Release Manifest & Checksums

## 1. Release Specification

- **Product:** AetherIDE
- **Release Version:** `v0.1.0-beta.2`
- **Release Channel:** Controlled Developer Beta
- **Baseline Git Commit:** `3e9c316d1a91f2cd2fd02d6641de075449a991bd`
- **Development Branch:** `feat/sprint-13-controlled-beta-pilot`
- **Build Date:** October 31, 2026
- **Architecture Workspace:** 24 Rust crates + TypeScript desktop workbench

---

## 2. Release Package Manifest

| Package Filename | Target Platform | Format | Size (Bytes) | SHA-256 Digest |
| :--- | :--- | :--- | :--- | :--- |
| `AetherIDE-v0.1.0-beta.2-windows-x64.zip` | Windows 10/11 x64 | Portable ZIP | 10,323,379 | `107d6a87d4cb22fe73d29e8c1af711984b4c611d0289431b1d3a001f441a3aab` |
| `AetherIDE-v0.1.0-beta.2-linux-x64.tar.gz` | Linux x64 (glibc 2.31+) | Compressed Tarball | *CI Built* | *Recorded in CI Build Artifact* |
| `AetherIDE-v0.1.0-beta.2-macos-arm64.tar.gz` | macOS 12+ (Apple Silicon) | Compressed Tarball | *CI Built* | *Recorded in CI Build Artifact* |

---

## 3. Machine-Readable Manifest (`RELEASE_MANIFEST.json`)

```json
{
  "release": "v0.1.0-beta.2",
  "generated_at": "2026-10-31T20:50:51Z",
  "artifacts": [
    {
      "filename": "AetherIDE-v0.1.0-beta.2-windows-x64.zip",
      "platform": "windows-x64",
      "size_bytes": 10323379,
      "sha256": "107d6a87d4cb22fe73d29e8c1af711984b4c611d0289431b1d3a001f441a3aab"
    }
  ]
}
```

---

## 4. Verification Instructions

To verify the integrity of the downloaded package on Windows using PowerShell:
```powershell
Get-FileHash -Algorithm SHA256 .\AetherIDE-v0.1.0-beta.2-windows-x64.zip
# Compare output against:
# 107D6A87D4CB22FE73D29E8C1AF711984B4C611D0289431B1D3A001F441A3AAB
```

To verify on Linux / macOS using POSIX tools:
```bash
sha256sum -c SHA256SUMS.txt
# or on macOS:
shasum -a 256 -c SHA256SUMS.txt
```
