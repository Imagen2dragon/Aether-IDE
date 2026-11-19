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
./scripts/code.bat --user-data-dir .build/aether-user-data --extensions-dir .build/aether-extensions
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
- Build, launch, file save, dirty-file close, terminal, extension-host and recovery checks remain pending. This baseline must stay a draft until those checks pass.
- Both upstream LFS assets and the migration branch uploaded successfully. Review: https://github.com/Imagen2dragon/Aether-IDE/pull/29 (draft).
- The existing installed AetherIDE application has not been replaced by this migration.

## Following steps, one pull request each

1. Finish this baseline: dependency installation, desktop build, isolated launch and native workflow checks.
2. Add AetherIDE product identity, original logo, application IDs and isolated settings/extensions storage; package and test an installable desktop app.
3. Validate and document editing, Explorer, search, Git, terminal, debugging, extensions, keyboard shortcuts, save/close and recovery as release gates. Fix demonstrated regressions within the upstream architecture.
4. Reintroduce Aether AI capabilities through services and extensions, with real provider configuration, cancellation and reviewable edits.
5. Add mobile toolchain discovery and diagnostics for Android and iOS/xtool, followed by actual sample-app build and device tests. Record platform and signing requirements explicitly.
6. Add repeatable packaging, updates, automated release checks and platform-specific acceptance evidence.

Each step requires its own reviewable pull request and honest validation results. A later feature must not be marked working merely because a button or view exists.
