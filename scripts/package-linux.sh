#!/usr/bin/env bash
# AetherIDE Linux Packaging Script
# Generates Portable Tarball, AppDir structure (AppImage), and Debian package layout
# Explicitly tracks artifact generation outcomes and eliminates silent skips

set -euo pipefail

VERSION="${1:-0.1.0-beta.2}"
PKG_TYPE="${2:-all}" # portable, appimage, deb, all
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
TARGET_DIR="${CARGO_TARGET_DIR:-$CACHE_BASE/aetheride/linux-x64}"
RELEASE_BIN_DIR="$TARGET_DIR/release"

echo "============================================================"
echo " Packaging AetherIDE Linux x64 Release ($VERSION)"
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

# 2. Portable Distribution
TAR_PACKAGE="$OUTPUT_DIR/AetherIDE-v$VERSION-linux-x64.tar.gz"
PORTABLE_DIR="$OUTPUT_DIR/linux-x64/portable"
if [ "$PKG_TYPE" = "portable" ] || [ "$PKG_TYPE" = "all" ]; then
    echo -e "\n[1/3] Assembling Portable Distribution..."
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

    tar -czf "$TAR_PACKAGE" -C "$OUTPUT_DIR/linux-x64" portable
    if [ -f "$TAR_PACKAGE" ]; then
        HASH=$(sha256sum "$TAR_PACKAGE" | awk '{print $1}')
        echo "$HASH  $(basename "$TAR_PACKAGE")" > "$TAR_PACKAGE.sha256"
        SIZE=$(stat -c%s "$TAR_PACKAGE" 2>/dev/null || stat -f%z "$TAR_PACKAGE")
        record_status "Portable TAR.GZ" "$TAR_PACKAGE" "BUILT" "Size: $SIZE bytes, SHA256: $HASH"
    else
        record_status "Portable TAR.GZ" "$TAR_PACKAGE" "FAILED" "Failed to create tar archive"
    fi
else
    record_status "Portable TAR.GZ" "$TAR_PACKAGE" "NOT_REQUESTED" "Skipped by scope parameter"
fi

# 3. AppImage / AppDir Structure
APPDIR="$OUTPUT_DIR/linux-x64/AppDir"
APPIMAGE_PACKAGE="$OUTPUT_DIR/AetherIDE-v$VERSION-x86_64.AppImage"
if [ "$PKG_TYPE" = "appimage" ] || [ "$PKG_TYPE" = "all" ]; then
    echo -e "\n[2/3] Assembling AppDir structure..."
    rm -rf "$APPDIR"
    mkdir -p "$APPDIR/usr/bin" "$APPDIR/usr/lib/aetheride" "$APPDIR/usr/share/icons/hicolor/256x256/apps"

    cp "$RELEASE_BIN_DIR/AetherIDE" "$APPDIR/usr/bin/aetheride"
    cp "$RELEASE_BIN_DIR/aether-daemon" "$APPDIR/usr/bin/aether-daemon"
    cp "$RELEASE_BIN_DIR/aether-cli" "$APPDIR/usr/bin/aether-cli"
    chmod +x "$APPDIR/usr/bin/"*

    if [ -d "$PORTABLE_DIR/workbench" ]; then
        cp -r "$PORTABLE_DIR/workbench" "$APPDIR/usr/lib/aetheride/"
    fi

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
        appimagetool "$APPDIR" "$APPIMAGE_PACKAGE"
        if [ -f "$APPIMAGE_PACKAGE" ]; then
            HASH=$(sha256sum "$APPIMAGE_PACKAGE" | awk '{print $1}')
            echo "$HASH  $(basename "$APPIMAGE_PACKAGE")" > "$APPIMAGE_PACKAGE.sha256"
            SIZE=$(stat -c%s "$APPIMAGE_PACKAGE" 2>/dev/null || stat -f%z "$APPIMAGE_PACKAGE")
            record_status "AppImage" "$APPIMAGE_PACKAGE" "BUILT" "Size: $SIZE bytes, SHA256: $HASH"
        else
            record_status "AppImage" "$APPIMAGE_PACKAGE" "FAILED" "appimagetool exited without output file"
        fi
    else
        MSG="appimagetool not found on PATH. Staged AppDir preserved at $APPDIR"
        if [ "$REQUIRE_ALL" = true ] || [ "$PKG_TYPE" = "appimage" ]; then
            record_status "AppImage" "$APPIMAGE_PACKAGE" "BLOCKED" "$MSG"
            if [ "$REQUIRE_ALL" = true ]; then
                echo "ERROR: $MSG (--require-all specified)"
                exit 1
            fi
        else
            record_status "AppImage" "$APPIMAGE_PACKAGE" "BLOCKED" "$MSG"
        fi
    fi
else
    record_status "AppImage" "$APPIMAGE_PACKAGE" "NOT_REQUESTED" "Skipped by scope parameter"
fi

# 4. Debian Package Structure
DEB_DIR="$OUTPUT_DIR/linux-x64/deb"
DEB_PACKAGE="$OUTPUT_DIR/AetherIDE-v$VERSION-linux-x64.deb"
if [ "$PKG_TYPE" = "deb" ] || [ "$PKG_TYPE" = "all" ]; then
    echo -e "\n[3/3] Assembling Debian Package Layout..."
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
    if [ -d "$PORTABLE_DIR/workbench" ]; then
        cp -r "$PORTABLE_DIR/workbench" "$DEB_DIR/usr/lib/aetheride/"
    fi
    cp "$APPDIR/aetheride.desktop" "$DEB_DIR/usr/share/applications/"

    if command -v dpkg-deb &> /dev/null; then
        echo "Building Debian package with dpkg-deb..."
        dpkg-deb --build "$DEB_DIR" "$DEB_PACKAGE"
        if [ -f "$DEB_PACKAGE" ]; then
            HASH=$(sha256sum "$DEB_PACKAGE" | awk '{print $1}')
            echo "$HASH  $(basename "$DEB_PACKAGE")" > "$DEB_PACKAGE.sha256"
            SIZE=$(stat -c%s "$DEB_PACKAGE" 2>/dev/null || stat -f%z "$DEB_PACKAGE")
            record_status "Debian (.deb)" "$DEB_PACKAGE" "BUILT" "Size: $SIZE bytes, SHA256: $HASH"
        else
            record_status "Debian (.deb)" "$DEB_PACKAGE" "FAILED" "dpkg-deb finished without output"
        fi
    else
        MSG="dpkg-deb tool not available. Staged package layout preserved at $DEB_DIR"
        if [ "$REQUIRE_ALL" = true ] || [ "$PKG_TYPE" = "deb" ]; then
            record_status "Debian (.deb)" "$DEB_PACKAGE" "BLOCKED" "$MSG"
            if [ "$REQUIRE_ALL" = true ]; then
                echo "ERROR: $MSG (--require-all specified)"
                exit 1
            fi
        else
            record_status "Debian (.deb)" "$DEB_PACKAGE" "BLOCKED" "$MSG"
        fi
    fi
else
    record_status "Debian (.deb)" "$DEB_PACKAGE" "NOT_REQUESTED" "Skipped by scope parameter"
fi

# 5. Summary Matrix Table
echo "============================================================"
echo " Linux Release Packaging Status Summary"
echo "============================================================"
printf "%-18s | %-12s | %s\n" "Format" "Outcome" "Details"
echo "------------------------------------------------------------"
for i in "${!STATUS_FORMAT[@]}"; do
    printf "%-18s | %-12s | %s\n" "${STATUS_FORMAT[$i]}" "${STATUS_OUTCOME[$i]}" "${STATUS_DETAILS[$i]}"
done
echo "============================================================"
