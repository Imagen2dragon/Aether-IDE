# AetherIDE — Deployment & Distribution Guide

## 1. Distribution Channels
- **Windows Desktop:** Single standalone executable installer (`AetherIDESetup.exe`) and portable ZIP archive (`aether-ide-windows-x64.zip`).
- **iOS/iPadOS Companion:** TestFlight external testing group and App Store release.
- **Web IDE:** Docker container image (`aetheride/web-workspace:latest`) deployed on AWS/GCP/Kubernetes with authenticated TLS ingress.

---

## 2. Windows Deployment Checklist
- [x] Rust binaries compiled in `--release` mode.
- [x] Native DLL dependencies bundled (`vcruntime140.dll` where required).
- [x] Extension TypeScript code bundled and minified.
- [x] SQLite database schema auto-migrates cleanly on first boot.
- [x] SHA256 verification hashes generated in `CHECKSUMS-SHA256.txt`.
- [x] Windows Defender SmartScreen compatibility tested.
