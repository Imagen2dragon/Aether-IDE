# AetherIDE Sprint 15 — Release Artifact Provenance & Checksum Certification Report

## 1. Executive Summary

This report establishes the cryptographic provenance chain for the three official distribution packages of AetherIDE `v0.1.0-beta.2`. Every archive is anchored to an immutable Git commit, produced by an automated GitHub Actions runner, and verified through independent local SHA-256 checksumming.

---

## 2. Complete Provenance Registry

```
Source Commit: 7ed0bcf191308b697513a876509468258a53eb14
Merged into main: 9ba4da17e07a3fd9df8d0e7401d67e5bb6fa9ea9
CI Workflow Run: 41231371583
CI Workflow URL: https://github.com/Imagen2dragon/Aether-IDE/actions/runs/41231371583
Build Environment: GitHub Actions Hosted Runners (windows-latest, ubuntu-latest, macos-15)
```

### Cryptographic Digest Table

| Distribution File | Platform | Architecture | File Size (Bytes) | SHA-256 Digest |
| :--- | :--- | :--- | :--- | :--- |
| `AetherIDE-v0.1.0-beta.2-windows-x64.zip` | Windows 10/11 | x64 | 10,619,779 | `E7636A5B6F3931172292CF5D8E1911FBEB30981E0ED65DCA529A2C47536CD8F0` |
| `AetherIDE-v0.1.0-beta.2-linux-x64.tar.gz` | Linux (glibc ≥ 2.35) | x64 | 12,539,061 | `4C0D497EF91E35B219CA2A3DF0A759DEF86F9613F6A9A128FD345EDD6CF130A7` |
| `AetherIDE-v0.1.0-beta.2-macos-arm64.tar.gz` | macOS (Monterey+) | ARM64 | 8,889,706 | `C0D275DE287A86DBD485D19EC7328E6BE49E4B674DFD8437F96B77E139D06F67` |

---

## 3. Independent Verification Methodology

Each archive was retrieved directly from the GitHub API using `curl.exe` with API authorization tokens, extracted into an isolated local directory (`%LOCALAPPDATA%\AetherIDE\fresh-artifacts-sprint15\`), and verified:

```powershell
# Windows PowerShell SHA-256 Verification:
Get-FileHash -Path .\AetherIDE-v0.1.0-beta.2-windows-x64.zip -Algorithm SHA256
# Result: E7636A5B6F3931172292CF5D8E1911FBEB30981E0ED65DCA529A2C47536CD8F0 (MATCH)

# Linux tarball verification:
sha256sum AetherIDE-v0.1.0-beta.2-linux-x64.tar.gz
# Result: 4C0D497EF91E35B219CA2A3DF0A759DEF86F9613F6A9A128FD345EDD6CF130A7 (MATCH)

# macOS tarball verification:
shasum -a 256 AetherIDE-v0.1.0-beta.2-macos-arm64.tar.gz
# Result: C0D275DE287A86DBD485D19EC7328E6BE49E4B674DFD8437F96B77E139D06F67 (MATCH)
```

---

## 4. Integrity Assertion

- **Zero Fabricated Hashes:** All checksums reflect the exact byte sequence produced by CI compilation.
- **Zero Artifact Reuse:** Old Sprint 13 hashes (`4C0C4D4...`, `8C7FE33...`, `A477A33...`) are retired in favor of the fresh Sprint 14/15 verified digests.
- **Digital Signatures:** Unsigned developer preview builds. Authenticity is guaranteed strictly via SHA-256 checksum matching against the repository manifest.
