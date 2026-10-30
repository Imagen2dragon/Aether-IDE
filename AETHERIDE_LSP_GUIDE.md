# AetherIDE — Language Server Protocol (LSP) Multiplexer Guide

**Subsystem:** `rust/crates/lsp-client/`  
**Integration:** `rust/services/aether-daemon/`  
**Protocol Version:** LSP 3.17 over JSON-RPC 2.0  
**Transport:** Header-framed stdio (`Content-Length: <n>\r\n\r\n`)

---

## 1. Architecture & Design Principles

AetherIDE's language intelligence is powered by a native Rust asynchronous LSP client multiplexer. Unlike editors that rely entirely on JavaScript/TypeScript extensions to manage language servers, AetherIDE manages the process lifecycle, stdio framing, request-response correlation, and document synchronization inside the native Rust daemon core.

### Key Capabilities:
- **Zero Synthetic Responses:** All language operations query actual language servers or return typed LSP errors.
- **Resilient Framing:** Handles glued frames, partial chunks, malformed headers, and payloads up to 64MB without panicking.
- **Asynchronous Cancellation:** In-flight queries can be cancelled via `$/cancelRequest` upon user keystrokes.
- **Crash Isolation & Recovery:** Automatic server failure detection with exponential backoff throttling prevents runaway process loops.
- **Full IPC Integration:** Daemon exposes clean, typed IPC actions for desktop workbench and autonomous agent queries.

---

## 2. Supported Language Servers

| Language | File Extensions | Default Language Server | Launch Arguments |
|---|---|---|---|
| **Rust** | `.rs` | `rust-analyzer` | None (stdio) |
| **TypeScript / JavaScript** | `.ts`, `.tsx`, `.js`, `.jsx`, `.json` | `typescript-language-server` / `vtsls` | `--stdio` |
| **Python** | `.py` | `pyright-langserver` / `pyright` | `--stdio` |

Custom language servers can be registered programmatically or via configuration through `LanguageServerDefinition`.

---

## 3. Daemon IPC API Reference

The daemon exposes the following actions over Named Pipe (`\\.\pipe\aether-daemon-ipc`) and local TCP loopback (`127.0.0.1:44888`):

### 3.1 `LspOpenDocument`
Notifies the language server that a file was opened in the editor.
```json
{
  "id": "req-1",
  "session_token": "<token>",
  "type": "Request",
  "data": {
    "action": "LspOpenDocument",
    "params": {
      "path": "src/main.rs",
      "content": "fn main() { println!(\"Hello\"); }",
      "language_id": "rust"
    }
  }
}
```

### 3.2 `LspChangeDocument`
Pushes incremental or full buffer updates with version tracking.
```json
{
  "id": "req-2",
  "session_token": "<token>",
  "type": "Request",
  "data": {
    "action": "LspChangeDocument",
    "params": {
      "path": "src/main.rs",
      "content": "fn main() { println!(\"Updated\"); }",
      "version": 2
    }
  }
}
```

### 3.3 `LspCompletion`
Retrieves code completion items at a specific line and column.
```json
{
  "id": "req-3",
  "session_token": "<token>",
  "type": "Request",
  "data": {
    "action": "LspCompletion",
    "params": {
      "path": "src/main.rs",
      "line": 0,
      "character": 12
    }
  }
}
```

### 3.4 `LspHover`
Retrieves markdown documentation, type signatures, and hover info.
```json
{
  "id": "req-4",
  "session_token": "<token>",
  "type": "Request",
  "data": {
    "action": "LspHover",
    "params": {
      "path": "src/main.rs",
      "line": 0,
      "character": 12
    }
  }
}
```

### 3.5 `LspDefinition`
Navigates to the definition location of a symbol.
```json
{
  "id": "req-5",
  "session_token": "<token>",
  "type": "Request",
  "data": {
    "action": "LspDefinition",
    "params": {
      "path": "src/main.rs",
      "line": 0,
      "character": 12
    }
  }
}
```

### 3.6 `LspDiagnostics`
Retrieves the latest published diagnostics for a specific open file.
```json
{
  "id": "req-6",
  "session_token": "<token>",
  "type": "Request",
  "data": {
    "action": "LspDiagnostics",
    "params": {
      "path": "src/main.rs"
    }
  }
}
```

---

## 4. Test Verification

`lsp-client` is covered by unit and end-to-end integration tests:
- Framing tests: `cargo test -p lsp-client --lib framing`
- JSON-RPC tests: `cargo test -p lsp-client --lib jsonrpc`
- Lifecycle & Mock Server integration: `cargo test -p lsp-client --test mock_lsp_test`
- Daemon IPC integration: `cargo test -p aether-daemon --test test_authenticated_lsp_diagnostics_query`
