# AetherIDE Sprint 16 — Controlled Pilot Invitation Drafts

**Product:** AetherIDE  
**Release Target:** `v0.1.0-beta.2`  
**Status:** DRAFT STAGED — NOT ISSUED (AWAITING USER AUTHORIZATION)  
**Security Notice:** Under Sprint 16 Governance Rules, no external invitations may be transmitted until the user explicitly authorizes pilot dispatch.

---

## 1. Pilot Cohort Roster & Platform Allocation

The controlled external developer pilot is designed for a focused cohort of 3 to 5 senior software engineers across distinct host operating systems:

| Pilot Candidate | Role / Background | Target Platform | Primary Evaluation Focus |
| :--- | :--- | :--- | :--- |
| **Candidate 1 (Win-Dev)** | Senior Systems / Rust Engineer | Windows 11 x64 | Win32/WebView2 stability, ConPTY terminal, DPAPI vault |
| **Candidate 2 (Linux-Dev)** | Open Source / Backend Go/Rust Lead | Ubuntu 24.04 LTS x64 | GTK3/WebKitGTK integration, DBus Secret Service, glibc compatibility |
| **Candidate 3 (Mac-Dev)** | Full-Stack TypeScript / Apple Silicon Lead | macOS Sonoma (M2 Max) | Cocoa/WKWebView rendering, Keychain storage, Gatekeeper handling |
| **Candidate 4 (FullStack-Dev)** | Lead Architect (React/Node + Python) | Windows 10 x64 | Multi-language workspace indexing, Low-Resource Mode responsiveness |
| **Candidate 5 (Security-Dev)** | Security Researcher & AppSec Auditor | Fedora 40 x64 | Capability filesystem containment (`cap-std`), Zero-telemetry audit |

---

## 2. Draft Invitation Communications

### Draft 1: Email / Message for Windows Evaluator

**Subject:** [Private Invitation] Evaluating AetherIDE Beta.2 (Native AI-Assisted IDE for Windows)

Hi [Name],

We are inviting you to participate in an exclusive, private evaluation of **AetherIDE `v0.1.0-beta.2`**, our new high-performance, AI-native IDE written in Rust for Windows and cross-platform desktop.

**Why we are asking for your feedback:**  
You have deep experience in high-performance desktop development and native tooling. We want your unfiltered feedback on:
1. Native Windows responsiveness (Win32 + Microsoft Edge WebView2 Evergreen host).
2. ConPTY terminal execution and build throughput.
3. Multi-agent planning, code generation, and our Monaco-based side-by-side diff review workflow.
4. OS-level security: all API keys and session secrets are encrypted via Windows DPAPI, and filesystem operations are sandboxed via capability containment (`cap-std`).

**Zero-Telemetry Guarantee:**  
AetherIDE includes **zero background telemetry or tracking**. Nothing leaves your workstation unless you explicitly export diagnostics or trigger an external AI inference call.

**How to get started:**  
- Download portable ZIP: `AetherIDE-v0.1.0-beta.2-windows-x64.zip` (10.6 MB, standalone portable, no installer required).  
- SHA-256 Checksum: `4C09E483DF3181F6F95961DB2CF9DE2B9D7B2657C28CB40622B790ADA2A7A27A`  
- Onboarding Guide: Attached / Linked in the private repository.

We would love your initial impressions within 3–5 days of hands-on use. If you encounter any bugs, our core team is on standby to triage them immediately.

Thank you for helping us shape the future of native developer environments!

Warm regards,  
The AetherIDE Engineering Team

---

### Draft 2: Email / Message for Linux Evaluator

**Subject:** [Private Invitation] AetherIDE Beta.2 Linux Pilot (GTK3 + WebKitGTK + Rust Core)

Hi [Name],

We would love your hands-on perspective for the private pilot of **AetherIDE `v0.1.0-beta.2`** on Linux.

AetherIDE pairs a lightweight, headless Rust daemon (`aether-daemon`) with a native GTK3/WebKitGTK host. It features an integrated Project Brain with SQLite FTS5 symbol indexing, pseudo-terminals, and local LLM discovery (Ollama/LM Studio) alongside standard cloud providers.

**Key areas we want you to test:**  
- Package extraction and native launch on Linux x86_64 (`tar -xzvf`).
- FreeDesktop Secret Service integration via DBus for credential security.
- High-throughput terminal streaming and compilation logs in xterm.js.
- Capability-based filesystem containment (`cap-std`) against directory escapes.

**Privacy & Security:**  
Zero tracking, zero analytics, zero background phone-home. The application can run in a 100% air-gapped environment with local Ollama models.

**Package Information:**  
- Package: `AetherIDE-v0.1.0-beta.2-linux-x64.tar.gz` (12.5 MB)  
- SHA-256: `615F6403092792020837D6D8AC0F82E2D10D5CFC7019AAB79F3D1DA9F9D67FFC`  

Looking forward to your critical feedback!

Best,  
The AetherIDE Engineering Team

---

### Draft 3: Email / Message for macOS (Apple Silicon) Evaluator

**Subject:** [Private Invitation] AetherIDE Beta.2 on macOS ARM64 (Cocoa + WKWebView)

Hi [Name],

We’re excited to invite you to test **AetherIDE `v0.1.0-beta.2`** natively on Apple Silicon (M1/M2/M3/M4).

AetherIDE is built from the ground up to avoid heavy Electron bloat. Our macOS desktop host is written in native Cocoa/AppKit with hardware-accelerated WKWebView and Metal rendering, consuming under 120MB of RAM under active load.

**What to expect:**  
- Instant launch times and sub-5ms symbol lookups in large projects.
- Native Apple Keychain credential security (`Security.framework`).
- Complete developer-in-the-loop AI agent workflows: interactive diffs, rollback checkpoints, and zero unexpected disk modifications.

**Note on Gatekeeper:**  
As this is a private beta build without commercial Apple Notarization, a simple `xattr -cr AetherIDE.app` after extraction bypasses Gatekeeper quarantine.

**Package Information:**  
- Package: `AetherIDE-v0.1.0-beta.2-macos-arm64.tar.gz` (8.88 MB)  
- SHA-256: `64C2A6A6D702528FE0DB4C0C9CDB62D4B883296997B1DA99DE38C60FB353F236`  

We appreciate your time and technical insights!

Best regards,  
The AetherIDE Engineering Team

---

## 3. Transmission Authorization Checklist

Before sending any of the drafts above, the following release gates must be formally satisfied:
- [x] Code and artifact hashes verified against CI run `41235513974`.
- [x] All 152 automated tests passing with zero Clippy warnings.
- [x] Desktop hosts verified across Windows, Linux, and macOS.
- [ ] User gives explicit approval in chat: *"Approved to issue pilot invitations."*

**Current Status:** **HELD IN STAGING.**
