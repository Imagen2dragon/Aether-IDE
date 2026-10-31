#!/usr/bin/env bash
# Generates SHA256SUMS.txt and RELEASE_MANIFEST.json for all release artifacts
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
IDE_ROOT="$(dirname "$SCRIPT_DIR")"
DIST_DIR="${1:-$IDE_ROOT/dist}"

if [ ! -d "$DIST_DIR" ]; then
    echo "Distribution directory not found: $DIST_DIR"
    exit 1
fi

echo "Scanning release packages in: $DIST_DIR"
cd "$DIST_DIR"

# Collect checksums for target release archives
SUM_FILE="SHA256SUMS.txt"
rm -f "$SUM_FILE"

find . -maxdepth 1 -type f \( -name "*.zip" -o -name "*.tar.gz" -o -name "*.deb" -o -name "*.AppImage" -o -name "*.dmg" -o -name "*.exe" \) | while read -r file; do
    fname="$(basename "$file")"
    if command -v sha256sum &> /dev/null; then
        sha256sum "$fname" >> "$SUM_FILE"
    elif command -v shasum &> /dev/null; then
        shasum -a 256 "$fname" >> "$SUM_FILE"
    fi
done

echo "Wrote $DIST_DIR/$SUM_FILE"
