# AetherIDE — Official Brand Identity & Design Guide

**Product Positioning:** AetherIDE — Your AI-Native Engineering Environment  
**Version:** `0.1.0-beta.2`  
**License:** Apache 2.0 / MIT Dual License  

---

## 1. Visual Philosophy

AetherIDE's brand identity represents:
- **Precision Systems Engineering:** High-performance systems programming in Rust.
- **Deep Intelligence:** AI-native orchestration with complete developer autonomy and capability containment.
- **Modernity & Speed:** Sub-second startup, low-memory footprints, and native desktop responsiveness.

The central mark features a faceted crystalline squircle enclosing a dual-beam cybernetic **'A'** letterform converging at an apex focus node, bridged by a quantum horizon bar and glowing synthesis core.

---

## 2. Color System

| Role | Color Name | Hex Code | RGB | Usage |
| :--- | :--- | :--- | :--- | :--- |
| **Primary Background** | Slate Void | `#0F172A` | `rgb(15, 23, 42)` | Application window, installer background, dark mode root |
| **Elevated Surface** | Deep Space | `#1E293B` | `rgb(30, 41, 59)` | Cards, panels, sidebars, wizard surfaces |
| **Border / Stroke** | Border Slate | `#334155` | `rgb(51, 65, 85)` | Panel borders, dividers, subtle separators |
| **Primary Accent** | Electric Cyan | `#38BDF8` | `rgb(56, 189, 248)` | Active indicators, focus rings, primary CTA, left logo beam |
| **Secondary Accent** | Synthetic Indigo | `#6366F1` | `rgb(99, 102, 241)` | Secondary gradients, right logo beam, AI indicators |
| **Primary Text** | Pure Frost | `#F8FAFC` | `rgb(248, 250, 252)` | Headings, high-contrast editor text, brand typography |
| **Secondary Text** | Muted Slate | `#94A3B8` | `rgb(148, 163, 184)` | Subtitles, metadata, status bar information |
| **Success Indicator**| Emerald Signal | `#22C55E` | `rgb(34, 197, 94)` | Validated states, daemon connected, pass indicators |
| **Warning Indicator**| Amber Warning | `#F59E0B` | `rgb(245, 158, 11)` | Non-fatal diagnostics, resource pressure alerts |
| **Error Indicator**  | Crimson Halt | `#EF4444` | `rgb(239, 68, 68)` | Failure alerts, security rejections, validation errors |

---

## 3. Master Asset Architecture

```text
branding/
├── source/
│   ├── aetheride-logo.svg           # Master horizontal logo with symbol and typography
│   ├── aetheride-symbol.svg         # Standalone 512x512 icon glyph
│   ├── aetheride-wordmark.svg       # Standalone typography wordmark
│   └── aetheride-monochrome.svg     # High-contrast single-color monochrome variant
├── png/
│   ├── aetheride-icon-16.png        # Taskbar small, window top-left
│   ├── aetheride-icon-24.png        # High-DPI small icons
│   ├── aetheride-icon-32.png        # Explorer list, taskbar standard
│   ├── aetheride-icon-48.png        # Explorer medium icons, desktop shortcuts
│   ├── aetheride-icon-64.png        # Explorer large icons, Start Menu tiles
│   ├── aetheride-icon-128.png       # High-DPI tiles, macOS dock
│   ├── aetheride-icon-256.png       # Windows Explorer extra large, Alt+Tab
│   └── aetheride-icon-512.png       # Master display raster
├── windows/
│   ├── aetheride.ico                # Multi-resolution embedded application icon
│   ├── aetheride-installer.ico      # Inno Setup executable icon
│   ├── aetheride-wizard.bmp         # Inno Setup left-side wizard panel (164x314)
│   └── aetheride-wizard-small.bmp   # Inno Setup top-right header icon (55x55)
└── branding-guide.md
```

---

## 4. Platform Deployment Rules

1. **Windows Executable (`AetherIDE.exe`):**
   - Must have `aetheride.ico` compiled directly into the binary's `.rsrc` section via `winres`.
   - Must set `SetCurrentProcessExplicitAppUserModelID(w!("AetherIDE.Desktop"))` on cold startup.
   - Must load and assign `hIcon` (32x32) and `hIconSm` (16x16) to the Win32 window class.
2. **Inno Setup Installer (`AetherIDE-Setup-0.1.0-beta.2-x64.exe`):**
   - Must specify `SetupIconFile=branding\windows\aetheride-installer.ico`.
   - Must specify `WizardImageFile=branding\windows\aetheride-wizard.bmp`.
   - Must specify `WizardSmallImageFile=branding\windows\aetheride-wizard-small.bmp`.
   - Must configure `AppUserModelId=AetherIDE.Desktop` on all generated shell shortcuts.
3. **Web Portal & Documentation:**
   - Must use `aetheride-logo.svg` in header navigation and `aetheride-symbol.svg` as favicon.
   - Zero remote CDN font or image dependencies.
