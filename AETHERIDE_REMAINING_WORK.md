# AETHERIDE — REMAINING WORK & SPRINT 04 ROADMAP
## Prioritized Engineering Backlog for Production Staging & General Availability

**Document Type:** Strategic Engineering Backlog  
**Date:** 2026-10-31  
**Project:** AetherIDE  
**Status:** ACTIVE BACKLOG  

---

## 1. Prioritized Backlog (P0 - P2)

### P0 — High Priority / Production Blockers
1. **Monaco Editor Workbench Integration:**
   - Replace the static code display in `apps/desktop/workbench/index.html` with Monaco Editor.
   - Wire Monaco's autocomplete and diagnostics providers directly to the native `lsp-client` via daemon WebSocket.
2. **True Native HWND WebView2 Embedding:**
   - Upgrade `aether-launcher` from external `--app=file:///...` launching to embedded `ICoreWebView2Controller` inside a native Rust-owned `HWND` using the `webview2-com` / `windows` crate.
   - Establish two-way Win32 `WebMessageReceived` / `PostWebMessageAsJson` COM bridge.
3. **Automated EAS Mobile Cloud Build Execution:**
   - Configure EAS API token and project ID to trigger live Android preview APK and iOS build jobs directly from the Mobile Studio dashboard.

### P1 — Medium Priority / Feature Parity
4. **macOS CI Runner & Apple Code Signing:**
   - Provision an authorized macOS runner to compile `apps/ios-companion/AetherCompanion.xcodeproj` into a signed `.ipa` archive and deploy to TestFlight.
5. **Code - OSS Shell Migration Roadmap:**
   - Evaluate packaging AetherIDE's native Rust daemon, 14 AI agents, and mobile studio as a first-party extension bundle (`aether-core`) hosted on an open-source Code - OSS distribution.
6. **Additional Language Server Bundling:**
   - Bundle or provide automated one-click installation for `typescript-language-server` and `pyright-langserver` directly inside the Windows distribution package.

### P2 — Polish & Optimization
7. **Authenticode Code Signing:**
   - Sign `AetherIDE.exe`, `aether-daemon.exe`, and installer packages with a Windows Hardware Developer Program Authenticode certificate to prevent SmartScreen warnings.
8. **Inno Setup / WiX Installer:**
   - Generate a single-click `.exe` installer with desktop shortcut and start menu registration alongside the portable `.zip`.
