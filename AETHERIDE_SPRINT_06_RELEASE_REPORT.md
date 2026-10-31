# AetherIDE — Sprint 06 Release Report
## Release Candidate: v0.1.0-alpha (Sprint 06 Distribution)

---

## 1. Release Artifacts & Checksums

| Artifact | Target Platform | Format | Size | SHA-256 Checksum |
|---|---|---|---|---|
| `aether-ide-windows-x64-v0.1.0-alpha.zip` | Windows 10/11 x64 | Zip Archive | 9.47 MB | `8C80E219A574B9804A7A686F5B941051483D065EE54B912CAFA93FB2411F2D82` |
| `AetherIDE.exe` | Windows 10/11 x64 | Native Host | 1.16 MB | Included in distribution |
| `aether-cli.exe` | Windows 10/11 x64 | Command Line | 3.01 MB | Included in distribution (`bin/`) |
| `aether-daemon.exe` | Windows 10/11 x64 | Background Service | 10.95 MB | Included in distribution (`bin/`) |
| `aether-setup.iss` | Windows 10/11 x64 | Inno Setup Script | Source | `packaging/windows/aether-setup.iss` |
| `AppxManifest.xml` | Windows 10/11 x64 | MSIX Manifest | Source | `packaging/windows/AppxManifest.xml` |
| `build-appimage.sh` | Linux x64 | AppImage Script | Source | `packaging/linux/build-appimage.sh` |
| `build-deb.sh` | Linux x64 (Debian/Ubuntu) | Debian Package Script | Source | `packaging/linux/build-deb.sh` |
| `build-app.sh` | macOS Apple Silicon | .app Bundle Script | Source | `packaging/macos/build-app.sh` |
| `build-dmg.sh` | macOS Apple Silicon | DMG Image Script | Source | `packaging/macos/build-dmg.sh` |

---

## 2. Release Highlights

1. **Storage Optimization & Zero OneDrive Cache:**
   - 11.81 GB reclaimed from OneDrive.
   - Build cache offloaded to `%LOCALAPPDATA%\AetherIDE\build-cache\`.
   - Repository size reduced from 12.24 GB to 2.49 GB (58.5% margin under the 6.00 GB budget).

2. **Cross-Platform Native Subsystems:**
   - Unified `TerminalBackend` trait with Windows ConPTY and Unix/macOS POSIX PTY.
   - Unified `DesktopHost` trait with Win32 WebView2, Linux WebKitGTK, and macOS WKWebView.
   - GitHub Actions multi-platform CI matrix (`.github/workflows/ci.yml`).

3. **Yrs CRDT Real-Time Collaboration:**
   - Real-time text collaboration with Yjs V1 wire format.
   - Ephemeral multi-user cursor positioning and selection awareness.
   - Ed25519 cryptographic signatures and 5-tier Role-Based Access Control.

4. **Autonomous AI Swarms & Three-Way Merging:**
   - DAG task graph with Kahn's algorithm cycle detection.
   - Worktree directory isolation preventing parallel write contention.
   - Line-by-line three-way merge engine with visual Git conflict markers.

5. **Enterprise Packaging & Dashboard:**
   - Complete installer scripts for Inno Setup, MSIX, AppImage, Debian, and DMG.
   - In-app "Build & Storage Center" dashboard in Monaco Workbench.
