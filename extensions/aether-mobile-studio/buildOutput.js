/* Copyright (c) Microsoft Corporation. Licensed under the MIT License. */
const vscode = require('vscode');

const viewId = 'aether.mobile.buildOutput';
const historyLimit = 20;

function registerBuildOutput(context, taskType) {
	const runs = new Map();
	const changes = new vscode.EventEmitter();
	const refresh = run => changes.fire(run);
	const getRun = execution => execution.task.definition.type === taskType ? runs.get(execution.task.definition.runId) : undefined;
	const provider = {
		onDidChangeTreeData: changes.event,
		getChildren(run) {
			if (!run) {
				return [...runs.values()].reverse();
			}
			if (run.kind !== 'build') {
				return [];
			}
			const items = run.artifacts.map(uri => ({ kind: 'artifact', uri }));
			if (run.note) {
				items.push({ kind: 'note', label: run.note });
			}
			return items;
		},
		getTreeItem(entry) {
			if (entry.kind === 'artifact') {
				const label = vscode.workspace.asRelativePath(entry.uri, false);
				const item = new vscode.TreeItem(label);
				item.tooltip = entry.uri.fsPath;
				item.iconPath = new vscode.ThemeIcon('package');
				if (entry.uri.path.toLowerCase().endsWith('.apk')) {
					item.contextValue = 'aether-apk-artifact';
				}
				item.command = { command: 'revealInExplorer', title: vscode.l10n.t('Reveal Artifact in Explorer'), arguments: [entry.uri] };
				return item;
			}
			if (entry.kind === 'note') {
				const item = new vscode.TreeItem(entry.label);
				item.tooltip = entry.label;
				return item;
			}
			const item = new vscode.TreeItem(entry.label, entry.artifacts.length || entry.note ? vscode.TreeItemCollapsibleState.Expanded : vscode.TreeItemCollapsibleState.None);
			item.id = entry.id;
			item.description = entry.elapsed === undefined ? entry.status : vscode.l10n.t('{0} · {1}s', entry.status, entry.elapsed);
			item.tooltip = vscode.l10n.t('{0}\n{1}\nFull logs are in the build task terminal.', item.description, entry.root.fsPath);
			item.iconPath = new vscode.ThemeIcon(entry.icon);
			return item;
		}
	};
	context.subscriptions.push(changes, vscode.window.registerTreeDataProvider(viewId, provider),
		vscode.commands.registerCommand('aether.mobile.openBuildOutput', () => vscode.commands.executeCommand(viewId + '.focus')),
		vscode.tasks.onDidStartTaskProcess(({ execution }) => {
			const run = getRun(execution);
			if (run) {
				run.started = Date.now();
				run.status = vscode.l10n.t('Running');
				run.icon = 'sync';
				refresh(run);
			}
		}),
		vscode.tasks.onDidEndTask(({ execution }) => {
			const run = getRun(execution);
			if (run && run.elapsed === undefined) {
				finish(run, vscode.l10n.t('Ended without an exit code'), 'circle-slash');
			}
		}),
		vscode.tasks.onDidEndTaskProcess(async ({ execution, exitCode }) => {
			const run = getRun(execution);
			if (!run) {
				return;
			}
			if (exitCode !== 0) {
				finish(run, exitCode === undefined ? vscode.l10n.t('Stopped') : vscode.l10n.t('Failed · Exit {0}', exitCode), 'error');
				return;
			}
			finish(run, vscode.l10n.t('Succeeded'), 'pass');
			try {
				run.artifacts = await vscode.workspace.findFiles(new vscode.RelativePattern(run.root, run.pattern), null, 32);
				run.note = run.artifacts.length ? undefined : vscode.l10n.t('No artifacts were found in the standard Gradle output folders.');
			} catch {
				run.note = vscode.l10n.t('Artifact discovery failed. Check the build terminal.');
			}
			refresh(run);
		}));

	function finish(run, status, icon) {
		run.status = status;
		run.icon = icon;
		run.elapsed = Math.round((Date.now() - run.started) / 1000);
		refresh(run);
	}

	return {
		track(task, root, pattern) {
			const run = {
				id: task.definition.runId, kind: 'build', root, pattern, label: task.name, artifacts: [],
				started: Date.now(), status: vscode.l10n.t('Queued'), icon: 'clock'
			};
			runs.set(run.id, run);
			if (runs.size > historyLimit) {
				runs.delete(runs.keys().next().value);
			}
			changes.fire(undefined);
			return run;
		},
		failedToStart(run, error) {
			run.note = error;
			finish(run, vscode.l10n.t('Could not start'), 'error');
		}
	};
}

module.exports = { registerBuildOutput };
