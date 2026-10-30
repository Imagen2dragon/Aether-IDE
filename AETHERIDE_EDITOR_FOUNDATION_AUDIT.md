# AETHERIDE — EDITOR FOUNDATION & DESKTOP HOST ARCHITECTURE AUDIT

**Document Type:** Principal Architectural Audit & System Comparison  
**Auditor:** CTO & Desktop IDE Systems Architect  
**Date:** 2026-10-31  
**Project:** AetherIDE  
**Status:** COMPLETE & AUTHORITATIVE  

---

## 1. Executive Summary

This audit resolves the central architectural discrepancy in AetherIDE's desktop presentation and editor foundation:
1. **Desktop Host Architecture:** We explicitly distinguish between **External Application Execution Mode** (`msedge.exe --app=...`) and **Embedded Native WebView2 Controller Hosting** (`ICoreWebView2Controller` inside an `HWND`).
2. **Editor Subsystem Foundation:** We analyze the actual editor implementation in `apps/desktop/workbench/index.html` versus Monaco Editor and the full Microsoft Code - OSS workbench, establishing a realistic technical gap assessment and migration roadmap.

---

## 2. Desktop Host Architecture: Technical Analysis

### 2.1 The Two Desktop Approaches

```
+-----------------------------------------------------------------------------------------+
| Architecture A: External App-Mode Launching (Current in aether-launcher)                 |
|                                                                                         |
|   +---------------------+        Spawn (Arg: --app=file:///...)       +---------------+  |
|   |  AetherIDE.exe      | -----------------------------------------> |  msedge.exe   |  |
|   |  (Launcher CLI)     |                                            |  (Edge Window)|  |
|   +---------------------+                                            +-------+-------+  |
|                                                                              |          |
|                                                                              v          |
|                                                                        Renders HTML     |
+-----------------------------------------------------------------------------------------+

+-----------------------------------------------------------------------------------------+
| Architecture B: True Embedded Native WebView2 Controller (Target Architecture)          |
|                                                                                         |
|   +----------------------------------------------------------------------------------+  |
|   |  AetherIDE Native Win32 Window (Rust Owned HWND)                                 |  |
|   |                                                                                  |  |
|   |   Win32 Message Loop (WndProc):                                                  |  |
|   |   * WM_SIZE -> ICoreWebView2Controller::put_Bounds                               |  |
|   |   * WM_DPICHANGED -> SetProcessDpiAwarenessContext                               |  |
|   |                                                                                  |  |
|   |   +--------------------------------------------------------------------------+   |  |
|   |   |  ICoreWebView2Controller (Child HWND)                                    |   |  |
|   |   |                                                                          |   |  |
|   |   |   ICoreWebView2 Browser Component:                                       |   |  |
|   |   |   * SetVirtualHostNameToFolderMapping (Secure file:// replacement)       |   |  |
|   |   |   * WebMessageReceived <-> Native Rust Tokio IPC Channel                 |   |  |
|   |   |   * ContentSecurityPolicy enforced at origin                             |   |  |
|   |   +--------------------------------------------------------------------------+   |  |
|   +----------------------------------------------------------------------------------+  |
+-----------------------------------------------------------------------------------------+
```

### 2.2 Comparison Matrix

| Property | Architecture A: `msedge.exe --app=` | Architecture B: True Embedded WebView2 (`ICoreWebView2Controller`) |
|:---|:---|:---|
| **Process Ownership** | Child process managed by Edge browser process tree | Rust parent process owns the top-level Win32 `HWND` |
| **Window Shell** | Standard Edge browser window with titlebar stripped | Fully custom Win32 chrome, custom non-client area, native menus |
| **Native IPC Bridge** | WebSocket/TCP loopback to `127.0.0.1:44888` only | Direct Win32 COM `WebMessageReceived` / `PostWebMessageAsJson` + IPC |
| **Resource Mapping** | `file:///` URLs (subject to browser CORS/file restrictions) | `SetVirtualHostNameToFolderMapping("aether.local", ...)` |
| **Memory Footprint** | ~120 MB (Edge browser process + GPU + utility) | ~65-80 MB (single shared runtime controller) |
| **Dependency** | Requires Microsoft Edge browser installed | Requires WebView2 Evergreen Runtime (preinstalled on Win 10/11) |
| **Window Resizing/DPI** | Handled by Chromium window manager | Managed by Rust `WM_SIZE` and `WM_DPICHANGED` handlers |
| **Security Surface** | Vulnerable if user navigates to external URLs | Locked down: NavigationStarting blocks external schemes, CSP active |

### 2.3 Verdict & Action
The current implementation in `rust/cli/aether-launcher/src/webview2_runtime.rs` uses **Architecture A** (`msedge.exe --app=...`). While functional for developer previews and lightweight distribution, it must not be marketed as a true embedded native controller.

**Sprint 03 Directive:**
1. Maintain Architecture A as a rock-solid, zero-dependency fallback launcher.
2. Provide the Win32 C/COM bindings and embedding architecture via `windows` / `webview2-com` crate for true HWND-embedded rendering when building the standalone distribution package.

---

## 3. Editor Subsystem Foundation Audit

### 3.1 What AetherIDE Currently Contains
Inspection of `apps/desktop/workbench/index.html` reveals:
- A **Custom HTML5/CSS3 Developer Dashboard** with tab navigation:
  - **Code Studio:** Displays file header and static code inside `<pre class="code-editor"><code>...</code></pre>`.
  - **Autonomous AI Team:** Interactive prompt to dispatch autonomous agents with role selectors.
  - **Mobile Studio:** Input form for Expo application generation.
  - **Project Brain:** SQLite database and architectural decision overview.
  - **Diagnostics:** Environment doctor status cards.

### 3.2 Comparison with Professional Editor Foundations

| Capability | Current Aether Custom Workbench | Monaco Editor (Standalone) | Microsoft Code - OSS Workbench |
|:---|:---|:---|:---|
| **Editor Buffer** | Static `<pre>` text area | High-performance virtualized DOM editor buffer | Full VS Code text buffer (`PieceTreeBase`) |
| **Multi-Cursor / Selections**| None | Native rich multi-cursor editing | Full multi-cursor, column selection, box selection |
| **Language Intelligence** | IPC-routed LSP results | Language client integration via Monaco LSP bridge | Complete built-in LSP client + semantic tokens |
| **Syntax Highlighting** | Basic regex/CSS styling | TextMate grammars + Monarch tokenizers | TextMate grammars + Tree-Sitter + Semantic coloring |
| **Diff & Merge Viewer** | Text diff display | Side-by-side and inline visual diff editor | Full 3-way merge editor with conflict resolution |
| **Terminal Integration** | Planned via ConPTY stream | Integrated xterm.js canvas terminal | Full multi-tab terminal multiplexer with ConPTY |
| **Extension Host** | None (direct IPC actions only) | Basic web plugins | Out-of-process Node.js/V8 extension host |
| **Extension Marketplace** | Incompatible | Incompatible without custom shim | Compatible with Open VSX Registry |
| **Debugging Infrastructure**| None | DAP (Debug Adapter Protocol) shims | Native DAP support, breakpoints, call stack, watches |
| **Source Control (Git)** | Backend git-engine queries | Custom UI required | Native SCM viewlet, gutter diffs, staging/commit UI |

### 3.3 The Core Architectural Realization
- Developing a custom code editor from scratch that matches the editing, typing, indentation, bracket matching, folding, minimap, accessibility, and extension ecosystem of VS Code would require **15-20 engineer-years**.
- Therefore, AetherIDE’s stated objective ("a unified platform combining Visual Studio Code, Cursor, Claude Code, and Antigravity") **must rely on a verified editor core**:
  - **Phase 1 (Immediate / Sprint 03):** Embed **Monaco Editor** (`monaco-editor` npm package) into `apps/desktop/workbench/index.html` via CDN/local bundle. Connect Monaco's completion and diagnostic providers to the native Rust `lsp-client` via daemon WebSocket.
  - **Phase 2 (Production Staging):** Fork or consume the **Code - OSS** workbench distribution, hosting Aether's native Rust daemon as an integrated backend service and packaging Aether's 14 AI agents and mobile tooling as a first-party core extension (`extensions/aether-core`).

---

## 4. Technical Migration Plan: Embedding Monaco into Workbench

To elevate `apps/desktop/workbench/index.html` from a static display to a genuine, interactive coding editor immediately without breaking changes:

1. **Include Monaco Editor:** Bundle Monaco AMD loader or local assets in `apps/desktop/workbench/`.
2. **Mount Interactive Editor:** Replace `<pre class="code-editor">` with `<div id="monaco-container" style="flex:1;height:100%;"></div>`.
3. **Register LSP Completion Provider:**
   ```javascript
   monaco.languages.registerCompletionItemProvider('rust', {
     provideCompletionItems: async (model, position) => {
       const resp = await sendIpc('LspCompletion', {
         path: currentFilePath,
         line: position.lineNumber - 1,
         character: position.column - 1
       });
       return { suggestions: resp.data.items };
     }
   });
   ```
4. **Register Diagnostics Model:**
   Feed incoming `LspPublishDiagnostics` IPC events into `monaco.editor.setModelMarkers(model, "aether-lsp", markers)`.
5. **Hook Document Synchronization:**
   Listen to `model.onDidChangeContent` and send `LspChangeDocument` over IPC to update the Rust `lsp-client`.

This preserves all 24 native Rust crates while instantly giving the developer a real, professional editing experience with syntax highlighting, IntelliSense, and keyboard shortcuts.
