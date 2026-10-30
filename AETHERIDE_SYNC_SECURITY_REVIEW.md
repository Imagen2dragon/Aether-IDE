# AETHERIDE — WORKSPACE SYNC CRYPTOGRAPHIC SECURITY REVIEW
## Zero-Knowledge Local Peer Synchronization, Wire Framing, and Protocol Formalization

**Document Type:** Principal Cryptographic Security Audit  
**Auditor:** Security & Distributed Systems Architect  
**Date:** 2026-10-31  
**Project:** AetherIDE (`rust/crates/workspace-sync`)  
**Status:** COMPLETE & AUTHORITATIVE  

---

## 1. Resolution of Wire Specification Discrepancy

A previous draft documentation contained a minor ambiguity between an 8-byte role salt versus a 4-byte prefix + 8-byte sequence counter.

This audit definitively formalizes the wire specification implemented in `workspace-sync::crypto`:

### 1.1 Nonce Construction (12 Bytes Total)
ChaCha20-Poly1305 (`ring::aead::CHACHA20_POLY1305`) strictly requires a 96-bit (12-byte) unique nonce for every message encrypted under a given key. In AetherIDE:

```
+------------------------------------+------------------------------------+
| Bytes 0..3 (4 Bytes)               | Bytes 4..11 (8 Bytes)              |
+------------------------------------+------------------------------------+
| Role Directional Salt Prefix       | Strictly Monotonic Sequence Counter|
+------------------------------------+------------------------------------+
```

1. **Role Directional Salt Prefix (4 Bytes):**
   - **Desktop Transmitter:** `[0x44, 0x53, 0x4B, 0x01]` (`"DSK\x01"`)
   - **Companion Transmitter:** `[0x4D, 0x4F, 0x42, 0x01]` (`"MOB\x01"`)
   Because the salt prefixes differ between Desktop and Companion, even if both peers transmit the exact same sequence number `1`, the nonces `DSK\x01\x00...\x01` and `MOB\x01\x00...\x01` are mathematically disjoint. Nonce collision across directional channels is impossible.

2. **Monotonic Sequence Counter (8 Bytes, Big-Endian):**
   - Transmitters start at `send_seq = 1` and increment by `1` per frame (`send_seq += 1`).
   - Receivers maintain `recv_seq`. Any incoming frame with `seq <= recv_seq` is rejected immediately with `SyncCryptoError::ReplayDetected(seq, recv_seq)`.
   - Overflow protection: An 8-byte unsigned integer allows $2^{64}-1$ frames (over 18 quintillion frames), exceeding any physical device lifetime before exhaustion.

### 1.2 Wire Frame Structure
Each self-contained AEAD wire frame is formatted as follows:

```
+---------------+--------------------+------------------------------------+
| Field         | Size               | Description                        |
+---------------+--------------------+------------------------------------+
| Sequence No.  | 8 Bytes (u64 BE)   | Sequence identifier for AAD check  |
| Payload Len   | 4 Bytes (u32 BE)   | Length of Ciphertext + 16-byte MAC |
| Ciphertext    | Variable           | Encrypted payload bytes            |
| Poly1305 Tag  | 16 Bytes           | Poly1305 Authenticated MAC tag     |
+---------------+--------------------+------------------------------------+
```

### 1.3 Additional Authenticated Data (AAD)
To prevent length-extension or frame truncation attacks, the AEAD sealing operation binds both the `sequence` (8 bytes) and original `plaintext_len` (4 bytes) into the Poly1305 MAC:
$$\text{AAD} = \text{seq} \mathbin{\Vert} \text{len}$$
Any modification to the sequence number or frame length in transit causes immediate Poly1305 authentication failure (`SyncCryptoError::AuthenticationFailed`).

---

## 2. Pairing Scheme Security

### 2.1 The Ephemeral QR URI
```
aether://pair?host=<IP>&port=<PORT>&session=<UUIDv4>&key=<PSK_HEX>&exp=<UNIX_TIMESTAMP>
```

### 2.2 Security Properties & Safeguards
1. **Zero Cloud Transit:** The pairing URI and Pre-Shared Key (PSK) never touch external cloud servers. The QR code is rendered directly on the desktop monitor and scanned optically by the companion camera.
2. **Ephemeral TTL (300 Seconds):** The expiration timestamp (`exp`) limits valid pairing to a 5-minute window from generation. Any session older than 300 seconds is discarded by both peers.
3. **Hardware OS Entropy:** The 256-bit PSK is generated using `ring::rand::SystemRandom`, tapping hardware entropy sources (`BCryptGenRandom` on Windows, `/dev/urandom` on POSIX).
4. **No Sensitive Persistence in Logs:** Daemon loggers explicitly redact or omit pairing keys, and URLs are never written to public temp files.
5. **Session Re-Keying on Reconnect:** Reconnecting endpoints trigger an ephemeral re-handshake to establish fresh session sequence numbers, preventing stale sequence synchronization issues.

---

## 3. End-to-End Live Transfer Verification

The protocol verifies:
1. `ManifestExchange`: SHA-256 Merkle root verification.
2. `ChunkTransfer`: 64 KB content-addressable blocks sliced and transmitted with independent chunk hashes.
3. `AtomicStaging`: Received chunks are assembled into a `.staging` file and atomically renamed upon SHA-256 validation.
4. `ConflictHandling`: Concurrent modification of files yields a `.conflict` snapshot preserving both developer versions.
