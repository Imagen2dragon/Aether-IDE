#!/usr/bin/env bash
# AetherIDE Linux Packaging Script
# Generates Portable Tarball, AppDir structure (AppImage), and Debian package layout

set -euo pipefail

VERSION="${1:-0.1.0-beta.2}"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
IDE_ROOT="$(dirname "$SCRIPT_DIR")"
OUTPUT_DIR="${OUTPUT_DIR:-$IDE_ROOT/dist}"

CACHE_BASE="${XDG_CACHE_HOME:-$HOME/.cache}/aetheride/build-cache"
TARGET_DIR="${CARGO_TARGET_DIR:-$CACHE_BASE/aetheride/linux-x64}"
RELEASE_BIN_DIR="$TARGET_DIR/release"

echo "============================================================"
echo " Packaging AetherIDE Linux x64 Release ($VERSION)"
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

# 2. Portable Distribution
PORTABLE_DIR="$OUTPUT_DIR/linux-x64/portable"
rm -rf "$PORTABLE_DIR"
mkdir -p "$PORTABLE_DIR/bin" "$PORTABLE_DIR/workbench"

cp "$RELEASE_BIN_DIR/AetherIDE" "$PORTABLE_DIR/AetherIDE"
cp "$RELEASE_BIN_DIR/aether-daemon" "$PORTABLE_DIR/bin/aether-daemon"
cp "$RELEASE_BIN_DIR/aether-cli" "$PORTABLE_DIR/bin/aether-cli"
chmod +x "$PORTABLE_DIR/AetherIDE" "$PORTABLE_DIR/bin/aether-daemon" "$PORTABLE_DIR/bin/aether-cli"

if [ -d "$IDE_ROOT/apps/desktop/workbench" ]; then
    cp -r "$IDE_ROOT/apps/desktop/workbench/"* "$PORTABLE_DIR/workbench/"
fi
if [ -d "$IDE_ROOT/apps/desktop/dist" ]; then
    cp -r "$IDE_ROOT/apps/desktop/dist" "$PORTABLE_DIR/workbench/"
fi
if [ -f "$IDE_ROOT/apps/desktop/package.json" ]; then
    cp "$IDE_ROOT/apps/desktop/package.json" "$PORTABLE_DIR/workbench/"
fi

cp "$IDE_ROOT/LICENSE" "$PORTABLE_DIR/LICENSE"
cp "$IDE_ROOT/README.md" "$PORTABLE_DIR/README.md"
echo "$VERSION" > "$PORTABLE_DIR/VERSION.txt"

cat << 'EOF' > "$PORTABLE_DIR/aetheride.sh"
#!/usr/bin/env bash
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
export PATH="$SCRIPT_DIR/bin:$PATH"
exec "$SCRIPT_DIR/AetherIDE" "$@"
EOF
chmod +x "$PORTABLE_DIR/aetheride.sh"

TAR_PACKAGE="$OUTPUT_DIR/AetherIDE-v$VERSION-linux-x64.tar.gz"
tar -czf "$TAR_PACKAGE" -C "$OUTPUT_DIR/linux-x64" portable
sha256sum "$TAR_PACKAGE" | awk '{print $1 "  " $2}' > "$TAR_PACKAGE.sha256"
echo "Created Portable Tarball: $TAR_PACKAGE"

# 3. AppImage / AppDir Structure
APPDIR="$OUTPUT_DIR/linux-x64/AppDir"
rm -rf "$APPDIR"
mkdir -p "$APPDIR/usr/bin" "$APPDIR/usr/lib/aetheride" "$APPDIR/usr/share/icons/hicolor/256x256/apps"

cp "$RELEASE_BIN_DIR/AetherIDE" "$APPDIR/usr/bin/aetheride"
cp "$RELEASE_BIN_DIR/aether-daemon" "$APPDIR/usr/bin/aether-daemon"
cp "$RELEASE_BIN_DIR/aether-cli" "$APPDIR/usr/bin/aether-cli"
chmod +x "$APPDIR/usr/bin/"*

cp -r "$PORTABLE_DIR/workbench" "$APPDIR/usr/lib/aetheride/"

cat << 'EOF' > "$APPDIR/AppRun"
#!/usr/bin/env bash
APPDIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
export PATH="$APPDIR/usr/bin:$PATH"
export AETHER_RESOURCE_PATH="$APPDIR/usr/lib/aetheride"
exec "$APPDIR/usr/bin/aetheride" "$@"
EOF
chmod +x "$APPDIR/AppRun"

cat << EOF > "$APPDIR/aetheride.desktop"
[Desktop Entry]
Name=AetherIDE
Comment=Autonomous AI Engineering Platform & Next-Generation IDE
Exec=aetheride %U
Icon=aetheride
Type=Application
Categories=Development;IDE;
Terminal=false
StartupNotify=true
EOF

cp "$APPDIR/aetheride.desktop" "$OUTPUT_DIR/linux-x64/aetheride.desktop"

if command -v appimagetool &> /dev/null; then
    echo "Compiling AppImage using appimagetool..."
    appimagetool "$APPDIR" "$OUTPUT_DIR/AetherIDE-v$VERSION-x86_64.AppImage"
    sha256sum "$OUTPUT_DIR/AetherIDE-v$VERSION-x86_64.AppImage" > "$OUTPUT_DIR/AetherIDE-v$VERSION-x86_64.AppImage.sha256"
fi

# 4. Debian Package Structure
DEB_DIR="$OUTPUT_DIR/linux-x64/deb"
rm -rf "$DEB_DIR"
mkdir -p "$DEB_DIR/DEBIAN" "$DEB_DIR/usr/bin" "$DEB_DIR/usr/lib/aetheride" "$DEB_DIR/usr/share/applications"

cat << EOF > "$DEB_DIR/DEBIAN/control"
Package: aetheride
Version: $VERSION
Section: devel
Priority: optional
Architecture: amd64
Maintainer: AetherIDE Engineering Organization <dev@aetheride.org>
Description: Autonomous AI Engineering Platform and Collaborative IDE
 High-performance developer platform with real-time CRDT synchronization,
 local daemon security, and deep intelligence capabilities.
Depends: libgtk-3-0, libwebkit2gtk-4.1-0 | libwebkit2gtk-4.0-37, libsoup2.4-1 | libsoup-3.0-0
EOF

cp "$RELEASE_BIN_DIR/AetherIDE" "$DEB_DIR/usr/bin/aetheride"
cp "$RELEASE_BIN_DIR/aether-daemon" "$DEB_DIR/usr/bin/aether-daemon"
cp "$RELEASE_BIN_DIR/aether-cli" "$DEB_DIR/usr/bin/aether-cli"
chmod +x "$DEB_DIR/usr/bin/"*
cp -r "$PORTABLE_DIR/workbench" "$DEB_DIR/usr/lib/aetheride/"
cp "$APPDIR/aetheride.desktop" "$DEB_DIR/usr/share/applications/"

if command -v dpkg-deb &> /dev/null; then
    echo "Building Debian package with dpkg-deb..."
    dpkg-deb --build "$DEB_DIR" "$OUTPUT_DIR/AetherIDE-v$VERSION-linux-x64.deb"
    sha256sum "$OUTPUT_DIR/AetherIDE-v$VERSION-linux-x64.deb" > "$OUTPUT_DIR/AetherIDE-v$VERSION-linux-x64.deb.sha256"
    echo "Created Debian Package: $OUTPUT_DIR/AetherIDE-v$VERSION-linux-x64.deb"
fi

echo "============================================================"
echo " Linux Packaging Complete!"
echo "============================================================"
