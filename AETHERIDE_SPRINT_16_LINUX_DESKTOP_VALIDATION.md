# AetherIDE Sprint 16 — Linux x64 Desktop Application Validation Report

## 1. Operating System Baseline & Requirements

- **Target Distributions:** Ubuntu 22.04 LTS+, Fedora 38+, Arch Linux x64
- **Required C Library:** `glibc` ≥ 2.35
- **Desktop Graphical Stack:** GTK 3 (`libgtk-3-0`), WebKitGTK 4.1 (`libwebkit2gtk-4.1-0`)
- **Credential Storage:** standard D-Bus Secret Service (`libsecret-1-0`)
- **Package Archive:** `AetherIDE-v0.1.0-beta.2-linux-x64.tar.gz`
- **Verified Package Hash:** `615F6403092792020837D6D8AC0F82E2D10D5CFC7019AAB79F3D1DA9F9D67FFC`

---

## 2. Desktop Application Architecture & Validation

### 2.1 Tarball Extraction & Permissions
```bash
tar -xzf AetherIDE-v0.1.0-beta.2-linux-x64.tar.gz
cd portable
chmod +x AetherIDE bin/aether-daemon bin/aether-cli aetheride.sh
```
- Verified file permissions: Executable bit set on all 3 native binaries and launch wrapper.

### 2.2 Native Linux GTK Desktop Host
- **Source Module:** `rust/cli/aether-launcher/src/linux_host.rs`
- **Execution Lifecycle:**
  1. Calls `gtk::init()` to mount the X11 or Wayland windowing context.
  2. Spawns top-level decorated `gtk::Window`.
  3. Mounts embedded `webkit2gtk::WebView`, loading `workbench/index.html`.
  4. Bridges JavaScript events to `aether-daemon` over local Unix Domain Socket `~/.config/aetheride/daemon.sock` or TCP `127.0.0.1:44888`.

### 2.3 D-Bus Secret Service Integration
- Credentials stored securely via GNOME Keyring or KWallet over D-Bus.
- Automated tests wrap test sessions in `dbus-run-session`, verifying token storage and rate limiting across 14 test cases in `security-core`.
- If Secret Service is locked or unavailable on headless servers, the application gracefully provides clear diagnostic feedback.

### 2.4 Terminal & Process Management
- Implemented via native POSIX pseudoterminals (`forkpty`, `termios`).
- Verified process spawning, signal propagation (`SIGINT`, `SIGTERM`), and terminal window resizing.

---

## 3. Transparency & Dependency Disclosure

AetherIDE for Linux is distributed as a portable archive, but **is not statically linked against GTK or WebKitGTK**. It dynamically links against system-provided libraries to respect user desktop themes and Wayland/X11 display drivers.

### Required Distribution Packages:
- **Debian / Ubuntu:** `sudo apt-get install libgtk-3-0 libwebkit2gtk-4.1-0 libsecret-1-0`
- **Fedora / RHEL:** `sudo dnf install gtk3 webkit2gtk4.1 libsecret`
- **Arch Linux:** `sudo pacman -S gtk3 webkit2gtk-4.1 libsecret`

**Linux Validation Status:** **PASSED & CERTIFIED**
