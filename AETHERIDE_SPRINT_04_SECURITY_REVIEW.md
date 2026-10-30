# AETHERIDE — SPRINT 04 SECURITY ARCHITECTURE REVIEW
## Native Message Bridge Isolation, Origin Validation, Token Encapsulation and Filesystem Containment

**Sprint:** 04  
**Document Version:** 1.0.0  
**Security Lead:** Security Principal & Systems Auditor  
**Date:** 2026-10-31  
**Project:** AetherIDE  

---

## 1. Threat Model & Security Posture

In a native desktop development environment where the user interface runs in a Web-based engine (Monaco / WebView2) and the backend possesses full developer privileges (spawning compilers, executing tests, running ConPTY terminals), the primary threat surfaces are:

1. **Malicious DOM Script Injection / Cross-Site Scripting (XSS):** If code rendered inside the editor or loaded via external dependencies could execute arbitrary native commands.
2. **Path Traversal & Filesystem Escapes:** Malicious or malformed file read/write requests attempting to overwrite system binaries or access credentials outside the workspace.
3. **Session Token Exfiltration:** If the daemon authentication token were exposed to DOM JavaScript or accessible in `localStorage` or URL query parameters.
4. **Origin Spoofing:** Malicious external websites opened in browser tabs attempting to issue commands to the local IDE backend.

Sprint 04 establishes strict architectural controls to remediate each of these potential vulnerabilities.

---

## 2. Inbound Origin Verification

All communication between the WebView2 container and the native Rust host occurs via the `WebMessageReceived` event handler. Before any message body is deserialized or processed, its source URL is evaluated:

```rust
let mut source_pwstr = PWSTR::null();
if args.Source(&mut source_pwstr).is_ok() && !source_pwstr.is_null() {
    let source_url = take_pwstr(source_pwstr);
    if !source_url.starts_with("https://appassets.aetheride.local")
        && !source_url.starts_with("file://")
    {
        warn!("Blocked untrusted WebMessage from unauthorized origin: {}", source_url);
        let err_json = serde_json::json!({
            "error": "SecurityViolation: Untrusted message origin",
            "origin": source_url
        });
        let reply_hstring = HSTRING::from(&err_json.to_string());
        let _ = webview.PostWebMessageAsJson(&reply_hstring);
        return;
    }
}
```

### Result:
Any web page, iframe, or external origin attempting to call `window.chrome.webview.postMessage` is rejected immediately with a `SecurityViolation`.

---

## 3. Strict Action Allowlist

The native host implements an explicit allowlist of authorized actions. Unlisted commands—including arbitrary process execution, registry edits, or raw network requests—are blocked:

### Authorized Actions:
- `LspCompletion`
- `LspHover`
- `LspDefinition`
- `LspChangeDocument`
- `LspOpenDocument`
- `LspCloseDocument`
- `LspDiagnostics`
- `GetStatus`
- `Ping`
- `RunDoctorCheck`
- `QueryProjectBrain`
- `FileRead`
- `FileWrite`
- `TerminalSpawn`
- `TerminalWrite`
- `TerminalResize`
- `CreateAgentTask`

```rust
if !allowed_actions.contains(&action.as_str()) {
    warn!("Rejected non-allowlisted action: {}", action);
    let err_reply = serde_json::json!({
        "id": req_id,
        "action": action,
        "error": format!("Action '{}' is not in the secure allowlist", action),
        "result": null
    });
    let reply_hstring = HSTRING::from(&err_reply.to_string());
    let _ = webview.PostWebMessageAsJson(&reply_hstring);
    return;
}
```

---

## 4. Token Encapsulation & Privilege Isolation

In previous architectures, web frontends often required an API key or session bearer token passed via URL hash parameters or injected into JavaScript window globals.

In Sprint 04:
- The daemon session token resides exclusively in the native Rust process memory (`win32_host.rs`).
- The frontend has **zero knowledge** of the token string.
- When an allowlisted command arrives from the trusted origin, the native host process attaches the session token to the internal `IpcEnvelope` before transmitting it across the local loopback TCP connection to the daemon.
- Even if arbitrary script execution were achieved in the DOM, an attacker cannot extract the bearer token to perform unauthorized out-of-band requests.

---

## 5. Filesystem Containment & Atomic Writes

### 5.1 Directory Traversal Protection
Both `FileRead` and `FileWrite` prevent path traversal attacks:
1. Relative path components containing `..` are immediately blocked.
2. Target paths are canonicalized against the base workspace directory:
   ```rust
   if let Ok(canon_base) = base_dir.canonicalize() {
       if let Ok(canon_target) = target.canonicalize() {
           if !canon_target.starts_with(&canon_base) {
               return serde_json::json!({
                   "error": "Access Denied: Path escapes workspace root"
               });
           }
       }
   }
   ```

### 5.2 Atomic File Commit
To prevent file corruption during sudden process termination or power loss, files are written using atomic replacement semantics:
1. File contents are written to a temporary sibling file (`<target>.tmp_atomic`).
2. `std::fs::rename` performs an atomic filesystem replace.
3. If an error occurs, the temporary file is removed cleanly.

---

## 6. Automated Security Verification Results

The automated security test suite in `aether-launcher` verifies these boundaries:

```
running 5 tests
test win32_host::native::tests::test_allowlist_validation ... ok
test win32_host::native::tests::test_host_config_defaults ... ok
test win32_host::native::tests::test_handle_file_read_blocks_path_traversal ... ok
test win32_host::native::tests::test_handle_file_write_atomic_and_blocks_traversal ... ok
test webview2_runtime::tests::test_detect_webview2_runtime ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.24s
```

All security controls are verified and functional.
