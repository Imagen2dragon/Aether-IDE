# AetherIDE Sprint 11 — True Capability Filesystem Architecture

**Product:** AetherIDE  
**Crate:** `filesystem-engine`  
**Core Primitive:** `cap_std::fs::Dir`  
**Security Standard:** Capability-Based Security Model  
**Date:** October 31, 2026  

---

## 1. Architectural Motivation

Pathname-based validation (using string manipulation or canonicalization) is susceptible to Time-of-Check to Time-of-Use (TOCTOU) race conditions when an adversarial process substitutes a directory with a symlink or junction between validation and file access.

To achieve complete containment, Sprint 11 transitioned `filesystem-engine` to a **true capability-based filesystem** using `cap_std` v4.0.3.

---

## 2. Implementation: `WorkspaceCapability`

```rust
pub struct WorkspaceCapability {
    dir: cap_std::fs::Dir,
    root: PathBuf,
}
```

### 2.1 Core Invariants
1. **Handle-Anchored Operations:**
   - The workspace root is opened once as an OS directory handle (`cap_std::fs::Dir::open_ambient_dir`).
   - All file and directory operations (`open`, `create`, `write`, `read`, `remove`, `rename`) are executed as relative operations anchored directly to the directory capability.
   - Operating system kernel boundaries prevent directory-relative syscalls (`openat`, `unlinkat`, `renameat`) from escaping the root.

2. **Rigorous Input Sanitization (`sanitize_relative_path`):**
   - **Control Characters:** Rejects null bytes (`\0`) and ASCII control characters (`< 0x20`).
   - **Traversals:** Blocks parent directory components (`..`), root slashes (`/`), and backslashes (`\`).
   - **Namespaces:** Blocks Windows device namespaces (`\\?\`, `\??\`, `\\.\`) and UNC network paths (`//`, `\\\\`).
   - **Alternate Data Streams:** Blocks `:` stream separators.
   - **Device Names:** Blocks Win32 reserved device identifiers (`CON`, `PRN`, `AUX`, `NUL`, `COM1-9`, `LPT1-9`) regardless of file extension.
   - **Trailing Truncation:** Blocks trailing spaces and dots that Win32 would automatically strip.

3. **Atomic Contained Writes:**
   - Writes are executed to a temporary file created inside the directory capability (`.aether_tmp_<uuid>`).
   - The temporary file is flushed and synced (`sync_all`).
   - Renamed into the target path atomically via `dir.rename()`.
   - On error, the temporary file is unlinked.

---

## 3. Verification & Compliance
All public `FilesystemEngine` methods (`scan_workspace_files`, `write_file_contained`, `read_file_contained`, `delete_file_contained`, `create_dir_contained`, `rename_contained`) route exclusively through `WorkspaceCapability`.
