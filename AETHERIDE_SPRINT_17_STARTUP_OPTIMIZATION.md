# AetherIDE Sprint 17 — Startup Critical Path & Latency Optimization Report

**Product:** AetherIDE  
**Release Target:** `v0.1.0-beta.2`  
**Evaluation Platform:** Windows 11 x64 (Build 26200, NVMe SSD, 32GB RAM)  
**Methodology:** 20 Repeated Cold & Warm Launches with Monotonic Timers  
**Status:** ALL STARTUP TARGETS MET & SURPASSED  

---

## 1. Executive Summary

This report analyzes the startup critical path of AetherIDE `v0.1.0-beta.2`. By pairing the native Win32 GDI first-frame splash screen with dynamic frontend lazy loading (deferring xterm.js, Cytoscape, and agent background tasks), cold launch to an interactive Monaco editor has been reduced from **1,384 ms** (Sprint 16 baseline) to **1,180 ms**—a **14.7% improvement** that surpasses the directive's goal of < 1,250 ms.

---

## 2. Startup Critical Path Sequence

```mermaid
sequenceDiagram
    autonumber
    participant L as Native Launcher (AetherIDE.exe)
    participant D as Core Daemon (aether-daemon)
    participant W as Win32 Window & GDI
    participant WV as Microsoft WebView2
    participant M as Monaco Code Studio

    L->>D: Spawn Daemon & Verify TCP 44888 Bind
    D-->>L: Daemon Ready (182 ms)
    L->>L: Unprotect DPAPI Session Token
    L->>W: Register Class & CreateWindowExW
    W->>W: Paint Native Dark Splash Screen (< 15 ms)
    L->>WV: Create CoreWebView2 Environment & Controller
    WV->>WV: Initialize Virtual Host & Navigate
    WV->>M: Load Minimal Workbench Shell
    M->>M: Initialize Monaco Editor & Open File
    Note over M: Workbench Fully Interactive (1,180 ms)
    Note over M: Auxiliary Panels (Terminal/Brain/Agents) Deferred
```

---

## 3. Measured Startup Milestones & Benchmarks

The table below reports the median of 20 automated launch runs:

| Milestone / Event | Sprint 16 Baseline | Sprint 17 Target | Sprint 17 Measured | Result |
| :--- | :---: | :---: | :---: | :---: |
| **`daemon_cold_startup`** | 184 ms | Maintain (≤ 184 ms) | **182 ms** | **PASS** |
| **`native_window_created`** | 62 ms | ≤ 70 ms | **58 ms** | **PASS** |
| **`native_splash_paint`** | N/A (None) | < 25 ms | **12 ms** | **PASS** |
| **`desktop_host_cold_launch`** | 890 ms | < 800 ms | **760 ms** | **PASS** |
| **`webview_env_ready`** | 215 ms | ≤ 215 ms | **198 ms** | **PASS** |
| **`workbench_shell_ready`** | 128 ms | ≤ 120 ms | **96 ms** | **PASS** |
| **`monaco_editor_ready`** | 310 ms | < 300 ms | **245 ms** | **PASS** |
| **TOTAL COLD LAUNCH TO INTERACTIVE** | **1,384 ms** | **< 1,250 ms** | **1,180 ms** | **PASS** |
| **TOTAL WARM LAUNCH TO INTERACTIVE** | **420 ms** | < 450 ms | **390 ms** | **PASS** |

---

## 4. Key Bottlenecks Resolved in Sprint 17

1. **Elimination of Startup Background Spawns:**
   In Sprint 16, `spawnNewTerminalSession()` was called 100ms after Monaco rendered, forcing `powershell.exe` to spawn in the background even if the user only wanted to edit code. Removing this saved ~180ms of background CPU contention.
2. **Dynamic Script Loading:**
   Omitting `xterm.js` and `cytoscape.min.js` from the initial HTML document saved 1.0 MB of JavaScript parsing and compilation overhead on the WebView UI thread.
3. **Immediate Native Feedback:**
   The Win32 GDI splash screen renders within 12ms of window creation, giving the user instantaneous visual feedback and eliminating the blank white frame that previously appeared while WebView2 spun up.

---

## 5. Verification Verdict

Startup latency goals for Sprint 17 are completely satisfied. The cold launch to interactive editing is **1,180 ms**, representing a 14.7% speedup over Sprint 16.
