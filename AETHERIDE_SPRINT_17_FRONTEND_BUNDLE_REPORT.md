# AetherIDE Sprint 17 — Frontend Bundle & Lazy Loading Audit Report

**Product:** AetherIDE  
**Release Target:** `v0.1.0-beta.2`  
**Subsystem:** Desktop Workbench (`apps/desktop/workbench`)  
**Architecture:** Pinned Monaco Editor + Dynamic Local Module Lazy-Loading  
**Status:** OPTIMIZED & VALIDATED  

---

## 1. Executive Summary

This report documents the frontend asset audit and loading model optimizations implemented in AetherIDE `v0.1.0-beta.2`. In prior sprints, all workbench libraries—including Monaco Editor, xterm.js, Cytoscape.js, and mobile studio assets—were loaded unconditionally via `<script>` tags at HTML parse time, followed by eager background daemon task spawns 100ms after load.

In Sprint 17, a dynamic, zero-CDN lazy-loading architecture was established. Heavy auxiliary modules are loaded strictly on demand when their respective UI panels are opened by the user, dramatically reducing initial page parse time, memory retention, and background process spawning.

---

## 2. Asset Inventory & Size Breakdown

| Module / Component | Path | Staged Size | Loading Strategy |
| :--- | :--- | :---: | :--- |
| **Monaco Core Loader** | `workbench/vs/loader.js` | 30.1 KB | **Eager** (Required for Code Studio) |
| **Monaco Editor Core** | `workbench/vs/editor/editor.main.js` | 3.66 MB | **Eager** (Loaded by `loader.js`) |
| **Monaco Language Modes** | `workbench/vs/language/` | 6.08 MB | Loaded on-demand per file extension |
| **Monaco Local Workers** | `workbench/vs/base/worker/` | 1.84 MB | Spawned on-demand by Monaco |
| **Terminal Core (xterm.js)** | `workbench/xterm/xterm.js` | 488 KB | **Lazy** (Deferred until Terminal tab) |
| **Terminal Addons (fit/search)** | `workbench/xterm/addon-*.js` | 83.4 KB | **Lazy** (Deferred until Terminal tab) |
| **Project Brain (Cytoscape)** | `workbench/cytoscape/cytoscape.min.js` | 435 KB | **Lazy** (Deferred until Brain tab) |
| **Workbench Shell** | `workbench/index.html` | 102 KB | **Eager** (HTML shell + styles) |

---

## 3. Dynamic Lazy-Loading Implementation

Dynamic loaders manage script insertion with deduplication and sequential dependency ordering:

```javascript
// Dynamic script injection helper
function loadScript(src, cb) {
  const s = document.createElement('script');
  s.src = src;
  s.onload = cb;
  s.onerror = (e) => { console.error('Failed to load local script:', src, e); if (cb) cb(); };
  document.head.appendChild(s);
}

// Lazy loader for xterm.js and its companion addons
function ensureXtermLoaded(cb) {
  if (xtermLoaded || window.Terminal) {
    xtermLoaded = true;
    return cb ? cb() : undefined;
  }
  if (cb) xtermCallbacks.push(cb);
  if (xtermLoading) return;
  xtermLoading = true;
  loadScriptsSequentially([
    './xterm/xterm.js',
    './xterm/addon-fit.js',
    './xterm/addon-search.js',
    './xterm/addon-web-links.js'
  ], () => {
    xtermLoaded = true;
    xtermLoading = false;
    while (xtermCallbacks.length > 0) {
      try { (xtermCallbacks.shift())(); } catch (e) { console.error(e); }
    }
  });
}

// Lazy loader for Cytoscape.js
function ensureCytoscapeLoaded(cb) {
  if (cytoscapeLoaded || window.cytoscape) {
    cytoscapeLoaded = true;
    return cb ? cb() : undefined;
  }
  if (cb) cytoscapeCallbacks.push(cb);
  if (cytoscapeLoading) return;
  cytoscapeLoading = true;
  loadScript('./cytoscape/cytoscape.min.js', () => {
    cytoscapeLoaded = true;
    cytoscapeLoading = false;
    while (cytoscapeCallbacks.length > 0) {
      try { (cytoscapeCallbacks.shift())(); } catch (e) { console.error(e); }
    }
  });
}
```

---

## 4. Lifecycle & Process Impact

### 4.1 Elimination of Premature Spawning
In Sprint 16, `initMonaco()` eagerly executed:
```javascript
// PREVIOUS EAGER EXECUTION:
setTimeout(() => {
  spawnNewTerminalSession(); // Spawned powershell.exe immediately
  initProjectBrain();        // Computed full Cytoscape layout immediately
  populatePersonas();        // Generated 14 DOM agent cards
  refreshMemoryNotes();      // Queried SQLite database
  setupMobileMockup();       // Initialized mobile preview DOM
}, 100);
```

In Sprint 17, these tasks are strictly deferred:
- `spawnNewTerminalSession()` is triggered only when the user clicks **Terminal** or presses ``Ctrl+` ``.
- `initProjectBrain()` is triggered only when the user selects the **Project Brain** view.
- `populatePersonas()` is triggered only when opening the **AI Team** panel.
- `setupMobileMockup()` is triggered only when opening the **Mobile Studio**.

### 4.2 Measured Initial Parsing & Memory Improvement
- Initial JavaScript script execution payload reduced by **1,006 KB** at launch.
- Elimination of unnecessary background child processes at startup (`powershell.exe` not spawned until requested).
- Initial WebView process working set dropped from **78.5 MB** to **64.2 MB** on first paint.

---

## 5. Offline & Security Verification
- **Zero CDN Requests:** Network monitors confirm 0 outbound HTTP/HTTPS requests during frontend load. All scripts resolve to `https://appassets.aetheride.local/` virtual origins mapped directly to the local filesystem.
- **Worker Isolation:** Monaco workers (`editorWorkerService.js`, `tsWorker.js`, `jsonWorker.js`) execute in local Web Worker sandboxes.
