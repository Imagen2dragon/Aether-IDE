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
		{ id: 'createIOSProject', label: vscode.l10n.t('Create iOS Project'), icon: 'add', command: true },
		{ id: 'validateIOSProject', label: vscode.l10n.t('Validate iOS Project'), icon: 'check' },
		{ id: 'buildIOS', label: vscode.l10n.t('Build iOS App With xtool'), icon: 'package', args: ['dev', 'build'] },
		{ id: 'deployIOS', label: vscode.l10n.t('Build and Install on iOS Device'), icon: 'play', args: ['dev'] },
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
	context.subscriptions.push(vscode.commands.registerCommand('aether.mobile.createIOSProject', () => createIOSProject(creatingProjects)));
	context.subscriptions.push(vscode.commands.registerCommand('aether.mobile.validateIOSProject', validateIOSWorkspace));
	const creatingProjects = new Map();
	context.subscriptions.push(vscode.tasks.onDidEndTaskProcess(async ({ execution, exitCode }) => {
		const project = creatingProjects.get(execution.task);
		if (!project) {
			return;
		}
		creatingProjects.delete(execution.task);
		if (exitCode !== 0) {
			void vscode.window.showErrorMessage(exitCode === undefined
				? vscode.l10n.t('iOS project creation stopped. Check the xtool task terminal.')
				: vscode.l10n.t('iOS project creation failed with exit code {0}. Check the xtool task terminal.', exitCode));
			return;
		}
		try {
			const result = await inspectIOSProject(project.uri);
			if (!result.valid) {
				void vscode.window.showErrorMessage(vscode.l10n.t('xtool finished, but the project is missing required files: {0}', result.missing.join(', ')));
				return;
			}
			void vscode.window.showInformationMessage(vscode.l10n.t('Created iOS project "{0}".', project.name));
			await vscode.commands.executeCommand('vscode.openFolder', project.uri, { forceNewWindow: false });
		} catch (error) {
			void vscode.window.showErrorMessage(vscode.l10n.t('Project was created, but AetherIDE could not open it: {0}', error.message));
		}
	}));
	const pending = new Set();
	for (const action of actions.filter(action => action.args)) {
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
				const folder = ['buildIOS', 'deployIOS'].includes(action.id) ? await selectProject() : undefined;
				if (['buildIOS', 'deployIOS'].includes(action.id) && !folder) {
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

async function createIOSProject(creatingProjects) {
	if (!vscode.workspace.isTrusted) {
		void vscode.window.showWarningMessage(vscode.l10n.t('Trust this workspace before running xtool.'));
		return;
	}
	const input = await vscode.window.showInputBox({
		title: vscode.l10n.t('Create iOS Project'),
		prompt: vscode.l10n.t('Choose a Swift package name. AetherIDE creates the project from xtool’s template.'),
		placeHolder: 'MyApp',
		validateInput: value => /^[A-Za-z][A-Za-z0-9]*$/.test(value.trim()) ? undefined : vscode.l10n.t('Use letters and numbers only, starting with a letter.')
	});
	if (!input) {
		return;
	}
	const name = input.trim();
	const parents = await vscode.window.showOpenDialog({
		canSelectFiles: false,
		canSelectFolders: true,
		canSelectMany: false,
		defaultUri: vscode.workspace.workspaceFolders?.[0]?.uri,
		openLabel: vscode.l10n.t('Choose Project Location'),
		title: vscode.l10n.t('Choose a parent folder for {0}', name)
	});
	const parent = parents?.[0];
	if (!parent) {
		return;
	}
	const projectUri = vscode.Uri.joinPath(parent, name);
	try {
		await vscode.workspace.fs.stat(projectUri);
		void vscode.window.showWarningMessage(vscode.l10n.t('A folder named "{0}" already exists in that location. Choose another name or location.', name));
		return;
	} catch (error) {
		if (!(error instanceof vscode.FileSystemError) || error.code !== 'FileNotFound') {
			void vscode.window.showErrorMessage(vscode.l10n.t('Unable to check the project destination: {0}', error.message));
			return;
		}
	}
	const task = new vscode.Task({ type, operation: 'createIOSProject' }, vscode.TaskScope.Global,
		vscode.l10n.t('Create iOS Project: {0}', name), 'Mobile Studio', createExecution(['new', name], parent.fsPath));
	task.presentationOptions = { reveal: vscode.TaskRevealKind.Always, panel: vscode.TaskPanelKind.Dedicated };
	creatingProjects.set(task, { name, uri: projectUri });
	try {
		await vscode.tasks.executeTask(task);
	} catch (error) {
		creatingProjects.delete(task);
		void vscode.window.showErrorMessage(vscode.l10n.t('Unable to start xtool project creation: {0}', error.message));
	}
}

async function validateIOSWorkspace() {
	const folders = vscode.workspace.workspaceFolders ?? [];
	if (!folders.length) {
		void vscode.window.showWarningMessage(vscode.l10n.t('Open an iOS project folder before validating it.'));
		return;
	}
	let folder = folders[0];
	if (folders.length > 1) {
		const selected = await vscode.window.showQuickPick(folders.map(candidate => ({ label: candidate.name, description: candidate.uri.fsPath, folder: candidate })), {
			placeHolder: vscode.l10n.t('Select the iOS project to validate')
		});
		folder = selected?.folder;
		if (!folder) {
			return;
		}
	}
	try {
		const result = await inspectIOSProject(folder.uri);
		if (!result.valid) {
			void vscode.window.showWarningMessage(vscode.l10n.t('This folder is not a complete xtool project. Missing: {0}', result.missing.join(', ')));
			return;
		}
		void vscode.window.showInformationMessage(result.hasBuildServerConfig
			? vscode.l10n.t('iOS project is ready: Package.swift, xtool.yml, and SourceKit-LSP configuration are present.')
			: vscode.l10n.t('iOS project is valid. Add .bsp/xtool.json to enable xtool build information in the editor.'));
	} catch (error) {
		void vscode.window.showErrorMessage(vscode.l10n.t('Unable to validate the iOS project: {0}', error.message));
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

async function isFile(uri) {
	try {
		const stat = await vscode.workspace.fs.stat(uri);
		return (stat.type & vscode.FileType.File) !== 0;
	} catch (error) {
		if (error instanceof vscode.FileSystemError && error.code === 'FileNotFound') {
			return false;
		}
		throw error;
	}
}

async function inspectIOSProject(uri) {
	const [hasPackageManifest, hasXtoolConfig, hasBuildServerConfig] = await Promise.all([
		isFile(vscode.Uri.joinPath(uri, 'Package.swift')),
		isFile(vscode.Uri.joinPath(uri, 'xtool.yml')),
		isFile(vscode.Uri.joinPath(uri, '.bsp', 'xtool.json'))
	]);
	const missing = [];
	if (!hasPackageManifest) {
		missing.push('Package.swift');
	}
	if (!hasXtoolConfig) {
		missing.push('xtool.yml');
	}
	return { valid: missing.length === 0, missing, hasBuildServerConfig };
}

async function findIOSProjects() {
	let count = 0;
	for (const folder of vscode.workspace.workspaceFolders ?? []) {
		try {
			const project = await inspectIOSProject(folder.uri);
			if (project.valid) {
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
			const project = await inspectIOSProject(folder.uri);
			if (project.valid) {
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
