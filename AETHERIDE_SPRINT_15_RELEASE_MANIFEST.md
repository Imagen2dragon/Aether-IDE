# AetherIDE Sprint 15 — Cryptographic Release Manifest & SHA256SUMS

## 1. Official Release Metadata

- **Product:** AetherIDE
- **Version:** `v0.1.0-beta.2`
- **Release Channel:** Controlled Developer Pilot (Prerelease)
- **Source Commit:** `7ed0bcf191308b697513a876509468258a53eb14` (Merged into `main` as `9ba4da17e07a3fd9df8d0e7401d67e5bb6fa9ea9`)
- **CI Workflow Run ID:** `41231371583`
- **Build Timestamp:** October 31, 2026 18:59:03 UTC
- **Publication Gate:** **STAGED DRAFT (UNPUBLISHED)**

---

## 2. Release Artifacts Matrix

| Operating System | Architecture | Package Format | File Name | Size (Bytes) | SHA-256 Digest |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **Windows** | x86_64 | Portable ZIP | `AetherIDE-v0.1.0-beta.2-windows-x64.zip` | 10,619,779 | `E7636A5B6F3931172292CF5D8E1911FBEB30981E0ED65DCA529A2C47536CD8F0` |
| **Linux** | x86_64 | Portable TAR.GZ | `AetherIDE-v0.1.0-beta.2-linux-x64.tar.gz` | 12,539,061 | `4C0D497EF91E35B219CA2A3DF0A759DEF86F9613F6A9A128FD345EDD6CF130A7` |
| **macOS** | Apple Silicon (arm64) | Portable TAR.GZ | `AetherIDE-v0.1.0-beta.2-macos-arm64.tar.gz` | 8,889,706 | `C0D275DE287A86DBD485D19EC7328E6BE49E4B674DFD8437F96B77E139D06F67` |

---

## 3. Cryptographic Verification Manifest (`SHA256SUMS.txt`)

```text
E7636A5B6F3931172292CF5D8E1911FBEB30981E0ED65DCA529A2C47536CD8F0  AetherIDE-v0.1.0-beta.2-windows-x64.zip
4C0D497EF91E35B219CA2A3DF0A759DEF86F9613F6A9A128FD345EDD6CF130A7  AetherIDE-v0.1.0-beta.2-linux-x64.tar.gz
C0D275DE287A86DBD485D19EC7328E6BE49E4B674DFD8437F96B77E139D06F67  AetherIDE-v0.1.0-beta.2-macos-arm64.tar.gz
```

---

## 4. Structured JSON Release Manifest (`RELEASE_MANIFEST.json`)

```json
{
  "product": "AetherIDE",
  "version": "0.1.0-beta.2",
  "source_commit": "7ed0bcf191308b697513a876509468258a53eb14",
  "ci_workflow_run_id": 41231371583,
  "build_timestamp": "2026-10-31T18:59:03Z",
  "packages": [
    {
      "os": "windows",
      "arch": "x86_64",
      "format": "zip",
      "file_name": "AetherIDE-v0.1.0-beta.2-windows-x64.zip",
      "size_bytes": 10619779,
      "sha256": "E7636A5B6F3931172292CF5D8E1911FBEB30981E0ED65DCA529A2C47536CD8F0",
      "signed": false,
      "notarized": false,
      "min_os_version": "Windows 10 Build 18362 (1903+)",
      "runtime_dependencies": ["Microsoft Edge WebView2 Evergreen Runtime", "MSVC 2015-2022 Redistributable"]
    },
    {
      "os": "linux",
      "arch": "x86_64",
      "format": "tar.gz",
      "file_name": "AetherIDE-v0.1.0-beta.2-linux-x64.tar.gz",
      "size_bytes": 12539061,
      "sha256": "4C0D497EF91E35B219CA2A3DF0A759DEF86F9613F6A9A128FD345EDD6CF130A7",
      "signed": false,
      "notarized": false,
      "min_os_version": "Ubuntu 22.04 LTS / Fedora 38+ (glibc 2.35+)",
      "runtime_dependencies": ["glibc >= 2.35", "libwebkit2gtk-4.1-0", "libsecret-1-0"]
    },
    {
      "os": "macos",
      "arch": "arm64",
      "format": "tar.gz",
      "file_name": "AetherIDE-v0.1.0-beta.2-macos-arm64.tar.gz",
      "size_bytes": 8889706,
      "sha256": "C0D275DE287A86DBD485D19EC7328E6BE49E4B674DFD8437F96B77E139D06F67",
      "signed": false,
      "notarized": false,
      "min_os_version": "macOS 12.0 Monterey",
      "runtime_dependencies": ["Native Cocoa / WKWebView"]
    }
  ]
}
```
