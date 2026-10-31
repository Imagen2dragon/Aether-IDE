#!/usr/bin/env bash
# AetherIDE macOS Packaging Script
# Generates .app bundle, portable tarball, and DMG disk image for Apple Silicon (ARM64)

set -euo pipefail

VERSION="${1:-0.1.0-beta.2}"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
IDE_ROOT="$(dirname "$SCRIPT_DIR")"
OUTPUT_DIR="${OUTPUT_DIR:-$IDE_ROOT/dist}"

CACHE_BASE="${XDG_CACHE_HOME:-$HOME/.cache}/aetheride/build-cache"
TARGET_DIR="${CARGO_TARGET_DIR:-$CACHE_BASE/aetheride/macos-arm64}"
RELEASE_BIN_DIR="$TARGET_DIR/release"

echo "============================================================"
echo " Packaging AetherIDE macOS ARM64 Release ($VERSION)"
echo " Output Directory: $OUTPUT_DIR"
echo " Target Directory: $TARGET_DIR"
echo "============================================================"

mkdir -p "$OUTPUT_DIR"

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
tar -czf "$TAR_PACKAGE" -C "$OUTPUT_DIR/macos-arm64" AetherIDE.app
shasum -a 256 "$TAR_PACKAGE" | awk '{print $1 "  " $2}' > "$TAR_PACKAGE.sha256"
echo "Created macOS Tarball: $TAR_PACKAGE"

# 4. Create DMG if hdiutil is present
DMG_PACKAGE="$OUTPUT_DIR/AetherIDE-v$VERSION-macos-arm64.dmg"
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
    shasum -a 256 "$DMG_PACKAGE" > "$DMG_PACKAGE.sha256"
    echo "Created macOS DMG: $DMG_PACKAGE"
fi

echo "============================================================"
echo " macOS Packaging Complete!"
echo "============================================================"
