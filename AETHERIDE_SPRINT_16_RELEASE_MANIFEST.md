# AetherIDE Sprint 16 — Cryptographic Release Manifest & SHA256SUMS

## 1. Release Identification

- **Product:** AetherIDE
- **Version:** `v0.1.0-beta.2`
- **Release Channel:** Controlled External Developer Pilot (Draft Prerelease)
- **Source Commit:** `a2e40deb0e7fd3bbfba742ad005269ff05b4d22e` (Verified `origin/main`)
- **CI Workflow Run ID:** `41235513974`
- **Build Timestamp:** October 31, 2026 19:50:22 UTC
- **Publication Governance:** **STAGED DRAFT — UNPUBLISHED** (Awaiting explicit user authorization)

---

## 2. Package Checksums (`SHA256SUMS.txt`)

```text
4C09E483DF3181F6F95961DB2CF9DE2B9D7B2657C28CB40622B790ADA2A7A27A  AetherIDE-v0.1.0-beta.2-windows-x64.zip
615F6403092792020837D6D8AC0F82E2D10D5CFC7019AAB79F3D1DA9F9D67FFC  AetherIDE-v0.1.0-beta.2-linux-x64.tar.gz
64C2A6A6D702528FE0DB4C0C9CDB62D4B883296997B1DA99DE38C60FB353F236  AetherIDE-v0.1.0-beta.2-macos-arm64.tar.gz
```

---

## 3. Structured JSON Manifest (`RELEASE_MANIFEST.json`)

```json
{
  "product": "AetherIDE",
  "version": "0.1.0-beta.2",
  "source_commit": "a2e40deb0e7fd3bbfba742ad005269ff05b4d22e",
  "ci_workflow_run_id": 41235513974,
  "build_timestamp": "2026-10-31T19:50:22Z",
  "release_type": "developer_preview",
  "packages": [
    {
      "platform": "windows",
      "architecture": "x86_64",
      "package_type": "zip",
      "file_name": "AetherIDE-v0.1.0-beta.2-windows-x64.zip",
      "file_size_bytes": 10619788,
      "inner_sha256": "4C09E483DF3181F6F95961DB2CF9DE2B9D7B2657C28CB40622B790ADA2A7A27A",
      "github_artifact_name": "AetherIDE-v0.1.0-beta.2-windows-x64",
      "github_artifact_id": 13056140499,
      "github_artifact_digest": "sha256:62acbf3ce145b0762664fd812ce816039dc485d007bd0b07673ea975352a1cc2",
      "signing_status": "unsigned",
      "notarization_status": "not_applicable",
      "minimum_os": "Windows 10 Build 18362 (1903+) or Windows 11",
      "dependencies": ["Microsoft Edge WebView2 Evergreen Runtime", "MSVC 2015-2022 Redistributable (x64)"]
    },
    {
      "platform": "linux",
      "architecture": "x86_64",
      "package_type": "tar.gz",
      "file_name": "AetherIDE-v0.1.0-beta.2-linux-x64.tar.gz",
      "file_size_bytes": 12538961,
      "inner_sha256": "615F6403092792020837D6D8AC0F82E2D10D5CFC7019AAB79F3D1DA9F9D67FFC",
      "github_artifact_name": "AetherIDE-v0.1.0-beta.2-linux-x64",
      "github_artifact_id": 13055681086,
      "github_artifact_digest": "sha256:ddd2c9eb8e6ac3ac79551ca3784f07c94f1dbd1c925df82c49474e8c42506b30",
      "signing_status": "unsigned",
      "notarization_status": "not_applicable",
      "minimum_os": "Ubuntu 22.04 LTS, Fedora 38+, Arch (glibc 2.35+)",
      "dependencies": ["glibc >= 2.35", "libwebkit2gtk-4.1-0", "libsecret-1-0", "libgtk-3-0"]
    },
    {
      "platform": "macos",
      "architecture": "arm64",
      "package_type": "tar.gz",
      "file_name": "AetherIDE-v0.1.0-beta.2-macos-arm64.tar.gz",
      "file_size_bytes": 8889706,
      "inner_sha256": "64C2A6A6D702528FE0DB4C0C9CDB62D4B883296997B1DA99DE38C60FB353F236",
      "github_artifact_name": "AetherIDE-v0.1.0-beta.2-macos-arm64",
      "github_artifact_id": 13055129688,
      "github_artifact_digest": "sha256:dce5ac40758220b9305a6ae7bb54551560abb3029dd0f61fa337b9a8fd1e0274",
      "signing_status": "unsigned",
      "notarization_status": "unnotarized_developer_build",
      "minimum_os": "macOS 12.0 Monterey or higher (Apple Silicon)",
      "dependencies": ["Native Cocoa / WKWebView"]
    }
  ]
}
```
