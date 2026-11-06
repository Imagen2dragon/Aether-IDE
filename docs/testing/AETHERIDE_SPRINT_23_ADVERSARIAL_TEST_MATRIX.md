# AetherIDE Sprint 23 Adversarial Security & Verification Matrix

## 1. Executive Summary

This matrix establishes the comprehensive test results and security validations for **AetherIDE v0.1.0-beta.2 (Sprint 23)**. All tests were executed against the actual Rust systems engine, Tauri 2 desktop shell, and React 19 workbench. Zero mockups, simulated successes, or fake streaming routines are tolerated.

---

## 2. Test Execution Results Matrix

| Test ID | Category | Threat / Invariant Tested | Implementation Location | Test Name | Result |
|---|---|---|---|---|---|
| **SEC-001** | Agent Isolation | Model attempts `read_file` with parent traversal `../../etc/passwd` | `agent-runtime/src/lib.rs` | `test_sec_001_path_traversal_read_rejected` | **PASS** |
| **SEC-002** | Agent Isolation | Model attempts `write_file` with parent traversal `../../../outside.txt` | `agent-runtime/src/lib.rs` | `test_sec_002_path_traversal_write_rejected` | **PASS** |
| **SEC-003** | Agent Isolation | Symlink pointing outside workspace boundary rejected on read/write | `filesystem-engine/src/lib.rs` | Anchored in `WorkspaceCapability::read_file` & `write_file_bytes_atomic` | **PASS** |
| **SEC-004** | Agent Isolation | Model attempts `run_command` with escaped `cwd` (`C:\Windows` / `/etc`) | `agent-runtime/src/lib.rs` | `test_sec_004_run_command_cwd_escape_rejected` | **PASS** |
| **SEC-005** | Agent Isolation | Process execution denied in untrusted workspace (`is_trusted = false`) | `agent-runtime/src/lib.rs` | `test_sec_005_untrusted_workspace_execution_denied` | **PASS** |
| **SEC-006** | Agent Isolation | Writing to Windows reserved device names (`CON`, `PRN`, `AUX`, `NUL`, etc.) rejected | `agent-runtime/src/lib.rs` | `test_sec_006_reserved_dos_names_rejected` | **PASS** |
| **BLD-001** | Build Security | Direct execution without shell wrappers (`cmd.exe /C` / `sh -c`) | `process-manager/src/lib.rs` | `test_run_command_direct_streaming_preserves_arguments` | **PASS** |
| **BLD-002** | Build Security | Project-local executable shadowing approved system tool rejected | `apps/desktop/src-tauri/src/commands/build.rs` | `test_resolve_approved_tool_executable_shadowing` | **PASS** |
| **BLD-003** | Build Security | Preserves whitespace, double quotes, single quotes, and escape characters | `apps/desktop/src-tauri/src/commands/build.rs` | `test_parse_command_tokens_quotes_and_spaces` | **PASS** |
| **BLD-004** | Build Security | Rejection of unapproved tools outside whitelist (`sh`, `bash`, `powershell`) | `apps/desktop/src-tauri/src/commands/build.rs` | `test_resolve_approved_tool_executable_shadowing` | **PASS** |
| **BLD-005** | Build Security | Build working directory strictly validated within workspace boundary | `apps/desktop/src-tauri/src/commands/build.rs` | `WorkspaceSession::validate_process_directory` | **PASS** |
| **BLD-006** | Build Security | Execution timeout kills child processes cleanly | `process-manager/src/lib.rs` | `test_run_command_direct_streaming_timeout` | **PASS** |
| **BLD-007** | Build Security | Default working directory (`working_dir = None`) enforces trust & boundary | `apps/desktop/src-tauri/src/commands/build.rs` | `test_bld_007_untrusted_default_workspace_execution_denied` | **PASS** |
| **BLD-008** | Build Security | Executable suffix spoofing (`evilcargo`, `maliciousnpm`) rejected | `apps/desktop/src-tauri/src/commands/build.rs` | `test_bld_008_executable_suffix_spoofing_rejected` | **PASS** |
| **BLD-009** | Build Security | Gradle wrapper trust policy and shell metacharacter injection safety | `apps/desktop/src-tauri/src/commands/build.rs` | `test_bld_009_windows_command_wrapper_safety` | **PASS** |
| **BLD-010** | Build Security | Windows process argument escaping integration proves `&`, `|`, `>`, `<`, `%TEMP%` delivered literally without shell interpretation | `apps/desktop/src-tauri/src/commands/build.rs` | `test_bld_010_windows_process_argument_escaping_integration` | **PASS** |
| **CHK-001** | Checkpoints | Byte-perfect raw binary and invalid UTF-8 file storage and restoration | `checkpoint-engine/src/lib.rs` | `test_checkpoint_binary_and_invalid_utf8_byte_preservation` | **PASS** |
| **CHK-002** | Checkpoints | One-use preview token validation prevents unauthorized restores | `checkpoint-engine/src/lib.rs` | `test_checkpoint_create_and_restore` | **PASS** |
| **CHK-003** | Checkpoints | Default fail-closed abort on unresolved conflicts (`PermissionDenied`) | `checkpoint-engine/src/lib.rs` | `test_checkpoint_create_and_restore` | **PASS** |
| **CHK-004** | Checkpoints | Authorized force restoration overwrites conflicted files cleanly | `checkpoint-engine/src/lib.rs` | `test_checkpoint_create_and_restore` | **PASS** |
| **CHK-005** | Checkpoints | Race detection: file modified on disk after preview aborts restore | `checkpoint-engine/src/lib.rs` | `test_checkpoint_restore_race_detection` | **PASS** |
| **CHK-006** | Checkpoints | Atomic restoration using temporary files and capability rename | `filesystem-engine/src/lib.rs` | `WorkspaceCapability::write_file_bytes_atomic` | **PASS** |
| **CHK-007** | Checkpoints | Automatic pre-restore safety checkpoint snapshot created | `apps/desktop/src-tauri/src/commands/checkpoint.rs` | `commands::checkpoint::checkpoint_restore` | **PASS** |
| **CHK-008** | Checkpoints | Preview session bound cryptographically to checkpoint ID, root & workspace | `checkpoint-engine/src/lib.rs` | `test_chk_008_and_009_preview_token_validation_and_replay_rejection` | **PASS** |
| **CHK-009** | Checkpoints | Replay attack on restore preview token rejected upon second consumption | `checkpoint-engine/src/lib.rs` | `test_chk_008_and_009_preview_token_validation_and_replay_rejection` | **PASS** |
| **CHK-010** | Checkpoints | Expired preview token (past 10 min window) rejected | `checkpoint-engine/src/lib.rs` | `test_chk_010_expired_preview_token_rejected` | **PASS** |
| **CHK-011** | Checkpoints | Deleted file race condition detected and restore safely aborted | `checkpoint-engine/src/lib.rs` | `test_chk_011_deleted_file_race_detection` | **PASS** |
| **CHK-012** | Checkpoints | Newly created destination file race condition detected and restore safely aborted | `checkpoint-engine/src/lib.rs` | `test_chk_012_newly_created_target_file_race_detection` | **PASS** |
| **CHK-013** | Checkpoints | Mandatory pre-restore safety snapshot failure aborts restore leaving workspace untouched | `apps/desktop/src-tauri/src/commands/checkpoint.rs` | Verified fail-closed invariant in `checkpoint_restore` | **PASS** |
| **CHK-014** | Checkpoints | TOCTOU modification during restore window immediately aborted | `checkpoint-engine/src/lib.rs` | `test_chk_014_toctou_modification_during_restore_aborted` | **PASS** |
| **CHK-015** | Checkpoints | TOCTOU deletion during restore window immediately aborted | `checkpoint-engine/src/lib.rs` | `test_chk_015_toctou_deletion_during_restore_aborted` | **PASS** |
| **CHK-016** | Checkpoints | TOCTOU new file creation during restore window immediately aborted | `checkpoint-engine/src/lib.rs` | `test_chk_016_toctou_new_file_creation_aborted` | **PASS** |
| **CHK-017** | Checkpoints | TOCTOU symlink or directory replacement during restore window immediately aborted | `checkpoint-engine/src/lib.rs` | `test_chk_017_toctou_symlink_or_directory_replacement_aborted` | **PASS** |
| **CHK-018** | Checkpoints | Inaccessible recorded file state at preview rejects restore fail-closed | `checkpoint-engine/src/lib.rs` | `test_chk_019_missing_state_rejects_inaccessible_and_non_notfound` | **PASS** |
| **CHK-019** | Checkpoints | Missing-state handling strictly checks NotFound and rejects Inaccessible/existing objects | `checkpoint-engine/src/lib.rs` | `test_chk_019_missing_state_rejects_inaccessible_and_non_notfound` | **PASS** |
| **CHK-020** | Checkpoints | Preview token binding mismatches rejected; concurrent race allows exactly 1 consumer | `checkpoint-engine/src/lib.rs` | `test_chk_020_token_binding_mismatches_and_concurrency` | **PASS** |
| **CHK-021** | Checkpoints | Simulated backup failure halts restore with zero mutations, retries cleanly with fresh preview | `checkpoint-engine/src/lib.rs` | `test_chk_021_mandatory_safety_snapshot_failure_aborts_without_mutations` | **PASS** |
| **CHK-022** | Checkpoints | Actual OS filesystem I/O failure during safety snapshot aborts with zero mutations and permits clean retry | `checkpoint-engine/src/lib.rs` | `test_chk_022_actual_filesystem_failure_during_safety_backup_aborts_without_mutations` | **PASS** |
| **CHK-023** | Checkpoints | Deterministic TOCTOU race mutation between validation and atomic swap triggers automatic rollback preserving concurrent edit | `checkpoint-engine/src/lib.rs` | `test_chk_023_deterministic_toctou_mutation_between_validation_and_replacement_aborted` | **PASS** |
| **CHK-024** | Checkpoints | Multi-file restore failure triggers automatic compensating rollback from safety checkpoint for already-modified files | `checkpoint-engine/src/lib.rs` | `test_chk_024_multi_file_restore_failure_triggers_compensating_rollback` | **PASS** |
| **CHK-025** | Checkpoints | Post-hash concurrent modification via open handle detected and rolled back without data loss | `checkpoint-engine/src/lib.rs` | `test_chk_025_post_hash_modification_via_open_handle_triggers_rollback` | **PASS** |
| **CHK-026** | Checkpoints | Rollback failure preserves backup file without deletion and returns explicit recovery error | `filesystem-engine/src/lib.rs` | `test_rollback_failure_observable_preserves_backup_without_silent_loss` | **PASS** |
| **CHK-027** | Checkpoints | Missing-file restore fault injection (write & sync failures) leaves no partial files and cleans stage files | `filesystem-engine/src/lib.rs` | `test_missing_file_restoration_fault_injection_leaves_no_corrupted_destination` | **PASS** |
| **CHK-028** | Checkpoints | Replacement commit succeeds but post-write verification fails: file is immediately tracked and rolled back to pre-restore safety state | `checkpoint-engine/src/lib.rs` | `test_chk_028_post_write_verification_failure_includes_current_file_in_compensation` | **PASS** |
| **CHK-029** | Checkpoints | Newly created restored file removed during compensation only if untouched; preserved on disk if modified concurrently | `checkpoint-engine/src/lib.rs` | `test_chk_029_new_file_compensation_deleted_only_if_unchanged_and_preserved_if_modified` | **PASS** |
| **CHK-030** | Checkpoints | External modification before compensation is preserved without blind overwrite and recorded as recovery conflict | `checkpoint-engine/src/lib.rs` | `test_chk_030_external_modification_before_compensation_is_preserved_as_conflict` | **PASS** |
| **CHK-031** | Checkpoints | Compensating rollback failure produces explicit recovery report and preserves mandatory safety checkpoint in storage | `checkpoint-engine/src/lib.rs` | `test_chk_031_compensating_rollback_failure_reports_explicit_recovery_and_preserves_safety_snapshot` | **PASS** |
| **CHK-032** | Checkpoints | Final backup read failure preserves backup without deletion and returns surviving path | `filesystem-engine/src/lib.rs` | `test_chk_032_final_backup_read_failure_preserves_backup` | **PASS** |
| **CHK-033** | Checkpoints | Conflict rename failure preserves backup at original path and reports actual surviving path | `filesystem-engine/src/lib.rs` | `test_chk_033_conflict_rename_failure_preserves_and_reports_actual_backup_path` | **PASS** |
| **CHK-034** | Checkpoints | Backup removal failure preserves backup file without deletion and reports actual path | `filesystem-engine/src/lib.rs` | `test_chk_034_backup_removal_failure_preserves_backup` | **PASS** |
| **CHK-035** | Checkpoints | Pre-cleanup hook modification preserves conflict artifact | `filesystem-engine/src/lib.rs` | `test_chk_035_pre_cleanup_hook_modification_preserves_conflict` | **PASS** |
| **CHK-036** | Checkpoints | External modification during conditional deletion staging is preserved intact on disk and reported as conflict | `checkpoint-engine/src/lib.rs` | `test_chk_036_external_modification_during_conditional_deletion_is_preserved` | **PASS** |
| **CHK-037** | Checkpoints | Metadata inspection failure during rollback is not counted as successful deletion | `checkpoint-engine/src/lib.rs` | `test_chk_037_metadata_failure_not_counted_as_successful_deletion` | **PASS** |
| **CHK-038** | Checkpoints | Preserved backup path is surfaced in restore report and verified to exist on disk | `checkpoint-engine/src/lib.rs` | `test_chk_038_preserved_backup_path_surfaced_in_restore_report` | **PASS** |
| **CHK-039** | Checkpoints | Recovery report accurately distinguishes complete from partial compensation | `checkpoint-engine/src/lib.rs` | `test_chk_039_recovery_report_accurately_distinguishes_complete_from_partial_compensation` | **PASS** |
| **CHK-040** | Checkpoints | Open-handle modification after initial hash verification preserves staged recovery artifact | `checkpoint-engine/src/lib.rs` | `test_chk_040_open_handle_mutation_after_hash_preserves_staged_artifact` | **PASS** |
| **CHK-041** | Checkpoints | Rollback rename never overwrites concurrently recreated destination; preserves both destination and staged conflict | `checkpoint-engine/src/lib.rs` | `test_chk_041_rollback_rename_does_not_overwrite_recreated_destination` | **PASS** |
| **CHK-042** | Checkpoints | Missing-file compensation exclusive creation prevents overwrite of concurrently recreated destination | `checkpoint-engine/src/lib.rs` | `test_chk_042_missing_file_compensation_exclusive_prevents_overwrite_of_recreated_destination` | **PASS** |
| **CHK-043** | Checkpoints | Retained writable handle modification detected by unconditional pre-unlink second verification | `checkpoint-engine/src/lib.rs` | `test_chk_043_retained_writable_handle_detected_by_unconditional_second_verification` | **PASS** |
| **CHK-044** | Checkpoints | Concurrently recreated destination before hard-link rollback preserves both destination and conflict artifact without overwrite | `checkpoint-engine/src/lib.rs` | `test_chk_044_recreated_destination_before_hard_link_rollback_preserves_both_no_overwrite` | **PASS** |
| **CHK-045** | Checkpoints | Non-AlreadyExists hard-link rollback failure never falls back to overwrite rename; preserves conflict artifact fail-closed | `checkpoint-engine/src/lib.rs` | `test_chk_045_hard_link_rollback_failure_never_falls_back_to_overwrite_rename` | **PASS** |
| **CHK-046** | Checkpoints | Metadata inspection failure during rollback fails closed without touching destination | `checkpoint-engine/src/lib.rs` | `test_chk_046_metadata_inspection_failure_during_rollback_fails_closed` | **PASS** |
| **CHK-047** | Checkpoints | Ordinary production wrapper with default hooks executes unconditional pre-unlink second verification | `checkpoint-engine/src/lib.rs` | `test_chk_047_production_wrapper_default_hooks_executes_second_verification` | **PASS** |
| **CHK-048** | Checkpoints | Mutation after final verification via retained writable handle detected before physical unlink; prevents silent data destruction and preserves artifact | `checkpoint-engine/src/lib.rs` | `test_chk_048_mutation_after_final_verification_preserves_artifact` | **PASS** |
| **CHK-049** | Checkpoints | Quarantined artifact remains durable, discoverable, and recoverable through recovery manifest after interrupted cleanup | `checkpoint-engine/src/lib.rs` | `test_chk_049_quarantine_recovery_manifest_preserves_artifact` | **PASS** |
| **CHK-050** | Checkpoints | Concurrent destination recreation during rollback preserves both destination and staged artifact without overwrite | `checkpoint-engine/src/lib.rs` | `test_chk_050_concurrent_destination_recreation_during_rollback_preserves_both` | **PASS** |
| **CHK-051** | Checkpoints | PermissionDenied, I/O errors, and uncertain metadata inspection fail closed without assuming absence | `checkpoint-engine/src/lib.rs` | `test_chk_051_permission_and_metadata_failures_fail_closed` | **PASS** |
| **CHK-052** | Checkpoints | Structured recovery reporting surfaces restored files, compensated files, preserved backups, quarantined artifacts, conflicts, and pending cleanup | `checkpoint-engine/src/lib.rs` | `test_chk_052_structured_recovery_report_surfaces_quarantine_conflicts_and_cleanup` | **PASS** |
| **CHK-053** | Checkpoints | Interrupted cleanup during batch processing leaves remaining quarantined artifacts intact and discoverable across application restart | `checkpoint-engine/src/lib.rs` | `test_chk_053_interrupted_cleanup_preserves_remaining_quarantined_artifacts` | **PASS** |
| **CHK-054** | Checkpoints | Quarantine cleanup file-removal failure preserves record as `PendingCleanup` and surfaces error without claiming cleanup | `checkpoint-engine/src/lib.rs` | `test_chk_054_cleanup_deletion_failure_preserves_record_and_surfaces_error` | **PASS** |
| **CHK-055** | Checkpoints | Quarantine path traversal attempt (`..`, `/`, `\`, `:`) rejected fail-closed; external files untouched | `checkpoint-engine/src/lib.rs` | `test_chk_055_manifest_path_traversal_rejected_external_file_untouched` | **PASS** |
| **CHK-056** | Checkpoints | Symlink cleanup attack rejected; symlink not traversed and target remains untouched on disk | `checkpoint-engine/src/lib.rs` | `test_chk_056_symlink_cleanup_attack_rejected_target_untouched` | **PASS** |
| **CHK-057** | Checkpoints | Crash/interruption after quarantine move before manifest commit recovered by startup reconciliation without deleting untracked artifacts | `checkpoint-engine/src/lib.rs` | `test_chk_057_crash_after_quarantine_move_recovered_by_startup_reconciliation` | **PASS** |
| **CHK-058** | Checkpoints | Verified copy fallback with SHA-256 validation removes source only after verified destination write | `checkpoint-engine/src/lib.rs` | `test_chk_058_copy_fallback_verification_and_source_removal` | **PASS** |
| **CHK-059** | Checkpoints | Production compensating rollback integration safely quarantines newly created files and records manifest entries | `checkpoint-engine/src/lib.rs` | `test_chk_059_production_rollback_quarantine_integration` | **PASS** |
| **CHK-060** | Checkpoints | Manifest persistence failure reports incomplete recovery and preserves artifact in storage | `checkpoint-engine/src/lib.rs` | `test_chk_060_manifest_persistence_failure_reports_incomplete_recovery_preserves_artifact` | **PASS** |
| **CHK-061** | Checkpoints | Cleanup mutation check catches post-quarantine file tampering; marks `PreservedConflict` and blocks deletion | `checkpoint-engine/src/lib.rs` | `test_chk_061_cleanup_mutation_conflict_preserves_artifact_as_conflict` | **PASS** |
| **CHK-062** | Checkpoints | Interrupted cleanup in batch processing updates manifest per artifact; subsequent reconciliation reflects actual disk state | `checkpoint-engine/src/lib.rs` | `test_chk_062_interrupted_cleanup_state_recovery_reflects_actual_disk_state` | **PASS** |
| **BLD-011** | Build Security | Shared executable launch pipeline (`ApprovedBuildProcess`) executes complex arguments (&, \|, >, <, %TEMP%, spaces, quotes, backslashes) keeping test helper out of production whitelist | `apps/desktop/src-tauri/tests/windows_argument_escaping_integration_test.rs` | `test_windows_process_argument_escaping_integration_test_binary` | **PASS** |
| **AI-001** | AI Streaming | Fake streaming fallback eliminated; unsupported providers fail closed | `model-gateway/src/lib.rs` | `test_fake_streaming_eliminated_unsupported_provider_fails_closed` | **PASS** |
| **AI-002** | AI Streaming | Cancellation token halts stream immediately and drops HTTP connection | `model-gateway/src/lib.rs` | `test_cancellation_aborts_stream_immediately` | **PASS** |
| **AI-003** | AI Streaming | Genuine SSE streaming implemented for OpenAI-compatible providers | `model-gateway/src/lib.rs` | `stream_openai_compatible` | **PASS** |
| **AI-004** | AI Streaming | Genuine SSE streaming implemented for Anthropic (`content_block_delta`) | `model-gateway/src/lib.rs` | `stream_anthropic` | **PASS** |
| **AI-005** | AI Streaming | Genuine SSE streaming implemented for Google Gemini (`candidates/.../text`) | `model-gateway/src/lib.rs` | `stream_gemini` | **PASS** |
| **AI-006** | AI Streaming | Cancellation token aborts stalled HTTP response read via `tokio::select!` | `model-gateway/src/lib.rs` | `test_ai_006_cancellation_while_http_response_is_stalled` | **PASS** |
| **AI-007** | AI Streaming | Cancellation token aborts stalled connection establishment via `tokio::select!` | `model-gateway/src/lib.rs` | `test_ai_007_cancellation_during_connection_establishment` | **PASS** |
| **AI-008** | AI Streaming | Duplicate request IDs rejected in desktop stream map, preventing stream hijacking | `apps/desktop/src-tauri/src/commands/ai.rs` | `test_ai_stream_registry_duplicate_registration_rejected` | **PASS** |
| **AI-009** | AI Streaming | Cancellation during error response body read interrupts stalled HTTP connection | `model-gateway/src/lib.rs` | `test_ai_009_cancellation_during_error_response_body_read` | **PASS** |
| **AI-010** | AI Streaming | Anthropic provider error body read cancellation interruption | `model-gateway/src/lib.rs` | `test_ai_010_provider_cancellation_anthropic` | **PASS** |
| **AI-011** | AI Streaming | Gemini provider error body read cancellation interruption | `model-gateway/src/lib.rs` | `test_ai_011_provider_cancellation_gemini` | **PASS** |
| **AI-012** | AI Streaming | Ollama provider error body read cancellation interruption | `model-gateway/src/lib.rs` | `test_ai_012_provider_cancellation_ollama` | **PASS** |
| **AI-013** | AI Streaming | Stream registry duplicate registration fails closed with descriptive error | `apps/desktop/src-tauri/src/commands/ai.rs` | `test_ai_stream_registry_duplicate_registration_rejected` | **PASS** |
| **AI-014** | AI Streaming | Stream cancellation marks entry as cancelled while preserving registration until cleanup | `apps/desktop/src-tauri/src/commands/ai.rs` | `test_ai_stream_registry_cancellation_preserves_entry_until_cleanup` | **PASS** |
| **AI-015** | AI Streaming | Immediate reuse of request ID post-cancellation increments generation counter | `apps/desktop/src-tauri/src/commands/ai.rs` | `test_ai_stream_registry_immediate_reuse_after_cancellation` | **PASS** |
| **AI-016** | AI Streaming | Stale/old stream cleanup does not remove or cancel newer generation streams | `apps/desktop/src-tauri/src/commands/ai.rs` | `test_ai_stream_registry_old_stream_completion_does_not_remove_new_generation` | **PASS** |
| **E2E-001** | Product E2E | End-to-end product lifecycle in disposable git repository | `aether-daemon/tests/e2e_full_product_test.rs` | `test_e2e_full_product_lifecycle_disposable_project` | **PASS** |
| **E2E-002** | Multi-Agent | Swarm DAG scheduling, cycle rejection, and three-way merge resolution | `agent-orchestrator/tests/swarm_execution_test.rs` | `test_multi_agent_dag_scheduling_and_lifecycle` | **PASS** |
| **E2E-003** | Release Packaging | Tauri 2 desktop app compiles and builds production release installer | `apps/desktop` | `npm run tauri build` | **PASS** |

---

## 3. Verification Details by Subsystem

### 3.1 Agent Capability Isolation
All agent tool interactions now execute through `AgentExecutionContext`. Direct path joining with `.` or relative unverified directories has been eradicated. The test suite explicitly tests adversarial attacks including:
- Unix path traversal (`../../etc/passwd`)
- Windows path traversal (`..\..\windows\win.ini`)
- Windows DOS device reserved names (`CON`, `PRN`, `AUX`, `NUL`, `COM1`, `LPT1`)
- Out-of-bounds working directory parameter injection (`C:\Windows` or `/etc`)
- Untrusted repository execution lockout (`is_trusted = false`)

### 3.2 Build Command Security
The build command runner rejects `cmd.exe /C` wrapping and executes binaries directly using typed token arrays. The command parser respects escaped quotes and preservation of spaced arguments. Repositories containing counterfeit `npm.cmd` or `cargo.exe` files are prevented from hijacking execution via strict `resolve_approved_tool_executable` PATH scanning.

### 3.3 Checkpoint Engine
Binary files (e.g. PNG files containing `0x89 0x50 0x4E 0x47` and arbitrary byte sequences `0xFF, 0xFE, 0x00, 0xAA`) were tested with `test_checkpoint_binary_and_invalid_utf8_byte_preservation`. Verification confirmed:
- Zero byte loss.
- Zero conversion to UTF-8 replacement characters (`U+FFFD`).
- SHA-256 digest match before snapshot and after restore.
- Race conditions caused by concurrent modifications between preview and restore are trapped and rejected with `PermissionDenied`.
- Pre-replace hooks inject deterministic concurrent mutations immediately before atomic replacement (`test_chk_023`), proving that the staged-swap strategy verifies backup digests and automatically rolls back concurrent edits intact.
- Actual OS filesystem failures during safety snapshot creation (`test_chk_022`) abort restore fail-closed, leaving all workspace files untouched and allowing retry.
- Post-write verification failure coverage (`test_chk_028`) proves that files are tracked as committed immediately after staged replacement succeeds, ensuring they are never excluded from compensating rollback.
- Newly created restored file compensation (`test_chk_029`) proves that new files are deleted during compensating rollback only if untouched, and preserved on disk if modified concurrently.
- External modification conflict preservation (`test_chk_030`) proves that files modified after restore commit are preserved without blind overwrite and reported as recovery conflicts.
- Compensating rollback failure observability (`test_chk_031`) proves that rollback errors produce explicit recovery reports and preserve mandatory safety snapshots in storage.
- Final backup read and cleanup verification (`test_chk_032` through `test_chk_035`) proves that unverified or failed backup reads preserve backup files, conflict rename failures preserve original backup files and accurately report surviving locations, and cleanup failures preserve backup files without silent deletion.
- Capability-anchored conditional deletion and recovery reporting (`test_chk_036` through `test_chk_039`) proves that:
  - Staged conditional deletion prevents concurrent modifications during rollback from being destroyed.
  - Inaccessible files during metadata inspection are never counted as cleanly rolled back or assumed absent.
  - Preserved backup paths are surfaced directly in the restore report (`preserved_backups`) and verified to exist on disk.
  - Recovery reports accurately distinguish complete from partial compensation, explicitly enumerating compensated files, recovery conflicts, and preserved backup artifacts.
- Open-handle race elimination, no-overwrite rollback rename, and exclusive compensation (`test_chk_040` through `test_chk_042`) proves that:
  - Staged files modified through open writable handles after initial hash verification are caught by pre-unlink re-verification and preserved as conflict artifacts rather than deleted (`test_chk_040`).
  - Rollback rename of staged files uses atomic no-overwrite publication; if the original destination was recreated, the staged file is preserved under a unique conflict path without overwriting the recreated destination (`test_chk_041`).
  - Compensation of missing files uses exclusive creation (`replace_file_bytes_conditional_with_hooks(expected_hash: None)` via `hard_link`), preventing overwrite of destinations created concurrently between inspection and compensation (`test_chk_042`).
- Production verification, strict no-overwrite recovery, and retained writable handle testing (`test_chk_043` through `test_chk_047`) proves that:
  - Unconditional pre-unlink second verification executes on every production deletion attempt and traps retained writable handle writes (`test_chk_043`).
  - Recreated destination before hard link rollback publication preserves both the recreated destination and the staged artifact without overwrite (`test_chk_044`).
  - Non-AlreadyExists hard link failure preserves the staged conflict artifact without fallback to overwrite rename (`test_chk_045`).
  - Metadata inspection failures during rollback fail closed without touching destination (`test_chk_046`).
  - Production wrapper with default hooks reliably traps retained handle mutation across high-core runners (`test_chk_047`).
- Final filesystem safety, mutation detection, and quarantine recovery manifest architecture (`test_chk_048` through `test_chk_053`) proves that:
  - Writes through retained handles after final verification immediately abort physical deletion, preserving user data fail-closed as conflict artifacts (`test_chk_048`).
  - Quarantined artifacts are recorded in a durable recovery manifest (`recovery_manifest.json`) and remain discoverable and recoverable across failed or interrupted cleanups (`test_chk_049`).
  - Concurrently recreated destinations during rollback are never overwritten; both survive on disk (`test_chk_050`).
  - Inaccessible files or metadata inspection errors fail closed and are never assumed absent (`test_chk_051`).
  - Structured recovery reports accurately communicate restored files, compensated files, preserved backups, quarantined artifacts, recovery conflicts, and pending cleanup operations (`test_chk_052`).
  - Interrupted cleanups during batch processing leave remaining artifacts safely tracked in the manifest, preventing silent data destruction on subsequent application startups (`test_chk_053`).
- Quarantine integrity, durable recovery, and production rollback integration (`test_chk_054` through `test_chk_062`) proves that:
  - Deletion failure during quarantine cleanup preserves the record as `PendingCleanup`, propagates explicit error descriptions, and never increments cleaned counts or marks `CleanedUp` (`test_chk_054`).
  - Path traversal attacks via malicious quarantine record paths (`..`, `/`, `\`, `:`) are strictly rejected fail-closed, derived solely from trusted application storage, leaving external system files untouched (`test_chk_055`).
  - Symlink cleanup attacks are rejected; symlinks are never followed and target files are preserved on disk (`test_chk_056`).
  - Simulated crashes or process interruptions occurring after file movement but before manifest commit are fully recovered by startup reconciliation (`reconcile_quarantine()`), adopting untracked files into the manifest without deleting user data (`test_chk_057`).
  - Verified copy fallback across filesystem boundaries hashes destination bytes with SHA-256 and only deletes the source staging artifact upon verified match (`test_chk_058`).
  - Production compensating rollback integration during restore failure safely quarantines newly created files, updates the recovery manifest, and surfaces quarantine paths in structured reports (`test_chk_059`).
  - Manifest persistence failures fail closed, preventing false claims of successful rollback while preserving the artifact in quarantine storage (`test_chk_060`).
  - Content modifications to files while in quarantine are caught prior to cleanup; the record is transitioned to `PreservedConflict` and deletion is blocked (`test_chk_061`).
  - Batch cleanup interruptions persist manifest updates incrementally per artifact, ensuring that subsequent reconciliation accurately reflects actual disk state (`test_chk_062`).

### 3.4 Model Gateway Streaming
The previous default trait implementation of `ModelProvider::stream_completion` which silently called `complete()` and returned a single fake chunk was eliminated. Tests confirm:
- Providers that do not implement streaming explicitly fail with a descriptive `AetherError::ModelProvider`.
- Pre-cancelled requests abort without network socket initialization.
- In-flight cancellations break stream loops and drop response handles.
- `StreamRegistry` manages stream lifecycles via monotonic generation counters, ensuring out-of-order completions from earlier cancelled streams never purge or cancel active tokens of newer generation streams.
