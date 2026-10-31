# AETHERIDE — SPRINT 12 REPORT: LINUX SECRET SERVICE VALIDATION

**Product:** AetherIDE (`v0.1.0-beta.2`)  
**Sprint:** Sprint 12 — Cross-Platform Credential Vault & Release Certification  
**Platform:** Linux x64 (x86_64-unknown-linux-gnu / Ubuntu)  
**Date:** 2026-10-31  
**Status:** Certified  

---

## 1. Pure-Rust D-Bus Secret Service Architecture

On Linux, AetherIDE connects to the Freedesktop Secret Service standard (`org.freedesktop.secrets`) implemented by GNOME Keyring, KWallet, or KeePassXC.

### Pure-Rust Implementation
Rather than linking against the legacy C library `libsecret-1.so` (which introduces dynamic shared object ABI version incompatibilities across distributions), AetherIDE utilizes `keyring` v4.2.0 with the `zbus-secret-service-keyring-store` feature.
- **Protocol:** D-Bus over UNIX domain sockets (`/run/user/$UID/bus`).
- **Driver:** Pure-Rust `zbus` (asynchronous D-Bus protocol implementation).
- **Zero C Dependencies:** Guarantees ABI stability across Debian, Ubuntu, Fedora, Arch, and Alpine.

```mermaid
flowchart TD
    App[AetherIDE Daemon / CLI] -->|OsKeyringStore| Store[zbus-secret-service-keyring-store]
    Store -->|Pure-Rust D-Bus IPC| Socket[/run/user/$UID/bus]
    Socket --> Daemon[GNOME Keyring / KWallet Daemon]
    Daemon --> Vault[(Encrypted Secret Storage)]
```

---

## 2. CI and Headless Container Handling

In Linux CI environments (`ubuntu-latest`), headless runners often lack an active D-Bus session bus.
In Sprint 12, we:
1. Updated `.github/workflows/ci.yml` to install `dbus-x11` and `gnome-keyring`.
2. Built robust error handling into `OsKeyringStore`: if no Secret Service daemon is registered on the D-Bus bus, `protect_credential` fails immediately with an explicit error:
   ```
   Err("OS credential service unavailable: ...")
   ```
   Under NO circumstances does it fall back to plaintext or repeating XOR masks.
3. Provided `EphemeralMemoryStore` for test suites and memory-only environments where disk and OS keyrings are omitted by design.
