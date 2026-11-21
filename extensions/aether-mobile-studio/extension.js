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
const { registerTestResults } = require('./testResults.js');

const taskType = 'aether-android';
const execFileAsync = promisify(execFile);

function taskGroup(operation) {
	if (operation === 'debugApk' || operation === 'releaseBundle') {
		return 'build';
	}
	if (operation === 'unitTests' || operation === 'instrumentedTests') {
		return 'tests';
	}
	if (operation === 'openLogcat') {
		return 'logcat';
	}
	return operation === 'startEmulator' ? 'emulator' : 'devices';
}

function activate(context) {
	const actions = [
		{ id: 'debugApk', label: vscode.l10n.t('Build Debug APK'), icon: 'package', gradleTask: 'assembleDebug', artifacts: '**/build/outputs/apk/**/debug/*.apk' },
		{ id: 'releaseBundle', label: vscode.l10n.t('Build Release App Bundle'), icon: 'archive', gradleTask: 'bundleRelease', artifacts: '**/build/outputs/bundle/{release,*Release}/*.aab' },
		{ id: 'unitTests', label: vscode.l10n.t('Run Unit Tests'), icon: 'beaker', gradleTask: 'testDebugUnitTest', test: true },
		{ id: 'instrumentedTests', label: vscode.l10n.t('Run Instrumented Tests'), icon: 'device-mobile', gradleTask: 'connectedDebugAndroidTest', test: true, requiresDevice: true },
		{ id: 'listDevices', label: vscode.l10n.t('List Android Devices'), icon: 'device-mobile' },
		{ id: 'startEmulator', label: vscode.l10n.t('Start Android Emulator'), icon: 'vm' },
		{ id: 'installApk', label: vscode.l10n.t('Install APK on Device'), icon: 'cloud-download' },
		{ id: 'launchApp', label: vscode.l10n.t('Launch App on Device'), icon: 'play' },
		{ id: 'openLogcat', label: vscode.l10n.t('Open Android Logcat'), icon: 'output' }
	];
	const buildOutput = registerBuildOutput(context, taskType);
	const outputAction = { id: 'openBuildOutput', label: vscode.l10n.t('Open Build Output'), icon: 'output' };
	const testResults = registerTestResults(context, taskType);
	const testResultsAction = { id: 'openTestResults', label: vscode.l10n.t('Open Test Results'), icon: 'beaker' };
	const treeChanges = new vscode.EventEmitter();
	const toolchain = { id: 'toolchain', label: vscode.l10n.t('Toolchain'), description: vscode.l10n.t('SDK, ADB, Java, Gradle'), icon: 'server', collapsibleState: vscode.TreeItemCollapsibleState.Collapsed };
	let toolchainRows;
	const refreshToolchain = async () => {
		toolchainRows = await collectToolchain();
		toolchain.description = vscode.l10n.t('{0} of {1} ready', toolchainRows.filter(row => row.icon === 'check').length, toolchainRows.length);
		treeChanges.fire(toolchain);
	};
	context.subscriptions.push(treeChanges);
	context.subscriptions.push(vscode.window.registerTreeDataProvider('aether.mobile.android', {
		onDidChangeTreeData: treeChanges.event,
		getChildren: async element => {
			if (element?.id === toolchain.id) {
				if (!toolchainRows) {
					await refreshToolchain();
				}
				return [
					{ id: 'checkEnvironment', label: vscode.l10n.t('Refresh Toolchain Check'), icon: 'refresh', command: true },
					...toolchainRows
				];
			}
			return [toolchain, ...actions, outputAction, testResultsAction];
		},
		getTreeItem: action => {
			const item = new vscode.TreeItem(action.label, action.collapsibleState);
			if (action.description) {
				item.description = action.description;
			}
			item.iconPath = new vscode.ThemeIcon(action.icon);
			item.tooltip = action.tooltip;
			if (action.command || action.id !== toolchain.id && action.kind !== 'diagnostic') {
				item.command = { command: 'aether.mobile.' + action.id, title: action.label };
			}
			return item;
		}
	}));
	context.subscriptions.push(vscode.commands.registerCommand('aether.mobile.checkEnvironment', refreshToolchain));

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
					if (build?.artifacts) {
						run = buildOutput.track(task, build.root, action.artifacts);
					} else if (build && action.test) {
						run = testResults.track(task, build.root);
					}
					const execution = await vscode.tasks.executeTask(task);
					if (vscode.tasks.taskExecutions.includes(execution)) {
						executions.set(group, execution);
					}
				}
			} catch (error) {
				if (run) {
					if (action.test) {
						testResults.failedToStart(run, error.message);
					} else {
						buildOutput.failedToStart(run, error.message);
					}
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
	if (action.requiresDevice) {
		const adb = await findAdb();
		const result = await runTool(adb, ['devices', '-l']);
		if (!result.available) {
			void vscode.window.showWarningMessage(vscode.l10n.t('ADB is required for instrumented tests. Install Android SDK Platform Tools first.'));
			return;
		}
		if (!result.output.split(/\r?\n/).some(line => /^\S+\s+device(?:\s|$)/.test(line))) {
			const startEmulator = vscode.l10n.t('Start Emulator');
			const choice = await vscode.window.showWarningMessage(vscode.l10n.t('Connect an Android device or start an emulator before running instrumented tests.'), startEmulator);
			if (choice === startEmulator) {
				await vscode.commands.executeCommand('aether.mobile.startEmulator');
			}
			return;
		}
	}
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
	const definition = { type: taskType, operation: action.id };
	if (action.artifacts || action.test) {
		definition.runId = randomUUID();
	}
	const task = new vscode.Task(definition, project.folder, action.label, 'Mobile Studio', execution);
	task.group = action.test ? vscode.TaskGroup.Test : vscode.TaskGroup.Build;
	task.presentationOptions = { panel: vscode.TaskPanelKind.Dedicated, showReuseMessage: false };
	return { task, root: project.root };
}

async function findAdb() {
	const sdk = await findSdk();
	const executable = process.platform === 'win32' ? 'adb.exe' : 'adb';
	const candidate = sdk && path.join(sdk, 'platform-tools', executable);
	return candidate && await isFile(vscode.Uri.file(candidate)) ? candidate : executable;
}

async function findSdk() {
	for (const sdk of await getSdkCandidates()) {
		try {
			const stat = await vscode.workspace.fs.stat(vscode.Uri.file(sdk));
			if (stat.type & vscode.FileType.Directory) {
				return sdk;
			}
		} catch (error) {
			if (!(error instanceof vscode.FileSystemError) || error.code !== 'FileNotFound') {
				throw error;
			}
		}
	}
	return undefined;
}

async function getSdkCandidates() {
	const defaultSdk = process.platform === 'win32'
		? process.env.LOCALAPPDATA && path.join(process.env.LOCALAPPDATA, 'Android', 'Sdk')
		: process.platform === 'darwin' ? path.join(os.homedir(), 'Library', 'Android', 'sdk') : path.join(os.homedir(), 'Android', 'Sdk');
	const candidates = [];
	for (const folder of vscode.workspace.workspaceFolders ?? []) {
		for (const root of [folder.uri, vscode.Uri.joinPath(folder.uri, 'android')]) {
			const properties = vscode.Uri.joinPath(root, 'local.properties');
			if (await isFile(properties)) {
				const contents = (await vscode.workspace.fs.readFile(properties)).toString();
				const sdkDir = contents.match(/^\s*sdk\.dir\s*[:=]\s*(.+?)\s*$/m)?.[1]
					?.replace(/\\\\/g, '\\').replace(/\\:/g, ':');
				if (sdkDir) {
					candidates.push(path.isAbsolute(sdkDir) ? sdkDir : path.resolve(root.fsPath, sdkDir));
				}
			}
		}
	}
	candidates.push(process.env.ANDROID_HOME, process.env.ANDROID_SDK_ROOT, defaultSdk);
	const seen = new Set();
	return candidates.filter(candidate => {
		if (!candidate) {
			return false;
		}
		const key = process.platform === 'win32' ? path.resolve(candidate).toLowerCase() : path.resolve(candidate);
		if (seen.has(key)) {
			return false;
		}
		seen.add(key);
		return true;
	});
}

async function collectToolchain() {
	const sdk = await findSdk();
	const adb = await findAdb();
	const emulatorName = process.platform === 'win32' ? 'emulator.exe' : 'emulator';
	const emulatorCandidate = sdk && path.join(sdk, 'emulator', emulatorName);
	const emulator = emulatorCandidate && await isFile(vscode.Uri.file(emulatorCandidate)) ? emulatorCandidate : 'emulator';
	const javaName = process.platform === 'win32' ? 'java.exe' : 'java';
	const javacName = process.platform === 'win32' ? 'javac.exe' : 'javac';
	const javaHome = process.env.JAVA_HOME;
	const javaHomeExecutable = javaHome && path.join(javaHome, 'bin', javaName);
	const javacHomeExecutable = javaHome && path.join(javaHome, 'bin', javacName);
	const java = javaHomeExecutable && await isFile(vscode.Uri.file(javaHomeExecutable)) ? javaHomeExecutable : 'java';
	const javac = javacHomeExecutable && await isFile(vscode.Uri.file(javacHomeExecutable)) ? javacHomeExecutable : 'javac';
	const [adbResult, emulatorVersion, avdResult, javaResult, javacResult, gradle] = await Promise.all([
		runTool(adb, ['version']),
		runTool(emulator, ['-version']),
		runTool(emulator, ['-list-avds']),
		runTool(java, ['-version']),
		runTool(javac, ['-version']),
		findGradleWrapper()
	]);
	const deviceResult = adbResult.available ? await runTool(adb, ['devices', '-l']) : undefined;
	const avds = avdResult.available ? avdResult.output.split(/\r?\n/).map(name => name.trim()).filter(name => /^[\w.-]+$/.test(name)) : [];
	const devices = deviceResult?.available ? deviceResult.output.split(/\r?\n/).filter(line => /^\S+\s+device(?:\s|$)/.test(line)).length : 0;
	const javaVersion = javaResult.available ? javaResult.output.match(/version\s+"?([^"\s]+)/i)?.[1] ?? 'Detected' : undefined;
	const javacVersion = javacResult.available ? javacResult.output.match(/javac\s+([^\s]+)/i)?.[1] ?? 'Detected' : undefined;
	const adbVersion = adbResult.available ? adbResult.output.match(/Version\s+([^\s]+)/i)?.[1] ?? 'Detected' : undefined;
	const emulatorVersionText = emulatorVersion.available ? emulatorVersion.output.match(/version\s+([^\s]+)/i)?.[1] ?? 'Detected' : undefined;
	return [
		diagnostic(vscode.l10n.t('Android SDK'), sdk ?? vscode.l10n.t('Not found'), sdk ? vscode.l10n.t('SDK location: {0}', sdk) : vscode.l10n.t('Set ANDROID_HOME or ANDROID_SDK_ROOT'), Boolean(sdk)),
		diagnostic(vscode.l10n.t('ADB Platform Tools'), adbVersion ? vscode.l10n.t('ADB {0}', adbVersion) : vscode.l10n.t('Not found'), adbVersion ? adb : vscode.l10n.t('Install Android SDK Platform Tools'), Boolean(adbVersion)),
		diagnostic(vscode.l10n.t('Android Emulator'), emulatorVersionText ? vscode.l10n.t('Emulator {0}', emulatorVersionText) : vscode.l10n.t('Not installed'), avds.length ? vscode.l10n.t('{0} virtual devices: {1}', avds.length, avds.join(', ')) : emulatorVersionText ? vscode.l10n.t('No AVD found. Create one in Device Manager.') : vscode.l10n.t('Install Android Emulator and create an AVD in Device Manager'), Boolean(emulatorVersionText && avds.length)),
		diagnostic(vscode.l10n.t('Java Development Kit'), javaVersion && javacVersion ? vscode.l10n.t('Java {0} · JDK {1}', javaVersion, javacVersion) : vscode.l10n.t('Incomplete'), javaVersion && javacVersion ? vscode.l10n.t('JDK location: {0}', javaHome ?? vscode.l10n.t('PATH')) : vscode.l10n.t('Install a JDK and set JAVA_HOME or add java and javac to PATH'), Boolean(javaVersion && javacVersion)),
		diagnostic(vscode.l10n.t('Gradle Wrapper'), gradle ?? vscode.l10n.t('Not found'), gradle ?? vscode.l10n.t('Open an Android Gradle project at its root or in android/'), Boolean(gradle)),
		diagnostic(vscode.l10n.t('Connected Devices'), devices ? vscode.l10n.t('{0} connected', devices) : vscode.l10n.t('None connected'), devices ? adb : vscode.l10n.t('Connect a device or start an AVD'), devices > 0)
	];
}

function diagnostic(label, status, description, ready) {
	return {
		id: 'diagnostic-' + label,
		label,
		description: status,
		tooltip: description,
		icon: ready ? 'check' : 'warning',
		kind: 'diagnostic'
	};
}

async function findGradleWrapper() {
	const wrapper = process.platform === 'win32' ? 'gradlew.bat' : 'gradlew';
	for (const folder of vscode.workspace.workspaceFolders ?? []) {
		for (const root of [folder.uri, vscode.Uri.joinPath(folder.uri, 'android')]) {
			if (!(await isFile(vscode.Uri.joinPath(root, wrapper))) ||
				!(await isFile(vscode.Uri.joinPath(root, 'settings.gradle'))) && !(await isFile(vscode.Uri.joinPath(root, 'settings.gradle.kts')))) {
				continue;
			}
			const properties = vscode.Uri.joinPath(root, 'gradle', 'wrapper', 'gradle-wrapper.properties');
			if (await isFile(properties)) {
				const contents = (await vscode.workspace.fs.readFile(properties)).toString();
				const version = contents.match(/^\s*distributionUrl\s*=\s*.*?gradle-([\d.]+)(?:-(?:all|bin))?\.zip/m)?.[1];
				return vscode.l10n.t('{0} · Gradle {1}', vscode.workspace.asRelativePath(root, true), version ?? vscode.l10n.t('version not declared'));
			}
			return vscode.workspace.asRelativePath(root, true);
		}
	}
	return undefined;
}

async function runTool(executable, args) {
	try {
		const { stdout, stderr } = await execFileAsync(executable, args, { windowsHide: true, timeout: 10000, maxBuffer: 256 * 1024 });
		return { available: true, output: `${stdout}\n${stderr}`.trim() };
	} catch (error) {
		if (error.code === 'ENOENT') {
			return { available: false, output: '' };
		}
		const output = `${error.stdout ?? ''}\n${error.stderr ?? ''}`.trim();
		return { available: Boolean(output), output };
	}
}

async function createDeviceTask(action, artifact) {
	const adb = await findAdb();
	if (action.id === 'listDevices') {
		const execution = new vscode.ProcessExecution(adb, ['devices', '-l']);
		return new vscode.Task({ type: taskType, operation: action.id }, vscode.TaskScope.Global, action.label, 'Mobile Studio', execution);
	}
	if (action.id === 'startEmulator') {
		return createEmulatorTask(action);
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

async function createEmulatorTask(action) {
	const sdk = await findSdk();
	const executableName = process.platform === 'win32' ? 'emulator.exe' : 'emulator';
	const emulator = sdk ? path.join(sdk, 'emulator', executableName) : executableName;
	let stdout;
	try {
		({ stdout } = await execFileAsync(emulator, ['-list-avds'], { windowsHide: true, timeout: 10000, maxBuffer: 256 * 1024 }));
	} catch (error) {
		if (error.code === 'ENOENT') {
			void vscode.window.showWarningMessage(vscode.l10n.t('Android Emulator is not installed. Add it with Android SDK Manager.'));
			return;
		}
		throw error;
	}
	const avds = stdout.split(/\r?\n/).map(name => name.trim()).filter(name => /^[\w.-]+$/.test(name));
	if (!avds.length) {
		void vscode.window.showWarningMessage(vscode.l10n.t('No virtual device is configured. Create an AVD in Device Manager.'));
		return;
	}
	const avd = await vscode.window.showQuickPick(avds, { title: action.label, placeHolder: vscode.l10n.t('Select a virtual device to start') });
	if (!avd) {
		return;
	}
	const execution = new vscode.ProcessExecution(emulator, ['-avd', avd]);
	const task = new vscode.Task({ type: taskType, operation: action.id }, vscode.TaskScope.Global, vscode.l10n.t('Start Emulator: {0}', avd), 'Mobile Studio', execution);
	task.presentationOptions = { panel: vscode.TaskPanelKind.Dedicated, showReuseMessage: false };
	return task;
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
		if (rows.length) {
			void vscode.window.showWarningMessage(vscode.l10n.t('Android device not ready. Authorize USB debugging or wait for the emulator to start.'));
		} else {
			const startEmulator = vscode.l10n.t('Start Emulator');
			const action = await vscode.window.showWarningMessage(vscode.l10n.t('No Android device is connected. Connect USB or start an emulator.'), startEmulator);
			if (action === startEmulator) {
				await vscode.commands.executeCommand('aether.mobile.startEmulator');
			}
		}
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
