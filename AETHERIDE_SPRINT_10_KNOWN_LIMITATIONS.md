# AetherIDE Sprint 10 Known Limitations & Beta Pilot Boundaries

## 1. Overview
In accordance with transparent engineering principles, this report catalogues the known technical boundaries, dependencies, and temporary constraints of AetherIDE `v0.1.0-beta.2`.

## 2. Platform & Toolchain Dependencies

### 2.1 Windows 10/11 x64
- **Microsoft WebView2 Runtime:** Required for the native Win32 desktop host. The Evergreen WebView2 Runtime is pre-installed on modern Windows 10/11 systems. If absent, the launcher automatically falls back to bundled Electron (if present) or default system browser.
- **ConPTY:** Interactive terminal emulation utilizes Windows ConPTY API (requires Windows 10 build 1809 or later). Older legacy console hosts are not supported.

### 2.2 Linux x64
- **WebKitGTK 4.0 / GTK3:** Required for the native Linux host window. Headless server environments can run `aether-daemon` and `aether-cli` independently without GTK libraries.

### 2.3 macOS ARM64
- **macOS 12+ (Monterey or later):** WKWebView host requires modern WebKit APIs.

## 3. AI Model Providers & Offline Operation
- **API Key Configuration:** Cloud models (Claude 3.5 Sonnet, GPT-4o, Gemini 1.5 Pro) require developer API keys set in system environment variables or through the Settings panel. When unconfigured, the system reports missing credentials in `aether-cli doctor`.
- **Local Fallback:** Offline local execution requires Ollama or LM Studio running locally. If neither is running, AI agent tasks will report that no local model endpoint is reachable.

## 4. Workspaces & Filesystems
- **External Junctions / Symlinks:** Directories linked outside the workspace root are intentionally omitted from search and symbol indexing as part of capability-based containment. Users requiring multi-root indexing should configure a multi-root workspace manifest.
- **Network Drives (SMB/NFS):** File modification watchers may experience latency on high-latency remote network mounts compared to local NVMe/SSD storage.
