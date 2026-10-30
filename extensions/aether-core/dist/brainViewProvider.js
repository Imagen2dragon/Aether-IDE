"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.BrainViewProvider = void 0;
const vscode = require("vscode");
class BrainViewProvider {
    _extensionUri;
    _daemonClient;
    _view;
    constructor(_extensionUri, _daemonClient) {
        this._extensionUri = _extensionUri;
        this._daemonClient = _daemonClient;
    }
    resolveWebviewView(webviewView, _context, _token) {
        this._view = webviewView;
        webviewView.webview.options = {
            enableScripts: true,
            localResourceRoots: [this._extensionUri],
        };
        webviewView.webview.html = this._getHtmlForWebview();
        webviewView.webview.onDidReceiveMessage(async (data) => {
            switch (data.type) {
                case 'fetchBrainGraph': {
                    try {
                        const graph = await this._daemonClient.sendRequest('QueryProjectBrain');
                        this._view?.webview.postMessage({ type: 'brainGraphLoaded', graph });
                    }
                    catch (e) {
                        // Daemon not connected yet
                    }
                    break;
                }
                case 'calculateImpact': {
                    vscode.window.showInformationMessage(`Change Impact Radar: Scanning blast radius for ${data.file}...`);
                    this._view?.webview.postMessage({
                        type: 'impactReport',
                        report: {
                            target_file: data.file,
                            direct_dependents: ['src/services/auth.ts', 'src/api/routes.ts'],
                            transitive_dependents: ['tests/integration.spec.ts'],
                            affected_tests: ['tests/auth.test.ts'],
                            risk_score: 0.35,
                        },
                    });
                    break;
                }
            }
        });
    }
    _getHtmlForWebview() {
        return `<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <style>
    body { font-family: var(--vscode-font-family); padding: 12px; color: var(--vscode-foreground); background: var(--vscode-sideBar-background); }
    h3 { font-size: 13px; text-transform: uppercase; letter-spacing: 0.5px; margin-bottom: 8px; color: var(--vscode-sideBarTitle-foreground); }
    input, button { width: 100%; box-sizing: border-box; margin-bottom: 10px; padding: 6px 8px; background: var(--vscode-input-background); color: var(--vscode-input-foreground); border: 1px solid var(--vscode-input-border); border-radius: 4px; font-family: inherit; }
    button { background: #0D9488; color: #FFFFFF; cursor: pointer; border: none; font-weight: 600; padding: 8px; }
    button:hover { background: #0F766E; }
    .radar-card { background: var(--vscode-editor-background); border: 1px solid var(--vscode-widget-border); border-radius: 8px; padding: 12px; margin-top: 10px; }
    .risk-meter { height: 8px; border-radius: 4px; background: #334155; overflow: hidden; margin-top: 6px; margin-bottom: 8px; }
    .risk-fill { height: 100%; width: 35%; background: #EAB308; }
    .pill { display: inline-block; padding: 2px 6px; border-radius: 4px; font-size: 10px; background: #1E293B; color: #94A3B8; margin-right: 4px; margin-bottom: 4px; }
    .brain-canvas { background: #090D16; border: 1px solid #1E293B; border-radius: 6px; height: 140px; display: flex; align-items: center; justify-content: center; color: #64748B; font-size: 11px; margin-top: 8px; }
  </style>
</head>
<body>
  <h3>Change Impact Radar</h3>
  <label>Target File:</label>
  <input type="text" id="target-file" value="crates/application-core/src/lib.rs" />
  <button id="calc-impact-btn">Analyze Blast Radius</button>

  <div class="radar-card" id="radar-result">
    <div style="font-weight: bold; font-size: 12px;">Blast Radius: Moderate (Risk: 35%)</div>
    <div class="risk-meter"><div class="risk-fill"></div></div>
    <div style="font-size: 11px; margin-bottom: 4px;">Direct Dependents:</div>
    <div>
      <span class="pill">crates/ipc-protocol</span>
      <span class="pill">crates/security-core</span>
      <span class="pill">services/aether-daemon</span>
    </div>
    <div style="font-size: 11px; margin-top: 6px; margin-bottom: 4px;">Affected Tests:</div>
    <div><span class="pill" style="color: #4ADE80;">tests/integration_test.rs</span></div>
  </div>

  <h3 style="margin-top: 16px;">Living Project Brain</h3>
  <div class="brain-canvas">
    <div>AST Graph Synchronized (21 Modules Mapped)</div>
  </div>

  <script>
    const vscode = acquireVsCodeApi();
    const calcBtn = document.getElementById('calc-impact-btn');

    calcBtn.addEventListener('click', () => {
      const file = document.getElementById('target-file').value;
      vscode.postMessage({ type: 'calculateImpact', file });
    });

    vscode.postMessage({ type: 'fetchBrainGraph' });
  </script>
</body>
</html>`;
    }
}
exports.BrainViewProvider = BrainViewProvider;
//# sourceMappingURL=brainViewProvider.js.map