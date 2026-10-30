# AETHERIDE — EDITOR COMPATIBILITY & EXTENSION ARCHITECTURE REPORT

**Document Type:** Technical Compatibility Specification & Standards Report  
**Author:** Desktop IDE Architect  
**Date:** 2026-10-31  
**Project:** AetherIDE  

---

## 1. Scope & Compatibility Targets

AetherIDE aims to deliver an AI-native engineering environment that bridges Visual Studio Code, Cursor, and autonomous agent orchestration. To achieve genuine developer adoption, the editor layer must maintain compatibility across key dimensions:

1. **Language Server Protocol (LSP 3.17):** Strict adherence to JSON-RPC 2.0 framing and standard data models (`CompletionItem`, `Hover`, `Location`, `Diagnostic`).
2. **Keybinding & Navigation Compatibility:** Standard VS Code shortcuts (`Ctrl+P` Quick Open, `Ctrl+Shift+P` Command Palette, `Ctrl+\`` Terminal, `F12` Go to Definition, `Alt+F12` Peek Definition).
3. **Workspace Configuration (`.vscode/settings.json`, `.aether/settings.json`):** Compatible JSON configuration schemas for formatter settings, linter rules, and tab size.
4. **Theme & Token Styling:** VS Code Dark+ / GitHub Dark theme palette matching.

---

## 2. Editor Component Comparison & Feature Matrix

| Feature | Custom HTML Workbench (`v0.1.0-alpha`) | Monaco Embedded (`Sprint 03 Target`) | Code - OSS Shell (`Production Staging`) |
|:---|:---|:---|:---|
| **Token-Level Highlighting** | Monolithic text | monarch tokenizer (20+ languages) | TextMate + Tree-sitter + Semantic tokens |
| **Line Numbers & Glyphs** | None | Line numbers, folding, breakpoints | Full gutter: line numbers, git diff, fold, debug |
| **IntelliSense Autocomplete**| None | Native Monaco completion dropdown | Native VS Code suggest widget with documentation |
| **Hover Tooltips** | None | Rich Markdown hover cards | Rich Markdown hover cards with quick actions |
| **Multi-File Tab Bar** | Static header | Dynamic tab manager | Full workbench tab bar with split editor support |
| **Integrated Terminal** | Diagnostics card | Embedded xterm.js via ConPTY stream | Full VS Code terminal panel (split, tabs, profiles) |
| **File Tree Explorer** | Static HTML mockup | Dynamic tree populated from daemon | Full VS Code explorer with drag-and-drop, rename |
| **Extension Host** | None | Web plugins | Separate Node.js V8 extension host process |

---

## 3. LSP Integration Architecture with Monaco

The integration between AetherIDE's native Rust `lsp-client` and Monaco Editor is illustrated below:

```
+-----------------------------------------------------------------------------------------+
|                                  AetherIDE Workbench                                    |
|                                                                                         |
|   +---------------------------------------------------------------------------------+   |
|   | Monaco Editor Instance (DOM)                                                    |   |
|   |                                                                                 |   |
|   |  - On Editor Keystroke -> monaco.languages.registerCompletionItemProvider       |   |
|   |  - On Cursor Hover     -> monaco.languages.registerHoverProvider                |   |
|   |  - On Buffer Change    -> editorModel.onDidChangeContent                        |   |
|   +---------------------------------------+-----------------------------------------+   |
|                                           | JavaScript IPC Wrapper                      |
|                                           v                                             |
|   +---------------------------------------------------------------------------------+   |
|   | daemonClient.js (WebSocket to ws://127.0.0.1:44888)                             |   |
|   | Envelope framing with session token: { "action": "LspCompletion", "params":... }|   |
|   +---------------------------------------+-----------------------------------------+   |
+-------------------------------------------|---------------------------------------------+
                                            | Localhost TCP/WebSocket
                                            v
+-----------------------------------------------------------------------------------------+
|                                    Aether Daemon (Rust)                                 |
|                                                                                         |
|   +---------------------------------------------------------------------------------+   |
|   | handle_ipc_envelope -> match LspCompletion / LspHover / LspChangeDocument       |   |
|   +---------------------------------------+-----------------------------------------+   |
|                                           | Rust async channel                          |
|                                           v                                             |
|   +---------------------------------------------------------------------------------+   |
|   | lsp-client::LspMultiplexer                                                      |   |
|   | Auto-routes by extension -> rust-analyzer / vtsls / pyright                      |   |
|   | Stdio JSON-RPC 2.0 framing (Content-Length: ...\r\n\r\n)                         |   |
|   +---------------------------------------------------------------------------------+   |
+-----------------------------------------------------------------------------------------+
```

---

## 4. Extension Host Strategy & Open VSX

AetherIDE will not attempt to implement a custom proprietary extension engine. Instead:
1. For core AI and platform capabilities, AetherIDE exposes first-party native Rust APIs through daemon IPC.
2. For community language support, themes, and linters, AetherIDE adopts the Code - OSS extension host runtime, connecting to the **Open VSX Registry** (`https://open-vsx.org`) in compliance with open-source licensing.
