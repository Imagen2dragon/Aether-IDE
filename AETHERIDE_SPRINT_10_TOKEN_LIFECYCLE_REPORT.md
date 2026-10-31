# AetherIDE Sprint 10 Token Lifecycle Report

## 1. Executive Summary
In Sprint 10, the token management system in `security-core` and `aether-daemon` was hardened against token replay, stale credential usage, and plain-text file leaks:
1. **Migration from Legacy File:** The legacy plaintext token file (`daemon_session.token`) has been retired and removed. Daemon startup explicitly scrubs any existing legacy file and writes authenticated session metadata exclusively into `desktop_bootstrap.json` with restricted permissions.
2. **Server-Controlled Token Store:** All tokens are registered as cryptographic `SessionRecord` entries in `TokenStore` with explicit Time-To-Live (TTL) parameters.
3. **Graceful Rotation:** Token rotation maintains a strict 30-second grace period for the retiring token before complete invalidation.
4. **Immediate Revocation Propagation:** Revocation via `TokenStore::revoke_session` or `TokenStore::revoke_peer_sessions` immediately terminates all active connections bound to that session or peer.

## 2. Token Lifecycle States

```
                +-------------------------+
                |  Generated / Registered |
                +------------+------------+
                             |
                             v
                +-------------------------+
                |     Active Session      |
                +----+---------------+----+
                     |               |
       Rotate Token  |               |  Explicit Revocation / Expiry
                     v               v
        +-------------------+  +-------------------+
        | Grace Period (30s)|  | Revoked / Expired |
        +---------+---------+  +-------------------+
                  |                      ^
       Elapsed    +----------------------+
```

## 3. Session Revocation Consistency
When a session is revoked:
- `TokenStore::revoke_session(token)` inserts the token into a dedicated revoked set and removes its session record.
- Any subsequent request bearing this token is matched by `token_store.authenticate_session(token)`, which evaluates to `AuthVerificationResult::Revoked` and immediately returns `401 Unauthorized` (`"Session token has been revoked"`).
- `TokenStore::revoke_peer_sessions(peer_id)` iterates across all active sessions registered to that peer and revokes every token associated with them.

## 4. Response Token Sanitization (SEC-03 / SEC-04)
Previously, `IpcEnvelope` responses defaulted to populating `session_token` with the server's master token. In Sprint 10:
- Every `IpcEnvelope` emitted by `aether-daemon` sets `session_token: None`.
- Bearer tokens are strictly one-way credentials delivered only via authorized bootstrap or pairing mechanisms.

## 5. Automated Verification Tests
- `test_sec_05_session_rotation_consistency`: Verifies that after token rotation, the old token works only within the 30-second grace window, and once expired or rotated out, it is permanently rejected.
- `test_token_store_revocation`: Verifies immediate invalidation upon calling `revoke_token`.
- `test_daemon_authorization_10_cases` (Case 10): Verifies that a revoked token receives 401 Unauthorized across all daemon endpoints.
