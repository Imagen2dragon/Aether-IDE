# ADR-003: End-to-End Encrypted Workspace Synchronization Security Architecture

**Status:** ACCEPTED  
**Date:** 2026-10-30  
**Authors:** Aether Security & Distributed Systems Engineering  
**Scope:** Desktop Host, iOS Companion (`apps/ios-companion`), Daemon Sync Engine

---

## 1. Context and Threat Model

AetherIDE enables developers to review, edit, inspect ASTs, and manage workflows from mobile devices (iOS/iPadOS) in real time over local networks. 

### 1.1 Operating Environment
- Shared and untrusted Local Area Networks (e.g. coffee shops, airport Wi-Fi, conference networks, home routers).
- Peers on the subnet cannot be assumed to be benevolent.

### 1.2 Threat Analysis
| Threat Vector | Severity | Attack Mechanism | Mitigation |
|---|---|---|---|
| **Eavesdropping** | High | Passive packet capture of source code files on Wi-Fi | Authenticated Encryption (AEAD ChaCha20-Poly1305 / AES-256-GCM) |
| **mDNS Spoofing** | High | Malicious device advertises fake `_aether-sync._tcp` service | Out-of-band QR pairing with ephemeral 256-bit PSK |
| **Replay Attacks** | High | Attacker captures valid sync frames and replays them later | Monotonic 64-bit frame sequence counter in AEAD nonces |
| **Data Tampering** | Critical | Active MITM modifies code or injects malicious binaries | Poly1305 cryptographic authentication tag verification |
| **Path Traversal** | Critical | Remote device sends paths like `../../Windows/System32` | Path sanitization, strict canonical workspace confinement |

---

## 2. Authentication & Key Exchange

### 2.1 Out-of-Band (OOB) QR Pairing Flow
```
+------------------------------------+             +------------------------------------+
|            Desktop Host            |             |            iOS Companion           |
+------------------------------------+             +------------------------------------+
                  |                                                  |
     1. Generate Session ID & PSK                                    |
        (32-byte cryptographic random)                               |
                  |                                                  |
     2. Display Ephemeral QR Code                                    |
        { host, port, session_id, psk }                              |
                  |                                                  |
                  | <------------- 3. Camera Scan QR -------------- |
                  |                                                  |
                  | <======== 4. TLS / AEAD Mutual Handshake =======>|
                  |    - Verifies PSK possession                     |
                  |    - Derives Tx/Rx Session Keys                  |
                  |                                                  |
```

1. **Generation:** When pairing is requested, the desktop host generates:
   - `session_id`: 128-bit UUID v4.
   - `psk`: 256-bit cryptographically secure random key.
   - `expires_at`: 300 seconds TTL.
2. **Delivery:** Displayed on screen as a QR code containing `aether://pair?host=<ip>&port=<port>&session=<id>&key=<hex>`.
3. **Handshake:** Companion connects to desktop socket and proves possession of `psk` via HMAC-SHA256 challenge-response before any workspace data is revealed.

---

## 3. Wire Protocol & AEAD Framing

Every network packet transmitted across the synchronization channel is encapsulated in a tamper-proof AEAD frame:

```
+-------------------+--------------------+--------------------+--------------------+
| Sequence (8 bytes)|  Payload Len (4 B) | Ciphertext (N B)   | Auth Tag (16 B)    |
+-------------------+--------------------+--------------------+--------------------+
```

- **AEAD Algorithm:** ChaCha20-Poly1305 (RFC 8439) with 256-bit session key.
- **Nonce Derivation:** 12-byte nonce = `4-byte role salt` || `8-byte big-endian sequence number`.
- **Replay Protection:** Receiver tracks the highest seen sequence number. Out-of-order, duplicate, or backwards sequence numbers cause immediate frame drop and connection termination.
- **Integrity Tag:** Any modification to ciphertext or associated data (sequence number + payload length) results in authentication rejection.

---

## 4. Manifest Synchronization & Chunking

### 4.1 Merkle Snapshot Engine
1. **Manifest Calculation:** Both peers compute a manifest of all tracked workspace files:
   - Relative POSIX path.
   - File size.
   - SHA-256 hash of entire file.
   - Last modified timestamp.
2. **Differential Comparison:** Manifests are exchanged over the encrypted channel. Set difference determines:
   - Files to push (present locally with different hash or absent remotely).
   - Files to pull.
   - Unchanged files (skipped entirely).

### 4.2 Content-Addressable Chunking
- Files larger than 64KB are partitioned into fixed 64KB content-addressable blocks.
- Each block is identified by `sha256(chunk_bytes)`.
- If a 10MB file changes by only 1 line, only the affected 64KB block is transferred.

### 4.3 Conflict Resolution
When both desktop and companion have modified the same file concurrently:
1. The remote modification is written to `<filename>.conflict.<timestamp>`.
2. The user is presented with a diff viewer in the UI to choose or merge changes.
3. No data is silently overwritten or lost.
