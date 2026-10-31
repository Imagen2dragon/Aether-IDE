# AetherIDE Download Portal & Static Website

This directory contains the public static download portal for **AetherIDE `v0.1.0-beta.2`**, designed for zero-dependency hosting on GitHub Pages.

## Directory Layout

```text
website/
├── index.html           # Main landing & platform download portal
├── downloads/
│   └── index.html       # Direct anchor redirect to platform downloads
├── assets/
│   ├── logo.svg         # High-resolution vector logo
│   ├── icon.svg         # Minimalist vector icon
│   └── styles.css       # Clean, modern, zero-telemetry CSS
└── README.md            # Hosting and deployment documentation
```

## GitHub Pages Configuration

To host this site via GitHub Pages:
1. Navigate to **Repository Settings > Pages**.
2. Under **Build and deployment > Source**, select **Deploy from a branch**.
3. Choose branch `main` and folder `/website` (or root via custom action).
4. The site is served securely via HTTPS at `https://imagen2dragon.github.io/Aether-IDE/`.

## Privacy & Performance Guarantees
- **Zero Telemetry:** No tracking scripts, Google Analytics, cookies, or external beacon requests.
- **Local Typography:** Uses native system font stacks (`-apple-system, BlinkMacSystemFont, 'Segoe UI'`) for zero external HTTP font latency.
- **Payload Size:** Total uncompressed website size is under 25 KB.
