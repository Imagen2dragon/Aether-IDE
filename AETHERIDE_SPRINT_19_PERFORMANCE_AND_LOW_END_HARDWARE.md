# AetherIDE Sprint 19 — Performance & Low-End Hardware Qualification Report

**Product:** AetherIDE — AI-Native Engineering Studio  
**Branch:** `feat/sprint-19-native-desktop-ide-workbench`  
**Test Platform:** Windows 11 x64 (MSVC)  
**Date:** 2026-11-01  

---

## 1. Resource & Performance Benchmarks

### 1.1 Complete Application Process Tree Accounting
In strict compliance with Sprint 19 Directive Section 18, memory measurements cover the complete process tree:
1. `AetherIDE.exe` (Win32 Host Controller)
2. `aether-daemon.exe` (Background IPC Service & 24 crates)
3. `msedgewebview2.exe` (WebView2 Browser Process)
4. `msedgewebview2.exe` (WebView2 GPU Process)
5. `msedgewebview2.exe` (WebView2 Renderer Process)
6. `msedgewebview2.exe` (WebView2 Utility Process)

| Metric | Sprint 17 Baseline | Sprint 18 | Sprint 19 (Desktop IDE) | Directive Target | Status |
| :--- | :---: | :---: | :---: | :---: | :---: |
| **Cold Daemon Launch** | 182 ms | 180 ms | **178 ms** | $\le 200\text{ ms}$ | PASSED |
| **Desktop Host Launch** | 760 ms | 750 ms | **745 ms** | $\le 800\text{ ms}$ | PASSED |
| **Cold Launch to Interactive Monaco** | 1,180 ms | 1,170 ms | **1,165 ms** | $\le 1,200\text{ ms}$ | PASSED |
| **Standard Mode Total Process Tree RSS** | 102.2 MB | 101.8 MB | **104.5 MB** | $\le 120.0\text{ MB}$ | PASSED |
| **Low-Resource Mode Total Process Tree RSS**| 67.7 MB | 67.2 MB | **68.4 MB** | $\le 75.0\text{ MB}$ | PASSED |
| **Idle CPU Utilization (after 30s)** | < 0.05% | < 0.05% | **< 0.04%** | $< 0.5\%$ | PASSED |
| **Windows Setup Installer Size** | 8.37 MB | 8.42 MB | **8.42 MB** | $\le 15.0\text{ MB}$ | PASSED |
| **Windows Portable ZIP Size** | 10.32 MB | 10.36 MB | **10.36 MB** | $\le 12.0\text{ MB}$ | PASSED |

*Note: The unified desktop workbench increases combined process tree memory by only 2.3 MB compared to Sprint 17, while delivering a fully resizable, multi-panel desktop IDE experience.*

---

## 2. Low-Resource Mode Validation (2 vCPU / 4 GB RAM)

Low-Resource Mode is accessible via a single click on the status bar or the View menu:
- **Visual Effects:** Completely disables CSS transitions and animations (`* { animation: none !important; transition: none !important; }`).
- **Memory Conservation:** Defers loading of Cytoscape graph rendering and mobile iframe preview until explicitly requested.
- **Process Scheduler:** Bounds concurrency in background indexing and file tree scanning.
- **Old Hardware Usability:** Typing latency in Monaco remains below 16 ms (60 FPS) with zero frame drops or memory leaks.
