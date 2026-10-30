# AetherIDE — Sprint 02 Test & Verification Plan

**Test Document Version:** 1.0.0  
**Target Milestone:** Sprint 02  
**Target Platforms:** Windows 11 x64, iOS Companion

---

## 1. Test Strategy & Quality Gates

Sprint 02 requires rigorous test-driven verification across all four gates. No feature is accepted without automated test coverage and reproducible end-to-end execution.

### Verification Standards:
1. `cargo fmt --all -- --check` must produce zero differences.
2. `cargo clippy --workspace --all-targets --all-features -- -D warnings` must produce zero warnings.
3. Every new crate and feature must include unit tests testing happy path, edge cases (partial chunks, corrupt framing, timeout, process crash), and security boundaries.
4. Total workspace tests must strictly increase beyond the Sprint 01 baseline of 28 tests.

---

## 2. Gate-by-Gate Test Matrix

| ID | Gate | Area | Test Description | Target Criteria |
|---|---|---|---|---|
| **TEST-01** | Gate 1 | Framing | JSON-RPC 2.0 framing encoder & decoder | Encodes `Content-Length: ...\r\n\r\n`, decodes exact, partial, and glued frames. |
| **TEST-02** | Gate 1 | Framing | Malformed header handling | Rejects invalid `Content-Length`, malformed headers, and non-UTF8 safely without panicking. |
| **TEST-03** | Gate 1 | Session | Mock LSP Server lifecycle handshake | Executes `initialize` -> `initialized` -> `shutdown` -> `exit` sequence accurately. |
| **TEST-04** | Gate 1 | Session | Request correlation & timeouts | Correlates concurrent requests by `id`; triggers timeout on unresponsive requests. |
| **TEST-05** | Gate 1 | Multiplexer | Multi-language routing | Routes `.rs` to `rust-analyzer`, `.ts` to `vtsls`, `.py` to `pyright`. |
| **TEST-06** | Gate 1 | Diagnostics | Diagnostic publication | Collects and decodes `publishDiagnostics` notifications into standard IDE diagnostic structures. |
| **TEST-07** | Gate 1 | Daemon IPC | Daemon LSP IPC Actions | Verifies `LspCompletion`, `LspHover`, `LspDefinition`, `LspOpenDocument` via daemon IPC pipe/socket. |
| **TEST-08** | Gate 2 | ConPTY | ConPTY creation and handle binding | Creates `HPCON` successfully, validates pipe handle assignment. |
| **TEST-09** | Gate 2 | ConPTY | Shell execution in Job Object | Spawns `cmd.exe` or `powershell.exe`, confirms child process joins Windows Job Object. |
| **TEST-10** | Gate 2 | ConPTY | ANSI stream I/O | Writes command (e.g. `echo AETHER_OK`) and reads back ANSI output stream with expected text. |
| **TEST-11** | Gate 2 | ConPTY | Window resize | Dispatches `ResizePseudoConsole` and confirms terminal dimensions update without error. |
| **TEST-12** | Gate 2 | ConPTY | Process termination & tree cleanup | Terminates terminal session, confirms child process tree is reaped via Job Object. |
| **TEST-13** | Gate 3 | WebView2 | Evergreen runtime detection | Detects installed Microsoft Edge WebView2 runtime version on Windows. |
| **TEST-14** | Gate 3 | WebView2 | Host window creation | Initializes Win32 window with dark titlebar and message loop handling. |
| **TEST-15** | Gate 3 | WebView2 | IPC message dispatch | Verifies postMessage JSON payload round-trip between webview and daemon client. |
| **TEST-16** | Gate 4 | Sync | AEAD Crypto Handshake | Generates ephemeral key pair, validates authenticated encryption (ChaCha20-Poly1305). |
| **TEST-17** | Gate 4 | Sync | Manifest generation & diffing | Hashes directory contents, detects added, modified, and deleted files. |
| **TEST-18** | Gate 4 | Sync | Chunk transfer & reassembly | Splits file into chunks, transfers, and reassembles with SHA-256 integrity check. |

---

## 3. Regression Testing

All 28 existing tests from Sprint 01 must continue to pass without regression:
- `application-core`: Workspace configuration and path validation.
- `security-core`: Constant-time token verification, path traversal checks, secret masking.
- `database-engine`: SQLite WAL mode, schema migrations, FTS5 full-text queries.
- `filesystem-engine`: Atomic file writes, path canonicalization, change watchers.
- `syntax-engine`: Syn 2.0 AST parser, symbol extraction, comment/docstring preservation.
- `process-manager`: ScopedJobObject process tree termination.
- `model-gateway`: Multi-provider configuration and schema validation.
- `mobile-build-manager`: EAS CLI environment checks.
- `aether-daemon`: Daemon authentication and IPC protocol handling.
