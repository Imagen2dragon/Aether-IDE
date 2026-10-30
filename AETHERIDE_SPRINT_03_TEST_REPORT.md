# AETHERIDE — SPRINT 03 BASELINE TEST REPORT

**Date:** 2026-10-31  
**Auditor:** QA Director & Principal Systems Architect  
**Branch:** `feat/sprint-03-product-integration`  
**Platform:** Windows 11 x64 (Build 10.0.26100)  
**Rust Version:** `rustc 1.85.0-nightly` / stable compatible  
**Total Crates:** 24  
**Total Tests:** 55  
**Passed:** 55  
**Failed:** 0  
**Ignored:** 0  

---

## 1. Automated Test Execution Ledger

Every test was executed fresh on 2026-10-31 using `cargo test --manifest-path rust/Cargo.toml --workspace --all-targets`.

### 1.1 `aether-daemon` (8/8 Passed)
| Test Name | Duration | Result | Purpose |
|:---|:---|:---|:---|
| `tests::test_unauthenticated_request_rejected` | 0.01s | PASS | Enforces 401 Unauthorized for missing token |
| `tests::test_invalid_token_request_rejected` | 0.01s | PASS | Enforces 401 Unauthorized for forged token |
| `tests::test_valid_token_ping_succeeds` | 0.01s | PASS | Authenticated `Ping` returns `pong: true` |
| `tests::test_unauthenticated_run_command_rejected` | 0.01s | PASS | Blocks unauthorized command execution |
| `tests::test_authenticated_lsp_diagnostics_query` | 0.01s | PASS | Queries LSP multiplexer diagnostics via IPC |
| `tests::test_authenticated_terminal_spawn_and_close` | 0.02s | PASS | Spawns and closes ConPTY session via IPC |
| `tests::test_authenticated_sync_generate_pairing` | 0.01s | PASS | Generates ephemeral QR pairing payload via IPC |
| `tests::test_authenticated_sync_get_manifest` | 0.01s | PASS | Computes recursive Merkle manifest via IPC |

### 1.2 `aether-launcher` (1/1 Passed)
| Test Name | Duration | Result | Purpose |
|:---|:---|:---|:---|
| `webview2_runtime::tests::test_detect_webview2_runtime` | 0.41s | PASS | Detects Evergreen WebView2 / Edge installation |

### 1.3 `lsp-client` (12/12 Passed)
| Test Name | Duration | Result | Purpose |
|:---|:---|:---|:---|
| `jsonrpc::tests::test_rpc_request_serialization` | 0.00s | PASS | Validates JSON-RPC 2.0 request formatting |
| `jsonrpc::tests::test_incoming_response_parse` | 0.00s | PASS | Parses standard JSON-RPC response with id |
| `jsonrpc::tests::test_incoming_notification_parse` | 0.00s | PASS | Parses un-correlated server notifications |
| `framing::tests::test_encode_and_decode_single_frame` | 0.00s | PASS | Encodes and decodes Content-Length stdio frame |
| `framing::tests::test_glued_frames` | 0.00s | PASS | Unpacks multiple glued frames in single buffer |
| `framing::tests::test_partial_chunks` | 0.00s | PASS | Accumulates fragmented chunked reads |
| `framing::tests::test_malformed_header_handling` | 0.00s | PASS | Rejects frames without Content-Length |
| `framing::tests::test_invalid_content_length` | 0.00s | PASS | Rejects non-numeric content lengths |
| `document::tests::test_path_uri_conversion` | 0.00s | PASS | Converts Windows paths to `file:///` URIs |
| `document::tests::test_did_open_params` | 0.00s | PASS | Formats `DidOpenTextDocumentParams` |
| `tests/mock_lsp_test.rs::test_lsp_client_mock_lifecycle_and_features` | 0.11s | PASS | Stdio process lifecycle: init, completion, hover, diags |
| `tests/mock_lsp_test.rs::test_lsp_multiplexer_routing` | 0.02s | PASS | Routes file extensions to correct server binary |

### 1.4 `terminal-manager` (4/4 Passed)
| Test Name | Duration | Result | Purpose |
|:---|:---|:---|:---|
| `conpty::win32::test_struct_sizes` | 0.00s | PASS | Validates Win32 memory struct sizes |
| `tests::test_resolve_shell_command` | 0.00s | PASS | Resolves `pwsh`, `powershell`, `cmd`, `bash` paths |
| `tests::test_terminal_session_lifecycle` | 0.00s | PASS | Creates and closes terminal sessions |
| `tests/conpty_test.rs::test_conpty_interactive_cmd_lifecycle` | 0.35s | PASS | Live ConPTY `cmd.exe` execution, VT output, resize |

### 1.5 `workspace-sync` (8/8 Passed)
| Test Name | Duration | Result | Purpose |
|:---|:---|:---|:---|
| `crypto::tests::test_aead_encrypt_decrypt_roundtrip` | 0.00s | PASS | ChaCha20-Poly1305 encrypt/decrypt roundtrip |
| `crypto::tests::test_aead_rejects_tampering` | 0.00s | PASS | Tampered ciphertext fails Poly1305 MAC |
| `crypto::tests::test_aead_rejects_replay_attack` | 0.00s | PASS | Out-of-order sequence numbers rejected |
| `pairing::tests::test_pairing_uri_roundtrip` | 0.00s | PASS | Generates and parses `aether://pair?` QR URIs |
| `discovery::tests::test_sync_service_info` | 0.00s | PASS | Formats mDNS TXT records |
| `manifest::tests::test_manifest_diff_detection` | 0.00s | PASS | Detects uploads, downloads, and conflicts |
| `chunk::tests::test_slice_and_reassemble_roundtrip` | 0.00s | PASS | Slices file into 64KB chunks and reassembles |
| `chunk::tests::test_rejects_corrupted_chunk` | 0.00s | PASS | Rejects chunk with altered byte content |

### 1.6 Supporting Core Crates (22/22 Passed)
- `agent-runtime` (4 passed): `test_agent_runtime_step_execution_and_proof_of_work`, `test_permission_engine_blocks_writes_in_observe_mode`, `test_tool_dispatcher_read_and_write_file`, `test_tool_dispatcher_run_command`.
- `ipc-protocol` (3 passed): `test_ipc_envelope_roundtrip`, `test_ffi_token_verification`, `test_ffi_generate_and_free_token`.
- `syntax-engine` (3 passed): `test_rust_ast_ignores_comments_and_strings`, `test_typescript_ignores_comments_and_strings`, `test_python_ignores_comments_and_strings`.
- `security-core` (2 passed): `test_session_token_verification`, `test_secret_redactor`.
- `permission-engine` (2 passed): `test_observe_mode_blocks_writes`, `test_autopilot_allows_safe_compilation`.
- `database-engine` (2 passed): `test_db_fts_symbol_search`, `test_proof_of_work_ledger`.
- `process-manager` (2 passed): `test_run_command_echo`, `test_run_command_timeout_clean_termination`.
- `model-gateway` (2 passed): `test_model_gateway_routing_and_models`, `test_missing_credentials_return_actionable_errors`.
- `mobile-build-manager` (2 passed): `test_expo_scaffold_and_config_generation`, `test_unauthenticated_eas_build_returns_actionable_error`.
- `application-core` (1 passed): `test_default_config`.
- `checkpoint-engine` (1 passed): `test_checkpoint_create_and_restore`.
- `diagnostics` (1 passed): `test_doctor_diagnoses_environment`.
- `filesystem-engine` (1 passed): `test_atomic_write_and_read`.
- `agent-orchestrator` (1 passed): `test_orchestrator_initializes_14_personas`.
- `project-brain` (1 passed): `test_project_brain_and_impact_radar`.

---

## 2. Code Quality & Format Checks

- `cargo fmt --manifest-path rust/Cargo.toml --all -- --check`: **EXIT 0 (Clean, 0 discrepancies)**.
- `cargo clippy --manifest-path rust/Cargo.toml --workspace --all-targets --all-features -- -D warnings`: **EXIT 0 (Clean, 0 warnings)**.
