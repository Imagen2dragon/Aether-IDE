"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.AgentViewProvider = void 0;
const vscode = require("vscode");
class AgentViewProvider {
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
                case 'spawnAgent': {
                    try {
                        const result = await this._daemonClient.sendRequest('CreateAgentTask', {
                            role: data.role,
                            objective: data.objective,
                            autonomy_mode: data.mode || 'assist',
                        });
                        this._view?.webview.postMessage({ type: 'taskCreated', task: result });
                        vscode.window.showInformationMessage(`Agent [${data.role}] task dispatched.`);
                    }
                    catch (e) {
                        vscode.window.showErrorMessage(`Agent spawn failed: ${e.message}`);
                    }
                    break;
                }
                case 'fetchStatus': {
                    try {
                        const status = await this._daemonClient.sendRequest('GetStatus');
                        this._view?.webview.postMessage({ type: 'statusUpdate', status });
                    }
                    catch (e) {
                        // Daemon not connected yet
                    }
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
    select, input, textarea, button { width: 100%; box-sizing: border-box; margin-bottom: 10px; padding: 6px 8px; background: var(--vscode-input-background); color: var(--vscode-input-foreground); border: 1px solid var(--vscode-input-border); border-radius: 4px; font-family: inherit; }
    button { background: var(--vscode-button-background); color: var(--vscode-button-foreground); cursor: pointer; border: none; font-weight: 600; padding: 8px; }
    button:hover { background: var(--vscode-button-hoverBackground); }
    .status-badge { display: inline-block; padding: 3px 8px; border-radius: 12px; font-size: 11px; font-weight: bold; background: #1E293B; color: #38BDF8; margin-bottom: 12px; }
    .task-card { background: var(--vscode-editor-background); border: 1px solid var(--vscode-widget-border); border-radius: 6px; padding: 10px; margin-top: 12px; }
    .role-tag { font-weight: bold; color: var(--vscode-textLink-foreground); font-size: 12px; }
    .step-log { font-family: var(--vscode-editor-font-family); font-size: 11px; background: rgba(0,0,0,0.2); padding: 6px; border-radius: 4px; max-height: 120px; overflow-y: auto; white-space: pre-wrap; margin-top: 6px; }
  </style>
</head>
<body>
  <div class="status-badge" id="daemon-status">Daemon: Connecting...</div>
  <h3>Autonomous Agent Team</h3>
  <label>Role:</label>
  <select id="agent-role">
    <option value="architect">Principal Software Architect</option>
    <option value="frontend">Frontend Developer</option>
    <option value="backend">Backend Developer</option>
    <option value="database">Database Engineer</option>
    <option value="mobile">Mobile Developer (React Native/Expo)</option>
    <option value="systems">Rust/C++ Systems Engineer</option>
    <option value="debugging">Debugging Specialist</option>
    <option value="testing">Testing Specialist</option>
    <option value="security">Security Reviewer</option>
    <option value="reviewer">Code Reviewer</option>
    <option value="devops">DevOps Engineer</option>
    <option value="researcher">Technical Researcher</option>
    <option value="documentation">Documentation Engineer</option>
    <option value="performance">Performance Engineer</option>
  </select>

  <label>Autonomy Mode:</label>
  <select id="autonomy-mode">
    <option value="observe">Observe Mode (Read-only)</option>
    <option value="assist" selected>Assist Mode (Require Approval)</option>
    <option value="autopilot">Autopilot Mode (Autonomous)</option>
  </select>

  <label>Task Objective:</label>
  <textarea id="task-objective" rows="3" placeholder="Describe the feature, bug fix, or refactor to execute..."></textarea>

  <button id="dispatch-btn">Dispatch Autonomous Task</button>

  <div id="tasks-container"></div>

  <script>
    const vscode = acquireVsCodeApi();
    const dispatchBtn = document.getElementById('dispatch-btn');
    const tasksContainer = document.getElementById('tasks-container');
    const statusBadge = document.getElementById('daemon-status');

    setInterval(() => {
      vscode.postMessage({ type: 'fetchStatus' });
    }, 5000);
    vscode.postMessage({ type: 'fetchStatus' });

    dispatchBtn.addEventListener('click', () => {
      const role = document.getElementById('agent-role').value;
      const mode = document.getElementById('autonomy-mode').value;
      const objective = document.getElementById('task-objective').value.trim();
      if (!objective) return;

      vscode.postMessage({ type: 'spawnAgent', role, mode, objective });
      document.getElementById('task-objective').value = '';
    });

    window.addEventListener('message', (event) => {
      const msg = event.data;
      if (msg.type === 'statusUpdate') {
        statusBadge.textContent = 'Daemon: v' + msg.status.version + ' (' + msg.status.models_count + ' Models Active)';
        statusBadge.style.color = '#4ADE80';
      } else if (msg.type === 'taskCreated') {
        const task = msg.task;
        const card = document.createElement('div');
        card.className = 'task-card';
        card.innerHTML = '<div class="role-tag">' + task.role.toUpperCase() + ' — ' + task.status + '</div>' +
          '<div>' + task.objective + '</div>' +
          '<div class="step-log">Task ID: ' + task.task_id + '\\nSteps: ' + (task.steps ? task.steps.length : 0) + '</div>';
        tasksContainer.prepend(card);
      }
    });
  </script>
</body>
</html>`;
    }
}
exports.AgentViewProvider = AgentViewProvider;
//# sourceMappingURL=agentViewProvider.js.map