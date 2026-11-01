# AetherIDE Sprint 17 — Known Limitations & Operational Boundaries

**Product:** AetherIDE  
**Release Target:** `v0.1.0-beta.2`  
**Classification:** Beta Release Operational Disclosure  
**Status:** AUDITED & STAGED  

---

## 1. Executive Summary

This document enumerates the known operational boundaries, certificate statuses, and platform constraints of AetherIDE `v0.1.0-beta.2`. Full transparency regarding these parameters ensures that developers, system evaluators, and pilot participants understand initial launch requirements and future milestone targets.

---

## 2. Platform Nuances & Code Signing Disclosures

### 2.1 Windows Authenticode & SmartScreen
- **Current Status:** `AetherIDE-Setup-0.1.0-beta.2-x64.exe` is an explicitly **unsigned beta installer**. AetherIDE does not embed private corporate Authenticode signing certificates in public repositories.
- **Impact:** On Windows 10 and Windows 11, Microsoft Defender SmartScreen will display an alert (*"Windows protected your PC — Microsoft Defender SmartScreen prevented an unrecognized app from starting"*).
- **Remediation:** Click **More info**, then click **Run anyway**. Once sufficient download reputation is established or an EV code signing certificate is configured in production CI, SmartScreen prompts will cease.

### 2.2 macOS Gatekeeper & Apple Notarization
- **Current Status:** `AetherIDE.app` is compiled natively for Apple Silicon, but is not signed with a paid Apple Developer ID or notarized via Apple Notary Service.
- **Impact:** macOS Monterey, Ventura, Sonoma, and Sequoia will block opening on first launch.
- **Remediation:** Run `xattr -cr AetherIDE.app` in Terminal or right-click `AetherIDE.app` in Finder and select **Open**.

### 2.3 Linux Shared Library Prerequisites
- **glibc Version:** Compiled on Ubuntu 22.04 runners against `glibc 2.35`. Does not execute on older distributions (CentOS 7, RHEL 8, Debian 10) without containerization.
- **Runtime Packages:** Requires `libgtk-3-0`, `libwebkit2gtk-4.1-0` (or `4.0`), and `libsecret-1-0` installed on the host.

---

## 3. Subsystem Boundaries

| Subsystem | Current Beta.2 State | Planned Horizon |
| :--- | :--- | :--- |
| **Local LLM Inference** | Supported via external Ollama / LM Studio on `localhost` | Native quantized GGUF runner (post-1.0) |
| **Language Support (LSP)** | Built-in for Rust and TypeScript; Python/Go manual path | One-click language server downloader (Beta.3) |
| **Remote Workspaces** | Local and mounted network filesystems only | Remote SSH & Docker Dev Containers (post-Beta) |
| **Multi-Window** | Single workbench window with multi-tab split editors | Detachable floating windows (post-Beta) |

---

## 4. Verification Verdict

All limitations are clearly documented in the onboarding documentation and public download portal. None affect local code integrity, sandboxing security, or application stability.
