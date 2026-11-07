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
   - **Residual Race Limitation & The Quarantine Architecture:** Because general-purpose OS kernels do not expose an atomic combined syscall for `hash_verify_and_unlink(fd, expected_hash)`, a minute residual window exists between the userland second hash read and the subsequent unlink syscall. A detached writable handle writing in this exact interval cannot be detected by userland before the unlink syscall executes. AetherIDE makes no unsupported claims of kernel-level immunity against uncooperative detached file descriptors; rather, it adheres to the principle of **recoverability over irreversible deletion**. Where exclusive deletion cannot be proven at the OS level, artifacts are quarantined into dedicated storage, tracked in a durable recovery manifest (`recovery_manifest.json`), and preserved until explicit user or maintenance cleanup.

### 5.6 Quarantine & Durable Recovery Manifest Architecture
To eliminate irreversible destruction of recoverable user data when external processes hold open handles, AetherIDE implements a durable quarantine subsystem:
1. **Durable Recovery Manifest (`recovery_manifest.json`):**
   - Quarantined artifacts are recorded in `.aether/checkpoints/recovery_manifest.json`.
   - Each entry records a unique UUID, original workspace relative path, quarantine filename, quarantine path on disk, ISO-8601 UTC timestamp (`chrono::Utc::now()`), expected digest, actual digest at quarantine time, reason, and status:
     * `PendingMove`: Initial intent record before file movement or during staging.
     * `PendingCleanup`: Quarantined, verified, and awaiting explicit user-controlled cleanup.
     * `CleanedUp`: Successfully cleaned up or confirmed absent.
     * `PreservedConflict`: Preserved due to conflict, tampering, or concurrent modification.
   - Manifest updates use atomic temporary write and rename (`save_with_fault`), with fsync of the file and its parent directory on Unix, preventing corruption across process crashes.
   - If the manifest file exists but cannot be parsed (corrupt JSON), `load_recovery_manifest` preserves the corrupt file by backing it up to `recovery_manifest.json.corrupt.<timestamp>`, initializes a fresh manifest with a `PreservedConflict` record tracking the corrupt backup, logs a critical alert, and continues reconciliation without silent data loss. Unreadable manifests (I/O or permission errors) fail closed immediately.
2. **Durable Quarantine Transaction Protocol & Fail-Closed Cross-Device Fallback:**
   - Movement from workspace into quarantine follows a multi-stage transaction:
     a. Capability-anchored validation of relative workspace path and rejection of symlinks/non-files.
     b. Physical quarantine directory boundary validation (`verify_quarantine_boundary`): ensures storage and quarantine directories are regular directories (not symlinks or Windows junctions/reparse points) and verifies canonical parent-child hierarchy.
     c. Generation of unique artifact ID and safe sanitized filename (alphanumeric, dots, dashes, underscores).
     d. Staging inside workspace for exclusive access and pre-delete hook invocation.
     e. Recording of `PendingMove` state in `recovery_manifest.json`.
     f. Movement into `quarantine_dir`:
        - Primary mechanism is an atomic filesystem rename.
        - **Fail-Closed Cross-Device Copy Fallback:** A copy fallback is ONLY triggered when the rename fails with an error positively identified as an OS cross-device link error (`EXDEV` on Unix, `ERROR_NOT_SAME_DEVICE` on Windows). All other rename failures (permissions, path issues, sharing violations) fail closed immediately, restore the staged source file back to the workspace, and abort without unlinking.
        - True cross-device copies verify destination bytes and SHA-256 against the staged hash.
        - **Pre-Unlink Source Re-Verification & Exclusive Stability Proof:** Immediately prior to unlinking the source file after a successful cross-device copy, the engine re-reads the staged source file, re-verifies its SHA-256 against the expected hash, and verifies exclusive handle stability (using `share_mode(0)` on Windows). If a concurrent process mutated the source via a retained open handle or exclusive stability cannot be verified, **THE SOURCE FILE IS NEVER UNLINKED**. Both copies are preserved: the source file is restored to the workspace, the verified quarantine copy remains in quarantine, both paths are surfaced in error reporting, and the conflict is committed to the manifest as `QuarantineStatus::PreservedConflict`.
     g. Atomically committing `PendingCleanup` state to the recovery manifest.
3. **Startup Reconciliation & Untracked Artifact Adoption (`initialize_and_reconcile`):**
   - Wired directly into application initialization (Tauri setup hook).
   - Audits disk state against the recovery manifest:
     * Records with missing physical files transition to `CleanedUp`.
     * `PendingMove` records where the target file exists with valid SHA-256 are committed to `PendingCleanup`.
     * Records whose target file has been modified or corrupted transition to `PreservedConflict`.
     * Untracked physical regular files found in `quarantine_dir` are adopted into the manifest as `PendingCleanup` (with reason `DiscoveredUntrackedArtifactOnStartup`) without deletion.
     * Untracked symlinks found in `quarantine_dir` are adopted as `PreservedConflict` (never eligible for cleanup).
     * Directory enumeration errors (`read_dir`) are not swallowed; errors propagate fail-closed.
   - **Fail-Closed Engine Gating (`RecoveryBlocked`):** If startup reconciliation encounters an error (I/O, boundary violation, or corrupt manifest), the engine enters an explicit `RecoveryBlocked` state (`is_recovery_blocked() == true`). While blocked:
     * Normal checkpoint creation (`create_checkpoint_with_root`) is strictly rejected.
     * Checkpoint restore operations (`restore_preview_bound`, `restore_checkpoint_with_token`) are rejected.
     * Quarantine cleanup operations (`cleanup_quarantine`, `cleanup_all_quarantined`) are rejected.
     * Quarantine artifact listing (`list_quarantined_artifacts`) fails closed, refusing to report an authoritative or empty manifest until reconciliation succeeds.
     * Recovery status is exposed to frontend via `checkpoint_get_recovery_status`.
     * On-demand retry via `checkpoint_reconcile_quarantine` clears the blocked state upon successful completion.
4. **Physical Quarantine Directory Boundary, Pinned Directory Handles & Symlink Defense:**
   - All quarantine paths are strictly derived from the trusted application storage `quarantine_dir()`.
   - `verify_quarantine_boundary()` validates `storage_dir` and `quarantine_dir` with `symlink_metadata`:
     * Rejects symlinks on all platforms.
     * Rejects `FILE_ATTRIBUTE_REPARSE_POINT` (junctions, mount points) on Windows.
     * Canonicalizes both paths and enforces `canonical_quarantine.parent() == Some(canonical_storage)`.
   - **Pinned Directory Handle Operations (`cap_std::fs::Dir`):** During cleanup (`cleanup_quarantine` and `cleanup_all_quarantined`), operations are performed relative to a pinned directory handle (`Dir::open_ambient_dir(&q_dir, ambient_authority())`) with no-follow semantics. Ambient directory replacement or ancestor symlink swaps cannot redirect deletion outside the opened quarantine directory.
   - **Pre-Unlink Target Object & Content Revalidation:** Immediately before calling `remove_file` through the pinned directory handle, the target's metadata and SHA-256 digest are re-inspected. If the target has been replaced with a symlink, non-regular file, or modified regular file, deletion is immediately aborted and the record is transitioned to `PreservedConflict`.
   - Path traversal attempts (`..`, absolute paths, drive letters, `/`, `\`, `:`) in quarantine records are rejected fail-closed.
5. **Backend Authorization on Cleanup Operations:**
   - Physical deletion (`cleanup_quarantine`, `cleanup_all_quarantined`) strictly enforces backend status authorization:
     * `PendingCleanup`: Authorized for physical removal.
     * `PreservedConflict`: Rejected with `AetherError::PermissionDenied`. Conflict artifacts preserved indefinitely.
     * `PendingMove`: Rejected with `AetherError::PermissionDenied`. Uncommitted artifacts cannot be deleted.
     * `CleanedUp`: Idempotent `Ok(true)` without touching disk, preventing deletion of recreated artifacts at the same path.
   - Prior to unlinking, the file's content hash is verified against `actual_sha256` / `expected_sha256`. If the file was tampered with or modified while in quarantine, cleanup is blocked, the record transitions to `PreservedConflict`, and the file is preserved.
   - **Immediate Batch Cleanup Halt on Persistence Failure:** In `cleanup_all_quarantined`, progress is saved after each individual file removal. If `save_with_fault` fails, **the batch halts immediately**. No further files are deleted. The returned error reports the exact number of files cleaned and the ID of the record that failed persistence.
6. **Transactional Corrupt-Manifest Handling:**
   - When `load_recovery_manifest` encounters JSON deserialization failure (indicating file corruption or truncation):
     * A collision-safe unique backup filename is constructed: `recovery_manifest.json.corrupt.<timestamp>.<uuid>`.
     * The backup file is opened exclusively using `create_new(true)`, guaranteeing existing recovery backups are never overwritten.
     * The entire raw corrupt byte stream is written, synced to disk, and verified via read-back SHA-256 digest against the original source bytes.
     * Any backup creation, write, verification, or persistence failure aborts the operation and **PRESERVES THE ORIGINAL CORRUPT MANIFEST UNMODIFIED**.
     * A fresh manifest is initialized, tracking the verified corrupt backup as `QuarantineStatus::PreservedConflict`.
     * Only after the fresh manifest is successfully persisted is the recovery sequence considered complete.
7. **Production Compensating Rollback Integration:**
   - `handle_compensating_rollback()` automatically applies the durable quarantine mechanism for newly created files during failed restore operations (`defer_to_quarantine: true`).
   - Unmutated newly created files are moved into quarantine, recorded in `recovery_manifest.json`, and surfaced in `CheckpointRestoreReport` under `quarantined_artifacts` with `cleanup_pending: true`.
   - Manifest persistence failures propagate as explicit rollback errors and preserve artifacts in quarantine storage.
8. **Structured UI Surfacing:**
   - Quarantined artifacts, preserved backups, and recovery conflicts are surfaced in the desktop workbench UI with explicit status badges (`PendingCleanup`, `CleanedUp`, `PreservedConflict`), requiring explicit user confirmation before executing any cleanup.
   - Reconcile action in the UI allows triggering on-demand storage reconciliation and displays reconciliation status banners. No automated destructive cleanup is executed in any normal UI workflow.

### 5.3 Sprint 23.2.11 Security Advancements: Target Object Identity, Source Stability & API Gating

1. **Directory Confinement vs. Target-Object Identity:**
   - Pinned directory handles (`cap_std::fs::Dir`) guarantee boundary confinement and prevent path traversal or symlink redirection outside the quarantine directory. However, a pinned directory handle alone does **not** guarantee that a relative filename continues to refer to the identical file object that was hashed if an adversarial or external process swaps or replaces that file between userland verification and unlinking.
   - To guarantee target-object identity, AetherIDE implements the **Stage-Within-Quarantine Protocol**:
     1. The candidate object is atomically moved within the pinned quarantine directory to a unique private staging name (`.aether-clean-stage-<uuid>`).
     2. Metadata and content hash are inspected directly on the isolated stage candidate.
     3. If a concurrent process replaces or recreates the original target filename while staged, cleanup detects the recreated object, aborts removal, and preserves both files without overwriting either.
     4. Rollback via `rollback_cleanup_stage` checks if the original filename was recreated; if so, it never overwrites the new file, but instead preserves the staged candidate under `.preserved-conflict.<uuid>`.
     5. Only the isolated staged candidate is ever unlinked, ensuring external replacements are never deleted or unlinked.

2. **Hash Verification vs. Atomic Deletion:**
   - Userland hash verification (reading bytes, computing a SHA-256 digest, and comparing against an approved hash) does **not** equal atomic check-and-unlink. A race window unavoidably exists between reading the final byte of a file and issuing an `unlink()` or `remove_file()` system call.
   - AetherIDE explicitly documents this limitation and never claims userland hash checks achieve atomic deletion. Instead, destructive cleanup is isolated to uniquely staged objects, and on Windows, relies on exclusive handle sharing semantics.

3. **Platform-Specific Source Stability: Windows Sharing vs. POSIX Retained Descriptors:**
   - **Windows:** Utilizes `remove_file_windows_exclusive`:
     * Opens the file with `share_mode(0)` (no sharing: denying concurrent read, write, or delete) and access rights `GENERIC_READ | GENERIC_WRITE | DELETE`.
     * If any other process holds an open handle, opening immediately fails, proving lack of exclusive stability.
     * While holding the exclusive handle, reads the file, verifies the SHA-256 digest against expected content, and sets `FileDispositionInfo (DeleteFile = 1)`.
     * Closing the handle atomically unlinks the file at the kernel level without race.
   - **POSIX (Linux, macOS):** Standard POSIX semantics do not enforce mandatory file sharing exclusion against uncooperative processes holding open writable descriptors. Therefore, exclusive stability through destructive cross-device operations cannot be proven in userland.
   - **POSIX Fail-Closed Rule:** On Linux and macOS, cross-device fallback **never unlinks the source file**. When cross-device copy occurs, both the workspace source and the quarantine copy are durably preserved, and recorded in the recovery manifest as `QuarantineStatus::PreservedConflict`. Data preservation strictly overrides deletion completion.

4. **RecoveryBlocked Gating: Authoritative vs. Diagnostic Access:**
   - When the engine enters `RecoveryBlocked` (e.g. startup reconciliation failure, database or manifest corruption):
     * **Authoritative Checkpoint APIs Blocked:** `create_checkpoint`, `create_checkpoint_with_root`, `list_checkpoints`, `restore_preview_bound`, `restore_preview`, `restore_checkpoint_with_token`, `consume_preview_token`, `cleanup_quarantine`, `cleanup_all_quarantined`, `list_quarantined_artifacts`, and `quarantine_file_with_cap` strictly reject operations with `PermissionDenied`. Stale state is never presented as authoritative.
     * **Diagnostic Queries Permitted:** Harmless diagnostic status methods (`is_recovery_blocked`, `recovery_blocked_reason`, and Tauri command `checkpoint_get_recovery_status`) remain fully accessible to surface the failure reason to the UI.
     * **Unblocking Authority:** Successful retry of `reconcile_quarantine()` is the **sole operation** authorized to clear `RecoveryBlocked`.

5. **Collision-Safe Corrupt-Manifest Recovery:**
   - When startup detects a corrupt manifest, backup creation executes a retry loop (up to 5 attempts) using `create_new(true)`.
   - Pre-existing backup files are never overwritten on collision.
   - The retry loop generates fresh collision-safe UUID backup names and verifies the read-back digest before constructing a fresh manifest. If interrupted or failed, the original corrupt manifest and all prior backups remain byte-for-byte intact.

### 5.4 Sprint 23.2.12 Security Advancements: POSIX Fail-Closed Disposal, Windows Reparse-Point Defense & Verified Publication

1. **POSIX Fail-Closed Quarantine Disposal:**
   - On Linux and macOS (POSIX), kernel filesystem semantics permit directory entries to be unlinked while detached open writable file descriptors remain connected to the underlying inode. Any write through such an open descriptor after userland verification alters unreferenced blocks before the file descriptor closes.
   - Renaming a quarantine file into a private staging name (`.aether-clean-stage-<uuid>`) within the pinned quarantine directory isolates directory path lookups, but does **not** revoke or invalidate pre-existing open file descriptors connected to that inode.
   - Userland check-and-unlink sequences cannot guarantee atomic exclusive disposal on POSIX without mandatory OS-level file locking. Advisory locks (`flock`, `fcntl`) do not constrain uncooperative external processes holding writable descriptors.
   - Therefore, AetherIDE strictly enforces **POSIX Fail-Closed Disposal**:
     * In both single cleanup (`cleanup_quarantine`) and batch cleanup (`cleanup_all_quarantined`), automated physical unlinking is unsupported and refused on POSIX systems.
     * The candidate artifact is safely rolled back into a verified conflict artifact (`<filename>.preserved-conflict.<uuid>`).
     * The recovery manifest is updated to `QuarantineStatus::PreservedConflict` referencing the actual surviving artifact path on disk.
     * An explicit `PermissionDenied` error is returned explaining that kernel semantics cannot guarantee exclusive stability against uncooperative open descriptors.
     * User data is never destroyed under uncertain or unprovable exclusive stability.

2. **Windows Reparse-Point & Object-Identity Defense (`remove_file_windows_exclusive`):**
   - Path-based file opens can follow reparse points (symlinks, junctions, mount points) if opened without explicit kernel flags.
   - To defend against symlink swapping or reparse point redirection during cleanup:
     * `remove_file_windows_exclusive` specifies `custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)`, opening the reparse point object itself rather than following the redirection target.
     * It requests `GENERIC_READ | GENERIC_WRITE | DELETE` access with exclusive sharing `share_mode(0)`. If any other process holds an open handle, open immediately fails.
     * It queries `GetFileType` and strictly verifies `FILE_TYPE_DISK`. Named pipes, character devices, and remote objects are rejected immediately.
     * It queries `GetFileInformationByHandle` and verifies `dwFileAttributes`:
       - If `FILE_ATTRIBUTE_REPARSE_POINT` is set, exclusive deletion is rejected immediately to protect external link targets.
       - If `FILE_ATTRIBUTE_DIRECTORY` is set, deletion is rejected immediately.
     * The exclusive handle is held continuously across SHA-256 digest verification and `SetFileInformationByHandle(FileDispositionInfo, DeleteFile = 1)`.
     * Dropping the handle commits atomic deletion at the kernel level without race.

3. **Strict Metadata Error Classification:**
   - Previous implementations relied on `.is_ok()` existence checks, which conflated `NotFound` (confirmed absence) with non-notfound errors (such as `PermissionDenied` or general I/O errors).
   - AetherIDE implements `classify_dir_entry_existence`, returning an explicit three-state `ObjectExistence`:
     * `ObjectExistence::Exists`: Metadata inspection succeeded (`Ok(metadata)`).
     * `ObjectExistence::ConfirmedAbsent`: The filesystem returned `ErrorKind::NotFound`.
     * `ObjectExistence::Uncertain(reason)`: Any other error (e.g., `PermissionDenied`, I/O failure).
   - In all rollback and cleanup operations, non-absent and uncertain states are treated as potentially existing objects. Staged artifacts are never overwritten or deleted under uncertain metadata; instead, they are preserved as conflict artifacts.

4. **Rollback Verified Actual Publication:**
   - `rollback_cleanup_stage` never assumes that attempting a rename succeeded.
   - It verifies destination absence prior to rename.
   - If the original filename exists or is uncertain, it executes a publication retry loop (up to 5 attempts) generating collision-safe conflict filenames (`<filename>.preserved-conflict.<uuid>`).
   - After rename, it verifies actual physical publication using `classify_dir_entry_existence`.
   - If all safe publication attempts fail, the candidate is preserved at its staging name without overwrite, and the engine enters `RecoveryBlocked` to alert administrators.

5. **Fail-Closed Manifest Persistence:**
   - Removed all occurrences of ignored manifest persistence (`let _ = manifest.save_with_fault(...)`).
   - If manifest persistence fails during single or batch cleanup, the operation halts immediately.
   - On Windows, if a file was physically removed before manifest persistence failed, the engine records the exact inconsistency and enters `RecoveryBlocked`, ensuring startup reconciliation can safely recover state without silent data loss.

### 5.5 Sprint 23.2.13 Security Advancements: Atomic No-Overwrite Rollback, Windows Staged-Object Identity & Fail-Closed Persistence

1. **Atomic No-Overwrite Publication via Hard Link (`publish_no_overwrite`):**
   - Ordinary POSIX/Windows `rename()` atomically replaces any existing destination file. Relying on an existence check followed by `rename()` creates a TOCTOU race: an external actor can recreate the destination file between the check and rename, causing the rollback to silently overwrite the newly created file.
   - AetherIDE replaces rename-based rollback with an atomic no-overwrite publication primitive using `q_cap_dir.hard_link(...)`:
     * Before attempting publication, destination absence is verified via `classify_dir_entry_existence`.
     * `q_cap_dir.hard_link(clean_stage_name, q_cap_dir, destination_name)` is invoked. Under NTFS and standard POSIX filesystems, hard link creation is atomic and strictly fails with `ErrorKind::AlreadyExists` if the destination path already exists.
     * If the destination was recreated concurrently, the hard link fails atomically without touching or overwriting the destination file.
     * The engine falls through to conflict publication, generating a collision-safe unique filename (`<filename>.preserved-conflict.<uuid>`).
     * Only after `hard_link` succeeds and post-publication existence is verified on disk does the engine remove `clean_stage_name`.

2. **Resilience to Staging Link Removal Failure:**
   - If the publication hard link succeeds and post-publication existence is confirmed, the artifact is safely and durably published at its destination.
   - If removing the staging link (`clean_stage_name`) subsequently fails (due to permissions or transient filesystem lock), the operation **does not** discard or invalidate the published file.
   - `StageRollbackResult` records `is_conflict` accurately, sets `surviving_filename` to the published destination, and includes a structured warning detailing the staging link cleanup error. Data is preserved without silent loss or false error reports.

3. **Staging Name Retention & Fail-Closed RecoveryBlocked Gating:**
   - If all publication attempts (original destination and 5 conflict attempts) fail, the staged candidate is preserved on disk at `clean_stage_name`.
   - The engine immediately activates `RecoveryBlocked`, recording the exact failure reason and staging filename.
   - The recovery manifest references the actual staging path, guaranteeing startup reconciliation can locate and recover the artifact.

4. **Windows Staged-Object Identity Binding (`FileObjectIdentity`):**
   - In NTFS and ReFS, an object's file system identity is uniquely determined by `(dwVolumeSerialNumber, nFileIndexHigh, nFileIndexLow)`.
   - In `CheckpointEngine`, immediately following the rename of a quarantine file to its private staging name (`clean_stage_name`), `filesystem_engine::query_file_object_identity` captures this identity.
   - In `remove_file_windows_exclusive`:
     * Opens the file with `custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)` and `share_mode(0)` (exclusive sharing).
     * Calls `GetFileType` and confirms `FILE_TYPE_DISK`.
     * Calls `GetFileInformationByHandle` and validates `actual_identity == expected_identity`.
     * Rejects any identity mismatch fail-closed, ensuring that if an external actor replaced or substituted the staged file (even with identical content bytes or an identical reparse point), deletion is aborted and the staged file is preserved.
     * Verifies content SHA-256 digest through the same open exclusive handle.
     * Applies `FileDispositionInfo (DeleteFile = 1)` only after identity and digest verification both succeed.

5. **Universal Fail-Closed Manifest Persistence (`persist_manifest_or_block_recovery`):**
   - Eliminates all unhandled or ignored manifest saves, including the previous unhandled save on missing entries in batch cleanup.
   - Every single-cleanup and batch-cleanup manifest update routes through `persist_manifest_or_block_recovery`:
     * If saving fails, `RecoveryBlocked` is activated with structured context: affected record ID, surviving artifact path (or `"(physically removed)"` / `"(already absent)"`), and the underlying I/O error.
     * In batch cleanup (`cleanup_all_quarantined`), any persistence error immediately halts processing; subsequent items are not touched.
     * The IDE refuses all further destructive or state-changing checkpoint operations until a successful startup reconciliation (`reconcile_quarantine`) re-synchronizes the manifest with disk state.

### 5.6 Sprint 23.2.14 Security Advancements: Race-Free Windows Identity Binding, Recoverable Hard-Link Rollback & Durable Staging Reconciliation

1. **Object Identity Binding to the Original Authorized Target (`FileObjectIdentity`):**
   - Previous implementations captured object identity only *after* staging the file into its isolated `.aether-clean-stage-*` name. If an adversary replaced the quarantine entry prior to or during staging, the newly created replacement identity would be adopted as authoritative.
   - In Sprint 23.2.14, `CheckpointEngine` establishes the physical file object identity (`FileObjectIdentity { volume_serial_number, file_index_high, file_index_low }`) at **Step 0** directly on the authorized original quarantine entry (`target_path`) before staging:
     * Cross-platform support: Windows queries `GetFileInformationByHandle` (`dwVolumeSerialNumber`, `nFileIndexHigh`, `nFileIndexLow`); Unix queries `st_dev` and `st_ino`. Reparse points, symlinks, and directories are strictly rejected.
     * Immediately following the atomic rename to `.aether-clean-stage-*`, the staged entry's object identity is queried and strictly compared against the original pre-staging identity.
     * Any drift, mismatch, reparse point substitution, or query failure immediately halts disposal, executes atomic no-overwrite rollback to preserve both objects, updates the manifest to `PreservedConflict`, and returns `PermissionDenied` fail-closed.
     * The original identity is passed directly to `remove_file_windows_exclusive`, ensuring that the kernel-level exclusive handle is verified against the original authorized artifact before deletion disposition is committed.

2. **Race-Free Hard-Link Rollback Publication Recoverability:**
   - When rolling back a staged artifact (`rollback_cleanup_stage`), creating a hard link at the target destination (`publish_no_overwrite`) establishes a new directory entry pointing to the underlying inode/file index.
   - Merely verifying that the destination name exists on disk after `hard_link` does **not** prove that the destination name refers to the staged object, because an external process could have created a different file at that path immediately following a link race or collision.
   - In Sprint 23.2.14, `publish_no_overwrite` establishes the staged file's object identity prior to linking and verifies that the created destination shares the **identical physical object identity** before considering publication verified:
     * Post-link destination identity is verified against the staged candidate's identity.
     * Before removing the staging link, the staging link's identity is re-verified.
     * If destination object identity does not match, or if verification fails, the staging link is strictly preserved on disk. The engine never unlinks the only known recoverable name of an authorized artifact.
     * The structured rollback result returns `RollbackPublicationStatus`:
       - `PublishedVerified`: Destination verified matching identity; staging link cleanly unlinked.
       - `PublishedWithRetainedStaging`: Destination verified; staging link retained on disk due to removal error.
       - `PublicationConflict`: Published as collision-safe unique conflict file.
       - `PublicationUncertainBlocked`: Neither original nor conflict destination could be proven recoverable; staging file retained and `RecoveryBlocked` activated.

3. **Durable Manifest Representation of Retained Staging Artifacts:**
   - Any staging link retained on disk during rollback is explicitly tracked in the durable recovery manifest via `retained_staging_filename: Option<String>` and `retained_staging_path: Option<String>` on `QuarantineRecord`.
   - The UI displays explicit badges for retained staging links, preventing untracked disk accumulation.
   - Startup reconciliation (`reconcile_quarantine`) scans quarantine storage for any `.aether-clean-stage-*` files:
     * If referenced in an existing record's `retained_staging_filename`, reconciliation validates existence and preserves the conflict state.
     * If unreferenced/untracked staging files exist on disk, reconciliation automatically adopts them into the durable manifest as `QuarantineStatus::PreservedConflict`, calculating SHA-256 digests and surfacing them for manual inspection.
     * An engine in `RecoveryBlocked` can only be unblocked once reconciliation successfully proves all artifacts are tracked and consistent.


### 5.7 Sprint 23.2.15 Security Advancements: Final Rollback Identity Integrity, Atomic Recoverability & Reconciliation Fail-Closed Audit

1. **Original Identity Binding Through Every Rollback Path:**
   - Previous implementations could capture a new identity during rollback if staging substitution occurred, potentially publishing an adversarial replacement to the original filename.
   - In Sprint 23.2.15, the original pre-staging `FileObjectIdentity` captured at Step 0 is strictly propagated through all rollback functions (`rollback_cleanup_stage` and `publish_no_overwrite`).
   - If the candidate file in staging does not match the original authorized identity:
     * It is **never** published to the original filename or accepted as the authorized artifact.
     * It is safely renamed to `<filename>.suspicious-replacement.<uuid>` in quarantine storage.
     * The actual SHA-256 digest of the surviving replacement is computed and recorded in the manifest.
     * `RecoveryBlocked` is activated with explicit diagnostics explaining that the original object was lost or substituted.
   - If Step 0 fails to capture identity due to any non-NotFound error, rollback also activates `RecoveryBlocked` fail-closed.
   - Batch cleanup (`cleanup_all_quarantined`) and single cleanup enforce identical identity-binding contracts.

2. **Kernel-Guaranteed vs. POSIX Retained Staging Disposal:**
   - In `publish_no_overwrite`:
     * Before removing the staging directory entry, `pre_staging_link_removal_hook` is called, and both destination and staging identities are re-verified against the original authorized identity.
     * On Windows: `remove_file_windows_exclusive(&stage_abs, &expected_hash, Some(expected_identity))` is used. This opens the handle exclusively (`share_mode(0)`), checks volume and file index against the authorized identity, verifies the byte digest, and applies `FileDispositionInfo (DeleteFile = 1)`. If an adversary replaced the file, the identity check fails and deletion is rejected with `PermissionDenied`. The replacement artifact is never deleted as a side effect.
     * On POSIX: Standard userland `unlink(2)` cannot provide an atomic identity-bound deletion guarantee across separate directory entries against open handles. Therefore, POSIX strictly retains the staging link (`PublishedWithRetainedStaging`), preventing any possibility of data loss or deleting the only remaining copy of an artifact.
     * Post-removal verification checks destination existence and identity on disk. If the destination is missing or its identity drifted, `RecoveryBlocked` is activated immediately.

3. **Explicit, Fail-Closed Reconciliation Error Classification:**
   - `reconcile_quarantine` strictly audits all entries. Only `std::io::ErrorKind::NotFound` marks a quarantine artifact absent (`CleanedUp`).
   - All other errors (permission denied, metadata errors, I/O read errors):
     * Transition the quarantine record to `PreservedConflict`.
     * Retained staging links that cannot be inspected never cause records to be marked `CleanedUp`.
     * Untracked or tracked symlinks/reparse points found in quarantine are recorded as suspicious objects without following them.
     * `RecoveryBlocked` is activated fail-closed with detailed diagnostic messages identifying the offending file and failure reason.
   - Clean reconciliation runs (where all artifacts are inspectable and verified) clear `RecoveryBlocked` and return the engine to normal operation.
   - Reconciliation is idempotent: repeated runs do not duplicate records for existing tracked or untracked artifacts.

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
