# AetherIDE Sprint 10 IPC Rate Limiting Report

## 1. Executive Summary
In Sprint 10, the transport and authentication rate-limiting systems in `security-core` and `aether-daemon` were enhanced to eliminate Denial of Service (DoS) vulnerabilities on both Windows Named Pipes and TCP loopback sockets.

## 2. Identified Vulnerability (SEC-07 & SEC-08)
Prior to Sprint 10:
- All Named Pipe connections were identified with the static source string `"windows-named-pipe"`.
- When an untrusted client connected over Named Pipe and repeatedly failed authentication (e.g. 5 failed attempts within 60 seconds), the `TokenStore` rate limiter blocked all subsequent requests from `"windows-named-pipe"`.
- Because the legitimate desktop workbench also connected over `"windows-named-pipe"`, an attacker or malfunctioning script could completely lock out the IDE desktop user interface.

## 3. Remediated Architecture

### 3.1 Isolated Named Pipe Client Identifiers
In `rust/services/aether-daemon/src/main.rs`:
```rust
static PIPE_CLIENT_COUNTER: AtomicU64 = AtomicU64::new(1);
...
let client_id = PIPE_CLIENT_COUNTER.fetch_add(1, Ordering::Relaxed);
let pipe_source = format!("named-pipe-client-{}", client_id);
```
Each individual Named Pipe connection stream receives a unique, isolated identifier (`named-pipe-client-<id>`).
- If a rogue connection repeatedly sends invalid tokens, only that specific connection's rate limiter bucket is incremented and locked.
- The desktop host runs on its own connection (`desktop-host-1` or `named-pipe-client-1`) and remains completely unaffected.

### 3.2 Layered TCP Rate Limiting
On TCP connections:
- The rate limiter bucket is keyed by the client's socket address (e.g. `127.0.0.1:<ephemeral-port>` or client IP).
- Failed authentication attempts increment a sliding window counter. After 5 consecutive failures, the source is locked for a cooling period of 60 seconds.
- Successful authentications reset the failure counter.

## 4. Verification Test Evidence
Verified in `rust/services/aether-daemon/src/main.rs`:
- `test_sec_07_and_08_transport_rate_limiting_isolation`:
  - An adversarial client floods the daemon with invalid authentication tokens on connection `attacker-pipe-1`.
  - The daemon locks out `attacker-pipe-1` after 5 failed attempts (returning `429 Too Many Requests`).
  - Concurrently, a legitimate desktop client connects on `desktop-host-1` with a valid owner token and issues requests.
  - The legitimate desktop client succeeds immediately with status 200, proving zero cross-client lockout.
