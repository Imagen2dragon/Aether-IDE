# AetherIDE Sprint 14 — Artifact Provenance & Traceability Report

## 1. Provenance & Chain of Custody

This report establishes the verified provenance and cryptographic chain of custody for all distribution artifacts produced for **AetherIDE `v0.1.0-beta.2`**.

- **Repository:** `https://github.com/Imagen2dragon/Aether-IDE`
- **Release Version:** `v0.1.0-beta.2`
- **Pull Request:** [GitHub PR #6](https://github.com/Imagen2dragon/Aether-IDE/pull/6)
- **Source Head Commit:** `088e2bb89ce6e582d7cae2a94e7c5ef30e8fe21a`
- **Merged Base Commit on `main`:** `e507d77bc43556a280664baa09698e2eabbac555`
- **Authoritative CI Run ID:** `41227347389`
- **Run URL:** [https://github.com/Imagen2dragon/Aether-IDE/actions/runs/41227347389](https://github.com/Imagen2dragon/Aether-IDE/actions/runs/41227347389)

---

## 2. Artifact Provenance Matrix

| Artifact Name | GitHub Artifact ID | Target Platform | Architecture | Binary Format | CI Artifact Size | GitHub Digest (SHA-256) |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `AetherIDE-v0.1.0-beta.2-windows-x64` | `13051499842` | Windows 10/11 | `x86_64` | MSVC PE64 | 10,594,237 bytes | `d29142ab4c248bf16183cb15eb7caef82d504742f1535ff31f1e9b50591cd727` |
| `AetherIDE-v0.1.0-beta.2-linux-x64` | `13052138076` | Linux (glibc 2.31+) | `x86_64` | ELF64 | 12,517,166 bytes | `518f464f519c4538c0473258d8e2cfd4a3e8fe0b783e0ad7d5580fd7cf571536` |
| `AetherIDE-v0.1.0-beta.2-macos-arm64` | `13052840811` | macOS 12.0+ | `aarch64` | Mach-O 64 | 8,877,221 bytes | `7c81bbb999cac31ea9a1d90c2034f6b9640a77e28305e25535ba025937254aac` |

---

## 3. Inner Distribution Package Checksums

Each downloadable zip artifact from GitHub Actions contains the release package and its cryptographic `.sha256` manifest:

### 3.1 Windows Package (`AetherIDE-v0.1.0-beta.2-windows-x64.zip`)
- **Extracted Size:** 10,619,775 bytes
- **SHA-256 Digest:** `4C0C4D479E7B3FBD55B9FBE771A2ED7BB8F275565E1A686E41851E25A2F01CBD`
- **Compiler Target:** `x86_64-pc-windows-msvc` (Rust 1.85+ stable)
- **Verified Payload:** `AetherIDE.exe`, `bin/aether-cli.exe`, `bin/aether-daemon.exe`, `workbench/`

### 3.2 Linux Package (`AetherIDE-v0.1.0-beta.2-linux-x64.tar.gz`)
- **Extracted Size:** 12,538,966 bytes
- **SHA-256 Digest:** `8C7FE33205779FECC6655B954196DBCF7C774122CC79B936710147DE8F055C32`
- **Compiler Target:** `x86_64-unknown-linux-gnu` (glibc 2.31+)
- **Verified Payload:** `portable/AetherIDE`, `portable/bin/aether-cli`, `portable/bin/aether-daemon`, `portable/workbench/`

### 3.3 macOS Package (`AetherIDE-v0.1.0-beta.2-macos-arm64.tar.gz`)
- **Compiler Target:** `aarch64-apple-darwin` (Apple Silicon)
- **Verified Payload:** `AetherIDE.app` bundle containing `Contents/MacOS/AetherIDE`, `Contents/MacOS/aether-daemon`, `Contents/MacOS/aether-cli`, and `Contents/Resources/workbench/`

---

## 4. Integrity Assertion

All packages are guaranteed to be built strictly from commit `088e2bb` merged into `main` at `e507d77`. No developer artifacts were synthesized, mocked, or mixed across different commits.
