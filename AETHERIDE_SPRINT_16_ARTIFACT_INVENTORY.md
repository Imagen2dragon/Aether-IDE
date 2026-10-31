# AetherIDE Sprint 16 — Package Archive Inventory & Payload Audit Report

## 1. Executive Summary

This report documents the archive inventory, binary permissions, and asset composition of the three official AetherIDE `v0.1.0-beta.2` packages generated from commit `a2e40de`.

---

## 2. Windows x64 Distribution Package
**File:** `AetherIDE-v0.1.0-beta.2-windows-x64.zip` (10,619,788 bytes compressed / ~34.2 MB unpacked)

| Relative Path | Size | Type | Function |
| :--- | :--- | :--- | :--- |
| `AetherIDE.exe` | 2,969,600 B | Win32 PE Executable | Win32 + WebView2 desktop launcher & CLI router |
| `AetherIDE.cmd` | 134 B | Batch Script | Command-line wrapper |
| `bin/aether-daemon.exe` | 11,084,288 B | Win32 PE Executable | Core daemon (IPC, models, agents, indexer) |
| `bin/aether-cli.exe` | 3,202,048 B | Win32 PE Executable | Developer CLI utility |
| `workbench/index.html` | 101,910 B | HTML5 UI | IDE workbench single-page application |
| `workbench/package.json` | 762 B | JSON | Workbench package metadata |
| `workbench/vs/loader.js` | 30,061 B | JavaScript | Monaco AMD module loader |
| `workbench/vs/editor/` | ~3.8 MB | Assets | Monaco editor core, workers, and styles |
| `workbench/vs/basic-languages/` | ~4.2 MB | Assets | Syntax tokenizers for Rust, TS, Python, etc. |
| `workbench/vs/language/` | ~2.1 MB | Assets | Monaco language intelligence features |
| `workbench/xterm/` | ~300 KB | Assets | xterm.js terminal emulator and addons |
| `workbench/cytoscape/` | 435,680 B | Assets | Cytoscape.js interactive graph visualizer |
| `workbench/dist/main.js` | 2,607 B | JavaScript | IPC bridge bindings |
| `workbench/dist/preload.js` | 283 B | JavaScript | Context isolation preload |
| `README.md` | 7,877 B | Markdown | User guide and quickstart |
| `LICENSE` | 1,101 B | Text | MIT License |
| `VERSION.txt` | 14 B | Text | `0.1.0-beta.2` |
| `RELEASE_INFO.json` | 252 B | JSON | Machine-readable release metadata |

---

## 3. Linux x64 Distribution Package
**File:** `AetherIDE-v0.1.0-beta.2-linux-x64.tar.gz` (12,538,961 bytes compressed / ~38.6 MB unpacked)

| Relative Path | Permissions | Size | Function |
| :--- | :--- | :--- | :--- |
| `portable/AetherIDE` | `0755` (rwxr-xr-x) | ~3.8 MB | Native GTK3 + WebKitGTK launcher |
| `portable/aetheride.sh` | `0755` (rwxr-xr-x) | 185 B | Launch script configuring library paths |
| `portable/bin/aether-daemon` | `0755` (rwxr-xr-x) | ~14.1 MB | Linux background daemon executable |
| `portable/bin/aether-cli` | `0755` (rwxr-xr-x) | ~4.5 MB | Linux CLI executable |
| `portable/workbench/` | `0755` / `0644` | ~11.2 MB | Monaco, xterm.js, Cytoscape web assets |
| `portable/LICENSE` | `0644` (rw-r--r--) | 1,101 B | MIT License |
| `portable/README.md` | `0644` (rw-r--r--) | 7,877 B | Linux setup documentation |
| `portable/VERSION.txt` | `0644` (rw-r--r--) | 14 B | `0.1.0-beta.2` |

---

## 4. macOS ARM64 Distribution Package
**File:** `AetherIDE-v0.1.0-beta.2-macos-arm64.tar.gz` (8,889,706 bytes compressed / ~29.4 MB unpacked)

| Relative Path | Permissions | Size | Function |
| :--- | :--- | :--- | :--- |
| `AetherIDE.app/Contents/MacOS/AetherIDE` | `0755` (rwxr-xr-x) | ~3.6 MB | Native Cocoa + WKWebView launcher (ARM64) |
| `AetherIDE.app/Contents/MacOS/bin/aether-daemon` | `0755` (rwxr-xr-x) | ~10.8 MB | Mach-O 64-bit arm64 daemon binary |
| `AetherIDE.app/Contents/MacOS/bin/aether-cli` | `0755` (rwxr-xr-x) | ~3.6 MB | Mach-O 64-bit arm64 CLI binary |
| `AetherIDE.app/Contents/Info.plist` | `0644` (rw-r--r--) | ~1.4 KB | Apple bundle configuration |
| `AetherIDE.app/Contents/Resources/workbench/` | `0755` / `0644` | ~11.2 MB | Bundled UI assets |

---

## 5. Cleanliness & Security Verification
- **Zero Secrets / Credentials:** Audited archives for private keys, tokens, or environment files. None found.
- **Zero In-Tree Target Outputs:** No `.pdb`, `.rlib`, or intermediate compiler files present.
- **Zero Development Caches:** No `node_modules` or `target/` directories bundled.
- **Binary Architecture Integrity:** Every binary matches its advertised target architecture (x86_64 for Windows/Linux, arm64 for macOS).
