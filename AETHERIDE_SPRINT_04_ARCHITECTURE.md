# AETHERIDE — SPRINT 04 ARCHITECTURAL SPECIFICATION
## Professional Monaco IDE Workbench and Native HWND-Embedded WebView2 Host

**Version:** 1.0.0  
**Author:** CTO & Principal Systems Architect  
**Date:** 2026-10-31  
**Project:** AetherIDE  

---

## 1. System Topology Overview

```
+-------------------------------------------------------------------------------------------------+
|                                    AetherIDE.exe (Native Windows x64)                           |
|                                                                                                 |
|   +-----------------------------------------------------------------------------------------+   |
|   | Top-Level Win32 HWND (Class: "AetherIDEWindowClass")                                    |   |
|   |  - Win32 Message Loop (WndProc):                                                        |   |
|   |    * WM_SIZE -> ICoreWebView2Controller::put_Bounds(0, 0, width, height)                |   |
|   |    * WM_DPICHANGED -> SetProcessDpiAwarenessContext, update window rect                 |   |
|   |    * WM_DESTROY -> PostQuitMessage(0), clean process shutdown                           |   |
|   |                                                                                         |   |
|   |   +---------------------------------------------------------------------------------+   |   |
|   |   | ICoreWebView2Controller (Child Window filling HWND client rect)                 |   |   |
|   |   |                                                                                 |   |   |
|   |   |  ICoreWebView2 Runtime Instance:                                                |   |   |
|   |   |   - Profile: %LOCALAPPDATA%\AetherIDE\webview_profile                               |   |   |
|   |   |   - Virtual Host Mapping: https://appassets.aetheride.local/ -> ./workbench/        |   |   |
|   |   |   - Origin Lockdown: NavigationStarting blocks external schemes                     |   |   |
|   |   |   - Secure WebMessage Bridge:                                                       |   |   |
|   |   |     * Inbound: WebMessageReceived -> validate origin & schema -> dispatch to daemon|   |   |
|   |   |     * Outbound: PostWebMessageAsJson -> forward events/responses to window.aetherIDE|   |   |
|   |   +---------------------------------------+-----------------------------------------+   |   |
|   +-------------------------------------------|---------------------------------------------+   |
+-----------------------------------------------|-------------------------------------------------+
                                                |
                            Authenticated Native Dispatch (IPC / Named Pipe)
                                                |
                                                v
+-------------------------------------------------------------------------------------------------+
|                                 aether-daemon.exe (Tokio Runtime)                               |
|   - Token Manager (401 Unauthorized check)        - ConPTY Terminal Manager                     |
|   - LSP Multiplexer (rust-analyzer / vtsls / py)  - Agent Orchestrator & Tool Sandbox           |
|   - SQLite WAL / FTS5 Symbol Index                - Encrypted Workspace Synchronization         |
+-------------------------------------------------------------------------------------------------+
```

---

## 2. Workbench & Monaco Architecture

```
+-------------------------------------------------------------------------------------------------+
|                                    AETHERIDE WORKBENCH UI                                       |
|                                                                                                 |
|  +--------------------+  +--------------------------------------------------------------------+ |
|  | Activity Bar       |  | Editor Tabs: [ calculator.rs ● ] [ main.rs ] [ + ]                  | |
|  | - Code Studio      |  +--------------------------------------------------------------------+ |
|  | - Autonomous Team  |  | Breadcrumbs: workspace > src > calculator.rs                       | |
|  | - Mobile Studio    |  +--------------------------------------------------------------------+ |
|  | - Project Brain    |  | Monaco Editor Core (Local Bundle, No CDN):                         | |
|  | - Diagnostics      |  |  * Virtualized DOM Text Buffer (monaco.editor.create)              | |
|  |                    |  |  * Multi-Cursor, Bracket Matching, Code Folding, Minimap          | |
|  |                    |  |  * Command Palette (Ctrl+Shift+P), Quick Open (Ctrl+P)              | |
|  |                    |  |  * Local Web Workers: editor.worker, ts.worker, etc.               | |
|  +--------------------+  +--------------------------------------------------------------------+ |
|  | File Tree Explorer |  | LSP Language Feature Providers:                                    | |
|  | 📁 src/            |  |  * registerCompletionItemProvider -> LspCompletion                 | |
|  |   📄 calculator.rs |  |  * registerHoverProvider          -> LspHover                      | |
|  |   📄 lib.rs        |  |  * registerDefinitionProvider     -> LspDefinition                 | |
|  | 📁 tests/          |  |  * setModelMarkers                <- LspPublishDiagnostics         | |
|  +--------------------+  +--------------------------------------------------------------------+ |
|  | Status Bar:  Rust | UTF-8 | CRLF | Ln 12, Col 24 | LSP: rust-analyzer (Online) | Git: main | |
+-------------------------------------------------------------------------------------------------+
```

---

## 3. Secure Native Message Bridge Specification

The frontend interacts with the native Rust layer exclusively via `window.chrome.webview.postMessage` and listens via `window.chrome.webview.addEventListener('message', ...)`.

### 3.1 Inbound Message Schema (Web to Native)
```json
{
  "id": "req-101",
  "action": "LspCompletion",
  "params": {
    "path": "src/calculator.rs",
    "line": 10,
    "character": 4
  }
}
```

### 3.2 Security Rules
1. **Origin Check:** Only messages originating from `https://appassets.aetheride.local` or the approved local file URI are processed.
2. **Token Encapsulation:** The daemon session token is injected exclusively by the native Rust host process. Frontend scripts have **zero access** to the token.
3. **Allowlisted Actions:** Arbitrary shell commands or arbitrary filesystem escapes are blocked. Only typed actions (`Lsp*`, `File*`, `Terminal*`, `Agent*`, `Doctor*`) are forwarded.
