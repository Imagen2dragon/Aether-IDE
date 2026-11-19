/*---------------------------------------------------------------------------------------------
 *  Copyright (c) Microsoft Corporation. All rights reserved.
 *  Licensed under the MIT License. See License.txt in the project root for license information.
 *--------------------------------------------------------------------------------------------*/

const path = require('node:path');
const os = require('node:os');
const vscode = require('vscode');

const taskType = 'aether-android';

function activate(context) {
	const actions = [
		{ id: 'debugApk', label: vscode.l10n.t('Build Debug APK'), icon: 'package', gradleTask: 'assembleDebug' },
		{ id: 'releaseBundle', label: vscode.l10n.t('Build Release App Bundle'), icon: 'archive', gradleTask: 'bundleRelease' },
		{ id: 'listDevices', label: vscode.l10n.t('List Android Devices'), icon: 'device-mobile' }
	];

	context.subscriptions.push(vscode.window.registerTreeDataProvider('aether.mobile.android', {
		getChildren: () => actions,
		getTreeItem: action => {
			const item = new vscode.TreeItem(action.label);
			item.iconPath = new vscode.ThemeIcon(action.icon);
			item.command = { command: 'aether.mobile.' + action.id, title: action.label };
			return item;
		}
	}));

	const executions = new Map();
	context.subscriptions.push(vscode.tasks.onDidEndTask(({ execution }) => {
		const group = execution.task.definition.operation === 'listDevices' ? 'devices' : 'build';
		if (executions.get(group) === execution) {
			executions.delete(group);
		}
	}));

	for (const action of actions) {
		context.subscriptions.push(vscode.commands.registerCommand('aether.mobile.' + action.id, async () => {
			if (!vscode.workspace.isTrusted) {
				void vscode.window.showWarningMessage(vscode.l10n.t('Trust this workspace before running Android tools.'));
				return;
			}
			const group = action.gradleTask ? 'build' : 'devices';
			if (executions.has(group) || vscode.tasks.taskExecutions.some(execution =>
				execution.task.definition.type === taskType && (execution.task.definition.operation !== 'listDevices') === (group === 'build'))) {
				void vscode.window.showInformationMessage(vscode.l10n.t('An Android task is already running. Use the Tasks menu to terminate it.'));
				return;
			}
			executions.set(group, undefined);
			try {
				const task = action.gradleTask ? await createBuildTask(action) : await createDeviceTask(action);
				if (task) {
					const execution = await vscode.tasks.executeTask(task);
					if (vscode.tasks.taskExecutions.includes(execution)) {
						executions.set(group, execution);
					}
				}
			} catch (error) {
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
	const task = new vscode.Task({ type: taskType, operation: action.id }, project.folder, action.label, 'Mobile Studio', execution);
	task.group = vscode.TaskGroup.Build;
	return task;
}

async function createDeviceTask(action) {
	const executable = process.platform === 'win32' ? 'adb.exe' : 'adb';
	const defaultSdk = process.platform === 'win32'
		? process.env.LOCALAPPDATA && path.join(process.env.LOCALAPPDATA, 'Android', 'Sdk')
		: process.platform === 'darwin' ? path.join(os.homedir(), 'Library', 'Android', 'sdk') : path.join(os.homedir(), 'Android', 'Sdk');
	let adb;
	for (const sdk of new Set([process.env.ANDROID_HOME, process.env.ANDROID_SDK_ROOT, defaultSdk].filter(Boolean))) {
		const candidate = path.join(sdk, 'platform-tools', executable);
		if (await isFile(vscode.Uri.file(candidate))) {
			adb = candidate;
			break;
		}
	}
	const execution = new vscode.ProcessExecution(adb ?? executable, ['devices', '-l']);
	return new vscode.Task({ type: taskType, operation: action.id }, vscode.TaskScope.Global, action.label, 'Mobile Studio', execution);
}

module.exports = { activate };
