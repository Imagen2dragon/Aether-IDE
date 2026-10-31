# AetherIDE Sprint 16 — Known Limitations & Operational Constraints

**Product:** AetherIDE  
**Release Target:** `v0.1.0-beta.2`  
**Classification:** Beta Pilot Disclosure  
**Baseline Commit:** `a2e40deb0e7fd3bbfba742ad005269ff05b4d22e`  

---

## 1. Executive Summary

This document enumerates the known limitations, packaging constraints, and architectural boundaries in **AetherIDE `v0.1.0-beta.2`**. Transparent disclosure of these constraints ensures that pilot developers, enterprise evaluators, and engineers have clear expectations regarding system prerequisites, platform nuances, and near-term roadmap milestones.

---

## 2. Packaging & Distribution Constraints

### 2.1 Portable Archive Delivery (No System Installers)
- **Current State:** Release artifacts are packaged exclusively as portable compressed archives (`.zip` on Windows, `.tar.gz` on Linux and macOS).
- **Limitation:** There are no native OS installer packages (`.msi` / InnoSetup for Windows, `.deb` / `.rpm` for Linux, or `.dmg` for macOS).
- **Operational Impact:** Application shortcuts must be manually created by the developer. Automatic background auto-updating is not supported in Beta.2; upgrades require extracting a new portable archive.

### 2.2 macOS Gatekeeper & Unnotarized Developer ID
- **Current State:** macOS binaries are compiled for Apple Silicon (`aarch64-apple-darwin`), but the archive is not signed with a paid Apple Developer ID certificate or notarized via Apple Notary Service.
- **Operational Impact:** On macOS Monterey, Ventura, Sonoma, and Sequoia, macOS Gatekeeper blocks execution on first launch with a warning (*"macOS cannot verify the developer"*).
- **Remediation:** Developers must execute `xattr -cr AetherIDE.app` in Terminal or use Finder Right-Click > **Open** to grant user execution consent.

### 2.3 Linux Shared Library & glibc Prerequisites
- **Current State:** The Linux package is compiled on GitHub Actions `ubuntu-22.04` runners against `glibc 2.35`.
- **Limitation:** Does not run on legacy Linux distributions with older C libraries (e.g., CentOS 7, RHEL 8, Debian 10).
- **Runtime Dependencies:** Requires `libgtk-3-0`, `libwebkit2gtk-4.1-0` (or `4.0`), and `libsecret-1-0` installed on the host.

### 2.4 Windows WebView2 Evergreen Requirement
- **Current State:** The Windows desktop host depends on the Microsoft Edge WebView2 Evergreen runtime.
- **Limitation:** While pre-installed by default on Windows 10 (version 1903+) and Windows 11, bare Windows Server installations or enterprise systems with locked-down group policies may require running Microsoft's standalone WebView2 installer.

---

## 3. Subsystem & Feature Boundaries

### 3.1 Language Server Protocol (LSP) Breadth
- **Supported Out-of-the-Box:** Rust (`rust-analyzer`) and TypeScript/JavaScript (`typescript-language-server`).
- **Limitation:** Full semantic code navigation, go-to-definition, and real-time hover diagnostics for Python (`pyright`), Go (`gopls`), and C/C++ (`clangd`) require developers to manually specify the binary path in `Settings > Languages`.
- **Roadmap:** Automated LSP toolchain discovery and one-click downloader scheduled for Beta.3.

### 3.2 Workspace Scope (Local Only)
- **Current State:** All workspaces must be located on local or mapped network filesystems accessible via standard OS directory paths.
- **Limitation:** Remote development over SSH, inside Docker containers (Dev Containers), or WSL bridge mounts is not natively supported in the Beta.2 UI.
- **Workaround:** For WSL2, developers can run the Linux native binary directly within the WSL2 environment using WSLg display forwarding.

### 3.3 Multi-Window & Multi-Monitor Support
- **Current State:** Each AetherIDE process instance manages a single primary desktop workbench window with multiple tabs and split editors.
- **Limitation:** Floating secondary windows or dragging editor tabs out to separate monitors is currently disabled in the host window manager.

### 3.4 Extensions & Marketplace
- **Current State:** The extension registry is currently limited to core first-party plugins and AI persona configurations bundled within the application.
- **Limitation:** Third-party VS Code `.vsix` extension compatibility is not available in Beta.2.

---

## 4. Workarounds & Mitigation Reference Table

| Limitation | Impacted Platform | Recommended Workaround | Scheduled Target |
| :--- | :--- | :--- | :---: |
| macOS Gatekeeper Dialog | macOS ARM64 | Run `xattr -cr AetherIDE.app` after extracting | v0.2.0 (Apple Notarized) |
| Linux glibc 2.35+ Minimum | Older Linux Distros | Use Docker or upgrade to Ubuntu 22.04+ / Fedora 38+ | v0.2.0 (AppImage / Flatpak) |
| Missing WebView2 on Win Server | Windows Server | Run Microsoft Evergreen WebView2 Bootstrapper | v0.2.0 (MSI Installer bundle) |
| Additional LSP Configuration | Python / Go / C++ | Configure path manually in `.aether/config.json` | v0.1.0-beta.3 |
| Remote Development / SSH | All Platforms | Run locally or use WSLg for Linux workloads | Post-Beta Roadmap |

---

## 5. Summary

None of the above limitations compromise core application stability, memory efficiency, capability-based filesystem security, or zero-telemetry guarantees. They are standard operational boundaries for a Beta.2 engineering milestone and are fully documented for pilot onboarding.
