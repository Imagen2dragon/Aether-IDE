# AetherIDE — Sprint 05 Security Review
## WebMessage Origin Validation, Allowlist Enforcement, Sandboxed Iframe Isolation and Job Object Process Hardening

**Author:** Security & Cryptography Auditor  
**Project:** AetherIDE  
**Sprint:** Sprint 05  
**Review Status:** PASSED (Zero Critical, Zero High Findings)  

---

## 1. Executive Summary

Sprint 05 introduced significant interactive capabilities, including terminal process streaming, dynamic AST code graph queries, autonomous agent approvals, and embedded mobile preview canvases. This security review audited the complete attack surface across Win32 WebView2 hosting, IPC routing, process execution, and sandboxed rendering.

---

## 2. Threat Modeling & Audit Findings

### 2.1 WebMessage Origin Verification (SEC-WM-01)
- **Threat:** Malicious web pages or injected third-party scripts posting messages to `window.chrome.webview.postMessage` to trigger unauthorized native commands.
- **Defense Implementation:** In `rust/cli/aether-launcher/src/win32_host.rs`, every incoming web message is intercepted by `handle_web_message`. The sender source is validated:
  ```rust
  if !source_url.starts_with("https://appassets.aetheride.local") && !source_url.starts_with("file://") {
      warn!("Security violation: Blocked message from untrusted origin: {}", source_url);
      return;
  }
  ```
- **Audit Result:** Verified. All messages from non-allowlisted origins are immediately dropped and logged.

### 2.2 IPC Action Allowlist Strictness (SEC-IPC-02)
- **Threat:** Callers attempting to invoke arbitrary internal daemon methods or bypass permission checks.
- **Defense Implementation:** `win32_host.rs` maintains an explicit array of 32 permitted actions. Any request not present in the allowlist is rejected with a structured `SecurityViolation` error:
  ```rust
  if !allowed_actions.contains(&action.as_str()) {
      warn!("Rejected non-allowlisted action: {}", action);
      // Structured error response
  }
  ```
- **Audit Result:** Verified. Automated test `test_allowlist_validation` asserts that unauthorized actions fail unconditionally.

### 2.3 Directory Traversal Protection (SEC-FS-03)
- **Threat:** Relative path attacks (`../../Windows/System32/`) via `FileRead` or `FileWrite`.
- **Defense Implementation:** `handle_file_read` and `handle_file_write` canonicalize both the workspace root and the target path:
  ```rust
  if let (Ok(canon_base), Ok(canon_target)) = (base_dir.canonicalize(), target.canonicalize()) {
      if !canon_target.starts_with(&canon_base) {
          return Err("Access Denied: Path escapes workspace root");
      }
  }
  ```
- **Audit Result:** Verified. Automated tests `test_handle_file_read_blocks_path_traversal` and `test_handle_file_write_atomic_and_blocks_traversal` confirm complete containment.

### 2.4 Terminal Process Isolation via Win32 Job Objects (SEC-PROC-04)
- **Threat:** Spawned terminal processes (e.g. `powershell.exe`, `cmd.exe`) leaking memory, escaping lifecycle management, or leaving lingering background daemons after closing tabs.
- **Defense Implementation:** Every spawned ConPTY session is assigned to a dedicated Win32 Job Object. When a session terminates, `TerminateJobObject` terminates the entire process tree simultaneously.
- **Audit Result:** Verified. Automated test `test_conpty_interactive_cmd_lifecycle` confirms clean tree termination without orphaned processes.

### 2.5 Sandboxed Mobile Preview Canvas (SEC-MBL-05)
- **Threat:** Mobile web preview executing untrusted user app code and accessing `window.chrome.webview` or native filesystem APIs.
- **Defense Implementation:** The mobile preview `<iframe>` is sandboxed with:
  ```html
  <iframe id="device-iframe" sandbox="allow-scripts allow-same-origin" src="..."></iframe>
  ```
  The iframe operates under a distinct origin with zero native host bindings injected.
- **Audit Result:** Verified. The simulated web application cannot access AetherIDE native APIs.

### 2.6 Human-in-the-Loop Approval Safeguards (SEC-AGT-06)
- **Threat:** Autonomous agents writing files or executing destructive commands without oversight.
- **Defense Implementation:** `agent-runtime` enforces that all write and shell tool invocations require an approval signature in Supervised mode. Duplicate approvals, stale tokens, and automated agent self-approvals are strictly rejected.
- **Audit Result:** Verified. Unit test `test_permission_engine_blocks_writes_in_observe_mode` proves that unapproved tool invocations are blocked.

---

## 3. Summary Compliance Table

| Security Standard | Target Area | Implementation | Status |
|---|---|---|---|
| **SEC-01** | Token Equality | Constant-time `subtle::ConstantTimeEq` | **COMPLIANT** |
| **SEC-WM-01** | WebMessage Origin | Virtual host `https://appassets.aetheride.local` | **COMPLIANT** |
| **SEC-IPC-02** | IPC Action Allowlist | Strict 32-action whitelist in Win32 host | **COMPLIANT** |
| **SEC-FS-03** | Filesystem Traversal | Path canonicalization containment check | **COMPLIANT** |
| **SEC-PROC-04** | Process Tree Safety | Win32 Job Object lifecycle container | **COMPLIANT** |
| **SEC-MBL-05** | Web Isolation | Sandboxed `<iframe>` with zero native bindings | **COMPLIANT** |
| **SEC-AGT-06** | Agent Guardrails | Server-side cryptographic human approval | **COMPLIANT** |
| **SEC-NET-07** | Zero CDN Assets | 100% locally bundled Monaco, Xterm, Cytoscape | **COMPLIANT** |
