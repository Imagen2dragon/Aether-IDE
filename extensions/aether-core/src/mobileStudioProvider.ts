import * as vscode from 'vscode';
import { DaemonClient } from './daemonClient';

export class MobileStudioProvider implements vscode.WebviewViewProvider {
  private _view?: vscode.WebviewView;

  constructor(
    private readonly _extensionUri: vscode.Uri,
    private readonly _daemonClient: DaemonClient
  ) {}

  public resolveWebviewView(
    webviewView: vscode.WebviewView,
    _context: vscode.WebviewViewResolveContext,
    _token: vscode.CancellationToken
  ) {
    this._view = webviewView;
    webviewView.webview.options = {
      enableScripts: true,
      localResourceRoots: [this._extensionUri],
    };

    webviewView.webview.html = this._getHtmlForWebview();

    webviewView.webview.onDidReceiveMessage(async (data) => {
      switch (data.type) {
        case 'scaffoldMobileApp': {
          try {
            const resp = await this._daemonClient.sendRequest('GenerateMobileApp', {
              app_name: data.appName,
              target_framework: data.framework,
              prompt: data.prompt || 'Mobile application',
            });
            vscode.window.showInformationMessage(`Mobile App [${data.appName}] scaffolded successfully!`);
            this._view?.webview.postMessage({ type: 'scaffoldComplete', project: resp });
          } catch (e: any) {
            vscode.window.showErrorMessage(`Mobile scaffolding failed: ${e.message}`);
          }
          break;
        }
        case 'triggerBuild': {
          try {
            const resp = await this._daemonClient.sendRequest('TriggerMobileBuild', {
              project_path: data.projectPath || '.',
              platform: data.platform,
              build_type: data.buildType || 'preview',
            });
            vscode.window.showInformationMessage(`Cloud EAS build queued for ${data.platform.toUpperCase()}`);
            this._view?.webview.postMessage({ type: 'buildQueued', job: resp });
          } catch (e: any) {
            vscode.window.showErrorMessage(`Cloud build trigger failed: ${e.message}`);
          }
          break;
        }
      }
    });
  }

  private _getHtmlForWebview(): string {
    return `<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <style>
    body { font-family: var(--vscode-font-family); padding: 12px; color: var(--vscode-foreground); background: var(--vscode-sideBar-background); }
    h3 { font-size: 13px; text-transform: uppercase; letter-spacing: 0.5px; margin-bottom: 8px; color: var(--vscode-sideBarTitle-foreground); }
    input, select, button { width: 100%; box-sizing: border-box; margin-bottom: 10px; padding: 6px 8px; background: var(--vscode-input-background); color: var(--vscode-input-foreground); border: 1px solid var(--vscode-input-border); border-radius: 4px; font-family: inherit; }
    button { background: #2563EB; color: #FFFFFF; cursor: pointer; border: none; font-weight: 600; padding: 8px; }
    button:hover { background: #1D4ED8; }
    .device-preview-card { background: #020617; border: 2px solid #334155; border-radius: 18px; padding: 16px; margin-top: 14px; text-align: center; }
    .device-screen { background: #0F172A; border-radius: 12px; padding: 20px 10px; min-height: 180px; display: flex; flex-direction: column; align-items: center; justify-content: center; }
    .device-screen-title { font-size: 14px; font-weight: bold; color: #F8FAFC; margin-bottom: 6px; }
    .device-screen-sub { font-size: 11px; color: #94A3B8; margin-bottom: 12px; }
    .preview-btn { background: #3B82F6; color: white; padding: 6px 14px; border-radius: 6px; font-size: 12px; }
    .build-actions { display: flex; gap: 8px; margin-top: 10px; }
    .build-actions button { flex: 1; font-size: 11px; padding: 6px; }
  </style>
</head>
<body>
  <h3>Mobile App Studio</h3>
  <label>App Name:</label>
  <input type="text" id="app-name" value="AetherCommerceApp" placeholder="MyAppName" />

  <label>Framework Engine:</label>
  <select id="framework-select">
    <option value="react-native-expo" selected>React Native + Expo (TypeScript + Router)</option>
    <option value="flutter">Flutter (Dart + Material 3)</option>
    <option value="native-android">Native Android (Kotlin + Compose)</option>
    <option value="native-ios">Native iOS (Swift + SwiftUI)</option>
  </select>

  <button id="scaffold-btn">Scaffold Production Mobile Project</button>

  <h3>Visual Device Canvas</h3>
  <div class="device-preview-card">
    <div style="font-size: 10px; color: #64748B; margin-bottom: 6px;">iPhone 16 Pro Frame Preview</div>
    <div class="device-screen">
      <div class="device-screen-title" id="preview-title">Welcome to Aether App</div>
      <div class="device-screen-sub">Expo Router Active</div>
      <div class="preview-btn">Interactive Screen</div>
    </div>
  </div>

  <h3 style="margin-top: 14px;">EAS Cloud Build Center</h3>
  <div class="build-actions">
    <button id="build-android-btn" style="background: #16A34A;">Build Android APK</button>
    <button id="build-ios-btn" style="background: #9333EA;">Build iOS IPA</button>
  </div>
  <div id="build-status-box" style="font-size: 11px; color: #94A3B8; margin-top: 6px;"></div>

  <script>
    const vscode = acquireVsCodeApi();
    const scaffoldBtn = document.getElementById('scaffold-btn');
    const buildAndroidBtn = document.getElementById('build-android-btn');
    const buildIosBtn = document.getElementById('build-ios-btn');
    const buildStatusBox = document.getElementById('build-status-box');

    scaffoldBtn.addEventListener('click', () => {
      const appName = document.getElementById('app-name').value;
      const framework = document.getElementById('framework-select').value;
      vscode.postMessage({ type: 'scaffoldMobileApp', appName, framework });
    });

    buildAndroidBtn.addEventListener('click', () => {
      vscode.postMessage({ type: 'triggerBuild', platform: 'android', buildType: 'preview' });
    });

    buildIosBtn.addEventListener('click', () => {
      vscode.postMessage({ type: 'triggerBuild', platform: 'ios', buildType: 'preview' });
    });

    window.addEventListener('message', (event) => {
      const msg = event.data;
      if (msg.type === 'scaffoldComplete') {
        document.getElementById('preview-title').textContent = msg.project.app_name;
        buildStatusBox.textContent = 'Project scaffolded at: ' + msg.project.project_path;
      } else if (msg.type === 'buildQueued') {
        buildStatusBox.innerHTML = 'Job ID: <b>' + msg.job.build_id + '</b> (' + msg.job.platform.toUpperCase() + ') - ' + msg.job.status;
      }
    });
  </script>
</body>
</html>`;
  }
}
