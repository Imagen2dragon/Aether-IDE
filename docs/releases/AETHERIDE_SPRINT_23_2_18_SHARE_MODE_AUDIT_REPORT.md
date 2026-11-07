# AetherIDE — Sprint 23.2.18 Windows Share-Mode Consistency Audit Report
## Windows Kernel Sharing Enforcement, Dual-Handle Destination Protection & CHK-210 Verification

**To:** ChatGPT — Lead Architect / Independent Engineering Reviewer  
**From:** Antigravity — Implementation Engineer  
**CC:** Project Owner  
**Repository:** `Imagen2dragon/Aether-IDE`  
**PR:** [#24](https://github.com/Imagen2dragon/Aether-IDE/pull/24) — `fix/sprint-23.2.6-production-delete-verification`  
**Starting HEAD:** `a2d64ad3e65423a888bb1566327477e40432f66b`  
**Base:** `d943bce93dcd9455817ee319d202c2db4d3f5bb5` on `main`  
**Release Target:** `v0.1.0-beta.2`  
**PR Status:** **OPEN and UNMERGED** (`mergeable: MERGEABLE`, `mergeStateStatus: CLEAN`)

---

## 1. Executive Summary & Governance Compliance

In strict compliance with the **Sprint 23.2.18 Engineering Directive**:
- **Zero Merges:** PR #24 remains strictly **OPEN and UNMERGED**. The Project Owner retains exclusive merge authorization.
- **Zero Direct Pushes:** No commits or pushes were made directly to `main`.
- **Zero Force-Pushes:** Git history is strictly preserved without rewrites (`git push --force` was never executed).
- **Core Security Controls Maintained & Strengthened:**
  - The production `FILE_SHARE_READ` sharing mode is strictly preserved without weakening.
  - In `remove_file_windows_exclusive()`, the destination path is now held open with `GENERIC_READ` and `share_mode(FILE_SHARE_READ)` concurrently with the staging handle (`GENERIC_READ | DELETE` and `share_mode(FILE_SHARE_READ)`).
  - This dual-handle lock physically prevents concurrent deletion, truncation, or modification of **both** the staging artifact and the destination artifact by the Windows NT kernel.
  - Destination object identity, file type, and non-reparse status are verified directly through the open destination handle (`GetFileInformationByHandle`), closing all TOCTOU windows.
  - Staging link deletion disposition (`SetFileInformationByHandle(FileDispositionInfo, DeleteFile: 1)`) executes only after both handles verify identity and digest integrity.
  - POSIX fail-closed retained staging behavior (`PublishedWithRetainedStaging`) remains completely unchanged.
  - `RecoveryBlocked` and manifest persistence safeguards remain intact.
  - Complete CHK-188 through CHK-219 test suite coverage is preserved and expanded (now **210 passing tests** in `checkpoint-engine`).

---

## 2. Actual Windows Kernel Sharing Behavior & Audit Findings

### 2.1 Windows File Sharing Investigation
We conducted targeted empirical execution on the live Windows kernel (NTFS filesystem, Windows 11 x64, MSVC toolchain) to investigate the interaction between `FILE_SHARE_READ`, hard links, and unlinking.

1. **Exact Windows Sharing Mode in `remove_file_windows_exclusive()`:**
   - **Staging Handle (`file`):**
     * `access_mode`: `GENERIC_READ | DELETE` (`0x80010000`)
     * `share_mode`: `FILE_SHARE_READ` (`0x00000001`)
     * `custom_flags`: `FILE_FLAG_OPEN_REPARSE_POINT` (`0x00200000`)
   - **Destination Handle (`dest_file`, added in Sprint 23.2.18):**
     * `access_mode`: `GENERIC_READ` (`0x80000000`)
     * `share_mode`: `FILE_SHARE_READ` (`0x00000001`)
     * `custom_flags`: `FILE_FLAG_OPEN_REPARSE_POINT` (`0x00200000`)

2. **Hard Link Identity at Moment of Hook Execution:**
   - Immediately prior to calling `remove_file_windows_exclusive()`, step 4 of `publish_no_overwrite()` invokes `q_cap_dir.hard_link(clean_stage_name, q_cap_dir, destination_name)`.
   - At the moment the hook executes, `dest` is confirmed to be a hard link to `stage_abs`:
     * `nNumberOfLinks == 2`
     * `dwVolumeSerialNumber`, `nFileIndexHigh`, and `nFileIndexLow` are identical.

3. **Can `std::fs::remove_file(dest)` succeed while `stage_abs` is open with `FILE_SHARE_READ`?**
   - **When only `stage_abs` is held open:**
     * Attempting `std::fs::remove_file(stage_abs)` is blocked by Windows with `ERROR_SHARING_VIOLATION` (OS error 32).
     * However, in NTFS, sharing access checks (`IoCheckShareAccess`) track deletion against the open file object and its specific directory entry name. Because `dest` is a hard link with `nNumberOfLinks > 1` and `dest` had no open handle, NTFS previously permitted removing the directory entry for `dest`.
   - **When both `stage_abs` AND `dest` are held open with `FILE_SHARE_READ` (Sprint 23.2.18 implementation):**
     * Attempting `std::fs::remove_file(dest)` **FAILS IMMEDIATELY** with:
       ```text
       Err(Os { code: 32, kind: Uncategorized, message: "The process cannot access the file because it is being used by another process." })
       ```
     * Attempting `std::fs::remove_file(stage_abs)` **FAILS IMMEDIATELY** with OS error 32.
     * Attempting to open `dest` or `stage_abs` for writing **FAILS IMMEDIATELY** with OS error 32.
     * Therefore, Windows kernel sharing enforcement guarantees that neither the destination nor the staging artifact can be unlinked or replaced by any concurrent process.

4. **Hook Execution & Assertion Reachability:**
   - In `test_chk_210`, the hook `set_hook_windows_post_exclusive_open` executes deterministically while both handles are held open.
   - All assertions inside the hook execute and succeed on Windows:
     * `assert_eq!(unlink_err.raw_os_error(), Some(32))`
     * `assert_eq!(stage_unlink_err.raw_os_error(), Some(32))`
     * `assert!(dest.exists())`
     * `assert!(stage.exists())`
     * `assert_eq!(dest_id, stage_id_now)`
     * `assert_eq!(Some(dest_id), stage_id_for_hook)`

---

## 3. Technical Implementation & Test Suite Architecture

### 3.1 Dual-Handle Exclusive Locking in `remove_file_windows_exclusive()`
**File Modified:** `rust/crates/filesystem-engine/src/lib.rs`

```rust
    // While exclusive handle on staging file remains open, open and verify destination handle (Sprint 23.2.16 & Sprint 23.2.18)
    let dest_handle = if let Some(dest) = destination_path {
        // Sprint 23.2.18: Open destination handle with GENERIC_READ and FILE_SHARE_READ.
        // This kernel lock guarantees:
        // 1. Any adversarial attempt to delete or unlink destination fails with ERROR_SHARING_VIOLATION (code 32).
        // 2. Any adversarial attempt to write or truncate destination fails with ERROR_SHARING_VIOLATION (code 32).
        // 3. Destination identity and attributes are verified directly from this verified open handle without TOCTOU races.
        let dest_file = std::fs::OpenOptions::new()
            .read(true)
            .access_mode(GENERIC_READ)
            .share_mode(FILE_SHARE_READ)
            .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
            .open(dest)
            .map_err(AetherError::Io)?;

        // Hook while exclusive handles on both staging and destination are held (CHK-210)
        if let Some(ref hook) = get_hook_windows_post_exclusive_open() {
            hook(path, dest);
        }

        // Check fault injection for destination identity query failure (CHK-213)
        if is_fault_inject_windows_dest_identity_query_failure() {
            return Err(AetherError::Io(std::io::Error::other(
                "Injected destination identity query failure while exclusive handle held",
            )));
        }

        let dest_file_type = unsafe { GetFileType(dest_file.as_raw_handle()) };
        if dest_file_type != FILE_TYPE_DISK {
            return Err(AetherError::PermissionDenied(format!(
                "Security rejection: Destination '{}' is not a standard disk file (file type: 0x{:x}); refusing exclusive deletion",
                dest.display(),
                dest_file_type
            )));
        }

        let mut dest_by_handle_info = std::mem::MaybeUninit::<BY_HANDLE_FILE_INFORMATION>::zeroed();
        let dest_info_res = unsafe {
            GetFileInformationByHandle(dest_file.as_raw_handle(), dest_by_handle_info.as_mut_ptr())
        };
        if dest_info_res == 0 {
            return Err(AetherError::Io(std::io::Error::last_os_error()));
        }
        let dest_file_info = unsafe { dest_by_handle_info.assume_init() };

        let dest_is_reparse = (dest_file_info.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT) != 0;
        let dest_is_dir = (dest_file_info.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY) != 0;
        if dest_is_reparse || dest_is_dir {
            return Err(AetherError::PermissionDenied(format!(
                "Security rejection: Destination '{}' is a reparse point or directory (attributes: 0x{:x}); refusing exclusive deletion to prevent target redirection",
                dest.display(),
                dest_file_info.dwFileAttributes
            )));
        }

        let dest_identity = FileObjectIdentity {
            volume_serial_number: dest_file_info.dwVolumeSerialNumber as u64,
            file_index_high: dest_file_info.nFileIndexHigh as u64,
            file_index_low: dest_file_info.nFileIndexLow as u64,
        };

        if is_fault_inject_windows_dest_identity_mismatch() {
            return Err(AetherError::PermissionDenied(format!(
                "Security rejection: Injected destination identity mismatch for '{}'",
                dest.display()
            )));
        }

        if dest_identity != actual_identity {
            return Err(AetherError::PermissionDenied(format!(
                "Security rejection: Destination '{}' identity ({:?}) does not match authorized object identity ({:?}) while exclusive handle held on staging path '{}'",
                dest.display(),
                dest_identity,
                actual_identity,
                path.display()
            )));
        }

        if let Some(expected) = expected_identity {
            if dest_identity != expected {
                return Err(AetherError::PermissionDenied(format!(
                    "Security rejection: Destination '{}' identity ({:?}) does not match expected identity ({:?})",
                    dest.display(),
                    dest_identity,
                    expected
                )));
            }
        }

        Some(dest_file)
    } else {
        None
    };
```

### 3.2 Refined CHK-210 Test Design
**File Modified:** `rust/crates/checkpoint-engine/src/lib.rs` (`test_chk_210_replace_destination_while_exclusive_handle_held_preserves_staging_without_loss`)

```rust
        #[cfg(windows)]
        {
            let stage_id_for_hook = stage_id;
            engine.set_hook_windows_post_exclusive_open(Some(move |stage: &Path, dest: &Path| {
                // 1. Attempt adversarial destination unlink while destination and staging handles are held
                let unlink_err = std::fs::remove_file(dest).expect_err(
                    "Windows kernel must reject adversarial destination unlink with ERROR_SHARING_VIOLATION while exclusive handle is held!",
                );
                assert_eq!(
                    unlink_err.raw_os_error(),
                    Some(32),
                    "Expected Windows ERROR_SHARING_VIOLATION (os error 32) on destination unlink, got {:?}",
                    unlink_err
                );

                // 2. Also assert adversarial staging unlink is rejected with sharing violation
                let stage_unlink_err = std::fs::remove_file(stage).expect_err(
                    "Windows kernel must reject adversarial staging unlink with ERROR_SHARING_VIOLATION!",
                );
                assert_eq!(
                    stage_unlink_err.raw_os_error(),
                    Some(32),
                    "Expected Windows ERROR_SHARING_VIOLATION (os error 32) on staging unlink, got {:?}",
                    stage_unlink_err
                );

                // 3. Assert destination and staging links remain intact on disk
                assert!(
                    dest.exists(),
                    "Destination hard link must remain intact after blocked adversarial unlink attempt"
                );
                assert!(
                    stage.exists(),
                    "Staging link must remain intact after blocked adversarial unlink attempt"
                );

                // 4. Verify their object identities still match
                let dest_id = filesystem_engine::query_file_object_identity(dest)
                    .expect("Must query destination identity");
                let stage_id_now = filesystem_engine::query_file_object_identity(stage)
                    .expect("Must query staging identity");
                assert_eq!(
                    dest_id, stage_id_now,
                    "Destination and staging identities must remain identical"
                );
                assert_eq!(
                    Some(dest_id),
                    stage_id_for_hook,
                    "Destination identity must match authorized original staging identity"
                );
            }));
        }

        let rolled = engine.rollback_cleanup_stage(
            &q_cap_dir,
            stage_name,
            "dest_210.txt",
            stage_id,
            Some(&stage_hash),
        );

        let dest_abs = q_dir.join("dest_210.txt");
        assert!(
            dest_abs.exists(),
            "Destination artifact must survive intact with authorized content!"
        );
        assert_eq!(std::fs::read(&dest_abs).unwrap(), original_data);

        #[cfg(windows)]
        {
            // On Windows: helper unlinks the staging link while destination remains safely published
            assert!(
                !stage_abs.exists(),
                "Staging link must be cleanly unlinked on Windows after verified publication"
            );
            assert_eq!(rolled.status, RollbackPublicationStatus::PublishedVerified);
            assert!(
                !engine.is_recovery_blocked(),
                "RecoveryBlocked must NOT be active when publication succeeded without unapproved replacement!"
            );
            engine.set_hook_windows_post_exclusive_open(None::<fn(&Path, &Path)>);
        }
```

### 3.3 New CHK-210B: Deterministic Destination Identity Mismatch Test
**File Added:** `rust/crates/checkpoint-engine/src/lib.rs` (`test_chk_210b_destination_identity_mismatch_preserves_staging_and_blocks_recovery`)

To test the helper's destination-identity-mismatch rejection branch deterministically without relying on concurrent filesystem replacement (which Windows sharing rules now actively prevent), we added a dedicated fault-injection test:
- Injects destination identity mismatch via `filesystem_engine::set_fault_inject_windows_dest_identity_mismatch(true)`.
- Verifies that `remove_file_windows_exclusive()` rejects the operation and preserves `stage_abs`.
- Verifies that `stage_abs.exists()` remains `true` containing the original authorized bytes.
- Verifies that `rolled.status` is `PublicationUncertainBlocked`.
- Verifies that `RecoveryBlocked` is activated.

### 3.4 CHK-209 Verification Maintained
**File Verified:** `rust/crates/checkpoint-engine/src/lib.rs` (`test_chk_209_replace_destination_after_pathname_checks_before_exclusive_open_preserves_staging`)

CHK-209 remains the test for adversarial destination replacement occurring *before* exclusive handle acquisition:
- Hook executes after pathname re-checks but before `remove_file_windows_exclusive` is called.
- Destination is unlinked and replaced with a distinct object (`assert!(Some(new_dest_id) != stage_id_for_hook)`).
- When `remove_file_windows_exclusive` acquires the handles, it inspects the opened destination handle, detects the identity mismatch, and aborts deletion.
- Staging file survives intact with original content.
- `RecoveryBlocked` is activated, and status is `PublicationUncertainBlocked`.

---

## 4. Local Verification & Static Analysis Results

| Test Suite / Tool | Command | Result |
| :--- | :--- | :--- |
| **`checkpoint-engine`** | `cargo test -p checkpoint-engine --lib` | **210 passed**, 0 failed, 0 ignored |
| **`filesystem-engine`** (unit) | `cargo test -p filesystem-engine --lib` | **24 passed**, 0 failed, 0 ignored |
| **`filesystem-engine`** (adversarial) | `cargo test -p filesystem-engine --test '*'` | **14 passed**, 0 failed, 0 ignored |
| **Entire Rust Workspace** | `cargo test --workspace` | **All tests passed** across all 15 crates |
| **Code Formatting** | `cargo fmt --all -- --check` | **Clean** (0 diffs) |
| **Rust Workspace Linter** | `cargo clippy --workspace --all-targets -- -D warnings` | **Clean** (0 warnings) |
| **Tauri Desktop Linter** | `cargo clippy --manifest-path apps/desktop/src-tauri/Cargo.toml --all-targets -- -D warnings` | **Clean** (0 warnings) |
| **Frontend Workbench Build** | `npm --prefix apps/desktop run build` | **Clean** (`tsc --noEmit && vite build`, 0 errors, 9.13s) |

---

## 5. Conclusion & Request for Independent Review

1. **Kernel Semantics Reconciled:** The Windows kernel behavior, NTFS hard link sharing rules, production dual-handle locking implementation, adversarial test assertions, and engineering reports are now in 100% agreement.
2. **Production Security Upgraded:** Destination artifacts are now held open with `FILE_SHARE_READ` during the exclusive cleanup phase, guaranteeing that Windows denies any concurrent unlinks (`code: 32`) or writes.
3. **Branch & Governance Status:**
   - Active Branch: `fix/sprint-23.2.6-production-delete-verification`
   - PR [#24](https://github.com/Imagen2dragon/Aether-IDE/pull/24) remains **OPEN and UNMERGED**.
   - Zero direct pushes to `main`. Zero force-pushes.
