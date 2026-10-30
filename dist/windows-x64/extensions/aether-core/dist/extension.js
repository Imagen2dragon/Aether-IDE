"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.activate = activate;
exports.deactivate = deactivate;
const vscode = require("vscode");
const daemonClient_1 = require("./daemonClient");
const agentViewProvider_1 = require("./agentViewProvider");
const mobileStudioProvider_1 = require("./mobileStudioProvider");
const brainViewProvider_1 = require("./brainViewProvider");
function activate(context) {
    console.log('[Aether] Activating Aether AI Engineering Core extension...');
    const daemonClient = new daemonClient_1.DaemonClient();
    // Status bar indicator
    const statusBar = vscode.window.createStatusBarItem(vscode.StatusBarAlignment.Right, 100);
    statusBar.text = '$(zap) Aether: Connecting';
    statusBar.tooltip = 'Aether Native Rust Engine Status';
    statusBar.show();
    context.subscriptions.push(statusBar);
    // Connect to daemon
    daemonClient.connect().then(() => {
        statusBar.text = '$(zap) Aether: Active';
        statusBar.color = '#38BDF8';
    }).catch(() => {
        statusBar.text = '$(warning) Aether: Daemon Offline';
    });
    // Register Webview Views
    const agentProvider = new agentViewProvider_1.AgentViewProvider(context.extensionUri, daemonClient);
    context.subscriptions.push(vscode.window.registerWebviewViewProvider('aether.agentView', agentProvider));
    const mobileProvider = new mobileStudioProvider_1.MobileStudioProvider(context.extensionUri, daemonClient);
    context.subscriptions.push(vscode.window.registerWebviewViewProvider('aether.mobileStudioView', mobileProvider));
    const brainProvider = new brainViewProvider_1.BrainViewProvider(context.extensionUri, daemonClient);
    context.subscriptions.push(vscode.window.registerWebviewViewProvider('aether.brainView', brainProvider));
    // Register Commands
    context.subscriptions.push(vscode.commands.registerCommand('aether.openChat', () => {
        vscode.commands.executeCommand('workbench.view.extension.aether-activity-bar');
    }));
    context.subscriptions.push(vscode.commands.registerCommand('aether.runDoctor', async () => {
        try {
            const report = await daemonClient.sendRequest('RunDoctorCheck');
            vscode.window.showInformationMessage(`Environment Doctor: All Healthy? ${report.all_healthy ? 'YES ✅' : 'ATTENTION ⚠️'}`);
        }
        catch (e) {
            vscode.window.showErrorMessage(`Doctor check failed: ${e.message}`);
        }
    }));
    context.subscriptions.push(vscode.commands.registerCommand('aether.inspectImpact', async () => {
        vscode.commands.executeCommand('workbench.view.extension.aether-brain-bar');
    }));
    context.subscriptions.push(vscode.commands.registerCommand('aether.mobileScaffold', async () => {
        vscode.commands.executeCommand('workbench.view.extension.aether-mobile-bar');
    }));
}
function deactivate() {
    console.log('[Aether] Deactivated Aether AI Engineering Core extension');
}
//# sourceMappingURL=extension.js.map