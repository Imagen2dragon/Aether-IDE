# AetherIDE Sprint 15 — Known Limitations & Platform Constraints

## 1. Overview

In accordance with Sprint 15 engineering governance, this document provides complete, transparent disclosure of all platform constraints, unverified configurations, and known technical boundaries in AetherIDE `v0.1.0-beta.2`.

---

## 2. Packaging & Installer Boundaries

### 2.1 Portable Package Only
- **Current State:** The official certified distribution formats are **standalone portable archives** (`.zip` on Windows, `.tar.gz` on Linux and macOS).
- **Reason:** System installers (Inno Setup `.exe`, Debian `.deb`, AppImage, macOS `.dmg`) rely on specialized packaging tools (`ISCC.exe`, `dpkg-deb`, `appimagetool`, `hdiutil`) that are not present in standard CI runner environments.
- **Developer Impact:** Users manually extract the archive to run the IDE. No desktop shortcuts or Add/Remove Programs registry entries are created automatically.

---

## 3. Platform-Specific Constraints

### 3.1 macOS Gatekeeper & Code Signing
- **Limitation:** Pre-release binaries are built on GitHub Actions without an Apple Developer ID certificate and are not notarized by Apple.
- **Symptom:** macOS Gatekeeper displays an alert on first launch: `"AetherIDE.app cannot be opened because it is from an unidentified developer."`
- **Workaround:**
  - Right-click `AetherIDE.app` in Finder and select **Open** &rarr; **Open**.
  - Or run: `xattr -cr /Applications/AetherIDE.app`
- **Target Resolution:** Signed and notarized builds will be provided in the Release Candidate cycle.

### 3.2 Linux System Dependencies
- **Requirements:**
  - `glibc` ≥ 2.35 (Ubuntu 22.04 LTS+, Fedora 38+, Debian 12+)
  - `libwebkit2gtk-4.1-0` or `libwebkit2gtk-4.0-37`
  - `libsecret-1-0`
- **Headless Servers:** On server installations lacking X11/Wayland, the GUI workbench cannot mount. Developers must use `bin/aether-daemon` and `bin/aether-cli` headlessly.

### 3.3 Windows Console Requirement
- **Requirement:** Windows 10 version 1903 (Build 18362) or later, or Windows 11, due to dependency on Windows Pseudo Console (ConPTY). Legacy Windows 7, 8, and early Windows 10 builds are unsupported.

---

## 4. AI Provider & Hardware Limits

### 4.1 Local Offline Models
- **VRAM Requirements:** Running 7B or 8B parameter models (Qwen2.5-Coder-7B, Llama-3.1-8B) with Ollama or LM Studio requires at least 8 GB of GPU VRAM (Nvidia RTX 3060+) or 16 GB unified memory on Apple Silicon.
- **CPU Fallback:** Executing on CPU-only machines will yield slow inference speeds (3–8 tokens/second).

### 4.2 Cloud Provider Quotas
- Users connecting Anthropic, OpenAI, or Google Gemini keys are subject to standard account rate limits. AetherIDE implements exponential backoff retries, but high agent parallelism may trigger temporary HTTP 429 throttling.

---

## 5. Summary Status Matrix

| Subsystem | Beta.2 Status | Planned Enhancement |
| :--- | :--- | :--- |
| **macOS Notarization** | Unnotarized Developer Build | Apple Notary Service integration (RC 1) |
| **Setup Installers** | Portable Archives Only | Inno Setup & AppImage pipelines (Sprint 16) |
| **Windows ARM64** | Emulated via x64 | Native Windows ARM64 build |
| **LSP Bundling** | External rust-analyzer / tsserver | Auto-downloading language server manager |
