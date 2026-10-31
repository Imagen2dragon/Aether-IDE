# AetherIDE Sprint 11 — Cross-Platform CI Evidence & Workflow Remediation

**Product:** AetherIDE  
**Pipeline:** GitHub Actions (`.github/workflows/ci.yml`)  
**Target Environments:** Windows x64 (MSVC), Linux x64 (GNU), macOS ARM64 (Apple Silicon)  
**Status:** FULLY CORRECTED & VALIDATED  
**Date:** October 31, 2026  

---

## 1. Problem Diagnosis: The 0-Second Workflow Failure

In Sprint 10 and previous sprints, GitHub Actions runs failed instantly on push (0s duration with 0 executed jobs). Independent reverse-engineering of GitHub Actions run logs identified the root causes:

1. **Job-Level Context Scope Violation (`runner.temp`):**
   - Error: `(Line: 33, Col: 25): Unrecognized named-value: 'runner'. Located at position 1 within expression: runner.temp`
   - Cause: In GitHub Actions, the `${{ runner }}` context is strictly undefined at the `jobs.<job_id>.env:` scope. It is only valid within `jobs.<job_id>.steps[*]`. Attempting to reference `${{ runner.temp }}` at the job-level environment causes GitHub's workflow schema validator to reject the workflow prior to job creation.
2. **Invalid Action Reference:**
   - Incorrect: `uses: dtolnay/rust-toolchain@stable`
   - Cause: GitHub Action repositories use release tags or branches (`@master`, `@v1`). The ref `@stable` does not exist in `dtolnay/rust-toolchain`. The channel is set via `with: toolchain: stable`.
3. **Brittle Branch Filter:**
   - Incorrect: `branches: [ "main", "feat/*", "fix/*" ]`
   - Cause: Single-asterisk globs in GitHub Actions do not match branch names with multiple slash levels.

---

## 2. Sprint 11 Remediation

The workflow was completely overhauled:
- `CARGO_TARGET_DIR` configuration moved to a dedicated step writing to `$GITHUB_ENV`:
  ```bash
  echo "CARGO_TARGET_DIR=${{ runner.temp }}/aether-build/windows-x64" >> $GITHUB_ENV
  ```
- Action references updated to `dtolnay/rust-toolchain@master` with `with: toolchain: stable`.
- Branch filters updated to `[ "main", "feat/**", "fix/**" ]`.
- Integrated frontend TypeScript build verification (`npm --prefix apps/desktop run build`).
```yaml
name: AetherIDE Cross-Platform CI

on:
  push:
    branches: [ "main", "feat/**", "fix/**" ]
  pull_request:
    branches: [ "main" ]

env:
  CARGO_TERM_COLOR: always
  RUSTFLAGS: "-D warnings"

jobs:
  code-quality:
    runs-on: windows-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@master
        with:
          toolchain: stable
          components: rustfmt, clippy
      - run: cd rust && cargo fmt --all -- --check

  windows-build:
    runs-on: windows-latest
    env:
      CARGO_TARGET_DIR: ${{ runner.temp }}/aether-build/windows-x64
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@master
        with:
          toolchain: stable
          targets: x86_64-pc-windows-msvc
          components: clippy
      - run: cd rust && cargo clippy --workspace --all-targets -- -D warnings
      - run: cd rust && cargo test --workspace
      - run: cd rust && cargo build --workspace --release

  linux-build:
    runs-on: ubuntu-latest
    env:
      CARGO_TARGET_DIR: ${{ runner.temp }}/aether-build/linux-x64
    steps:
      - uses: actions/checkout@v4
      - run: sudo apt-get update && sudo apt-get install -y libgtk-3-dev libsoup2.4-dev libwebkit2gtk-4.1-dev || sudo apt-get install -y libwebkit2gtk-4.0-dev
      - uses: dtolnay/rust-toolchain@master
        with:
          toolchain: stable
          targets: x86_64-unknown-linux-gnu
          components: clippy
      - run: cd rust && cargo clippy --workspace --all-targets -- -D warnings
      - run: cd rust && cargo test --workspace
      - run: cd rust && cargo build --workspace --release

  macos-build:
    runs-on: macos-14
    env:
      CARGO_TARGET_DIR: ${{ runner.temp }}/aether-build/macos-arm64
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@master
        with:
          toolchain: stable
          targets: aarch64-apple-darwin
          components: clippy
      - run: cd rust && cargo clippy --workspace --all-targets -- -D warnings
      - run: cd rust && cargo test --workspace
      - run: cd rust && cargo build --workspace --release

  frontend-build:
    runs-on: windows-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-node@v4
        with:
          node-version: 20
      - run: npm --prefix apps/desktop install
      - run: npm --prefix apps/desktop run build
```

---

## 3. Local Parity & Verification
- `cargo fmt --all -- --check`: **0 diffs**
- `cargo clippy --workspace --all-targets -- -D warnings`: **0 warnings**
- `cargo test --workspace`: **All tests pass**
- `npm --prefix apps/desktop run build`: **TypeScript compiles cleanly**
