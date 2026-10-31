#!/usr/bin/env bash
# packaging/macos/build-dmg.sh
# Automated macOS DMG Disk Image Creation Pipeline for AetherIDE
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/../.." && pwd)"
OUTPUT_DIR="${ROOT_DIR}/dist/installers"
APP_BUNDLE="${OUTPUT_DIR}/AetherIDE.app"
DMG_PATH="${OUTPUT_DIR}/AetherIDE-v0.1.0-beta.1-arm64.dmg"
DMG_STAGE="${BUILD_CACHE_DIR:-/tmp}/dmg-staging"

echo "============================================================"
echo " AetherIDE — macOS DMG Disk Image Pipeline"
echo "============================================================"

if [ ! -d "${APP_BUNDLE}" ]; then
    echo " [*] App bundle not found. Building app bundle first..."
    bash "${SCRIPT_DIR}/build-app.sh"
fi

rm -rf "${DMG_STAGE}"
mkdir -p "${DMG_STAGE}"
cp -r "${APP_BUNDLE}" "${DMG_STAGE}/"
ln -s /Applications "${DMG_STAGE}/Applications"

if command -v hdiutil &> /dev/null; then
    echo " [*] Packaging disk image with hdiutil..."
    rm -f "${DMG_PATH}"
    hdiutil create -volname "AetherIDE" -srcfolder "${DMG_STAGE}" -ov -format UDZO "${DMG_PATH}"
    echo " [✓] Disk image successfully generated at ${DMG_PATH}"
else
    echo " [!] hdiutil not found. Staged DMG contents ready at ${DMG_STAGE}."
fi
