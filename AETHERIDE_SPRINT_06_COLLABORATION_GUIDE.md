# AetherIDE — Sprint 06 Real-Time Collaboration Guide
## Yrs CRDT Engine, Ephemeral Presence and Cryptographic Access Control

---

## 1. Collaborative Engine Architecture

AetherIDE provides real-time multi-peer document collaboration built directly in native Rust:

1. **Yrs CRDT Core (`workspace-sync::crdt`):**
   - Implements Conflict-Free Replicated Data Types using the Yjs V1 wire format.
   - Text state is managed via `yrs::Text` operations (`push`, `insert`, `remove_range`).
   - Every mutation produces a compact binary delta update.

2. **Synchronization Handshake:**
   - Client A sends its `StateVector` (Base64) to Client B.
   - Client B computes `encode_diff_v1(remote_sv)` representing only missing changes.
   - Client A applies the diff: both peers converge to identical text states deterministically.

---

## 2. Ephemeral Cursor Awareness (`workspace-sync::awareness`)

Collaborator presence is tracked in real-time:
- **Peer Presence Data:**
  - `peer_id`: Unique identifier (UUIDv4).
  - `display_name`: Human-readable collaborator handle.
  - `user_color`: Hex color for editor cursor render.
  - `current_file`: File path currently focused by the collaborator.
  - `cursor`: Line and column coordinates.
  - `selection`: Multi-line selection range span.
- **Heartbeat & Pruning:**
  - Heartbeats are refreshed on every cursor movement or edit.
  - Inactive peers are automatically pruned after 30 seconds of silence.

---

## 3. Cryptographic Role-Based Access Control (RBAC) (`workspace-sync::auth`)

Workspaces enforce cryptographic identity and role verification:

| Role | Read Documents | Edit Documents (CRDT Write) | Admin & Invites |
|---|---|---|---|
| **Viewer** | Yes | No (Mutations Rejected) | No |
| **Reviewer** | Yes | No (Comments/Review only) | No |
| **Editor** | Yes | Yes (Authorized) | No |
| **Admin** | Yes | Yes (Authorized) | Yes |
| **Owner** | Yes | Yes (Authorized) | Full Control |

- **Identity Generation:** Generated via `DeviceIdentity::generate()` with random Ed25519 keypairs.
- **Digital Signatures:** Critical mutations and pairing requests are signed with `sign_message()` and verified by remote peers with `verify_signature()`.
