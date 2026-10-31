#!/usr/bin/env bash
# packaging/linux/build-deb.sh
# Automated Debian/Ubuntu .deb Packaging Pipeline for AetherIDE
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/../.." && pwd)"
DIST_DIR="${ROOT_DIR}/dist/linux-x64"
OUTPUT_DIR="${ROOT_DIR}/dist/installers"
PKG_VERSION="0.1.0"
DEB_ROOT="${BUILD_CACHE_DIR:-/tmp}/aetheride-deb-root"

echo "============================================================"
echo " AetherIDE — Debian/Ubuntu Package Pipeline"
echo "============================================================"

mkdir -p "${OUTPUT_DIR}"
rm -rf "${DEB_ROOT}"
mkdir -p "${DEB_ROOT}/DEBIAN"
mkdir -p "${DEB_ROOT}/opt/aetheride"
mkdir -p "${DEB_ROOT}/usr/bin"
mkdir -p "${DEB_ROOT}/usr/share/applications"

cat << EOF > "${DEB_ROOT}/DEBIAN/control"
Package: aetheride
Version: ${PKG_VERSION}
Section: devel
Priority: optional
Architecture: amd64
Depends: libc6 (>= 2.31), libwebkit2gtk-4.0-37 (>= 2.30.0) | libwebkit2gtk-4.1-0, libgtk-3-0 (>= 3.24.0)
Maintainer: Aether Technologies <dev@aetheride.dev>
Description: Production AI-Native Software Engineering Studio
 AetherIDE is an enterprise-grade AI-native software engineering IDE
 featuring multi-model swarms, real-time CRDT collaboration, native terminals,
 and integrated mobile development suites.
EOF

echo " [*] Staging files into Debian package layout..."
if [ -d "${DIST_DIR}" ]; then
    cp -r "${DIST_DIR}"/* "${DEB_ROOT}/opt/aetheride/"
    ln -sf "/opt/aetheride/AetherIDE" "${DEB_ROOT}/usr/bin/aetheride"
    ln -sf "/opt/aetheride/bin/aether-cli" "${DEB_ROOT}/usr/bin/aether"
fi

cat << 'EOF' > "${DEB_ROOT}/usr/share/applications/aetheride.desktop"
[Desktop Entry]
Name=AetherIDE
Comment=Production AI-Native Software Engineering Studio
Exec=/usr/bin/aetheride %F
Icon=/opt/aetheride/assets/icon.png
Type=Application
Categories=Development;IDE;
Terminal=false
StartupWMClass=AetherIDE
EOF

echo " [*] Building .deb package with dpkg-deb..."
if command -v dpkg-deb &> /dev/null; then
    dpkg-deb --build --root-owner-group "${DEB_ROOT}" "${OUTPUT_DIR}/aetheride_${PKG_VERSION}_amd64.deb"
    echo " [✓] Debian package generated successfully at ${OUTPUT_DIR}/aetheride_${PKG_VERSION}_amd64.deb"
else
    echo " [!] dpkg-deb not found. Staged package layout ready at ${DEB_ROOT}."
fi
