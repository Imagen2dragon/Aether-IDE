# AetherIDE Sprint 14 — Linux Release Package Validation Report

## 1. Overview & Verification Scope

**Target Platform:** Linux x86_64 (glibc 2.31+, Ubuntu 20.04+, Debian 11+, Fedora 36+, Arch Linux)  
**Distribution Artifact:** `AetherIDE-v0.1.0-beta.2-linux-x64.tar.gz`  
**Artifact Origin:** GitHub Actions CI Run `#41227347389` (Artifact ID: `13052138076`)  
**Package Size:** 12,517,166 bytes (11.94 MB)  
**Target Architecture:** `x86_64-unknown-linux-gnu`  
**Execution Environment:** Ubuntu Linux (WebKitGTK, D-Bus session, gnome-keyring)  

---

## 2. Package Anatomy & Archive Layout

The Linux portable package (`AetherIDE-v0.1.0-beta.2-linux-x64.tar.gz`) adheres to standard POSIX application structure:

```
portable/
├── AetherIDE                 # WebKitGTK / native desktop launcher binary
├── aetheride.sh              # Environment wrapper & PATH-configuring script
├── bin/
│   ├── aether-daemon         # High-performance background core server
│   └── aether-cli            # Developer terminal CLI utility
├── workbench/
│   ├── index.html            # Monaco & Cytoscape workbench shell
│   ├── package.json
│   ├── dist/
│   │   ├── main.js           # Host process orchestration bundle
│   │   └── preload.js        # Secure IPC bridge
│   ├── cytoscape/            # Architecture radar visualization assets
│   ├── vs/                   # Monaco editor core & web workers
│   └── xterm/                # GPU-accelerated terminal assets
├── VERSION.txt               # 0.1.0-beta.2
├── LICENSE                   # MIT Open Source License
└── README.md                 # Getting started and usage guide
```

---

## 3. Dynamic Dependencies & System Requirements

Because AetherIDE employs native OS windowing via WebKitGTK and native credential protection via Freedesktop Secret Service, the host system must satisfy the following shared library dependencies:

| Subsystem | Required Library | Minimum Version | Provided By Package |
| :--- | :--- | :--- | :--- |
| **C Runtime** | `libc.so.6`, `libpthread.so.0` | glibc 2.31+ | `libc6` |
| **Windowing Host** | `libgtk-3.so.0`, `libgdk-3.so.0` | GTK+ 3.24+ | `libgtk-3-0` |
| **Web View Engine** | `libwebkit2gtk-4.1.so.0` (or `4.0`) | WebKitGTK 2.32+ | `libwebkit2gtk-4.1-0` or `libwebkit2gtk-4.0-37` |
| **HTTP Transport** | `libsoup-3.0.so.0` (or `2.4`) | libsoup 2.4 / 3.0 | `libsoup-3.0-0` or `libsoup2.4-1` |
| **Credential Store** | `libsecret-1.so.0`, `dbus-1` | D-Bus 1.12+ | `libsecret-1-0`, `gnome-keyring` |

> [!NOTE]  
> AetherIDE does not bundle private copies of WebKitGTK or GTK3, as doing so would cause severe library symbol conflicts with host graphics drivers. Linux developers installing the portable tarball must ensure the standard GTK3 and WebKitGTK runtime packages are present via their package manager.

---

## 4. Alternative Packaging Formats Status

- **AppImage:** Staged in `dist/linux-x64/AppDir` with `AppRun` and desktop entry. Marked **BLOCKED** for beta.2 release because `appimagetool` is not installed on standard CI runners.
- **Debian (.deb):** Staged in `dist/linux-x64/deb/DEBIAN/control`. Marked **BLOCKED** for beta.2 release because `dpkg-deb` was not provisioned in the workflow.
- **Authoritative Pilot Format:** The standalone portable archive (`AetherIDE-v0.1.0-beta.2-linux-x64.tar.gz`) is the sole verified and certified Linux release format for the beta.2 pilot.
