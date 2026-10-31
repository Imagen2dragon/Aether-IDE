#!/usr/bin/env bash
# AetherIDE Build Orchestration Script (POSIX / Linux & macOS)
set -euo pipefail

CMD="${1:-check}"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
RUST_DIR="$PROJECT_ROOT/rust"

# 1. Resolve External Build Cache
CACHE_BASE="${XDG_CACHE_HOME:-$HOME/.cache}/aetheride/build-cache"
mkdir -p "$CACHE_BASE/aetheride/linux-x64" "$CACHE_BASE/aetheride/macos-arm64"

OS="$(uname -s)"
if [ "$OS" = "Darwin" ]; then
    DEFAULT_TARGET_DIR="$CACHE_BASE/aetheride/macos-arm64"
else
    DEFAULT_TARGET_DIR="$CACHE_BASE/aetheride/linux-x64"
fi

export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$DEFAULT_TARGET_DIR}"

if command -v sccache &> /dev/null; then
    export RUSTC_WRAPPER="sccache"
    export SCCACHE_DIR="$CACHE_BASE/sccache"
    echo "[AetherBuild] Compiler cache sccache enabled (cache: $SCCACHE_DIR)"
fi

echo "[AetherBuild] Target Directory: $CARGO_TARGET_DIR"

cd "$RUST_DIR"

case "$CMD" in
    "inspect-target")
        echo "Active CARGO_TARGET_DIR: $CARGO_TARGET_DIR"
        if [ -d "$CARGO_TARGET_DIR" ]; then
            du -sh "$CARGO_TARGET_DIR"
        fi
        ;;
    "check")
        echo "[AetherBuild] Running cargo check across workspace..."
        cargo check --workspace --all-targets
        ;;
    "dev")
        echo "[AetherBuild] Building workspace (development profile)..."
        cargo build --workspace --all-targets
        ;;
    "test")
        echo "[AetherBuild] Running test suite across workspace..."
        cargo test --workspace --all-targets
        ;;
    "release")
        echo "[AetherBuild] Building workspace (release profile)..."
        cargo build --workspace --release
        ;;
    *)
        echo "Unknown command: $CMD"
        echo "Usage: ./scripts/build.sh [check|dev|test|release|inspect-target]"
        exit 1
        ;;
esac
