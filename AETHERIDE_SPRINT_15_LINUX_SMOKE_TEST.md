# AetherIDE Sprint 15 — Linux x64 Desktop Smoke Test & Validation Report

## 1. Test Environment & Baseline Requirements

- **Distribution:** Ubuntu 22.04 LTS / 24.04 LTS x64 (CI Runner: `ubuntu-latest`)
- **Package Archive:** `AetherIDE-v0.1.0-beta.2-linux-x64.tar.gz`
- **Verified SHA-256:** `4C0D497EF91E35B219CA2A3DF0A759DEF86F9613F6A9A128FD345EDD6CF130A7`
- **Dynamic Linker / C Library:** `glibc` 2.35+
- **Desktop Subsystem:** GTK 3 (`libgtk-3-0`), WebKitGTK 4.1 (`libwebkit2gtk-4.1-0`), Secret Service (`libsecret-1-0`)

---

## 2. Test Execution & Workflow Evidence

### Step 1: Tarball Extraction & Permissions
```bash
tar -xzf AetherIDE-v0.1.0-beta.2-linux-x64.tar.gz
cd portable
ls -la
```
- Permissions verified: `AetherIDE` (`0755`), `bin/aether-daemon` (`0755`), `bin/aether-cli` (`0755`), `aetheride.sh` (`0755`).
- **Verdict:** **PASSED**

### Step 2: Native GTK Desktop Host Verification
The Linux desktop host is implemented via `linux_host::LinuxGtkHost`:
- Initializes GTK application context (`gtk::init`).
- Creates top-level `gtk::Window` titled `"AetherIDE — AI-Native Engineering Studio"`.
- Embeds `webkit2gtk::WebView`, loading `portable/workbench/index.html`.
- Registers WebKit user content manager script messages to bridge IPC requests to `bin/aether-daemon`.
- **Verdict:** **PASSED**

### Step 3: D-Bus Secret Service & Keyring Validation
In automated CI and test environments, `dbus-run-session` provisions a local D-Bus daemon:
- The Linux keyring provider communicates via standard D-Bus Secret Service API.
- In headless containerized environments lacking an active keyring unlock agent, the daemon safely falls back to session-bound ephemeral protection without hanging or crashing.
- **Verdict:** **PASSED**

### Step 4: CLI & Daemon Subsystem
- `bin/aether-cli doctor`: Validates Linux compilers, Node.js, and Git.
- `bin/aether-daemon`: Binds loopback TCP `127.0.0.1:44888` and Unix Domain Socket `~/.config/aetheride/daemon.sock`.
- `bin/aether-cli ping`: Responds with `{ "pong": true, "version": "0.1.0-beta.2" }`.
- **Verdict:** **PASSED**

---

## 3. Platform Limitations & Guidance for Pilot Developers
- **Headless Linux Servers:** On minimal distributions lacking X11 or Wayland, the graphical workbench cannot launch; pilot developers can run `bin/aether-daemon` and `bin/aether-cli` headlessly.
- **WebKitGTK Compatibility:** Systems with older WebKitGTK 4.0 must install `libwebkit2gtk-4.1-0` (`sudo apt-get install libwebkit2gtk-4.1-0 libsecret-1-0`).
