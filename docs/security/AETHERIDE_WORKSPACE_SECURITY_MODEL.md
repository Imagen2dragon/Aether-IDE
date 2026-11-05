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

### 4.1 Universal Trust Enforcement (BLD-007)
Both execution paths (explicit `working_dir` or default `working_dir = None` defaulting to the primary workspace root) execute `validate_build_working_directory`. The directory must be strictly within an authorized workspace root AND that workspace root must be trusted (`r.is_trusted == true`). Process execution in untrusted workspaces is denied unconditionally.

### 4.2 Elimination of Shell Wrappers & Metacharacter Injection
Build commands are executed directly via `tokio::process::Command` without wrapping in `cmd.exe /c` or `sh -c`. This neutralizes shell injection attacks (`&`, `|`, `;`, `&&`, `` ` ``, `$()`). Command arguments are tokenized preserving quoted strings and spaces, and passed as typed argv arrays.

### 4.3 Approved Tool Whitelist & Exact Name Resolution (BLD-008)
AetherIDE restricts execution to explicitly approved build utilities:
`cargo`, `rustc`, `npm`, `npx`, `yarn`, `pnpm`, `bun`, `python`, `python3`, `py`, `pytest`, `gradle`, `gradlew`, `./gradlew`, `make`, `cmake`.
Tool resolution enforces exact stem matching. Suffix spoofing (`tool_name.ends_with(tool)` attacks such as `evilcargo`, `fakepnpm`, or `maliciousnpm`) is strictly rejected.

### 4.4 Script Extension Safety & Gradle Wrapper Policy (BLD-009)
- On Windows, `.cmd` and `.bat` extensions are strictly permitted only for script-based tools (`npm`, `npx`, `yarn`, `pnpm`, `gradlew`). Compiled binary tools (`cargo`, `python`, `rustc`) cannot execute batch script files (`cargo.bat` is rejected).
- Gradle wrapper scripts (`./gradlew`, `gradlew.bat`) are project-local and are permitted to execute only if the workspace root has been verified as trusted (`is_trusted = true`).

### 4.5 Shadow Binary Defense (BLD-002)
To prevent a malicious repository from dropping a trojaned `npm.cmd` or `cargo.exe` into its project directory to hijack build triggers:
- The runner inspects the target directory for local binaries shadowing approved tools.
- Project-local executables matching tool names are rejected with `Security Denial: Project-local binary shadows approved build tool`.
- Approved tools are resolved exclusively against the system `PATH`.

---

## 5. Checkpoint Engine & Byte-Perfect Integrity

### 5.1 Raw Byte Preservation (CHK-001)
Checkpoints record raw binary bytes. Lossy conversions (`String::from_utf8_lossy`) are strictly eliminated. Binary files, compiled assets, images, and non-UTF-8 text files are preserved with exact byte integrity and verified via SHA-256 content hashes.

### 5.2 Conflict Detection & Cryptographic Preview Sessions (CHK-008, CHK-009, CHK-010)
- Prior to restoring a checkpoint, `checkpoint_restore_preview` scans the workspace, calculates current disk states, and generates a cryptographically random `preview_token` (using OS CSPRNG).
- The preview session is cryptographically bound to the `checkpoint_id`, `workspace_id`, and canonical root, with a strict 10-minute expiration window.
- The token is atomically consumed on restore; replay attempts are rejected immediately (`CHK-009`).
- By default, restoration aborts on unresolved conflicts (`PermissionDenied`) unless the user explicitly authorizes them via approved conflicts (`CHK-003`, `CHK-004`).

### 5.3 Explicit Disk State & Full Race Detection (CHK-005, CHK-011, CHK-012)
The preview records each affected file's disk state as `Present { sha256 }`, `Missing`, or `Inaccessible`. Immediately before applying restore bytes:
- If an existing file was modified after preview: rejected (`CHK-005`).
- If an existing file was deleted after preview: rejected (`CHK-011`).
- If a missing target file was created after preview: rejected (`CHK-012`).
- If a file became inaccessible: rejected.

### 5.4 Mandatory Pre-Restore Safety Snapshots (CHK-013)
Before applying any file modifications during restore, Tauri command `checkpoint_restore` creates an internal safety checkpoint backup snapshot of the current workspace state. If creating this backup fails, restoration immediately aborts fail-closed, leaving the workspace files untouched.

---

## 6. AI Gateway Streaming & Cancellation

### 6.1 Genuine SSE Streaming (AI-001, AI-003, AI-004, AI-005)
All official model providers (OpenAI, Anthropic, Google Gemini, DeepSeek, xAI, Local Ollama) implement genuine chunk-by-chunk streaming over Server-Sent Events (SSE) or chunked HTTP.
The default trait method on `ModelProvider` fails closed (`Streaming is not supported for provider '...'`), eliminating fake single-chunk completion disguises.

### 6.2 Asynchronous Cancellation via CancellationToken (AI-002, AI-006, AI-007)
Cancellation utilizes `tokio_util::sync::CancellationToken`:
- Both HTTP connection requests (`request.send()`) and response chunk reads (`stream.next()`) are wrapped in `tokio::select!` with `cancel.cancelled()`.
- If user triggers `ai_cancel_stream`, cancellation immediately aborts pending connection handshakes (`AI-007`) and stalled chunk reads (`AI-006`).
- The underlying `reqwest` HTTP response byte stream is dropped, terminating the TCP/TLS connection to the provider and saving API tokens.
