# AetherIDE — Sprint 02 Architecture Specification
## Native Language Intelligence, Windows ConPTY, WebView2 Desktop Host & Encrypted Workspace Sync

**Document Version:** 1.0.0  
**Target Milestone:** Sprint 02  
**Target Operating Systems:** Windows 10/11 x64, iOS/iPadOS 17+

---

## 1. System Overview

Sprint 02 integrates four mission-critical native subsystems into the AetherIDE architecture:
1. **LSP Multiplexer (`lsp-client`):** Rust-native asynchronous client for the Language Server Protocol (v3.17). Manages language server processes (`rust-analyzer`, `typescript-language-server`, `pyright`), routes document events, performs stdio JSON-RPC framing, and bridges code intelligence to the workbench and autonomous agents via daemon IPC.
2. **Native Windows ConPTY Engine (`terminal-manager`):** Replaces virtual pseudoterminal stubs with the official Windows Pseudoconsole API (`CreatePseudoConsole`). Binds spawned shells to Windows Job Objects from `process-manager` to guarantee tree termination, streams raw ANSI VT sequences across IPC, and processes window resizing.
3. **Native Rust Desktop Host (Microsoft WebView2):** Provides a direct Win32 + WebView2 application host in `aether-launcher`, eliminating heavy Electron runtimes while keeping full web-based workbench fidelity and embedding a zero-latency bidirectional postMessage bridge to the daemon.
4. **End-to-End Encrypted Device Synchronization:** Implements local peer discovery via mDNS, out-of-band QR pairing with ephemeral key exchange and authenticated encryption (ChaCha20-Poly1305), content-addressable chunked file sync, and iOS companion synchronizer.

```
+-----------------------------------------------------------------------------------+
|                              AetherIDE Desktop Host                               |
|   +---------------------------------------------------------------------------+   |
|   |         Native Win32 Window + Evergreen Microsoft WebView2 Engine         |   |
|   |   +-------------------------------------------------------------------+   |   |
|   |   |                 AetherIDE Workbench (TypeScript / HTML5)          |   |   |
|   |   +---------------------------------+---------------------------------+   |   |
|   +-------------------------------------|-------------------------------------+   |
|                                         | Window.chrome.webview.postMessage       |
|                                         v                                         |
|   +---------------------------------------------------------------------------+   |
|   |                  Aether Host IPC Dispatcher / Message Bridge              |   |
+---|-------------------------------------|-----------------------------------------+
                                          | Named Pipe (\\.\pipe\aether-daemon-ipc)
                                          v
+-----------------------------------------------------------------------------------+
|                            aether-daemon (Tokio Core)                             |
|                                                                                   |
|  +--------------------+  +----------------------+  +----------------------------+ |
|  |    lsp-client      |  |   terminal-manager   |  |       workspace-sync       | |
|  |  +---------------+ |  |  +-----------------+ |  |  +-----------------------+ | |
|  |  | JSON-RPC 2.0  | |  |  | Win32 ConPTY    | |  |  | mDNS Peer Discovery   | | |
|  |  | Stdio Framing | |  |  | HPCON Handle    | |  |  | QR Code Pairing       | | |
|  |  +---------------+ |  |  +-----------------+ |  |  +-----------------------+ | |
|  |  | rust-analyzer | |  |  | ScopedJobObject | |  |  | ChaCha20-Poly1305 AEAD| | |
|  |  | vtsls / tsserver|  |  | Async Pipe IO   | |  |  | Hash Manifest Merkle  | | |
|  |  | pyright       | |  |  | Bounded Channels| |  |  | Chunked File Transfer | | |
|  |  +---------------+ |  |  +-----------------+ |  |  +-----------------------+ | |
|  +--------------------+  +----------------------+  +----------------------------+ |
+-----------------------------------------------------------------------------------+
                                                                  | Encrypted LAN
                                                                  v
                                              +-------------------------------------+
                                              | iOS Companion App (SwiftUI + FFI)   |
                                              | - Local workspace mirror & review   |
                                              +-------------------------------------+
```

---

## 2. Gate 1: Rust Language Server Protocol Multiplexer (`lsp-client`)

### 2.1 JSON-RPC 2.0 & Stdio Framing
LSP utilizes header-delimited JSON-RPC 2.0 messages:
```
Content-Length: <length-in-bytes>\r\n
\r\n
{"jsonrpc":"2.0","id":1,"method":"textDocument/completion","params":{...}}
```
- **Framer Specification:** Custom Tokio `Decoder` and `Encoder` handling streaming bytes, partial chunks, malformed headers, and Unicode UTF-8 payloads.
- **Request Correlation:** Thread-safe pending request registry utilizing `tokio::sync::oneshot` channels keyed by `RequestId` (`i64` or `String`).
- **Cancellation & Timeout:** In-flight requests can be cancelled via `$/cancelRequest` if user input invalidates the query; default timeout applied per request.

### 2.2 Server Session Lifecycle
```
[Uninitialized] ---> [Spawning Process] ---> [Sending 'initialize']
        ^                                               |
        |                                               v
[Process Dead] <--- [Exit & Cleanup] <--- ['shutdown' Response]
```
1. **Spawn:** Command execution via `tokio::process::Command` with redirected `stdin`, `stdout`, and `stderr`.
2. **Initialize:** Client sends `initialize` request with client capabilities (completions, hover, definition, diagnostics).
3. **Initialized:** Client sends `initialized` notification upon successful response.
4. **Active Operation:**
   - Synchronizes open documents: `textDocument/didOpen`, `textDocument/didChange` (incremental or full), `textDocument/didSave`, `textDocument/didClose`.
   - Dispatches queries: `textDocument/completion`, `textDocument/hover`, `textDocument/definition`.
   - Handles notifications: `textDocument/publishDiagnostics` published onto daemon event bus.
5. **Shutdown & Exit:** Graceful `shutdown` request followed by `exit` notification and child process reaping.
6. **Fault Tolerance:** Crash detection with automatic respawn and exponential backoff restart throttle (max 3 restarts within 60 seconds).

### 2.3 Language Routing Matrix
- `.rs`, `Cargo.toml` -> `rust-analyzer`
- `.ts`, `.tsx`, `.js`, `.jsx`, `.json` -> `typescript-language-server` or `vtsls`
- `.py` -> `pyright-langserver` or `pyright`

---

## 3. Gate 2: Native Windows ConPTY Terminal Engine (`terminal-manager`)

### 3.1 Win32 Pseudoconsole Architecture
The Windows Pseudoconsole (ConPTY) creates an OS-level terminal driver that translates console API output into ANSI escape sequences:
1. `CreatePseudoConsole(COORD size, HANDLE hInput, HANDLE hOutput, DWORD dwFlags, HPCON* phPC)`.
2. Interprocess pipes created with `CreatePipe`:
   - `conpty_in_read` / `conpty_in_write` (Terminal stdin)
   - `conpty_out_read` / `conpty_out_write` (Terminal stdout/stderr)
3. Process creation via `CreateProcessW` using `STARTUPINFOEXW`:
   - Attributes allocated with `InitializeProcThreadAttributeList`.
   - `UpdateProcThreadAttribute(..., PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE, phPC, ...)`.
   - Created in suspended state (`CREATE_SUSPENDED`), assigned to `ScopedJobObject` from `process-manager`, then resumed (`ResumeThread`).
4. Asynchronous I/O bridge:
   - Dedicated blocking worker threads read `conpty_out_read` into Tokio bounded channels (`mpsc::Sender<Vec<u8>>`).
   - Write stream forwards Tokio buffer chunks into `conpty_in_write`.
5. Lifecycle controls:
   - Resize via `ResizePseudoConsole(phPC, new_size)`.
   - Graceful termination via `ClosePseudoConsole(phPC)`.
   - Forced kill via Job Object if unresponsive.

---

## 4. Gate 3: Native Rust Microsoft WebView2 Desktop Host

### 4.1 Architecture Decision Summary
Electron introduces 150MB+ binary footprint and 300MB+ baseline memory overhead. Microsoft WebView2 is pre-installed on Windows 10/11 Evergreen runtimes, sharing memory with the OS Edge runtime.
- Host executable in `rust/cli/aether-launcher` (or dedicated binary) initializes Win32 window with dark titlebar and high-DPI scaling.
- Instantiates `CoreWebView2Environment` and `CoreWebView2Controller`.
- Direct virtual folder mapping: `https://aether.workbench/` maps directly to local `dist/workbench/`.
- Bidirectional messaging:
  - Frontend to Host: `window.chrome.webview.postMessage(...)`
  - Host to Daemon: Direct Named Pipe connection (`\\.\pipe\aether-daemon-ipc`).
  - Host to Frontend: `controller.CoreWebView2().PostWebMessageAsString(...)`.

---

## 5. Gate 4: End-to-End Encrypted Device Synchronization

### 5.1 Discovery & Pairing Protocol
1. **Local Discovery:** Zero-configuration service announcement over mDNS (`_aether-sync._tcp.local.`). Transmits ephemeral peer ID and port; does not leak project or user data.
2. **Ephemeral QR Pairing:**
   - Desktop displays ephemeral QR code containing: `{ host, port, session_id, psk: [32-byte-hex] }`.
   - iOS companion scans QR, establishing initial TLS / Noise-like mutual authentication using the pre-shared key.
3. **Channel Encryption:** All sync traffic encrypted via ChaCha20-Poly1305 AEAD with unique nonces derived from monotonic sequence counters.

### 5.2 Merkle Manifest & Chunk Synchronization
1. Desktop and companion compute recursive SHA-256 tree of files (relative path, file size, content hash, mtime).
2. Manifest diffing identifies missing, updated, and deleted blocks.
3. Large files sliced into 64KB content-addressable chunks; deduplicated across transfers.
4. Conflict resolution: If both desktop and companion modified a file concurrently, companion preserves remote change and saves local conflict copy (`<filename>.conflict.<timestamp>`).
