/*---------------------------------------------------------------------------------------------
 *  Copyright (c) Microsoft Corporation. All rights reserved.
 *  Licensed under the MIT License. See License.txt in the project root for license information.
 *--------------------------------------------------------------------------------------------*/

const vscode = require('vscode');
const { execFile } = require('node:child_process');
const { promisify } = require('node:util');

const type = 'aether-xtool';
const execFileAsync = promisify(execFile);

function activate(context) {
	const actions = [
		{ id: 'buildIOS', label: vscode.l10n.t('Build iOS App With xtool'), icon: 'package', args: ['dev', 'build'] },
		{ id: 'listIOSDevices', label: vscode.l10n.t('List iOS Devices'), icon: 'device-mobile', args: ['devices'] }
	];
	const toolchain = { id: 'toolchain', label: vscode.l10n.t('iOS Toolchain'), icon: 'server', collapsibleState: vscode.TreeItemCollapsibleState.Collapsed };
	const treeChanges = new vscode.EventEmitter();
	let toolchainRows;
	const refreshToolchain = async () => {
		toolchainRows = await collectToolchain();
		const tools = toolchainRows.filter(row => row.kind !== 'project');
		toolchain.description = vscode.l10n.t('{0} of {1} ready', tools.filter(row => row.ready).length, tools.length);
		treeChanges.fire(toolchain);
	};
	context.subscriptions.push(treeChanges);
	context.subscriptions.push(vscode.window.registerTreeDataProvider('aether.mobile.ios', {
		onDidChangeTreeData: treeChanges.event,
		getChildren: async element => {
			if (element?.id === toolchain.id) {
				if (!toolchainRows) {
					await refreshToolchain();
				}
				return [
					{ id: 'checkIOSToolchain', label: vscode.l10n.t('Refresh iOS Toolchain Check'), icon: 'refresh', command: true },
					...toolchainRows
				];
			}
			return [toolchain, ...actions];
		},
		getTreeItem: action => {
			const item = new vscode.TreeItem(action.label);
			if (action.collapsibleState !== undefined) {
				item.collapsibleState = action.collapsibleState;
			}
			if (action.description) {
				item.description = action.description;
			}
			item.iconPath = new vscode.ThemeIcon(action.icon);
			item.tooltip = action.tooltip;
			if (action.command || action.id !== toolchain.id && !action.kind) {
				item.command = { command: 'aether.mobile.' + action.id, title: action.label };
			}
			return item;
		}
	}));
	context.subscriptions.push(vscode.commands.registerCommand('aether.mobile.checkIOSToolchain', refreshToolchain));
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

async function collectToolchain() {
	const xtool = await runXtool(['--help']);
	const swift = await runXtool(['swift', '--version']);
	const sdk = await runXtool(['swift', 'sdk', 'list']);
	const usbmuxd = await runXtool(['usbmuxd', '--help']);
	const projectCount = await findIOSProjects();
	const swiftVersion = swift.available ? swift.output.match(/Swift version\s+([^\s(]+)/i)?.[1] : undefined;
	const hasDarwinSdk = sdk.available && /\bdarwin\b/i.test(sdk.output);
	return [
		diagnostic(vscode.l10n.t('xtool CLI'), xtool.available ? vscode.l10n.t('Installed') : vscode.l10n.t('Not found'), xtool.available ? vscode.l10n.t('Command: {0}', xtool.command) : vscode.l10n.t('Install xtool in the selected WSL distribution or PATH.'), xtool.available),
		diagnostic(vscode.l10n.t('Swift Toolchain'), swiftVersion ? vscode.l10n.t('Swift {0}', swiftVersion) : vscode.l10n.t('Not found'), swiftVersion ? swift.command : vscode.l10n.t('Install the Swift toolchain in the xtool environment.'), Boolean(swiftVersion)),
		diagnostic(vscode.l10n.t('iOS SDK'), hasDarwinSdk ? vscode.l10n.t('Darwin SDK ready') : vscode.l10n.t('Not installed'), hasDarwinSdk ? sdk.command : vscode.l10n.t('Configure xtool with an iOS SDK, then verify with swift sdk list.'), Boolean(hasDarwinSdk)),
		diagnostic(vscode.l10n.t('USB Device Bridge'), usbmuxd.available ? vscode.l10n.t('usbmuxd ready') : vscode.l10n.t('Not found'), usbmuxd.available ? usbmuxd.command : vscode.l10n.t('Install usbmuxd in the xtool environment and enable USB passthrough on Windows.'), usbmuxd.available),
		diagnostic(vscode.l10n.t('iOS Project'), projectCount ? vscode.l10n.t('{0} project(s) found', projectCount) : vscode.l10n.t('Not found'), projectCount ? vscode.l10n.t('Projects must contain xtool.yml and Package.swift.') : vscode.l10n.t('Open a folder with xtool.yml and Package.swift.'), projectCount > 0, 'project')
	];
}

async function runXtool(args) {
	const invocation = createCommand(args);
	try {
		const { stdout, stderr } = await execFileAsync(invocation.executable, invocation.args, { windowsHide: true, timeout: 15000, maxBuffer: 512 * 1024 });
		return { available: true, output: `${stdout}\n${stderr}`.trim(), command: invocation.description };
	} catch (error) {
		if (error.code === 'ENOENT') {
			return { available: false, output: '', command: invocation.description };
		}
		return { available: false, output: `${error.stdout ?? ''}\n${error.stderr ?? ''}`.trim(), command: invocation.description };
	}
}

function createCommand(args, projectPath) {
	if (process.platform !== 'win32') {
		return { executable: args[0], args: args.slice(1), description: vscode.l10n.t('PATH') };
	}
	const distribution = vscode.workspace.getConfiguration('aether.mobile').get('wslDistribution', '').trim();
	const wslArgs = distribution ? ['--distribution', distribution] : [];
	if (projectPath) {
		wslArgs.push('--cd', projectPath);
	}
	return {
		executable: 'wsl.exe',
		args: [...wslArgs, '--exec', ...args],
		description: distribution || vscode.l10n.t('Default WSL distribution')
	};
}

function createExecution(args, projectPath) {
	const command = createCommand(['xtool', ...args], projectPath);
	const options = process.platform === 'win32' || !projectPath ? undefined : { cwd: projectPath };
	return new vscode.ProcessExecution(command.executable, command.args, options);
}

function diagnostic(label, description, tooltip, ready, kind = 'diagnostic') {
	return { id: 'ios-diagnostic-' + label, label, description, tooltip, icon: ready ? 'check' : 'warning', kind, ready };
}

async function findIOSProjects() {
	let count = 0;
	for (const folder of vscode.workspace.workspaceFolders ?? []) {
		try {
			const [config, manifest] = await Promise.all([
				vscode.workspace.fs.stat(vscode.Uri.joinPath(folder.uri, 'xtool.yml')),
				vscode.workspace.fs.stat(vscode.Uri.joinPath(folder.uri, 'Package.swift'))
			]);
			if ((config.type & vscode.FileType.File) && (manifest.type & vscode.FileType.File)) {
				count++;
			}
		} catch (error) {
			if (!(error instanceof vscode.FileSystemError) || error.code !== 'FileNotFound') {
				throw error;
			}
		}
	}
	return count;
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
