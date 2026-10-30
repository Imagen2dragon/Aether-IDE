# AetherIDE — iOS Companion Build & Distribution Guide

## 1. Overview
The AetherIDE iOS/iPadOS companion is a native Swift/SwiftUI application located in `apps/ios-companion/`. It connects to desktop or cloud workspaces via end-to-end encrypted WebSocket relays or direct LAN pairing.

---

## 2. Xcode & Remote macOS Infrastructure Requirements
Because Apple iOS applications (IPA binaries) require Xcode, building on Windows uses cloud macOS runners (Codemagic, GitHub Actions macOS-14/15, or dedicated macOS hosts).

### Build Script (`apps/ios-companion/ci_build.sh`):
```bash
#!/bin/bash
set -e

echo "Building Aether Companion iOS Application..."
cd apps/ios-companion

# 1. Compile Rust UniFFI static library for iOS architectures
# aarch64-apple-ios (Physical Device) & aarch64-apple-ios-sim (Simulator)
cargo build --package application-core --target aarch64-apple-ios --release

# 2. Build Xcode Archive
xcodebuild archive \
  -scheme AetherCompanion \
  -archivePath ./build/AetherCompanion.xcarchive \
  -destination 'generic/platform=iOS' \
  -configuration Release

# 3. Export IPA
xcodebuild -exportArchive \
  -archivePath ./build/AetherCompanion.xcarchive \
  -exportOptionsPlist ./ExportOptions.plist \
  -exportPath ./build/ipa
```

---

## 3. TestFlight & App Store Connect Preparation
1. **Bundle Identifier:** `com.aetheride.companion`
2. **Entitlements:**
   - Network Egress (LAN Discovery & Bonjour)
   - Push Notifications (APNs for task completion alerts)
   - Camera (QR code desktop pairing)
3. **Distribution:** Upload the exported `.ipa` to App Store Connect via `altool` or Fastlane.
