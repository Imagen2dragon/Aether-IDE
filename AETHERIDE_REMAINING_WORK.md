# AETHERIDE — PRIORITIZED REMEDIATION PLAN & ENGINEERING BACKLOG

**Audit Date:** 2026-10-30  
**Status:** Post-Audit Actionable Technical Roadmap  

---

## PRIORITY P0: ESSENTIAL DEFECTS & SECURITY FIXES

### P0-1: Remediate Unauthenticated Remote Command Execution on TCP Port 44888
- **Description:** Any local process or web page accessing `127.0.0.1:44888` can execute arbitrary shell commands via the daemon without authentication.
- **Existing Implementation:** [`rust/services/aether-daemon/src/main.rs:147`](file:///c:/Users/abdal/OneDrive/Desktop/ide/rust/services/aether-daemon/src/main.rs)
- **Evidence:** `handle_ipc_envelope()` matches `IpcRequest::RunCommand` without checking `envelope.session_token`. `aether-cli/src/main.rs:109` sends `session_token: None` and succeeds.
- **Required Correction:**
  1. Reject any request where `envelope.session_token != state.session_token` with an HTTP/JSON-RPC 401 Unauthorized envelope.
  2. Update `aether-cli` and `DaemonClient.ts` to read the session token from `%LOCALAPPDATA%\AetherIDE\daemon_session.token` and include it in all headers.
- **Complexity:** Low (1 engineering day).
- **Dependencies:** None.
- **Acceptance Criteria:** A TCP request without the valid session token receives `success: false, error: "Unauthorized"` and cannot execute `RunCommand`.

---

### P0-2: Package Standalone Code - OSS / Electron Desktop Shell
- **Description:** The current distribution contains no desktop IDE executable; `AetherIDE.cmd` invokes Microsoft VS Code (`code`) on the user's `PATH`.
- **Existing Implementation:** [`dist/windows-x64/AetherIDE.cmd`](file:///c:/Users/abdal/OneDrive/Desktop/ide/dist/windows-x64/AetherIDE.cmd)
- **Evidence:** Running `AetherIDE.cmd` on a machine without VS Code installed fails. Package size is only 9.85 MB.
- **Required Correction:**
  1. Build and bundle the open-source Microsoft Code - OSS workbench or Electron shell in `apps/desktop/`.
  2. Package Electron binaries, Chromium runtime, and V8 snapshot directly into `dist/windows-x64/bin/code-oss/`.
  3. Update `AetherIDE.cmd` to launch the bundled executable.
- **Complexity:** High (5–7 engineering days).
- **Dependencies:** Code - OSS build pipeline, Node.js, Electron packaging tools.
- **Acceptance Criteria:** Double-clicking `AetherIDE.cmd` launches a full editor workbench on a clean Windows virtual machine with zero pre-existing software installed.

---

### P0-3: Implement Real HTTP Providers for OpenAI, Gemini, and DeepSeek
- **Description:** The Universal Model Gateway returns hardcoded mock strings for OpenAI, Gemini, and DeepSeek rather than issuing HTTP requests.
- **Existing Implementation:** [`rust/crates/model-gateway/src/lib.rs:343, 401, 461`](file:///c:/Users/abdal/OneDrive/Desktop/ide/rust/crates/model-gateway/src/lib.rs)
- **Evidence:** `OpenAiCompatibleProvider::complete`, `GoogleGeminiProvider::complete`, and `DeepSeekProvider::complete` do not make network calls.
- **Required Correction:**
  1. Implement genuine `reqwest` POST requests to `https://api.openai.com/v1/chat/completions`, `https://generativelanguage.googleapis.com/v1beta/models/...:generateContent`, and `https://api.deepseek.com/chat/completions`.
  2. Check respective environment variables (`OPENAI_API_KEY`, `GEMINI_API_KEY`, `DEEPSEEK_API_KEY`).
  3. Parse structured JSON responses and calculate prompt/completion tokens.
- **Complexity:** Medium (2 engineering days).
- **Dependencies:** `reqwest`, `serde_json`.
- **Acceptance Criteria:** Providing a valid `OPENAI_API_KEY` results in an actual API response from GPT-4o.

---

## PRIORITY P1: IMPORTANT MISSING FUNCTIONALITY FOR USABLE RELEASE

### P1-1: Dynamic Agent Tool Formulation & Real File Modification Loop
- **Description:** The agent runtime hardcodes a single step executing `cargo --version` and produces no diffs.
- **Existing Implementation:** [`rust/crates/agent-runtime/src/lib.rs:115–140`](file:///c:/Users/abdal/OneDrive/Desktop/ide/rust/crates/agent-runtime/src/lib.rs)
- **Evidence:** Executing an agent task via `aether-cli agent` produces hardcoded `output_json: "cargo 1.93.1 ..."` with `diff: null`.
- **Required Correction:**
  1. Implement an LLM tool-calling loop supporting tools: `read_file`, `write_file`, `execute_command`, `git_diff`.
  2. Spawn an isolated Git worktree (`git worktree add -b aether-agent/<id>`) per task.
  3. Apply file edits in the worktree, run tests, and compute unified diffs for user approval.
- **Complexity:** High (4–5 engineering days).
- **Dependencies:** `git-engine`, `model-gateway`, `process-manager`.
- **Acceptance Criteria:** An agent tasked with fixing a bug creates a Git worktree, edits the source file, executes the test suite, and presents a non-null diff for approval.

---

### P1-2: Replace Regex Lexer with Tree-sitter Incremental Parser
- **Description:** Symbol extraction in `syntax-engine` uses basic regexes (`RUST_FN_RE`, `TS_FN_RE`) rather than a real AST parser.
- **Existing Implementation:** [`rust/crates/syntax-engine/src/lib.rs:64`](file:///c:/Users/abdal/OneDrive/Desktop/ide/rust/crates/syntax-engine/src/lib.rs)
- **Evidence:** Complex syntax (nested structs, macros, generics, multiline signatures) fails to parse or produces incorrect spans.
- **Required Correction:**
  1. Integrate `tree-sitter` and grammar crates (`tree-sitter-rust`, `tree-sitter-typescript`, `tree-sitter-python`).
  2. Extract symbol nodes by querying AST syntax kinds (`function_item`, `struct_item`, `class_declaration`).
- **Complexity:** Medium (3 engineering days).
- **Dependencies:** `tree-sitter`.
- **Acceptance Criteria:** Multiline functions and complex Rust trait implementations are extracted with accurate start and end line ranges.

---

### P1-3: Windows Job Object Process Sandboxing in `process-manager`
- **Description:** Child processes are spawned without Windows Job Object limits or cleanup guarantees.
- **Existing Implementation:** [`rust/crates/process-manager/src/lib.rs:26`](file:///c:/Users/abdal/OneDrive/Desktop/ide/rust/crates/process-manager/src/lib.rs)
- **Evidence:** Commands run directly via `tokio::process::Command::new("cmd.exe")`.
- **Required Correction:**
  1. Implement Windows API bindings (`CreateJobObjectW`, `SetInformationJobObject`, `AssignProcessToJobObject`).
  2. Enable `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` to prevent orphaned background processes.
  3. Enforce configurable memory ceilings.
- **Complexity:** Medium (2–3 engineering days).
- **Dependencies:** `windows-sys` crate.
- **Acceptance Criteria:** Killing a parent build command terminates all spawned compiler sub-processes cleanly.

---

### P1-4: Proper EAS CLI Cloud Build Integration & Error Handling
- **Description:** When the `eas` CLI is missing or unauthenticated, the build dispatcher fakes a `"queued"` status with a synthetic artifact URL.
- **Existing Implementation:** [`rust/crates/mobile-build-manager/src/lib.rs:186–215`](file:///c:/Users/abdal/OneDrive/Desktop/ide/rust/crates/mobile-build-manager/src/lib.rs)
- **Evidence:** `trigger_eas_build` returns synthetic URL `https://expo.dev/artifacts/{build_id}` on failure.
- **Required Correction:**
  1. Verify `eas --version` and check `eas whoami` before triggering builds.
  2. If unauthenticated, return `status: "blocked"` with an explicit error explaining that EAS CLI login is required.
- **Complexity:** Low (1 engineering day).
- **Dependencies:** `process-manager`.
- **Acceptance Criteria:** Unauthenticated build requests report `status: "failed"` with clear remediation instructions rather than fabricated artifact links.

---

### P1-5: iOS Companion Xcode Project & UniFFI Integration
- **Description:** The iOS companion lacks an Xcode project and uses mock Swift string operations instead of UniFFI Rust bindings.
- **Existing Implementation:** [`apps/ios-companion/RustBridge/AetherBridge.swift`](file:///c:/Users/abdal/OneDrive/Desktop/ide/apps/ios-companion/RustBridge/AetherBridge.swift)
- **Evidence:** No `.xcodeproj` or `Package.swift` exists. `AetherBridge.swift` does not import or link a native C/Rust library.
- **Required Correction:**
  1. Generate `AetherCompanion.xcodeproj` via Xcode or Tuist.
  2. Define UniFFI interface (`aether.udl`) in `application-core` and generate Swift bindings.
  3. Create macOS CI build workflow (`.github/workflows/ios-build.yml`).
- **Complexity:** High (4–5 engineering days).
- **Dependencies:** macOS runner, Xcode 15+, UniFFI CLI.
- **Acceptance Criteria:** `xcodebuild -scheme AetherCompanion` compiles cleanly on macOS and invokes native Rust functions via UniFFI.

---

### P1-6: Resolve Clippy and Rustfmt Warnings
- **Description:** `cargo fmt --check` and `cargo clippy -- -D warnings` fail.
- **Existing Implementation:** `rust/crates/filesystem-engine/src/lib.rs`, `rust/crates/database-engine/src/lib.rs`
- **Evidence:** 4 clippy errors (redundant closures, manual flatten) and line-formatting diffs.
- **Required Correction:** Run `cargo fmt` and refactor the 4 clippy warnings to use `.flatten()` and direct variant constructors.
- **Complexity:** Low (1–2 hours).
- **Dependencies:** None.
- **Acceptance Criteria:** `cargo fmt --check` and `cargo clippy -- -D warnings` both exit with code 0.

---

## PRIORITY P2: ADVANCED CAPABILITIES & FUTURE OPTIMIZATIONS

### P2-1: Windows Standalone Installers (EXE & MSI)
- **Description:** Build automated Inno Setup / NSIS `.exe` installer and WiX `.msi` package.
- **Complexity:** Medium (2–3 engineering days).
- **Acceptance Criteria:** Installer creates desktop icon, Start Menu entry, registers file associations, and provides clean uninstaller in Windows Settings.

### P2-2: Full SSE Token Streaming in Model Gateway
- **Description:** Stream tokens character-by-character to the UI via Server-Sent Events rather than blocking until full completion.
- **Complexity:** Medium (2 engineering days).
- **Acceptance Criteria:** AI response tokens appear progressively in the Webview chat panel with `< 50ms` time-to-first-token.

### P2-3: Advanced AST Blast Radius Graph
- **Description:** Upgrade Change Impact Radar from basic file imports to fine-grained function call graph traversal.
- **Complexity:** High (4 engineering days).
- **Acceptance Criteria:** Modifying a function flags only the specific downstream callers and tests invoking that function.
