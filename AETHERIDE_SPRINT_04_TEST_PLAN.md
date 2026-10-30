# AETHERIDE — SPRINT 04 TEST PLAN
## Monaco IDE Workbench, Win32 HWND WebView2 Host, and Integrated System Verification

**Version:** 1.0.0  
**Author:** QA Director & Systems Test Architect  
**Date:** 2026-10-31  
**Project:** AetherIDE  

---

## 1. Test Strategy & Scope

Sprint 04 tests must validate:
1. **Local Monaco Bundle & Worker Loading:** Zero external CDN network requests; workers (`editor.worker.js`, `ts.worker.js`, `json.worker.js`) initialize cleanly from local files.
2. **Editor Functional Workflows:** Multi-tab document management, file opening, file saving via atomic filesystem writes, dirty-state indicators, keyboard shortcuts (`Ctrl+P`, `Ctrl+Shift+P`, `Ctrl+S`).
3. **Monaco + Rust LSP Bridge:** Auto-completion (`LspCompletion`), hover tooltips (`LspHover`), definition jump (`LspDefinition`), and diagnostics markers (`setModelMarkers`).
4. **Native HWND WebView2 Hosting:** Genuine Win32 `HWND` creation, `ICoreWebView2Controller` parenting, window resizing, DPI scaling, and process tree validation.
5. **Secure Native Message Bridge:** Typed allowlist validation, rejection of unauthorized origins and arbitrary shell injection, token encapsulation.
6. **Non-Regression:** All 60 existing Rust tests across all 24 crates must remain 100% passing.

---

## 2. Acceptance Test Matrix

| Test Suite | Test Case | Expected Outcome | Verification Method |
|:---|:---|:---|:---|
| **Monaco Bundle** | Offline Asset Loading | All Monaco scripts/workers load locally with zero network calls | Inspect network requests / offline run |
| **Monaco Tabs** | Multi-File Model Switching | Switching tabs preserves cursor, scroll position, and undo/redo stacks | DOM state & model persistence test |
| **Monaco File I/O** | Atomic Workspace Save | Pressing `Ctrl+S` writes file via daemon IPC without escaping workspace root | File integrity check on disk |
| **Monaco LSP** | Live Code Completion | Typing inside a Rust file displays suggestions from `rust-analyzer` via daemon | Live autocomplete dropdown test |
| **Monaco Diagnostics** | Diagnostic Marker Rendering | Errors reported by `rust-analyzer` render red squiggly underlines & hover info | Monaco `getModelMarkers` inspection |
| **Win32 Window** | Native HWND Creation | Process owns top-level HWND with custom title, icon, and WndProc | Win32 API `FindWindowW` / `GetWindowLongPtrW` |
| **WebView2 Host** | Embedded Controller | `ICoreWebView2Controller` created with parent HWND; no separate browser process | Process tree check (`AetherIDE.exe` -> runtime children) |
| **Message Bridge** | Typed Command Allowlist | Untrusted messages and path traversal attempts return structured error | Bridge injection fuzz test |
