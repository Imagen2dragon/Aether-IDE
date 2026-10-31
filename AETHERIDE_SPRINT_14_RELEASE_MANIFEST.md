# AetherIDE Sprint 14 — Authoritative Release Manifest & Checksums
## Version: v0.1.0-beta.2

**Release Channel:** Controlled Developer Pilot  
**Source Commit:** `e507d77bc43556a280664baa09698e2eabbac555`  
**CI Build Run:** `41227347389`  
**Generated Date:** October 31, 2026  

---

## 1. Concrete Distribution Artifacts

| Artifact Filename | Platform | Arch | Format | Package Size | SHA-256 Digest |
| :--- | :--- | :--- | :--- | :--- | :--- |
| `AetherIDE-v0.1.0-beta.2-windows-x64.zip` | Windows 10/11 | `x86_64` | Portable ZIP | 10,619,775 bytes | `4c0c4d479e7b3fbd55b9fbe771a2ed7bb8f275565e1a686e41851e25a2f01cbd` |
| `AetherIDE-v0.1.0-beta.2-linux-x64.tar.gz` | Linux (glibc 2.31+) | `x86_64` | Portable Tarball | 12,538,966 bytes | `8c7fe33205779fecc6655b954196dbcf7c774122cc79b936710147de8f055c32` |
| `AetherIDE-v0.1.0-beta.2-macos-arm64.tar.gz` | macOS 12.0+ | `arm64` | App Bundle Tarball | 8,889,704 bytes | `a477a333c40a6fdc4c72f4588bfcca79c078a7ab224e597177fed3fed82d4e10` |

---

## 2. Checksum Manifest (`SHA256SUMS.txt`)

```text
4c0c4d479e7b3fbd55b9fbe771a2ed7bb8f275565e1a686e41851e25a2f01cbd  AetherIDE-v0.1.0-beta.2-windows-x64.zip
8c7fe33205779fecc6655b954196dbcf7c774122cc79b936710147de8f055c32  AetherIDE-v0.1.0-beta.2-linux-x64.tar.gz
a477a333c40a6fdc4c72f4588bfcca79c078a7ab224e597177fed3fed82d4e10  AetherIDE-v0.1.0-beta.2-macos-arm64.tar.gz
```

---

## 3. Machine-Readable Metadata (`RELEASE_MANIFEST.json`)

```json
{
  "product": "AetherIDE",
  "version": "0.1.0-beta.2",
  "channel": "controlled-developer-pilot",
  "commit": "e507d77bc43556a280664baa09698e2eabbac555",
  "ci_run_id": "41227347389",
  "generated_at": "2026-10-31T21:40:00Z",
  "packages": [
    {
      "platform": "windows",
      "architecture": "x86_64",
      "package_type": "portable_zip",
      "filename": "AetherIDE-v0.1.0-beta.2-windows-x64.zip",
      "size_bytes": 10619775,
      "sha256": "4c0c4d479e7b3fbd55b9fbe771a2ed7bb8f275565e1a686e41851e25a2f01cbd",
      "signing_status": "unsigned",
      "minimum_os": "Windows 10 (21H2+)",
      "runtime_dependencies": ["Microsoft.WebView2"]
    },
    {
      "platform": "linux",
      "architecture": "x86_64",
      "package_type": "portable_tarball",
      "filename": "AetherIDE-v0.1.0-beta.2-linux-x64.tar.gz",
      "size_bytes": 12538966,
      "sha256": "8c7fe33205779fecc6655b954196dbcf7c774122cc79b936710147de8f055c32",
      "signing_status": "unsigned",
      "minimum_os": "glibc 2.31+ (Ubuntu 20.04+, Debian 11+, Fedora 36+)",
      "runtime_dependencies": ["libgtk-3-0", "libwebkit2gtk-4.1-0 | libwebkit2gtk-4.0-37", "gnome-keyring | ksecretservice"]
    },
    {
      "platform": "macos",
      "architecture": "arm64",
      "package_type": "portable_tarball",
      "filename": "AetherIDE-v0.1.0-beta.2-macos-arm64.tar.gz",
      "size_bytes": 8889704,
      "sha256": "a477a333c40a6fdc4c72f4588bfcca79c078a7ab224e597177fed3fed82d4e10",
      "signing_status": "unsigned",
      "notarization_status": "unnotarized",
      "minimum_os": "macOS 12.0 Monterey",
      "runtime_dependencies": ["Apple Silicon (M1/M2/M3/M4)"]
    }
  ]
}
```

---

## 4. Verification Protocol

### Windows (PowerShell)
```powershell
Get-FileHash -Algorithm SHA256 .\AetherIDE-v0.1.0-beta.2-windows-x64.zip
# Must match: 4C0C4D479E7B3FBD55B9FBE771A2ED7BB8F275565E1A686E41851E25A2F01CBD
```

### Linux / POSIX
```bash
sha256sum -c SHA256SUMS.txt
```
