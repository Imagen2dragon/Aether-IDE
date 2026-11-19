/*---------------------------------------------------------------------------------------------
 *  Copyright (c) Microsoft Corporation. All rights reserved.
 *  Licensed under the MIT License. See License.txt in the project root for license information.
 *--------------------------------------------------------------------------------------------*/

const vscode = require('vscode');

const type = 'aether-xtool';

function activate(context) {
	const actions = [
		{ id: 'buildIOS', label: vscode.l10n.t('Build iOS App With xtool'), icon: 'package', args: ['dev', 'build'] },
		{ id: 'listIOSDevices', label: vscode.l10n.t('List iOS Devices'), icon: 'device-mobile', args: ['devices'] }
	];
	context.subscriptions.push(vscode.window.registerTreeDataProvider('aether.mobile.ios', {
		getChildren: () => actions,
		getTreeItem: action => {
			const item = new vscode.TreeItem(action.label);
			item.iconPath = new vscode.ThemeIcon(action.icon);
			item.command = { command: 'aether.mobile.' + action.id, title: action.label };
			return item;
		}
	}));
	const pending = new Set();
	for (const action of actions) {
		context.subscriptions.push(vscode.commands.registerCommand('aether.mobile.' + action.id, async () => {
			if (!vscode.workspace.isTrusted) {
				void vscode.window.showWarningMessage(vscode.l10n.t('Trust this workspace before running xtool.'));
				return;
			}
			if (pending.has(action.id) || vscode.tasks.taskExecutions.some(({ task }) =>
				task.definition.type === type && task.definition.operation === action.id)) {
				void vscode.window.showInformationMessage(vscode.l10n.t('This xtool task is already running.'));
				return;
			}
			pending.add(action.id);
			try {
				const folder = action.id === 'buildIOS' ? await selectProject() : undefined;
				if (action.id === 'buildIOS' && !folder) {
					return;
				}
				const task = new vscode.Task({ type, operation: action.id }, folder ?? vscode.TaskScope.Global,
					action.label, 'Mobile Studio', createExecution(action.args, folder?.uri.fsPath));
				if (folder) {
					task.group = vscode.TaskGroup.Build;
				}
				await vscode.tasks.executeTask(task);
			} catch (error) {
				void vscode.window.showErrorMessage(vscode.l10n.t('Unable to start xtool: {0}', error.message));
			} finally {
				pending.delete(action.id);
			}
		}));
	}
}

function createExecution(args, projectPath) {
	if (process.platform !== 'win32') {
		return new vscode.ProcessExecution('xtool', args, projectPath ? { cwd: projectPath } : undefined);
	}
	const distribution = vscode.workspace.getConfiguration('aether.mobile').get('wslDistribution', '').trim();
	const wslArgs = distribution ? ['--distribution', distribution] : [];
	if (projectPath) {
		wslArgs.push('--cd', projectPath);
	}
	return new vscode.ProcessExecution('wsl.exe', [...wslArgs, '--exec', 'xtool', ...args]);
}

async function selectProject() {
	const options = [];
	for (const folder of vscode.workspace.workspaceFolders ?? []) {
		try {
			const config = await vscode.workspace.fs.stat(vscode.Uri.joinPath(folder.uri, 'xtool.yml'));
			const manifest = await vscode.workspace.fs.stat(vscode.Uri.joinPath(folder.uri, 'Package.swift'));
			if ((config.type & vscode.FileType.File) && (manifest.type & vscode.FileType.File)) {
				options.push({ label: folder.name, description: folder.uri.fsPath, folder });
			}
		} catch (error) {
			if (!(error instanceof vscode.FileSystemError) || error.code !== 'FileNotFound') {
				throw error;
			}
		}
	}
	if (!options.length) {
		void vscode.window.showWarningMessage(vscode.l10n.t('Open an xtool project folder containing xtool.yml and Package.swift.'));
		return;
	}
	if (options.length === 1) {
		return options[0].folder;
	}
	return (await vscode.window.showQuickPick(options, { placeHolder: vscode.l10n.t('Select the iOS project to build') }))?.folder;
}

module.exports = { activate };
