# AetherIDE Sprint 14 — Known Limitations & Platform Constraints

## 1. Overview

**Target Release:** `v0.1.0-beta.2`  
**Purpose:** Provide full transparency regarding platform constraints, unverified configurations, and known technical boundaries for pilot testers and engineers.

---

## 2. Packaging & Distribution Limitations

### 2.1 Portable Distribution Only
- **Status:** In `v0.1.0-beta.2`, the primary certified distribution format across all platforms is the **standalone portable archive** (`.zip` on Windows, `.tar.gz` on Linux and macOS).
- **Explanation:** Automated setup installers (Windows `.exe` via Inno Setup, Linux AppImage/`.deb`, macOS `.dmg`) are supported in repository packaging scripts, but require specialized native toolchains (`ISCC.exe`, `appimagetool`, `dpkg-deb`, `hdiutil`) that are not present in standard CI environments.
- **Impact:** Users must manually extract the archives to run the application. No automatic system shortcut creation or uninstall registry entries are generated.

---

## 3. Platform-Specific Constraints

### 3.1 macOS Gatekeeper & Code Signing
- **Issue:** macOS pre-release binaries are compiled on GitHub Actions runners without an Apple Developer ID certificate and have not been notarized by Apple.
- **Symptom:** macOS Gatekeeper displays a security dialog indicating: `"AetherIDE.app cannot be opened because it is from an unidentified developer."`
- **Resolution:**
  - Pilot developers must right-click `AetherIDE.app` in Finder and select **Open**, or execute:
    ```bash
    xattr -cr /Applications/AetherIDE.app
    ```
  - Full Apple Developer ID notarization and signed `.dmg` images will be introduced in the GA release candidate cycle.

### 3.2 Linux System Libraries
- **Issue:** AetherIDE leverages native system GTK and WebKit libraries for the desktop shell and Secret Service for credential management.
- **Requirements:**
  - `glibc` ≥ 2.35 (Ubuntu 22.04 LTS+, Fedora 38+, Debian 12+)
  - `libwebkit2gtk-4.1-0` or `libwebkit2gtk-4.0-37`
  - `libsecret-1-0`
- **Workaround on Headless / Minimal Linux:** On headless or server distributions lacking X11/Wayland, the GUI workbench cannot launch. However, `bin/aether-daemon` and `bin/aether-cli` function without graphical dependencies.

### 3.3 Windows Terminal & ConPTY
- **Issue:** Terminal emulation in AetherIDE relies on the Windows Pseudo Console (ConPTY) API.
- **Requirement:** Windows 10 version 1903 (Build 18362) or later, or Windows 11. Legacy Windows 7/8/8.1 and earlier Windows 10 builds are unsupported.

---

## 4. AI Provider & Hardware Boundaries

### 4.1 Local Model Hardware Prerequisites
- Running local models via Ollama or LM Studio requires appropriate hardware:
  - **7B / 8B Parameter Models (e.g., Qwen2.5-Coder-7B, Llama-3.1-8B):** Minimum 8 GB VRAM on a dedicated GPU (Nvidia RTX 3060+) or 16 GB unified memory on Apple Silicon.
  - **CPU-Only Fallback:** If executed entirely on CPU, token generation speeds will be constrained to 3–8 tokens/second depending on processor core count.

### 4.2 Cloud Provider Rate Limits
- When utilizing cloud providers (Anthropic Claude 3.5 Sonnet, OpenAI GPT-4o, Google Gemini 1.5 Pro), users are subject to their respective API rate limits and tier quotas.
- AetherIDE provides automatic exponential backoff retry logic, but excessive multi-agent parallelism on low-tier API keys may result in temporary HTTP 429 throttling.

---

## 5. Summary Matrix

| Limitation Area | Description | Target Closure Milestone |
| :--- | :--- | :--- |
| **macOS Notarization** | Apple Developer ID signature & notarization | Sprint 15 / RC 1 |
| **System Installers** | Automated Inno Setup `.exe`, Debian `.deb`, AppImage | Sprint 15 / RC 1 |
| **Windows ARM64** | Native Windows on ARM64 binary build | Post-v0.1.0 Roadmap |
| **LSP Language Expansion** | Bundled language servers for Go, Python, C++ | `v0.1.0-beta.3` |
