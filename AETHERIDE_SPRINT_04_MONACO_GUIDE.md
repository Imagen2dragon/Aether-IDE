# AETHERIDE — MONACO EDITOR INTEGRATION GUIDE
## Professional Editor Architecture, Local Bundling, Multi-Tab Document Management and LSP Binding

**Sprint:** 04  
**Document Version:** 1.0.0  
**Target Platform:** Windows 10/11 x64 (Native Win32 / WebView2 Host & Electron)  
**Editor Engine:** Microsoft Monaco Editor `v0.49.0` (AMD distribution)

---

## 1. Architectural Overview

Monaco Editor replaces the earlier static `<pre>` text display in the AetherIDE workbench. In Sprint 04, the editor was integrated with the following architectural goals:

1. **Zero Runtime Network/CDN Requests:**  
   The complete AMD distribution of Monaco Editor is bundled directly within the workspace under `apps/desktop/workbench/vs/` and mirrored in the distribution folder `dist/windows-x64/workbench/vs/`. All language definition files, syntaxes, codicon fonts, and web workers (`editor.worker.js`, `ts.worker.js`, `css.worker.js`, `html.worker.js`, `json.worker.js`) are served locally.
2. **Multi-Model Document Buffer Management:**  
   Every opened file receives an isolated `monaco.editor.ITextModel`. The editor dynamically swaps the active model upon tab selection while maintaining scroll positions, view states, cursor locations, and undo/redo histories.
3. **Rust Language Server Protocol (LSP) Bridge:**  
   Standard Monaco language provider hooks (`registerCompletionItemProvider`, `registerHoverProvider`, `registerDefinitionProvider`) communicate through the secure native bridge with the Tokio-based `aether-daemon`, triggering high-performance LSP responses from `rust-analyzer` or other multiplexed language servers.

---

## 2. Directory Layout & Local Assets

```
apps/desktop/workbench/
├── index.html                   # Workbench main UI, Monaco mount container, & bridge logic
├── vs/                          # Self-contained Monaco Editor minified distribution
│   ├── loader.js                # RequireJS AMD loader for Monaco
│   ├── editor/                  # Monaco core editor styling and scripts
│   │   ├── editor.main.css      # Core editor themes and layout styles
│   │   ├── editor.main.js       # Core Monaco API definitions
│   │   └── editor.main.nls.*.js # Localization strings
│   ├── base/                    # Codicons, workers, and browser primitives
│   │   ├── browser/ui/codicons/ # VS Code icon font assets (woff/ttf)
│   │   └── worker/workerMain.js # Background web worker entrypoint
│   ├── basic-languages/         # Syntax definitions (Rust, Python, TS, C++, TOML, JSON, etc.)
│   └── language/                # Advanced language features (TypeScript, JSON, CSS, HTML)
```

### Worker Configuration

To avoid CORS restrictions and cross-origin worker instantiation issues in WebView2 and local `file://` contexts, worker paths are configured using local data URIs and AMD loader paths:

```javascript
window.MonacoEnvironment = {
  getWorkerUrl: function (workerId, label) {
    if (label === 'json') {
      return './vs/language/json/jsonWorker.js';
    }
    if (label === 'css' || label === 'scss' || label === 'less') {
      return './vs/language/css/cssWorker.js';
    }
    if (label === 'html' || label === 'handlebars' || label === 'razor') {
      return './vs/language/html/htmlWorker.js';
    }
    if (label === 'typescript' || label === 'javascript') {
      return './vs/language/typescript/tsWorker.js';
    }
    return './vs/base/worker/workerMain.js';
  }
};
```

---

## 3. Multi-Tab Document Management

The Document Manager manages active open tabs, active document tracking, dirty markers, and model lifecycle.

### State Model

```javascript
const documentManager = {
  models: new Map(), // path -> monaco.editor.ITextModel
  viewStates: new Map(), // path -> monaco.editor.ICodeEditorViewState
  activePath: null,
  dirtyFiles: new Set(),
};
```

### Document Operations

- **`openDocument(path, content, language)`:**
  1. Checks if a model already exists in `models`. If not, creates one via `monaco.editor.createModel(content, language, monaco.Uri.file(path))`.
  2. Saves the view state of the previously active document.
  3. Sets `editor.setModel(model)` and restores the saved view state if available.
  4. Renders or activates the corresponding UI tab with filename, icon, and close button.
  5. Updates the breadcrumbs bar: `workspace > folder > filename`.
  6. Sends `LspOpenDocument` through the native bridge to notify the language server.
- **`switchTab(path)`:**
  Swaps the active model, restores cursor position and view state, updates active tab styling, and refreshes the status bar.
- **`closeTab(path)`:**
  Prompts to save if marked dirty, removes the tab from DOM, disposes the Monaco model if not needed, and switches to the nearest open tab.
- **`saveActiveDocument()` (`Ctrl+S`):**
  Retrieves text from `editor.getValue()`, submits `FileWrite` via the native bridge (which performs an atomic disk write), clears the dirty indicator, and notifies the daemon via `LspChangeDocument`.

---

## 4. Monaco + Rust LSP Integration

### 4.1 Auto-Completion (`LspCompletion`)

Registered via `monaco.languages.registerCompletionItemProvider('rust', ...)`:
- Invoked on trigger characters (`.`, `:`, `>`, ` `).
- Transmits `{ path, line: position.lineNumber, character: position.column }` to the native bridge.
- Maps returned `CompletionItem` array to Monaco completion items with `monaco.languages.CompletionItemKind` (Class, Function, Struct, Module, Variable).

### 4.2 Hover Tooltips (`LspHover`)

Registered via `monaco.languages.registerHoverProvider('rust', ...)`:
- Invoked when the user hovers over any symbol.
- Dispatches `LspHover` request with position coordinates.
- Returns formatted Markdown strings rendered in the Monaco hover popup.

### 4.3 Definition Jump (`LspDefinition`)

Registered via `monaco.languages.registerDefinitionProvider('rust', ...)`:
- Invoked on `F12` or `Ctrl+Click`.
- Dispatches `LspDefinition` request.
- Returns target URI and line/column range, automatically navigating to the definition target.

### 4.4 Live Diagnostic Markers (`LspPublishDiagnostics`)

- Listens for diagnostic events pushed from the language server.
- Translates LSP diagnostic severity levels (`1 = Error`, `2 = Warning`, `3 = Information`, `4 = Hint`) into Monaco marker severities (`monaco.MarkerSeverity.Error`, etc.).
- Calls `monaco.editor.setModelMarkers(model, "aether-lsp", markers)` to render squiggly underlines and error tooltips.

---

## 5. Keyboard Shortcuts & User Controls

| Keybinding | Command | Description |
|---|---|---|
| `Ctrl+S` | `saveDocument` | Saves current file via atomic write and clears dirty flag |
| `Ctrl+P` | `quickOpen` | Opens Quick Open palette to search and switch between files |
| `Ctrl+Shift+P` | `commandPalette` | Displays full AetherIDE Command Palette |
| `F12` | `goToDefinition` | Jumps to symbol definition using Rust LSP |
| `Alt+F12` | `peekDefinition` | Peeks symbol definition in inline editor |
| `Ctrl+/` | `toggleComment` | Toggles line comments based on active language |
| `Ctrl+F` | `find` | Opens Monaco built-in find widget |
| `Ctrl+H` | `replace` | Opens Monaco replace widget |
| `Ctrl+` `\`` | `toggleTerminal` | Focuses the integrated ConPTY Windows terminal |
