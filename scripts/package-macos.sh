#!/usr/bin/env bash
# AetherIDE macOS Packaging Script
# Generates .app bundle, portable tarball, and DMG disk image for Apple Silicon (ARM64)
# Explicitly tracks artifact generation outcomes and eliminates silent skips

set -euo pipefail

VERSION="${1:-0.1.0-beta.2}"
PKG_TYPE="${2:-all}" # portable, dmg, all
REQUIRE_ALL=false

for arg in "$@"; do
    if [ "$arg" = "--require-all" ]; then
        REQUIRE_ALL=true
    fi
done

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
IDE_ROOT="$(dirname "$SCRIPT_DIR")"
OUTPUT_DIR="${OUTPUT_DIR:-$IDE_ROOT/dist}"

CACHE_BASE="${XDG_CACHE_HOME:-$HOME/.cache}/aetheride/build-cache"
TARGET_DIR="${CARGO_TARGET_DIR:-$CACHE_BASE/aetheride/macos-arm64}"
RELEASE_BIN_DIR="$TARGET_DIR/release"

echo "============================================================"
echo " Packaging AetherIDE macOS ARM64 Release ($VERSION)"
echo " Package Type Scope: $PKG_TYPE"
echo " Output Directory: $OUTPUT_DIR"
echo " Target Directory: $TARGET_DIR"
echo "============================================================"

mkdir -p "$OUTPUT_DIR"

declare -a STATUS_FORMAT
declare -a STATUS_PATH
declare -a STATUS_OUTCOME
declare -a STATUS_DETAILS

record_status() {
    STATUS_FORMAT+=("$1")
    STATUS_PATH+=("$2")
    STATUS_OUTCOME+=("$3")
    STATUS_DETAILS+=("$4")
}

# 1. Verify binaries
for bin in aether-daemon aether-cli AetherIDE; do
    if [ ! -f "$RELEASE_BIN_DIR/$bin" ]; then
        echo "Binary not found: $RELEASE_BIN_DIR/$bin. Compiling..."
        (cd "$IDE_ROOT/rust" && cargo build --release --bin aether-daemon --bin aether-cli --bin AetherIDE)
        break
    fi
done

# 2. Assemble .app Bundle
APP_BUNDLE="$OUTPUT_DIR/macos-arm64/AetherIDE.app"
rm -rf "$APP_BUNDLE"
mkdir -p "$APP_BUNDLE/Contents/MacOS" "$APP_BUNDLE/Contents/Resources/workbench"

cp "$RELEASE_BIN_DIR/AetherIDE" "$APP_BUNDLE/Contents/MacOS/AetherIDE"
cp "$RELEASE_BIN_DIR/aether-daemon" "$APP_BUNDLE/Contents/MacOS/aether-daemon"
cp "$RELEASE_BIN_DIR/aether-cli" "$APP_BUNDLE/Contents/MacOS/aether-cli"
chmod +x "$APP_BUNDLE/Contents/MacOS/"*

if [ -d "$IDE_ROOT/apps/desktop/workbench" ]; then
    cp -r "$IDE_ROOT/apps/desktop/workbench/"* "$APP_BUNDLE/Contents/Resources/workbench/"
fi
if [ -d "$IDE_ROOT/apps/desktop/dist" ]; then
    cp -r "$IDE_ROOT/apps/desktop/dist" "$APP_BUNDLE/Contents/Resources/workbench/"
fi
if [ -f "$IDE_ROOT/apps/desktop/package.json" ]; then
    cp "$IDE_ROOT/apps/desktop/package.json" "$APP_BUNDLE/Contents/Resources/workbench/"
fi

cat << EOF > "$APP_BUNDLE/Contents/Info.plist"
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleName</key>
    <string>AetherIDE</string>
    <key>CFBundleDisplayName</key>
    <string>AetherIDE</string>
    <key>CFBundleIdentifier</key>
    <string>org.aetheride.desktop</string>
    <key>CFBundleVersion</key>
    <string>$VERSION</string>
    <key>CFBundleShortVersionString</key>
    <string>$VERSION</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleSignature</key>
    <string>????</string>
    <key>CFBundleExecutable</key>
    <string>AetherIDE</string>
    <key>LSMinimumSystemVersion</key>
    <string>12.0</string>
    <key>NSHighResolutionCapable</key>
    <true/>
</dict>
</plist>
EOF

# 3. Create Tarball Package
TAR_PACKAGE="$OUTPUT_DIR/AetherIDE-v$VERSION-macos-arm64.tar.gz"
if [ "$PKG_TYPE" = "portable" ] || [ "$PKG_TYPE" = "all" ]; then
    echo -e "\n[1/2] Creating macOS Tarball..."
    tar -czf "$TAR_PACKAGE" -C "$OUTPUT_DIR/macos-arm64" AetherIDE.app
    if [ -f "$TAR_PACKAGE" ]; then
        if command -v shasum &> /dev/null; then
            HASH=$(shasum -a 256 "$TAR_PACKAGE" | awk '{print $1}')
        else
            HASH=$(sha256sum "$TAR_PACKAGE" | awk '{print $1}')
        fi
        echo "$HASH  $(basename "$TAR_PACKAGE")" > "$TAR_PACKAGE.sha256"
        SIZE=$(stat -c%s "$TAR_PACKAGE" 2>/dev/null || stat -f%z "$TAR_PACKAGE")
        record_status "macOS Tarball" "$TAR_PACKAGE" "BUILT" "Size: $SIZE bytes, SHA256: $HASH"
    else
        record_status "macOS Tarball" "$TAR_PACKAGE" "FAILED" "Failed to produce tarball"
    fi
else
    record_status "macOS Tarball" "$TAR_PACKAGE" "NOT_REQUESTED" "Skipped by scope parameter"
fi

# 4. Create DMG if hdiutil is present
DMG_PACKAGE="$OUTPUT_DIR/AetherIDE-v$VERSION-macos-arm64.dmg"
if [ "$PKG_TYPE" = "dmg" ] || [ "$PKG_TYPE" = "all" ]; then
    echo -e "\n[2/2] Checking DMG Creation Tools..."
    if command -v hdiutil &> /dev/null; then
        echo "Creating macOS DMG with hdiutil..."
        DMG_STAGE="$OUTPUT_DIR/macos-arm64/dmg-stage"
        rm -rf "$DMG_STAGE"
        mkdir -p "$DMG_STAGE"
        cp -r "$APP_BUNDLE" "$DMG_STAGE/"
        ln -s /Applications "$DMG_STAGE/Applications"
        
        rm -f "$DMG_PACKAGE"
        hdiutil create -volname "AetherIDE" -srcfolder "$DMG_STAGE" -ov -format UDZO "$DMG_PACKAGE"
        rm -rf "$DMG_STAGE"
        if [ -f "$DMG_PACKAGE" ]; then
            HASH=$(shasum -a 256 "$DMG_PACKAGE" | awk '{print $1}')
            echo "$HASH  $(basename "$DMG_PACKAGE")" > "$DMG_PACKAGE.sha256"
            SIZE=$(stat -c%s "$DMG_PACKAGE" 2>/dev/null || stat -f%z "$DMG_PACKAGE")
            record_status "Apple DMG" "$DMG_PACKAGE" "BUILT" "Size: $SIZE bytes, SHA256: $HASH"
        else
            record_status "Apple DMG" "$DMG_PACKAGE" "FAILED" "hdiutil completed without output file"
        fi
    else
        MSG="hdiutil not available in non-macOS environment. Staged .app bundle preserved at $APP_BUNDLE"
        if [ "$REQUIRE_ALL" = true ] || [ "$PKG_TYPE" = "dmg" ]; then
            record_status "Apple DMG" "$DMG_PACKAGE" "BLOCKED" "$MSG"
            if [ "$REQUIRE_ALL" = true ]; then
                echo "ERROR: $MSG (--require-all specified)"
                exit 1
            fi
        else
            record_status "Apple DMG" "$DMG_PACKAGE" "BLOCKED" "$MSG"
        fi
    fi
else
    record_status "Apple DMG" "$DMG_PACKAGE" "NOT_REQUESTED" "Skipped by scope parameter"
fi

# 5. Summary Matrix Table
echo "============================================================"
echo " macOS Release Packaging Status Summary"
echo "============================================================"
printf "%-18s | %-12s | %s\n" "Format" "Outcome" "Details"
echo "------------------------------------------------------------"
for i in "${!STATUS_FORMAT[@]}"; do
    printf "%-18s | %-12s | %s\n" "${STATUS_FORMAT[$i]}" "${STATUS_OUTCOME[$i]}" "${STATUS_DETAILS[$i]}"
done
echo "============================================================"
