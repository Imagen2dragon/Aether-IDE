# AetherIDE — Mobile App Studio Guide

## 1. Introduction
The **Mobile App Studio** is a dedicated first-class environment inside AetherIDE for designing, previewing, and building cross-platform Android and iOS applications directly from source code and AI prompts.

---

## 2. Default Framework: React Native + Expo
AetherIDE defaults cross-platform mobile development to:
- **Runtime:** React Native 0.76+
- **Tooling:** Expo SDK 52+
- **Routing:** Expo Router (File-based navigation)
- **Language:** TypeScript
- **Styling:** StyleSheet / NativeWind (Tailwind CSS for React Native)

### Scaffold Structure:
```
my-mobile-app/
├── app/
│   ├── _layout.tsx      # Root Navigation Slot
│   ├── index.tsx        # Home Screen
│   ├── profile.tsx      # Profile Screen
│   └── settings.tsx     # Settings Screen
├── assets/              # Icons, Splash Screens, Fonts
├── app.json             # Expo Manifest
├── eas.json             # Cloud Build Profiles
└── package.json
```

---

## 3. Visual Screen Designer
The Visual Screen Designer provides:
- Live rendering with two-way AST synchronization: drag components on the canvas to update JSX, or edit JSX to update the canvas.
- Presets for iPhone 16 Pro, Google Pixel 9, iPad Pro, and Galaxy Fold.
- Device capability toggles: Camera, Biometrics, SecureStore, Geolocation, Network State.

---

## 4. Cloud Builds via EAS (Expo Application Services)
Developers do NOT need a local Mac to build iOS IPAs. AetherIDE triggers EAS Cloud Builds directly from the IDE:
```powershell
# In terminal or via UI button
eas build --platform android --profile preview    # Generates installable APK
eas build --platform ios --profile preview        # Generates installable IPA
eas build --platform all --profile production     # Generates Store AAB & IPA
```
Artifacts are automatically polled and downloaded to `.aether/mobile_builds/`.
