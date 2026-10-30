# AetherIDE — Encrypted Device Synchronization Guide
## Local Network mDNS Discovery, QR-Based Pairing, and Zero-Knowledge AEAD Workspace Synchronization

**Document Version:** 1.0.0  
**Status:** Implemented & Verified (Sprint 02 Gate 4)  
**Security Model:** End-to-End Authenticated Encryption (ChaCha20-Poly1305), Zero Cloud Transit, Local Subnet Only  
**Compatible Targets:** Windows 10/11 Desktop, iOS 17+, iPadOS 17+

---

## 1. Overview & Architectural Principles

AetherIDE provides seamless, cryptographic synchronization between the developer's primary desktop workstation and mobile companions (iOS/iPadOS). Unlike conventional cloud-tethered developer tools, AetherIDE's sync subsystem is designed around three strict principles:

1. **Local-Only Zero-Knowledge Transit**: All synchronization traffic stays entirely on the local area network (LAN/Wi-Fi). No project source code, file chunks, or tokens are ever uploaded to cloud servers or third-party intermediaries.
2. **Cryptographic Forward Confidentiality & AEAD Security**: Every session uses ChaCha20-Poly1305 authenticated encryption with unique 256-bit pre-shared keys (PSK), 96-bit salt-based nonces, and monotonic 64-bit sequence counters to prevent message tampering, truncation, and replay attacks.
3. **Content-Addressable Differential Snapshots**: Workspaces are synchronized via 64 KB content-addressable chunking and SHA-256 Merkle tree manifests. Only modified or missing chunks are transferred.

---

## 2. Subsystem Architecture

```
+-----------------------------------------------------------------------------------------+
|                                    Windows Desktop                                      |
|                                                                                         |
|  +-----------------------+     +------------------------+     +----------------------+  |
|  |   mDNS Broadcaster    |     |   Ephemeral QR Session |     |  Workspace Snapshot  |  |
|  | _aether-sync._tcp.loc |     | aether://pair?...      |     |  SHA-256 Merkle Root |  |
|  +-----------+-----------+     +-----------+------------+     +----------+-----------+  |
|              |                             |                             |              |
|              v                             v                             v              |
|  +-----------------------------------------------------------------------------------+  |
|  |                   workspace-sync Crate (Native Rust Core)                         |  |
|  |    * ChaCha20-Poly1305 AEAD Framing   * 64 KB Content-Addressable Slicer          |  |
|  |    * Monotonic 64-bit Nonce Counter   * 3-Way Snapshot Diffing Engine             |  |
|  +-----------------------------------------+-----------------------------------------+  |
|                                            |                                            |
|                                            v                                            |
|                             +------------------------------+                            |
|                             | aether-daemon (IPC / TCP)    |                            |
|                             | Port 44888 / Pipe IPC        |                            |
|                             +--------------+---------------+                            |
+--------------------------------------------|--------------------------------------------+
                                             |
                         Local Wi-Fi / LAN (Zero Cloud)
                         Encrypted Wire Frames (AEAD)
                                             |
                                             v
+-----------------------------------------------------------------------------------------+
|                                iOS / iPadOS Companion                                   |
|                                                                                         |
|  +------------------------+     +-------------------------+     +--------------------+  |
|  |  AVCapture QR Scanner  | --> |   PairingSession Parser | --> |  AetherBridge      |  |
|  |  Camera Feed           |     |   Expiry & Host Check   |     |  Swift/NWConnection|  |
|  +------------------------+     +-------------------------+     +---------+----------+  |
|                                                                           |             |
|                                                                           v             |
|                                                                 +--------------------+  |
|                                                                 | Swift Manifest Diff|  |
|                                                                 | & Local Cache      |  |
|                                                                 +--------------------+  |
+-----------------------------------------------------------------------------------------+
```

---

## 3. Pairing Protocol & QR Scheme

### 3.1 QR URI Format
The desktop daemon generates an ephemeral pairing URI encoded into a standard QR code:

```
aether://pair?host=<IP>&port=<PORT>&session=<UUID>&key=<PSK_HEX>&exp=<UNIX_TIMESTAMP>
```

| Parameter | Type | Description |
| :--- | :--- | :--- |
| `host` | `String` | IPv4 address of the workstation on the local network (e.g., `192.168.1.100`) |
| `port` | `UInt16` | TCP listener port of the Aether daemon (default: `44888`) |
| `session` | `UUIDv4` | Unique session identifier for this pairing interaction |
| `key` | `Hex String` | 256-bit cryptographically secure random Pre-Shared Key (32 bytes = 64 hex chars) |
| `exp` | `UInt64` | Expiration timestamp in seconds since UNIX epoch (default TTL: 300 seconds) |

### 3.2 Pairing Flow
1. Developer clicks **"Pair Mobile Companion"** in AetherIDE desktop.
2. AetherIDE requests `SyncGeneratePairing` over daemon IPC.
3. The daemon generates a 256-bit random key via OS entropy (`ring::rand::SystemRandom`), creates a 5-minute ephemeral session, and displays the QR code.
4. The developer points the iOS camera at the QR code.
5. The iOS companion parses the URI, verifies that `now < expiresAt`, stores the PSK in iOS Keychain, and immediately initiates an authenticated AEAD handshake.

---

## 4. Cryptographic Transport Protocol

All frames exchanged between desktop and mobile companions are encrypted using **ChaCha20-Poly1305** (`ring::aead::CHACHA20_POLY1305`).

### 4.1 Wire Frame Structure
Each wire frame consists of:
```
+-----------------------------------+------------------------------------+
| Field                             | Size / Type                        |
+-----------------------------------+------------------------------------+
| Monotonic Sequence Counter        | 8 bytes (u64 big-endian)           |
| Encrypted Payload + Poly1305 Tag  | Variable length + 16 bytes tag     |
+-----------------------------------+------------------------------------+
```

### 4.2 Nonce Construction & Anti-Replay
- **Salt**: An 8-byte role-based salt is derived at session establishment (`SyncRole::Desktop` vs `SyncRole::Mobile`).
- **Nonce**: 12 bytes = `Salt (4 bytes) || Monotonic Sequence (8 bytes big-endian)`.
- **Replay Protection**: The receiver tracks `last_received_seq`. Any incoming frame where `seq <= last_received_seq` is rejected immediately with `SyncCryptoError::ReplayAttackDetected`.
- **Tamper Protection**: Any bit modification to the ciphertext or truncated frames results in Poly1305 tag verification failure (`SyncCryptoError::DecryptionFailed`).

---

## 5. Differential Workspace Synchronization

### 5.1 Manifest Generation
Before sending any file content, the desktop generates a `WorkspaceManifest`:
- Scans files recursively, respecting default exclusions (`.git`, `node_modules`, `target`, `.build`).
- Calculates SHA-256 hash, file size, and modification timestamp for every file entry.
- Computes a global Merkle `root_hash` by sorting all relative paths and hashing the concatenated `path || sha256`.

### 5.2 3-Way Conflict Detection
When manifests are compared (`diff_manifests`), the engine identifies:
- `to_upload`: Files present locally that are missing on remote, or where hashes differ and timestamps indicate local is newer.
- `to_download`: Files present on remote that are missing locally.
- `conflicts`: Files modified concurrently on both peers with distinct hashes and distinct modification timestamps.

### 5.3 64 KB Content-Addressable Chunking
Files needing synchronization are sliced into 64 KB chunks (`FileChunk`):
- Each chunk contains `path`, `chunk_index`, `total_chunks`, raw `data`, and chunk-level `sha256`.
- The receiving peer verifies chunk SHA-256 integrity before storing.
- When all `total_chunks` arrive, `reassemble_chunks` reconstructs the original file atomically.

---

## 6. IPC Interface Reference

### 6.1 `SyncGeneratePairing`
Generates an ephemeral QR pairing session.

**Request:**
```json
{
  "id": "req-101",
  "session_token": "<DAEMON_TOKEN>",
  "type": "Request",
  "data": {
    "action": "SyncGeneratePairing",
    "params": {
      "device_name": "Studio-Workstation",
      "host": "192.168.1.150",
      "port": 44888
    }
  }
}
```

**Response:**
```json
{
  "success": true,
  "status_code": 200,
  "data": {
    "device_name": "Studio-Workstation",
    "qr_uri": "aether://pair?host=192.168.1.150&port=44888&session=...&key=...&exp=...",
    "pairing_payload": {
      "host": "192.168.1.150",
      "port": 44888,
      "session_id": "9b1deb4d-3b7d-4bad-9bdd-2b0d7b3dcb6d",
      "psk_hex": "a1b2c3d4...",
      "expires_at": 1730336000
    }
  }
}
```

### 6.2 `SyncGetManifest`
Generates a snapshot manifest for a workspace directory.

**Request:**
```json
{
  "id": "req-102",
  "session_token": "<DAEMON_TOKEN>",
  "type": "Request",
  "data": {
    "action": "SyncGetManifest",
    "params": {
      "workspace_path": "C:\\Projects\\MyProject",
      "workspace_name": "MyProject"
    }
  }
}
```

**Response:**
```json
{
  "success": true,
  "status_code": 200,
  "data": {
    "workspace_name": "MyProject",
    "root_hash": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
    "entries": {
      "src/main.rs": {
        "path": "src/main.rs",
        "size": 1024,
        "sha256": "4b227777d4dd1fc61c6f884f48641d02b4d121d3fd328cb08b5531fcacdabf8a",
        "modified_at": 1730332000
      }
    },
    "generated_at": 1730335000
  }
}
```

---

## 7. Security Guarantees & Verification Checklist

| Security Property | Implementation Mechanism | Test Verification |
| :--- | :--- | :--- |
| **Authentication & Confidentiality** | ChaCha20-Poly1305 AEAD (`ring`) | `test_aead_encrypt_decrypt_roundtrip` passed |
| **Tamper Resistance** | Poly1305 128-bit MAC tag verification | `test_aead_rejects_tampering` passed |
| **Anti-Replay Protection** | Strictly monotonic 64-bit sequence counters | `test_aead_rejects_replay_attack` passed |
| **Integrity Verification** | Chunk-level and root-level SHA-256 hashes | `test_rejects_corrupted_chunk` passed |
| **Ephemeral Session Security** | 300s TTL + OS entropy (`SystemRandom`) | `test_pairing_uri_roundtrip` passed |
| **Daemon Authentication** | 401 Unauthorized check on all sync actions | `test_unauthenticated_request_rejected` passed |
