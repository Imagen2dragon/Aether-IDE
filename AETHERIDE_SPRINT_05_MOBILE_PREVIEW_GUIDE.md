# AetherIDE — Mobile Studio Live Device Preview Guide
## React Native & Expo 52 Integration, Multi-Device Frames and ADB Discovery

**Author:** Mobile Engineering Lead & Desktop IDE Engineer  
**Project:** AetherIDE  
**Sprint:** Sprint 05  

---

## 1. Subsystem Architecture Overview

The Mobile App Studio bridges native desktop development with cross-platform mobile engineering. It manages the lifecycle of the React Native / Expo development server, renders live application previews inside realistic device frames, provides physical device pairing via QR codes, and detects attached Android hardware and emulators via ADB.

```
+--------------------------------------------------------------------------+
|                       AetherIDE Mobile Studio                            |
|  +--------------------+  +--------------------------------------------+  |
|  | Server Supervisor  |  |           Device Simulator Frame           |  |
|  |  - Start / Stop    |  |  [iPhone 16 / Pixel 8 / iPad Pro]          |  |
|  |  - Fast Refresh    |  |  +--------------------------------------+  |  |
|  |  - Port 8081       |  |  | Isolated Sandboxed Iframe (Metro)    |  |  |
|  +--------------------+  |  | http://localhost:8081                |  |  |
|  | QR Code (LAN Pair) |  |  +--------------------------------------+  |  |
|  | ADB Device Scan    |  +--------------------------------------------+  |
|  | Live Metro Logs    |                                                  |
|  +--------------------+                                                  |
+--------------------------------------------------------------------------+
```

---

## 2. Dev Server Supervisor & Process Management

- **Lifecycle Controls:** Engineers can launch, restart, hot-reload, and terminate the Expo development server directly from the workbench.
- **Port Management:** The supervisor monitors port 8081 (default Metro bundler port) and parses stdout to detect server readiness and fast-refresh triggers.
- **IPC Actions:**
  - `StartExpoServer`: Spawns `npx expo start --web` using `process_manager` with clean child process tracking.
  - `GetExpoStatus`: Queries current bundler status, bound port, and local LAN pairing IP.

---

## 3. Interactive Multi-Device Simulator

The simulator renders realistic mobile device mockups directly inside the workbench canvas:
- **Device Frame Presets:**
  - **Apple iPhone 16:** 390 x 844 px portrait viewport with Dynamic Island.
  - **Google Pixel 8:** 412 x 915 px portrait viewport with punch-hole camera.
  - **Apple iPad Pro:** 820 x 1180 px tablet viewport.
- **Orientation Switcher:** Instant rotation between Portrait and Landscape modes with dynamic CSS transform scaling.
- **Sandboxed Security Isolation:** The preview renders inside an `<iframe>` configured with `sandbox="allow-scripts allow-same-origin"`. It runs on an isolated web origin with zero access to `window.chrome.webview` or the native host bridge, preventing previewed web bundles from issuing unauthorized native IPC calls.

---

## 4. Physical Device QR Pairing & ADB Discovery

- **QR Code LAN Pairing:** The studio automatically constructs the Expo Go pairing URL (`exp://<LAN_IP>:8081`) and displays a scannable QR code. Developers on the same Wi-Fi network can scan the code using the native iOS Camera app or the Android Expo Go app for instant physical device execution.
- **ADB Discovery:**
  - Queries `adb devices -l` to enumerate USB-connected Android phones and local Android Studio emulators (e.g. `emulator-5554`).
  - Reports device serials, connection states (`device`, `offline`, `unauthorized`), and product models.
  - Offers one-click "Launch on Device" automation.

---

## 5. Live Metro Log Console

The Mobile Studio includes a dedicated log stream panel displaying:
- Metro compilation events and bundle build timings.
- Fast Refresh notifications.
- Runtime warnings, TypeScript diagnostic errors, and React Native stack traces.
- Log level filtering (All, Info, Warn, Error) and clear-log controls.
