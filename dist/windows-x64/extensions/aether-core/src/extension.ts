import * as vscode from 'vscode';
import { DaemonClient } from './daemonClient';
import { AgentViewProvider } from './agentViewProvider';
import { MobileStudioProvider } from './mobileStudioProvider';
import { BrainViewProvider } from './brainViewProvider';

export function activate(context: vscode.ExtensionContext) {
  console.log('[Aether] Activating Aether AI Engineering Core extension...');

  const daemonClient = new DaemonClient();

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
  const agentProvider = new AgentViewProvider(context.extensionUri, daemonClient);
  context.subscriptions.push(
    vscode.window.registerWebviewViewProvider('aether.agentView', agentProvider)
  );

  const mobileProvider = new MobileStudioProvider(context.extensionUri, daemonClient);
  context.subscriptions.push(
    vscode.window.registerWebviewViewProvider('aether.mobileStudioView', mobileProvider)
  );

  const brainProvider = new BrainViewProvider(context.extensionUri, daemonClient);
  context.subscriptions.push(
    vscode.window.registerWebviewViewProvider('aether.brainView', brainProvider)
  );

  // Register Commands
  context.subscriptions.push(
    vscode.commands.registerCommand('aether.openChat', () => {
      vscode.commands.executeCommand('workbench.view.extension.aether-activity-bar');
    })
  );

  context.subscriptions.push(
    vscode.commands.registerCommand('aether.runDoctor', async () => {
      try {
        const report = await daemonClient.sendRequest('RunDoctorCheck');
        vscode.window.showInformationMessage(
          `Environment Doctor: All Healthy? ${report.all_healthy ? 'YES ✅' : 'ATTENTION ⚠️'}`
        );
      } catch (e: any) {
        vscode.window.showErrorMessage(`Doctor check failed: ${e.message}`);
      }
    })
  );

  context.subscriptions.push(
    vscode.commands.registerCommand('aether.inspectImpact', async () => {
      vscode.commands.executeCommand('workbench.view.extension.aether-brain-bar');
    })
  );

  context.subscriptions.push(
    vscode.commands.registerCommand('aether.mobileScaffold', async () => {
      vscode.commands.executeCommand('workbench.view.extension.aether-mobile-bar');
    })
  );
}

export function deactivate() {
  console.log('[Aether] Deactivated Aether AI Engineering Core extension');
}
