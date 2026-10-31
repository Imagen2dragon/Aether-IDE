# AetherIDE Sprint 10 Threat Model Report

## 1. Threat Modeling Scope & Trust Boundaries
AetherIDE operates across four distinct trust boundaries:
1. **User / Desktop OS Context:** The logged-in developer running the AetherIDE native window host (Win32 + WebView2 on Windows, GTK on Linux, Cocoa on macOS).
2. **Local Daemon IPC Boundary:** The background Tokio daemon listening on `127.0.0.1:44888` and Windows Named Pipe (`\\.\pipe\aetheride-ipc`).
3. **Workspace Filesystem Boundary:** The target source repository root opened by the user, within which all reads, writes, and directory traversals must be contained.
4. **Network & Collaboration Boundary:** Peer-to-peer workspace synchronization, mobile companion syncing, and remote AI model gateway endpoints.

---

## 2. Threat Actors & Attack Vectors

| Threat Actor | Description | Potential Attack Vector | Mitigations in Sprint 10 |
| :--- | :--- | :--- | :--- |
| **Malicious Local Process (Non-Owner)** | Unprivileged local process running on the same workstation. | Probing IPC port `44888` to execute commands, forge peer IDs, or read sensitive tokens. | Strict bearer token authentication (`authenticate_session`); caller-supplied `peer_id` rejected; zero master token leakage in IPC responses. |
| **Compromised Workspace / Malicious Git Repo** | Repository containing craftily placed symlinks, junctions, or directory traversal sequences (`../`). | Escaping workspace directory during AST indexing or build operations to overwrite arbitrary system files. | Capability-based containment; `walk_dir` symlink verification via `symlink_metadata`; `create_new(true)` temp writes and atomic renames. |
| **Impersonating Collaborator / Peer** | Network peer with basic collaborator role attempting privilege escalation. | Forging CRDT edit attribution, cursor presence, or invoking admin IPC requests (e.g. `RunCommand`). | Server-controlled session binding; role checks enforced before request handling; CRDT and presence strictly attributed to authenticated `SessionRecord.peer_id`. |
| **Local Denial of Service Attacker** | Local client flooding connection attempts or deliberately failing authentication. | Exhausting connection rate limits to lock out the genuine desktop host. | Independent Named Pipe client identifiers (`named-pipe-client-{id}`) and TCP connection limits; genuine desktop client never locked out by rogue connection attempts. |

---

## 3. STRIDE Analysis & Security Controls

### 3.1 Spoofing (S)
- **Threat:** Caller sets `peer_id: "desktop-owner"` in the IPC envelope to execute privileged actions.
- **Control:** The server checks `token_store.authenticate_session(token)`. If the claimed `peer_id` doesn't match the server record, the request is rejected with `401 Unauthorized`.

### 3.2 Tampering (T)
- **Threat:** Modifying files outside the workspace root using race conditions (TOCTOU) with symbolic links.
- **Control:** `write_file_contained` creates temporary files with `create_new(true)`, checks symlink metadata before atomic replacement, and refuses to follow symlinks to outside destinations.

### 3.3 Repudiation (R)
- **Threat:** A collaborator denies applying an unauthorized or destructive edit in a multi-user session.
- **Control:** All CRDT updates (`CrdtApplyUpdate`) and presence pulses are attributed directly to the cryptographic session ID and verified peer ID recorded in the server-side `SessionRecord`.

### 3.4 Information Disclosure (I)
- **Threat:** Unauthenticated ping or command response leaking the master session token to an attacker.
- **Control:** `IpcEnvelope.session_token` in response messages is unconditionally sanitized to `None`.

### 3.5 Denial of Service (D)
- **Threat:** Brute-force token guessing locking out all clients via shared rate limiting.
- **Control:** Rate-limiting state is partitioned by client connection ID on Named Pipes and IP address on TCP.

### 3.6 Elevation of Privilege (E)
- **Threat:** Remote companion or read-only collaborator requesting terminal spawning or code execution.
- **Control:** Centralized RBAC evaluation enforces that only `DesktopOwner` or authorized `Developer` sessions can spawn terminals, run shell commands, or trigger builds.
