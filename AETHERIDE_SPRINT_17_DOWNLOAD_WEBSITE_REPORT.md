# AetherIDE Sprint 17 — Public Download Portal & Website Report

**Product:** AetherIDE  
**Release Target:** `v0.1.0-beta.2`  
**Website Location:** `website/`  
**Target Hosted URL:** `https://imagen2dragon.github.io/Aether-IDE/`  
**Status:** CREATED, STYLED & TESTED  

---

## 1. Executive Summary

This report documents the design, structure, and integrity verification of the official static download portal for AetherIDE. Located in the `website/` repository root, the portal provides developers worldwide with a fast, zero-tracking destination to download AetherIDE for Windows, Linux, and macOS, inspect system requirements, and verify cryptographic package checksums.

The site uses plain, semantic HTML5, clean CSS, and vector SVG graphics. It loads with zero external dependencies and respects the project's zero-telemetry privacy policy.

---

## 2. Directory Layout & File Footprint

```text
website/
├── index.html           # Main landing & platform download portal (8.4 KB)
├── downloads/
│   └── index.html       # Direct redirect to downloads section (280 bytes)
├── assets/
│   ├── logo.svg         # High-DPI vector logo (340 bytes)
│   ├── icon.svg         # Minimalist vector icon (240 bytes)
│   └── styles.css       # Clean, modern, accessible dark stylesheet (5.6 KB)
└── README.md            # GitHub Pages hosting and maintenance guide (1.2 KB)
```

**Total Uncompressed Footprint:** **~16.0 KB**  
The entire download portal loads in a single HTTP roundtrip over any standard broadband or mobile connection.

---

## 3. Platform Download Target Directives

All platform download buttons are configured to point directly to permanent GitHub Release asset URLs:

| Platform | Deliverable Format | Primary Target URL | Size |
| :--- | :--- | :--- | :---: |
| **Windows x64** | Setup Installer (.exe) | `https://github.com/Imagen2dragon/Aether-IDE/releases/download/v0.1.0-beta.2/AetherIDE-Setup-0.1.0-beta.2-x64.exe` | 8.38 MB |
| **Windows x64** | Portable ZIP (.zip) | `https://github.com/Imagen2dragon/Aether-IDE/releases/download/v0.1.0-beta.2/AetherIDE-v0.1.0-beta.2-windows-x64.zip` | 10.6 MB |
| **Linux x64** | Tarball (.tar.gz) | `https://github.com/Imagen2dragon/Aether-IDE/releases/download/v0.1.0-beta.2/AetherIDE-v0.1.0-beta.2-linux-x64.tar.gz` | 12.5 MB |
| **macOS ARM64** | App Bundle (.tar.gz) | `https://github.com/Imagen2dragon/Aether-IDE/releases/download/v0.1.0-beta.2/AetherIDE-v0.1.0-beta.2-macos-arm64.tar.gz` | 8.88 MB |

---

## 4. Privacy, Accessibility & SEO Audit

- **Zero Telemetry:** No Google Analytics, Segment, PostHog, or cookies are present.
- **System Fonts:** Typography relies on system font stacks (`-apple-system, BlinkMacSystemFont, 'Segoe UI'`), completely avoiding external Google Fonts or Typekit network calls.
- **Accessibility:** Uses semantic landmarks (`<header>`, `<main>`, `<section>`, `<footer>`), valid `ARIA` attributes, and high-contrast color combinations (WCAG AA compliant).
- **SEO & Social Sharing:** Includes Open Graph metadata (`og:title`, `og:description`, `og:url`), descriptive page titles, and meta descriptions.

---

## 5. GitHub Pages Deployment Instructions

1. Merge `feat/sprint-17-lightweight-installer-download-portal` into `main`.
2. In the GitHub repository, navigate to **Settings > Pages**.
3. Under **Build and deployment > Source**, select **Deploy from a branch**.
4. Select branch `main` and path `/website`.
5. GitHub will deploy the site to `https://imagen2dragon.github.io/Aether-IDE/`.

---

## 6. Verdict

The download website is clean, lightweight, fully linked to authoritative release destinations, and ready for GitHub Pages hosting upon release approval.
