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

### 5.2 Conflict Detection & Cryptographic Single-Use Preview Tokens
- Prior to restoring a checkpoint, `restore_preview_bound` scans the workspace, calculates current disk hashes, and generates a cryptographically random, single-use `preview_token` formatted as `prev-<uuid4>-<sha256>`. The token is generated via system CSPRNG (`uuid::Uuid::new_v4()`) and is strictly bound to `(token, checkpoint_id, workspace_id, canonical_root)`.
- If any file was modified on disk since the checkpoint snapshot, a conflict is flagged.
- By default, restoration aborts on unresolved conflicts (`PermissionDenied`) unless the user explicitly authorizes them via `force: true` or `explicit_overwrite_conflicts`.
- Legacy un-tokenized restoration methods (`restore_checkpoint` and `restore_checkpoint_into`) are deprecated and strictly reject all invocations with `PermissionDenied`, eliminating any bypass of preview token validation.

### 5.3 Staged-Swap Conditional Replacement & TOCTOU Defense
To close the Time-of-Check to Time-of-Use (TOCTOU) race condition window between preview generation and restoration across Windows, Linux, and macOS:
1. **In-Memory Blob Preflight:** All checkpoint blobs are pre-verified and loaded into memory before touching disk, preventing partial restoration due to mid-flight blob corruption or missing files.
2. **In-Process Path Synchronization:** `CheckpointEngine` synchronizes restoration across concurrent threads using an internal `active_restore_paths` set guarded by RAII `PathLockGuard`. Attempting to restore to a path currently undergoing an active restore immediately rejects the concurrent operation fail-closed.
3. **Immediate Per-File Pre-Flight Revalidation:** Immediately before replacement, `CheckpointEngine::verify_file_disk_state` inspects the destination path with `symlink_metadata`:
   - Distinguishes `ErrorKind::NotFound` from `ErrorKind::PermissionDenied` and other I/O errors fail-closed.
   - Rejects the restoration if the file was modified, deleted, newly created, or replaced with a symlink or directory after preview generation.
4. **Failure-Safe Missing-File Restoration:**
   - Content is written to a temporary staged file (`.aether-stage.<uuid>`) and explicitly synced to durable storage (`sync_all()`).
   - If writing or syncing fails, the temporary stage file is cleaned up immediately, guaranteeing that no partial, empty, or corrupted destination file is ever left at the target path.
   - The verified file is published using an atomic exclusive operation (`hard_link`). If another process created a file at the target path concurrently, the operation rejects fail-closed with `AlreadyExists`, cleans the stage file, and leaves the existing file untouched.
5. **Conditional Replacement with Staged-Swap, Post-Hash Recheck & Rollback:**
   - **For Present Files:** Writes incoming data to `.aether-stage.<uuid>` and syncs it. Atomically renames the destination file to `.aether-backup.<uuid>`.
   - **Initial Hash Verification:** Computes the SHA-256 hash of `.aether-backup` to ensure it matches the pre-flight expected hash.
   - **Post-Hash Re-Verification (Open Handle Trap):** Re-checks the hash and metadata of `.aether-backup` immediately before committing the replacement. If a non-cooperating process holding an open file handle modified the file after the initial hash check, the mismatch is detected and restoration aborts.
   - **Automatic Rollback:** On detected race or modification, the engine immediately rolls `.aether-backup` back to the destination path, preserving concurrent edits intact.
   - **Trailing Modification & Integrity Guard:** After the stage file is committed to the target path, `.aether-backup` is re-inspected before removal:
     - A failed or incomplete backup read is **never** interpreted as proof that the backup is unchanged. Any read failure causes the engine to preserve the backup unconditionally.
     - If trailing modifications or unverified states are detected, the backup is preserved rather than unlinked.
     - The engine attempts to rename the preserved backup to `.aether-conflict-<uuid>`. If rename fails, the file remains preserved at `.aether-backup-<uuid>`, and the engine reports the actual surviving path rather than a fictitious conflict path.
     - Backup cleanup checks removal results. If removal fails, the surviving backup file path is returned in `ConditionalReplaceOutcome` for transparent tracking.

#### 5.3.1 Observable & Recoverable Rollback Failures
Every rollback operation is strictly checked (`perform_rollback`). Ignored rollback results (`let _ = rename(...)`) are eliminated:
- If a rollback rename fails (e.g. destination file locked exclusively by another process or OS permissions altered):
  - The backup file is **never deleted**. It is preserved intact at `.aether-backup-<uuid>`.
  - The engine returns an explicit `AetherError::Internal` stating `RESTORE ROLLBACK FAILURE`, reporting the exact failure reason, and providing the full absolute path of the preserved backup file for manual recovery.
  - The engine never reports that rollback succeeded when it failed.

#### 5.3.2 Residual Threat Model & OS Platform Invariants
Neither POSIX (`rename`) nor Windows Win32 (`MoveFileExW`) provides an atomic kernel-level syscall of the form `rename_if_source_hash_matches(from, to, expected_hash)`. Consequently:
- **Threat Boundary:** On general-purpose operating systems without kernel volume snapshots (VSS, ZFS/Btrfs snapshot transactions), non-cooperating external processes holding pre-existing write handles write to the underlying file inode/object. The engine's two-phase recheck catches modifications before commit (rolling back intact) and guards backup cleanup after commit (preserving diverged backups or unverified states). If an external process writes to an open handle after post-verification and before unlink, OS-level handle behavior governs: on Windows, active file locks typically prevent unlink; on POSIX systems, the open handle writes to the unlinked inode. While this minimizes concurrent data loss risks, kernel volume snapshots remain necessary for absolute OS-level transaction atomicity.
- **Rollback Guarantee:** If any error or digest mismatch occurs during the swap, the engine initiates synchronous atomic rollback from `.aether-backup` to restore the pre-existing disk state before surfacing an error to the user.
- **Symlink Protection:** Directory and file traversals are guarded by `WorkspaceCapability` path verification, preventing symlink escapes or directory clobbering.

### 5.4 Mandatory Pre-Restore Safety Snapshots
Before applying any file modifications during restore, `CheckpointEngine::restore_checkpoint_with_token` automatically creates a mandatory safety checkpoint snapshot of the current workspace state. If snapshot generation fails for any reason (including actual OS filesystem permission errors or disk I/O failures), restoration immediately aborts fail-closed, leaving all workspace files completely untouched. Clean retry is supported once filesystem permissions or storage directories are corrected.

### 5.5 Multi-File Restoration Failure & Conflict-Aware Compensating Rollback
Multi-file restore across a workspace directory is **NOT** an ACID filesystem transaction, because general-purpose operating system filesystems do not support atomic multi-file multi-directory commits without filesystem-level volume snapshots:
1. **Immediate Commit Tracking:** The engine tracks each file in the committed set immediately after its staged replacement succeeds. If post-write verification, reading, or hashing subsequently fails, the current file is already tracked and cannot be omitted from compensating rollback.
2. **Capability-Anchored Conditional Rollback (Conflict Awareness):** The engine rejects unconditional overwriting or deletion during compensation:
   - Before compensating an existing file, disk content is verified against the restored bytes. If unmodified, the pre-restore content from the safety checkpoint is restored using staged conditional replacement. If modified concurrently by an external process or user, the file is **preserved on disk without overwrite** and reported as a recovery conflict.
   - When restoring an originally existing file that is missing on disk (`NotFound`), compensation uses exclusive no-overwrite creation (`replace_file_bytes_conditional_with_hooks` with `expected_hash: None` via `hard_link`). If an external process recreates the destination between inspection and publication, the concurrently recreated destination is **never overwritten**, the conflict is recorded, and clean rollback count is not incremented.
   - Newly created files are conditionally deleted using capability-anchored staged deletion (`delete_file_conditional_with_hooks`). The file is moved to a private staging name (`.aether-del-stage-<uuid>`), verified against its expected hash, re-verified unconditionally immediately before unlinking in all production paths to catch open writable handle mutations, and unlinked only if verified.
   - If an open writable handle modifies the staged file, if verification fails, or if deletion removal fails, the staged file is **preserved as a recovery conflict artifact** instead of deleted.
   - When rolling back a staged file, the engine uses **atomic no-overwrite publication only (`hard_link`)** with **zero overwrite-capable rename fallbacks**. If the hard-link publication fails for any reason (including `AlreadyExists`, permissions, or cross-device limitations), the staged file is preserved under a unique conflict path (`.aether-del-conflict-<uuid>`) and the recreated destination is **never overwritten**. Both files survive on disk.
   - File absence during rollback is verified strictly via capability `symlink_metadata`, treating **strictly `NotFound`** as confirmed destination absence. Inaccessible files (`PermissionDenied`), I/O errors, and uncertain metadata states are treated as conflicts and fail closed into conflict preservation.
3. **Explicit Recovery Reporting:** The engine distinguishes cleanly compensated files from files requiring manual recovery:
   - `CheckpointRestoreReport` surfaces `restored_files`, `preserved_backups`, `compensated_files`, and `recovery_conflicts`.
   - Desktop workbench UI directly surfaces surviving artifact and preserved backup paths to the user.
   - The engine never reports complete or clean rollback when any staged artifact remains on disk or cleanup fails.
4. **Safety Snapshot Preservation:** Regardless of whether compensating rollback succeeds, partially succeeds, or fails, the mandatory safety checkpoint snapshot is permanently preserved in storage (`.aether/checkpoints/<id>`) for manual recovery.
5. **Exact Verification Guarantees & Residual Kernel Boundaries:**
   - **Verified Content at Time of Final Hash:** Unconditional pre-unlink verification ensures that content matches the expected digest at the microsecond of the second verification read. Any modification written through pre-existing open handles before this read is detected and causes deletion to abort fail-closed.
   - **Atomic No-Overwrite Publication:** Rollback to destination occurs strictly via atomic `hard_link`. Unlike `rename(stage, dest)`, `hard_link` atomically fails if `dest` already exists at the OS kernel level, guaranteeing that concurrent file recreation can never be clobbered.
   - **Platform-Specific Open-Handle Behavior:**
     - On Windows, file locking and sharing flags (`FILE_SHARE_READ`, `FILE_SHARE_WRITE`, `FILE_SHARE_DELETE`) govern handle interactions. Retained handles without delete sharing cause staging rename or unlinking to fail with sharing violations; AetherIDE treats these fail-closed and preserves the artifacts.
     - On Linux and macOS, POSIX filesystem semantics allow unlinking a directory entry while open file descriptors remain valid and connected to the inode. Data written through an open descriptor after the unlink syscall executes is held on the unreferenced inode until the descriptor closes, at which point the kernel releases the storage blocks.
   - **Residual Race Limitation:** Because general-purpose OS kernels do not expose an atomic combined syscall for `hash_verify_and_unlink(fd, expected_hash)`, a minute residual window exists between the userland second hash read and the subsequent unlink syscall. A detached writable handle writing in this exact interval cannot be detected by userland before the unlink syscall executes. AetherIDE makes no unsupported claims of kernel-level immunity against uncooperative detached file descriptors; rather, it guarantees that all detectable mutations, I/O uncertainties, and rollback collisions choose data preservation over silent loss.

---

## 6. AI Gateway Streaming & Cancellation

### 6.1 Genuine SSE Streaming
All official model providers (OpenAI, Anthropic, Google Gemini, DeepSeek, xAI, Local Ollama) implement genuine chunk-by-chunk streaming over Server-Sent Events (SSE) or chunked HTTP.
The default trait method on `ModelProvider` fails closed (`Streaming is not supported for provider '...'`), eliminating fake single-chunk completion disguises.

### 6.2 Connection Abort on Cancellation
When a user clicks "Stop" or cancels an active stream (`ai_cancel_stream`):
- The atomic cancellation token (`CancellationToken`) is flagged.
- The active streaming loop terminates immediately.
- Stalled HTTP error body reads (`resp.text()`) and connection establishments are bound to cancellation via `tokio::select!`, preventing un-cancellable blocking on unresponsive upstreams.
- The underlying `reqwest` HTTP response byte stream is dropped, terminating the TCP/TLS connection to the provider and saving user API tokens.

### 6.3 Stream Registry Lifecycle & Monotonic Generation Tracking
In desktop `ai_stream_prompt`, active streams are managed by `StreamRegistry`:
- **Duplicate Request Rejection:** Active, non-cancelled requests strictly reject duplicate IDs fail-closed.
- **Immediate Reuse via Generations:** When a user cancels a stream and immediately resubmits using the same `request_id`, `StreamRegistry::register` allocates a monotonically increasing `generation: u64` counter and issues a fresh, un-cancelled `CancellationToken`.
- **Race-Free Cleanup:** When the previous asynchronous stream task terminates and invokes `cleanup(request_id, generation)`, it only removes the registry entry if its generation matches. Stale completions from earlier cancelled streams cannot prematurely delete or invalidate the active token of a newer generation.
