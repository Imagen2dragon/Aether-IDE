/*---------------------------------------------------------------------------------------------
 *  Copyright (c) Microsoft Corporation. All rights reserved.
 *  Licensed under the MIT License. See License.txt in the project root for license information.
 *--------------------------------------------------------------------------------------------*/

const path = require('node:path');
const os = require('node:os');
const { randomUUID } = require('node:crypto');
const { execFile } = require('node:child_process');
const { promisify } = require('node:util');
const vscode = require('vscode');
const { registerBuildOutput } = require('./buildOutput.js');

const taskType = 'aether-android';
const execFileAsync = promisify(execFile);

function taskGroup(operation) {
	if (operation === 'debugApk' || operation === 'releaseBundle') {
		return 'build';
	}
	return operation === 'openLogcat' ? 'logcat' : 'devices';
}

function activate(context) {
	const actions = [
		{ id: 'debugApk', label: vscode.l10n.t('Build Debug APK'), icon: 'package', gradleTask: 'assembleDebug', artifacts: '**/build/outputs/apk/**/debug/*.apk' },
		{ id: 'releaseBundle', label: vscode.l10n.t('Build Release App Bundle'), icon: 'archive', gradleTask: 'bundleRelease', artifacts: '**/build/outputs/bundle/{release,*Release}/*.aab' },
		{ id: 'listDevices', label: vscode.l10n.t('List Android Devices'), icon: 'device-mobile' },
		{ id: 'installApk', label: vscode.l10n.t('Install APK on Device'), icon: 'cloud-download' },
		{ id: 'launchApp', label: vscode.l10n.t('Launch App on Device'), icon: 'play' },
		{ id: 'openLogcat', label: vscode.l10n.t('Open Android Logcat'), icon: 'output' }
	];
	const buildOutput = registerBuildOutput(context, taskType);
	const outputAction = { id: 'openBuildOutput', label: vscode.l10n.t('Open Build Output'), icon: 'output' };

	context.subscriptions.push(vscode.window.registerTreeDataProvider('aether.mobile.android', {
		getChildren: () => [...actions, outputAction],
		getTreeItem: action => {
			const item = new vscode.TreeItem(action.label);
			item.iconPath = new vscode.ThemeIcon(action.icon);
			item.command = { command: 'aether.mobile.' + action.id, title: action.label };
			return item;
		}
	}));

	const executions = new Map();
	context.subscriptions.push(vscode.tasks.onDidEndTask(({ execution }) => {
		if (execution.task.definition.type !== taskType) {
			return;
		}
		const group = taskGroup(execution.task.definition.operation);
		if (executions.get(group) === execution) {
			executions.delete(group);
		}
	}));

	for (const action of actions) {
		context.subscriptions.push(vscode.commands.registerCommand('aether.mobile.' + action.id, async entry => {
			if (!vscode.workspace.isTrusted) {
				void vscode.window.showWarningMessage(vscode.l10n.t('Trust this workspace before running Android tools.'));
				return;
			}
			const group = taskGroup(action.id);
			if (executions.has(group) || vscode.tasks.taskExecutions.some(execution =>
				execution.task.definition.type === taskType && taskGroup(execution.task.definition.operation) === group)) {
				void vscode.window.showInformationMessage(vscode.l10n.t('An Android task is already running. Use the Tasks menu to terminate it.'));
				return;
			}
			executions.set(group, undefined);
			let run;
			try {
				const build = action.gradleTask ? await createBuildTask(action) : undefined;
				const task = action.gradleTask ? build?.task : await createDeviceTask(action, entry?.uri);
				if (task) {
					if (build) {
						run = buildOutput.track(task, build.root, action.artifacts);
					}
					const execution = await vscode.tasks.executeTask(task);
					if (vscode.tasks.taskExecutions.includes(execution)) {
						executions.set(group, execution);
					}
				}
			} catch (error) {
				if (run) {
					buildOutput.failedToStart(run, error.message);
				}
				void vscode.window.showErrorMessage(vscode.l10n.t('Unable to start Android task: {0}', error.message));
			} finally {
				if (!executions.get(group)) {
					executions.delete(group);
				}
			}
		}));
	}
}

async function isFile(uri) {
	try {
		const stat = await vscode.workspace.fs.stat(uri);
		return Boolean(stat.type & vscode.FileType.File);
	} catch (error) {
		if (error instanceof vscode.FileSystemError && error.code === 'FileNotFound') {
			return false;
		}
		throw error;
	}
}

async function createBuildTask(action) {
	const wrapper = process.platform === 'win32' ? 'gradlew.bat' : 'gradlew';
	const projects = [];
	for (const folder of vscode.workspace.workspaceFolders ?? []) {
		for (const root of [folder.uri, vscode.Uri.joinPath(folder.uri, 'android')]) {
			const launcher = vscode.Uri.joinPath(root, wrapper);
			if (await isFile(launcher) &&
				(await isFile(vscode.Uri.joinPath(root, 'settings.gradle')) || await isFile(vscode.Uri.joinPath(root, 'settings.gradle.kts')))) {
				projects.push({ label: vscode.workspace.asRelativePath(root, true), description: root.fsPath, folder, root, launcher });
			}
		}
	}
	if (!projects.length) {
		void vscode.window.showWarningMessage(vscode.l10n.t('Open an Android project with a Gradle wrapper at the workspace root or in its android folder.'));
		return;
	}
	const project = projects.length === 1 ? projects[0] : await vscode.window.showQuickPick(projects, {
		placeHolder: vscode.l10n.t('Select the Android project to build')
	});
	if (!project) {
		return;
	}
	const execution = new vscode.ShellExecution(project.launcher.fsPath, [action.gradleTask, '--console=plain'], {
		cwd: project.root.fsPath
	});
	const task = new vscode.Task({ type: taskType, operation: action.id, runId: randomUUID() }, project.folder, action.label, 'Mobile Studio', execution);
	task.group = vscode.TaskGroup.Build;
	task.presentationOptions = { panel: vscode.TaskPanelKind.Dedicated, showReuseMessage: false };
	return { task, root: project.root };
}

async function findAdb() {
	const executable = process.platform === 'win32' ? 'adb.exe' : 'adb';
	const defaultSdk = process.platform === 'win32'
		? process.env.LOCALAPPDATA && path.join(process.env.LOCALAPPDATA, 'Android', 'Sdk')
		: process.platform === 'darwin' ? path.join(os.homedir(), 'Library', 'Android', 'sdk') : path.join(os.homedir(), 'Android', 'Sdk');
	for (const sdk of new Set([process.env.ANDROID_HOME, process.env.ANDROID_SDK_ROOT, defaultSdk].filter(Boolean))) {
		const candidate = path.join(sdk, 'platform-tools', executable);
		if (await isFile(vscode.Uri.file(candidate))) {
			return candidate;
		}
	}
	return executable;
}

async function createDeviceTask(action, artifact) {
	const adb = await findAdb();
	if (action.id === 'listDevices') {
		const execution = new vscode.ProcessExecution(adb, ['devices', '-l']);
		return new vscode.Task({ type: taskType, operation: action.id }, vscode.TaskScope.Global, action.label, 'Mobile Studio', execution);
	}
	const device = await selectDevice(adb, action.label);
	if (!device) {
		return;
	}
	switch (action.id) {
		case 'installApk': return createInstallTask(adb, device, artifact);
		case 'launchApp': return createLaunchTask(adb, device);
		case 'openLogcat': return createLogcatTask(adb, device);
	}
}

async function selectDevice(adb, title) {
	let stdout;
	try {
		({ stdout } = await execFileAsync(adb, ['devices', '-l'], { windowsHide: true, timeout: 10000, maxBuffer: 256 * 1024 }));
	} catch (error) {
		if (error.code === 'ENOENT') {
			throw new Error(vscode.l10n.t('ADB was not found. Install Android SDK Platform Tools or add adb to PATH.'));
		}
		throw error;
	}
	const rows = stdout.split(/\r?\n/).map(line => line.match(/^(\S+)\s+(device|offline|unauthorized)(?:\s+(.*))?$/)).filter(Boolean);
	const devices = rows.filter(([, , state]) => state === 'device').map(([, serial, , details]) => {
		const model = details?.match(/(?:^|\s)model:(\S+)/)?.[1]?.replaceAll('_', ' ');
		return { label: model ?? serial, description: model ? serial : undefined, serial };
	});
	if (!devices.length) {
		const message = rows.length
			? vscode.l10n.t('No Android device is ready. Authorize USB debugging on the device or wait for the emulator to finish starting.')
			: vscode.l10n.t('No Android device or emulator is connected. Connect a device with USB debugging enabled or start an emulator.');
		void vscode.window.showWarningMessage(message);
		return;
	}
	return vscode.window.showQuickPick(devices, { title, placeHolder: vscode.l10n.t('Select an Android device') });
}

async function createInstallTask(adb, device, artifact) {
	const [apk] = artifact instanceof vscode.Uri ? [artifact] : await vscode.window.showOpenDialog({
		canSelectFiles: true,
		canSelectFolders: false,
		canSelectMany: false,
		defaultUri: vscode.workspace.workspaceFolders?.[0]?.uri,
		filters: { APK: ['apk'] },
		openLabel: vscode.l10n.t('Install APK on {0}', device.label)
	}) ?? [];
	if (!apk) {
		return;
	}
	if (!['file', 'vscode-remote'].includes(apk.scheme) || !path.isAbsolute(apk.fsPath) || !apk.path.toLowerCase().endsWith('.apk') || !await isFile(apk)) {
		throw new Error(vscode.l10n.t('Select an existing APK file on the build host.'));
	}
	const label = vscode.l10n.t('Install {0} on {1}', path.basename(apk.fsPath), device.label);
	return createTargetTask('installApk', label, adb, device, ['install', '-r', apk.fsPath]);
}

async function createLaunchTask(adb, device) {
	const validAppId = value => /^[a-zA-Z][a-zA-Z0-9_]*(?:\.[a-zA-Z][a-zA-Z0-9_]*)+$/.test(value);
	const input = await vscode.window.showInputBox({
		title: vscode.l10n.t('Launch App on {0}', device.label),
		prompt: vscode.l10n.t('Enter the application ID of an installed app with a launcher activity.'),
		placeHolder: 'com.example.app',
		validateInput: value => validAppId(value.trim()) ? undefined : vscode.l10n.t('Enter an application ID such as com.example.app.')
	});
	const appId = input?.trim();
	if (appId && validAppId(appId)) {
		return createTargetTask('launchApp', vscode.l10n.t('Launch {0} on {1}', appId, device.label), adb, device,
			['shell', 'am', 'start', '-W', '-a', 'android.intent.action.MAIN', '-c', 'android.intent.category.LAUNCHER', appId]);
	}
}

async function createLogcatTask(adb, device) {
	const severity = await vscode.window.showQuickPick([
		{ label: vscode.l10n.t('Info and above'), value: 'I' },
		{ label: vscode.l10n.t('Warnings and errors'), value: 'W' },
		{ label: vscode.l10n.t('Errors only'), value: 'E' },
		{ label: vscode.l10n.t('All levels'), value: 'V' }
	], { title: vscode.l10n.t('Logcat on {0}', device.label), placeHolder: vscode.l10n.t('Select the minimum log severity') });
	if (severity) {
		return createTargetTask('openLogcat', vscode.l10n.t('Logcat on {0} ({1}+)', device.label, severity.value), adb, device,
			['logcat', '-T', '1', '-v', 'threadtime', `*:${severity.value}`]);
	}
}

function createTargetTask(operation, label, adb, device, args) {
	const execution = new vscode.ProcessExecution(adb, ['-s', device.serial, ...args]);
	const task = new vscode.Task({ type: taskType, operation }, vscode.TaskScope.Global, label, 'Mobile Studio', execution);
	task.presentationOptions = { panel: vscode.TaskPanelKind.Dedicated, showReuseMessage: false };
	return task;
}

module.exports = { activate };
