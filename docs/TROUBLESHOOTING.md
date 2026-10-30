# AetherIDE — Troubleshooting & Diagnostics Guide

## 1. Diagnostic Triage Workflow
If you experience any issues launching AetherIDE, run the built-in diagnostic suite:
```powershell
cd rust
cargo run --bin aether-cli -- doctor
```

---

## 2. Common Issues & Remediations

### Issue: "Cannot connect to Aether daemon at 127.0.0.1:44888"
- **Cause:** The `aether-daemon` background process is not running or crashed due to port collision.
- **Resolution:**
  1. Check if the port is already in use: `netstat -ano | findstr 44888`.
  2. Launch the daemon manually in a terminal to inspect error logs:
     ```powershell
     cd rust
     cargo run --bin aether-daemon
     ```

### Issue: "SQLite database locked or corrupted"
- **Cause:** Abrupt power termination while in transaction.
- **Resolution:**
  1. Delete temporary lock files in `%LOCALAPPDATA%\AetherIDE\aether.db-shm` and `aether.db-wal`.
  2. AetherIDE will automatically re-run WAL recovery on next launch.

### Issue: "EAS build command not found"
- **Cause:** EAS CLI not installed globally.
- **Resolution:**
  ```powershell
  npm install -g eas-cli
  ```
