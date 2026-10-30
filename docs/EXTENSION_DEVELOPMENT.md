# AetherIDE — Extension Development Guide

## 1. Extension Model
AetherIDE maintains 100% compatibility with the VS Code / Code - OSS extension standard and Open VSX marketplace while exposing rich native extensions via the Aether Core IPC bridge.

---

## 2. Creating an Aether Extension
An Aether extension is a standard TypeScript extension with access to `@types/vscode` and the `DaemonClient` IPC protocol:
```typescript
import * as vscode from 'vscode';
import { DaemonClient } from './daemonClient';

export function activate(context: vscode.ExtensionContext) {
  const daemon = new DaemonClient();
  
  const cmd = vscode.commands.registerCommand('myext.analyze', async () => {
    // Send request directly to Rust native core
    const result = await daemon.sendRequest('IndexWorkspace', { workspace_path: '.' });
    vscode.window.showInformationMessage(`Indexed ${result.files_scanned} files in Rust!`);
  });
  
  context.subscriptions.push(cmd);
}
```

---

## 3. Custom Webview Views
Extensions can contribute panels to the activity bar (`package.json` -> `contributes.viewsContainers`) and render interactive HTML/CSS/JS interfaces that communicate bi-directionally with the extension host and daemon.
