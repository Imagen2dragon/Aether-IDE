# AetherIDE Sprint 13 — CI Reliability & Determinism Report

## 1. Overview and Objectives

In Sprint 12, CI workflows successfully compiled and tested across Windows, Linux, and macOS. However, two non-deterministic patterns were identified for remediation in Sprint 13:
1. **Linux Fallback Bypass:** The Linux CI test step included an unauthoritative fallback: `dbus-run-session -- cargo test --workspace || cargo test --workspace`. If the D-Bus session failed, CI proceeded with unauthenticated headless test execution, masking Secret Service integration failures.
2. **Non-Deterministic macOS Keychain Setup:** The macOS job initialized keychains using `|| true` suppressions without verifying keychain accessibility, risking silent test masking or keychain pollution across runner sessions.

Sprint 13 remediates both issues and integrates automated artifact packaging and upload steps via `actions/upload-artifact@v4`.

---

## 2. Linux CI Hardening

### 2.1 Elimination of Fallback Bypass
The Linux test execution in `.github/workflows/ci.yml` was upgraded to require an authoritative D-Bus session:
```yaml
- name: Run Authoritative Linux Tests (D-Bus Session)
  run: |
    cd rust
    dbus-run-session -- cargo test --workspace
```
Under this configuration, any failure to initialize D-Bus or connect to the system Secret Service fails the CI job immediately. Silent fallback to non-keychain mock execution is structurally prevented.

### 2.2 System Service Dependencies
The workflow ensures all required packages are present before running tests:
```bash
sudo apt-get update
sudo apt-get install -y libgtk-3-dev libsoup2.4-dev || sudo apt-get install -y libgtk-3-dev libsoup-3.0-dev
sudo apt-get install -y libwebkit2gtk-4.1-dev || sudo apt-get install -y libwebkit2gtk-4.0-dev
sudo apt-get install -y dbus-x11 gnome-keyring xvfb
```

---

## 3. macOS CI Keychain Determinism

### 3.1 Dedicated Ephemeral Keychain
Instead of modifying the runner's shared login keychain with masked errors, Sprint 13 configures an isolated ephemeral keychain with explicit credentials:
```yaml
- name: Setup Deterministic Headless macOS Keychain
  run: |
    KEYCHAIN_PATH="${RUNNER_TEMP}/aether_ci.keychain-db"
    KEYCHAIN_PASS="ci_test_password"
    security create-keychain -p "$KEYCHAIN_PASS" "$KEYCHAIN_PATH"
    security default-keychain -s "$KEYCHAIN_PATH"
    security unlock-keychain -p "$KEYCHAIN_PASS" "$KEYCHAIN_PATH"
    security set-keychain-settings -lut 3600 "$KEYCHAIN_PATH"
    security list-keychains -d user -s "$KEYCHAIN_PATH"
    security show-keychain-info "$KEYCHAIN_PATH"
```
The step executes `security show-keychain-info` to deterministically verify that the keychain is unlocked, valid, and responsive before any Rust test binary is invoked.

### 3.2 Guaranteed Post-Execution Teardown
To prevent credential leaks or keychain lock contention across CI runner runs, a guaranteed cleanup step runs regardless of test outcome:
```yaml
- name: Cleanup macOS Keychain
  if: always()
  run: |
    KEYCHAIN_PATH="${RUNNER_TEMP}/aether_ci.keychain-db"
    if [ -f "$KEYCHAIN_PATH" ]; then
      security delete-keychain "$KEYCHAIN_PATH" || true
    fi
```

---

## 4. Release Artifact Packaging & Upload

All platform jobs now package native release binaries and workbench bundles, uploading downloadable artifacts using `actions/upload-artifact@v4`:
- `AetherIDE-v0.1.0-beta.2-windows-x64`: Portable ZIP package and SHA-256 hash.
- `AetherIDE-v0.1.0-beta.2-linux-x64`: Portable tarball and AppDir/DEB package structures.
- `AetherIDE-v0.1.0-beta.2-macos-arm64`: `.app` bundle, portable tarball, and DMG disk image.

Deprecation warnings regarding Node 20 runtimes are resolved by targeting Node 22 in all setup actions.
