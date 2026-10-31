# AetherIDE Sprint 10 Daemon RBAC Audit Report

## 1. Executive Summary
In Sprint 10, every `IpcRequest` variant processed by `aether-daemon` was audited against a strict Role-Based Access Control (RBAC) policy.
Unauthenticated requests are strictly rejected (with the exception of `Ping`, which returns public version metadata without sensitive session data).
Furthermore, all mutating actions require either `Owner` or authorized `Developer` roles, while read-only operations allow `Viewer` access.

## 2. Comprehensive RBAC Matrix

| IPC Request Variant | Allowed Roles / Session Types | Authentication Required? | Description & Security Boundary |
| :--- | :--- | :--- | :--- |
| `Ping` | Any (Unauthenticated allowed) | No (Anonymous OK) | Returns daemon availability and `application_core::AETHER_VERSION`. Response contains `session_token: None`. |
| `GetStatus` | Owner, Admin, Developer, Viewer | Yes (Token required) | Returns system diagnostics, active peer ID, model status, and memory stats. |
| `IndexWorkspace` | Owner, Admin, Developer | Yes (Write role) | Triggers background AST parsing and symbol indexing. Mutating action blocked for `Viewer`. |
| `SearchSymbols` | Owner, Admin, Developer, Viewer | Yes (Token required) | Queries SQLite FTS5 index. Read-only operation. |
| `RunCommand` | Owner, Admin | Yes (Owner/Admin only) | Spawns system processes via `process-manager`. Strictly forbidden for `Collaborator` / `Viewer`. |
| `KillCommand` | Owner, Admin | Yes (Owner/Admin only) | Terminates running processes. Restricted to administrative roles. |
| `CreateAgentTask` | Owner, Admin, Developer | Yes (Write role) | Dispatches autonomous agent workflows. Blocked for `Viewer`. |
| `ApproveAgentStep` | Owner, Admin, Developer | Yes (Write role) | Approves agent plan step execution. |
| `RejectAgentStep` | Owner, Admin, Developer | Yes (Write role) | Rejects agent plan step execution. |
| `CancelAgentTask` | Owner, Admin, Developer | Yes (Write role) | Cancels running agent task. |
| `SaveProjectMemoryNote` | Owner, Admin, Developer | Yes (Write role) | Writes architecture notes to SQLite WAL store. |
| `DeleteProjectMemoryNote`| Owner, Admin, Developer | Yes (Write role) | Deletes architecture notes. |
| `StartExpoServer` | Owner, Admin, Developer | Yes (Write role) | Launches local Node/Expo bundler. |
| `TriggerMobileBuild` | Owner, Admin, Developer | Yes (Write role) | Triggers EAS / local iOS/Android build. |
| `GenerateMobileApp` | Owner, Admin, Developer | Yes (Write role) | Generates React Native / Expo template. |
| `TerminalSpawn` | Owner, Admin | Yes (Owner/Admin only) | Spawns interactive ConPTY / pseudo-terminal. Restricted to Owner. |
| `TerminalWrite` | Owner, Admin | Yes (Owner/Admin only) | Sends keystrokes to terminal stdin. Restricted to Owner. |
| `TerminalResize` | Owner, Admin | Yes (Owner/Admin only) | Adjusts terminal columns/rows. Restricted to Owner. |
| `TerminalClose` | Owner, Admin | Yes (Owner/Admin only) | Closes active terminal instance. Restricted to Owner. |
| `LspOpenDocument` | Owner, Admin, Developer | Yes (Write role) | Opens file buffer in language server. |
| `LspChangeDocument` | Owner, Admin, Developer | Yes (Write role) | Applies buffer edit in language server. |
| `LspCloseDocument` | Owner, Admin, Developer | Yes (Write role) | Closes buffer in language server. |
| `RegisterPeerRole` | Owner, Admin | Yes (Owner/Admin only) | Binds roles to new device keys in `RoleRegistry`. |
| `RevokePeerRole` | Owner, Admin | Yes (Owner/Admin only) | Revokes role from device key in `RoleRegistry`. |
| `RevokeSessionToken` | Owner, Admin | Yes (Owner/Admin only) | Revokes session token in memory. |
| `SyncGeneratePairing` | Owner, Admin | Yes (Owner/Admin only) | Generates cryptographic pairing token for companion devices. |
| `SyncGetManifest` | Owner, Admin, Developer, Viewer | Yes (Token required) | Generates SHA256 workspace file manifest. |
| `CrdtApplyUpdate` | Owner, Admin, Developer | Yes (Caller bound) | Applies Yrs binary CRDT delta. Edits strictly attributed to caller's verified session identity. |
| `UpdatePresence` | Owner, Admin, Developer, Viewer | Yes (Caller bound) | Updates cursor and awareness. Forced to caller's session peer ID. |
| `SwarmAddTask` | Owner, Admin, Developer | Yes (Write role) | Submits task node to multi-agent DAG. |
| `SwarmSendMessage` | Owner, Admin, Developer | Yes (Write role) | Sends message between swarm agent nodes. |
| `ApplyMergeResolution`| Owner, Admin, Developer | Yes (Write role) | Applies 3-way conflict resolution to files. |

## 3. Verification & Automated Test Evidence
The RBAC matrix is verified via the following automated tests in `rust/services/aether-daemon/src/main.rs`:
- `tests::test_rbac_daemon_permission_matrix`: Probes Viewer attempting mutating actions (Index, RunCommand, TerminalSpawn) and verifies rejection.
- `tests::test_unauthenticated_request_rejected`: Probes unauthenticated requests and verifies 401 Unauthorized.
- `tests::test_unauthenticated_run_command_rejected`: Probes command execution without valid bearer token.
- `tests::test_crdt_payload_role_spoofing_rejected`: Probes collaborator attempting to claim Owner role in CRDT payload.
