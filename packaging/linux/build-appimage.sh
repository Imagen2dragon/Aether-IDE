#!/usr/bin/env bash
# packaging/linux/build-appimage.sh
# Automated Linux AppImage Packaging Pipeline for AetherIDE
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/../.." && pwd)"
DIST_DIR="${ROOT_DIR}/dist/linux-x64"
OUTPUT_DIR="${ROOT_DIR}/dist/installers"
APP_DIR="${BUILD_CACHE_DIR:-/tmp}/AetherIDE.AppDir"

echo "============================================================"
echo " AetherIDE — Linux AppImage Packaging Pipeline"
echo "============================================================"

mkdir -p "${OUTPUT_DIR}"
rm -rf "${APP_DIR}"
mkdir -p "${APP_DIR}/usr/bin"
mkdir -p "${APP_DIR}/usr/share/applications"
mkdir -p "${APP_DIR}/usr/share/icons/hicolor/256x256/apps"
mkdir -p "${APP_DIR}/usr/lib/aetheride"

echo " [*] Staging Linux binaries and workbench..."
if [ -d "${DIST_DIR}" ]; then
    cp -r "${DIST_DIR}"/* "${APP_DIR}/usr/lib/aetheride/"
    ln -sf "../lib/aetheride/AetherIDE" "${APP_DIR}/usr/bin/aetheride"
    ln -sf "../lib/aetheride/bin/aether-cli" "${APP_DIR}/usr/bin/aether"
fi

cat << 'EOF' > "${APP_DIR}/usr/share/applications/aetheride.desktop"
[Desktop Entry]
Name=AetherIDE
Comment=Production AI-Native Software Engineering Studio
Exec=aetheride %F
Icon=aetheride
Type=Application
Categories=Development;IDE;
Terminal=false
StartupWMClass=AetherIDE
EOF

cat << 'EOF' > "${APP_DIR}/AppRun"
#!/bin/sh
HERE="$(dirname "$(readlink -f "${0}")")"
export PATH="${HERE}/usr/bin:${PATH}"
export LD_LIBRARY_PATH="${HERE}/usr/lib:${HERE}/usr/lib/aetheride:${LD_LIBRARY_PATH:-}"
exec "${HERE}/usr/lib/aetheride/AetherIDE" "$@"
EOF
chmod +x "${APP_DIR}/AppRun"

echo " [*] Checking for appimagetool..."
if command -v appimagetool &> /dev/null; then
    ARCH=x86_64 appimagetool "${APP_DIR}" "${OUTPUT_DIR}/AetherIDE-v0.1.0-alpha-x86_64.AppImage"
    echo " [✓] AppImage generated successfully at ${OUTPUT_DIR}/AetherIDE-v0.1.0-alpha-x86_64.AppImage"
else
    echo " [!] appimagetool not found on PATH. Staged AppDir ready at ${APP_DIR}."
fi
