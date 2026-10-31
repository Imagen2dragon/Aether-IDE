#!/usr/bin/env bash
# packaging/macos/build-app.sh
# Automated macOS .app Bundle Creation Pipeline for AetherIDE
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/../.." && pwd)"
DIST_DIR="${ROOT_DIR}/dist/macos-arm64"
OUTPUT_DIR="${ROOT_DIR}/dist/installers"
APP_BUNDLE="${OUTPUT_DIR}/AetherIDE.app"

echo "============================================================"
echo " AetherIDE — macOS Application Bundle Pipeline"
echo "============================================================"

mkdir -p "${OUTPUT_DIR}"
rm -rf "${APP_BUNDLE}"
mkdir -p "${APP_BUNDLE}/Contents/MacOS"
mkdir -p "${APP_BUNDLE}/Contents/Resources"

cat << 'EOF' > "${APP_BUNDLE}/Contents/Info.plist"
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleExecutable</key>
    <string>AetherIDE</string>
    <key>CFBundleIdentifier</key>
    <string>dev.aetheride.studio</string>
    <key>CFBundleName</key>
    <string>AetherIDE</string>
    <key>CFBundleDisplayName</key>
    <string>AetherIDE</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleShortVersionString</key>
    <string>0.1.0</string>
    <key>CFBundleVersion</key>
    <string>1</string>
    <key>LSMinimumSystemVersion</key>
    <string>12.0</string>
    <key>NSHighResolutionCapable</key>
    <true/>
    <key>NSSupportsAutomaticGraphicsSwitching</key>
    <true/>
</dict>
</plist>
EOF

echo " [*] Staging macOS binaries and assets..."
if [ -d "${DIST_DIR}" ]; then
    cp "${DIST_DIR}/AetherIDE" "${APP_BUNDLE}/Contents/MacOS/"
    chmod +x "${APP_BUNDLE}/Contents/MacOS/AetherIDE"
    cp -r "${DIST_DIR}"/* "${APP_BUNDLE}/Contents/Resources/"
fi

echo " [✓] macOS App bundle successfully created at ${APP_BUNDLE}"
