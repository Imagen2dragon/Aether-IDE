# AetherIDE Workspace Security Model

## 1. Architectural Overview & Threat Boundary

AetherIDE operates as a native desktop AI engineering studio. It allows autonomous agents, background build tools, and integrated language servers to operate directly on source code projects. Because these systems ingest untrusted inputs (model-generated tool parameters, external repository contents, and LLM completions), AetherIDE enforces a strict, fail-closed capability boundary.

No process, agent, or file operation is permitted ambient filesystem access. Every filesystem mutation, read, or process execution must pass through an authorized capability handle rooted in an explicitly user-approved workspace.

```
+-----------------------------------------------------------------------+
|                             AetherIDE                                 |
|                                                                       |
|  +--------------------+   +-------------------+   +----------------+  |
|  | AI Assistant /     |   | Build Command     |   | Checkpoint     |  |
|  | Autonomous Agents  |   | Runner            |   | Engine         |  |
|  +---------+----------+   +---------+---------+   +-------+--------+  |
|            |                        |                     |           |
|            v                        v                     v           |
|  +--------------------+   +-------------------+   +----------------+  |
|  | AgentExecution-    |   | Direct Exec       |   | Byte-Perfect   |  |
|  | Context            |   | (No Shell)        |   | Tokenized RW   |  |
|  +---------+----------+   +---------+---------+   +-------+--------+  |
|            |                        |                     |           |
|            +-------------------+    |    +----------------+           |
|                                |    |    |                            |
|                                v    v    v                            |
|                     +--------------------------+                      |
|                     |   WorkspaceCapability    |                      |
|                     | (cap-std anchored handles)|                      |
|                     +-------------+------------+                      |
|                                   |                                   |
+-----------------------------------|-----------------------------------+
                                    v
                     +--------------------------+
                     | Approved Workspace Root  |
                     | (Canonicalized Boundary) |
                     +--------------------------+
```

---

## 2. Capability-Based Directory Isolation (`WorkspaceCapability` & `cap-std`)

### 2.1 Anchored Handles
AetherIDE utilizes `cap-std::fs::Dir` to represent workspace roots. All read, write, rename, and delete operations execute as handle-relative operations via OS-level primitives (`openat` on POSIX, handle-relative operations on Windows). Ambient path traversal is completely severed.

### 2.2 Path Sanitization
Incoming paths undergo strict canonicalization prior to resolution:
- **Parent Directory Traversal (`..`):** Any token matching `..` is rejected upfront with `AetherError::PermissionDenied`.
- **Root/Prefix Escapes:** Leading slashes, Windows drive prefixes (e.g. `C:\`), and UNC paths (`\\server\share`) are rejected.
- **Reserved Device Names (Windows):** Paths containing `CON`, `PRN`, `AUX`, `NUL`, `COM1..COM9`, `LPT1..LPT9` (case-insensitive, with or without file extensions) are rejected with `AetherError::PermissionDenied`.
- **Empty & Whitespace Paths:** Zero-length or whitespace-only paths fail closed.

### 2.3 Symlink & Junction Containment
To prevent malicious repositories or rogue agent tools from escaping the workspace via symbolic links or directory junctions:
- Symlink metadata is checked prior to open or read. External targets are rejected with `Access Denied: Symlink points outside workspace boundary`.
- Atomic writes refuse to overwrite or follow destination symlinks.
- Directory validation resolves targets and asserts that the canonical target remains strictly within the workspace boundary via prefix validation.

---

## 3. Agent Execution Context (`AgentExecutionContext`)

Autonomous agents execute with an explicit `AgentExecutionContext` rather than ambient directory paths:
1. **Binding:** Every agent task binds to a specific `workspace_id`, `root_id`, `canonical_root`, `is_trusted` flag, and `Arc<WorkspaceCapability>`.
2. **Tool Dispatcher Guard:** Tools (`read_file`, `write_file`, `git_diff`, `run_command`) can only interact with the filesystem through `ctx.authorize_read()`, `ctx.authorize_write()`, and `ctx.authorize_process()`.
3. **Working Directory Lockdown:** In `run_command`, any model-supplied `cwd` parameter is strictly validated:
   - If omitted, it defaults to the canonical workspace root.
   - If provided, it is canonicalized and validated against `FilesystemEngine::is_within_boundary`. Any attempt to point `cwd` outside the workspace (e.g. `C:\Windows` or `/etc`) is aborted with a `Security Denial`.
   - Untrusted workspaces (`is_trusted = false`) reject all process executions unconditionally.

---

## 4. Build Tool Security & Direct Execution

### 4.1 Elimination of Shell Wrappers
Build commands are executed directly via `std::process::Command` without wrapping in `cmd.exe /c` or `sh -c`. This neutralizes shell injection attacks (`&`, `|`, `;`, `&&`, `` ` ``, `$()`).
On Windows, `.cmd` and `.bat` tools (such as `npm.cmd` or `cargo.cmd`) are resolved to their system executable and spawned directly.

### 4.2 Approved Tool Whitelist
AetherIDE restricts execution to explicitly approved build utilities:
`cargo`, `npm`, `npx`, `yarn`, `pnpm`, `node`, `python`, `pip`, `pytest`, `dotnet`, `go`, `flutter`, `gradle`, `mvn`, `git`.

### 4.3 Shadow Binary Defense
To prevent a malicious repository from dropping a trojaned `npm.cmd` or `cargo.exe` into its project directory to hijack build triggers:
- The runner inspects the target directory for local binaries shadowing approved tools.
- Project-local executables matching tool names are rejected with `Security Denial: Project-local executable shadows approved tool`.
- Approved tools are resolved exclusively against the system `PATH`.

---

## 5. Checkpoint Engine & Byte-Perfect Integrity

### 5.1 Raw Byte Preservation
Checkpoints record raw binary bytes. Lossy conversions (`String::from_utf8_lossy`) are strictly eliminated. Binary files, compiled assets, images, and non-UTF-8 text files are preserved with exact byte integrity and verified via SHA-256 content hashes.

### 5.2 Conflict Detection & One-Use Preview Tokens
- Prior to restoring a checkpoint, `checkpoint_restore_preview` scans the workspace, calculates current disk hashes, and generates a cryptographically random, one-use `preview_token`.
- If any file was modified on disk since the checkpoint snapshot, a conflict is flagged.
- By default, restoration aborts on unresolved conflicts (`PermissionDenied`) unless the user explicitly authorizes them via `force: true` or `approved_conflicts`.

### 5.3 Post-Preview Race Detection
During restoration, the engine re-hashes disk files and compares them against `preview.disk_hashes`. If a file was modified concurrently between preview generation and restore execution, the restore aborts immediately, preventing accidental loss of concurrent developer edits.

### 5.4 Pre-Restore Safety Snapshots
Before applying any file modifications during restore, Tauri command `checkpoint_restore` automatically creates an internal safety checkpoint snapshot of the current workspace state.

---

## 6. AI Gateway Streaming & Cancellation

### 6.1 Genuine SSE Streaming
All official model providers (OpenAI, Anthropic, Google Gemini, DeepSeek, xAI, Local Ollama) implement genuine chunk-by-chunk streaming over Server-Sent Events (SSE) or chunked HTTP.
The default trait method on `ModelProvider` fails closed (`Streaming is not supported for provider '...'`), eliminating fake single-chunk completion disguises.

### 6.2 Connection Abort on Cancellation
When a user clicks "Stop" or cancels an active stream (`ai_cancel_stream`):
- The atomic cancellation token (`AtomicBool`) is flagged.
- The active streaming loop terminates immediately.
- The underlying `reqwest` HTTP response byte stream is dropped, terminating the TCP/TLS connection to the provider and saving user API tokens.
