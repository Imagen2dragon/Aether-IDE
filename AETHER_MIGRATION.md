# AetherIDE migration to Code - OSS

The desktop foundation is now the Microsoft Code - OSS source tree. The previous custom desktop implementation remains available in the repository history; this migration does not claim feature parity with it or a finished AetherIDE release.

## Provenance and history

- Source: https://github.com/microsoft/vscode.git
- Imported commit: `3c2765791fc611b8372133bc6b12d7d4a8d9ac83`.
- Migration parent: Aether-IDE main at `ca6eb19ab8ec79152239042608db34a2fb45508d`.
- The import commit has the exact upstream Git tree and the existing Aether-IDE main as its parent. Upstream history is available from the source URL rather than duplicated into this repository.
- Upstream license and notices are retained. Microsoft service access and branding are not implied by this source import.

## Reproduce the Windows x64 baseline

Install Visual Studio 2022 C++ tools, a Windows SDK, C++ Spectre libraries, and Python before installing native dependencies. Git LFS is required for upstream LFS assets. The bootstrap uses the versions in `.nvmrc` and `package.json`; it does not replace the global Node or pnpm installation.

```powershell
git lfs install
git lfs pull
./scripts/aether/dev.ps1 -Task Setup
./scripts/aether/dev.ps1 -Task Install
./scripts/aether/dev.ps1 -Task Build
./scripts/aether/dev.ps1 -Task Electron
./scripts/aether/dev.ps1 -Task Launch
```

The bootstrap verifies downloaded Node archives with official SHA-256 checksums and pnpm archives with npm SHA-512 integrity metadata. Downloads and dependency stores stay under ignored `.build/aether-tools`. Existing installations can be supplied through `-NodeDirectory` and `-PnpmDirectory`; their versions are checked before use.

For the desktop source preview when the optional runtime downloads are unavailable, use `./scripts/aether/dev.ps1 -Task Install -SkipDictationRuntime -SkipBrowserDownload`. These explicit options print their limitations and restore the calling environment afterward. Local dictation and browser-suite validation remain pending; Electron is still required for desktop validation.

## Step 1 validation

- Pristine source import and existing-main ancestry verified.
- Bootstrap PowerShell syntax parsed successfully.
- Verified Node `24.22.1` and pnpm `12.16.0` ran successfully.
- The initial frozen-lockfile dependency installation failed because node-gyp could not find a registered Windows SDK. Administrator prerequisite repair subsequently registered the SDK, and the retry successfully compiled native components including `@parcel/watcher`.
- The administrator repair installed the missing Spectre libraries. Subsequent native builds passed the previous `MSB8040` failure.
- Foundry's local dictation runtime download failed across its configured feeds. The source preview uses the SDK's documented `FOUNDRY_LOCAL_SKIP_INSTALL=1` option; local dictation is unavailable and is not an acceptance claim. To reproduce that preview, set this environment variable only for the `Install` command, then restore it. A full release needs the runtime installed and tested.
- The desktop-preview dependency install completed successfully with the two explicit runtime download skips. The full `build-fast` development build completed successfully for client, bundled extensions and Copilot. Extension media typechecks reported zero errors; this is not a full client typecheck.
- Desktop smoke checks passed: file editing, Ctrl+S persisted to disk, dirty-file close prompt, Cancel retaining changes, application close, hot-exit restoring the unsaved edit, and saving the restored text. A local extension host started successfully. A direct Electron/native-PTY smoke test verified shell spawn, output and exit; terminal UI behavior in a trusted project remains a later acceptance check.
- Fixed a reproduced startup race by registering agent management IPC before awaiting optional local endpoint setup. Full client typechecking and all 25 focused local agent-host Electron tests passed. A rebuilt desktop launch established its agent protocol connection without the prior management-channel timeout.
- Packaged application identity, installation and installed-build smoke checks follow in the next migration step. AI provider access, local dictation, browser-suite execution, debugging and mobile builds are not validated by these baseline checks.
- Both upstream LFS assets and the migration branch uploaded successfully. Review: https://github.com/Imagen2dragon/Aether-IDE/pull/29 (draft).
- The existing installed AetherIDE application has not been replaced by this migration.

## Windows desktop preview (step 2)

AetherIDE Preview `1.148.0` was packaged and installed from source commit `3a96c5b0abf2043a24bc8b1e9ee925cc2e0ad30f`. It has separate product, settings, extension, server, mutex and protocol identities. The Windows icon is copied byte-for-byte from the existing AetherIDE project; the existing SVG symbol is reused. Upstream license notices remain included.

```powershell
./scripts/aether/dev.ps1 -Task Package
./scripts/aether/install-preview.ps1
```

Packaging requires a clean, committed checkout and discovers the installed Windows SDK x64 signing tools. The display name identifies the preview while the package uses the standard OSS build path. After fixing SDK tool discovery, native packaging completed through upstream's `vscode-win32-x64-ci` task using the already validated application bundles; the intervening change only affected the PowerShell bootstrap.

The installer creates a new release directory under `$env:LOCALAPPDATA/Programs/AetherIDE Preview/releases`, verifies the executable and product metadata, then creates AetherIDE Preview shortcuts in the Desktop and Start Menu. Its `current.json` records the source commit and retained previous release. Legacy installations are preserved. The installed executable is self-contained and runs from the Programs directory.

Installed-build checks passed: launch, direct Ctrl+S persistence, the dirty-file close prompt, explicit native Save-on-close, application exit, restoration of an unsaved edit after reopening, and saving the recovered edit. Native terminal spawn, output and exit also passed under the installed Electron runtime using its packaged node-pty dependency. Dialog and recovery checks used an isolated test profile. The reproduced agent-management startup timeout did not return.

Review: https://github.com/Imagen2dragon/Aether-IDE/pull/30, stacked on the baseline PR. Release signing, automatic updates, a production installer, remaining upstream onboarding copy, trusted-project terminal UI, debugging, AI providers and mobile toolchains remain later work. Local dictation and browser-suite validation retain the source-preview limitations above.

## Following steps, one pull request each

1. Completed: source baseline, dependencies, desktop build and core native workflow checks.
2. Completed for the local Windows preview: Aether identity, original icon, separate application data, packaging, versioned installation and installed-build smoke checks.
3. Validate and document editing, Explorer, search, Git, terminal, debugging, extensions, keyboard shortcuts, save/close and recovery as release gates. Fix demonstrated regressions within the upstream architecture.
4. Reintroduce Aether AI capabilities through services and extensions, with real provider configuration, cancellation and reviewable edits.
5. Add mobile toolchain discovery and diagnostics for Android and iOS/xtool, followed by actual sample-app build and device tests. Record platform and signing requirements explicitly.
6. Add repeatable packaging, updates, automated release checks and platform-specific acceptance evidence.

Each step requires its own reviewable pull request and honest validation results. A later feature must not be marked working merely because a button or view exists.
