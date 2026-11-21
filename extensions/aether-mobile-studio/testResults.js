/* Copyright (c) Microsoft Corporation. Licensed under the MIT License. */
const vscode = require('vscode');

const viewId = 'aether.mobile.testResults';
const historyLimit = 20;
const resultPatterns = {
	unitTests: {
		junit: '**/build/test-results/testDebugUnitTest/TEST-*.xml',
		html: '**/build/reports/tests/**/index.html'
	},
	instrumentedTests: {
		junit: '**/build/outputs/androidTest-results/connected/**/*.xml',
		html: '**/build/reports/androidTests/connected/**/index.html'
	}
};

function registerTestResults(context, taskType) {
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
			if (run.kind !== 'test') {
				return [];
			}
			const items = [];
			if (run.summary) {
				items.push({ kind: 'summary', ...run.summary });
			}
			items.push(...run.reports.map(uri => ({ kind: 'report', uri })));
			if (run.note) {
				items.push({ kind: 'note', label: run.note });
			}
			return items;
		},
		getTreeItem(entry) {
			if (entry.kind === 'report') {
				const item = new vscode.TreeItem(vscode.l10n.t('Open HTML Test Report'));
				item.description = vscode.workspace.asRelativePath(entry.uri, false);
				item.tooltip = entry.uri.fsPath;
				item.iconPath = new vscode.ThemeIcon('open-preview');
				item.command = { command: 'aether.mobile.openTestReport', title: item.label, arguments: [entry.uri] };
				return item;
			}
			if (entry.kind === 'summary') {
				const item = new vscode.TreeItem(entry.label);
				item.description = entry.description;
				item.iconPath = new vscode.ThemeIcon(entry.failed ? 'error' : 'pass');
				return item;
			}
			if (entry.kind === 'note') {
				const item = new vscode.TreeItem(entry.label);
				item.tooltip = entry.label;
				return item;
			}
			const item = new vscode.TreeItem(entry.label, entry.summary || entry.reports.length || entry.note ? vscode.TreeItemCollapsibleState.Expanded : vscode.TreeItemCollapsibleState.None);
			item.id = entry.id;
			item.description = entry.elapsed === undefined ? entry.status : vscode.l10n.t('{0} · {1}s', entry.status, entry.elapsed);
			item.tooltip = vscode.l10n.t('{0}\n{1}\nFull logs are in the test task terminal.', item.description, entry.root.fsPath);
			item.iconPath = new vscode.ThemeIcon(entry.icon);
			return item;
		}
	};

	context.subscriptions.push(changes, vscode.window.registerTreeDataProvider(viewId, provider),
		vscode.commands.registerCommand('aether.mobile.openTestResults', () => vscode.commands.executeCommand(viewId + '.focus')),
		vscode.commands.registerCommand('aether.mobile.openTestReport', uri => vscode.env.openExternal(uri)),
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
			finish(run, exitCode === 0 ? vscode.l10n.t('Succeeded') : exitCode === undefined ? vscode.l10n.t('Stopped') : vscode.l10n.t('Failed · Exit {0}', exitCode), exitCode === 0 ? 'pass' : 'error');
			await loadResults(run);
			refresh(run);
		}));

	async function loadResults(run) {
		const patterns = resultPatterns[run.operation];
		if (!patterns) {
			return;
		}
		try {
			const [xmlFiles, reportFiles] = await Promise.all([
				vscode.workspace.findFiles(new vscode.RelativePattern(run.root, patterns.junit), null, 100),
				vscode.workspace.findFiles(new vscode.RelativePattern(run.root, patterns.html), null, 20)
			]);
			run.reports = reportFiles;
			const totals = await countResults(xmlFiles);
			if (totals.tests > 0 || totals.failures > 0) {
				run.summary = {
					label: vscode.l10n.t('Test Summary'),
					description: vscode.l10n.t('{0} passed · {1} failed · {2} skipped', Math.max(0, totals.tests - totals.failures - totals.skipped), totals.failures, totals.skipped),
					failed: totals.failures > 0
				};
			}
			if (!run.summary && !run.reports.length) {
				run.note = vscode.l10n.t('No test report was found. Check the test task terminal for details.');
			}
		} catch {
			run.note = vscode.l10n.t('Test report discovery failed. Check the test task terminal.');
		}
	}

	async function countResults(files) {
		const totals = { tests: 0, failures: 0, skipped: 0 };
		for (const file of files) {
			const xml = (await vscode.workspace.fs.readFile(file)).toString();
			for (const [, attributes] of xml.matchAll(/<testsuite\b([^>]*)>/g)) {
				const readCount = name => Number(attributes.match(new RegExp('\\b' + name + '="(\\d+)"'))?.[1] ?? 0);
				totals.tests += readCount('tests');
				totals.failures += readCount('failures') + readCount('errors');
				totals.skipped += readCount('skipped');
			}
		}
		return totals;
	}

	function finish(run, status, icon) {
		run.status = status;
		run.icon = icon;
		run.elapsed = Math.round((Date.now() - run.started) / 1000);
		refresh(run);
	}

	return {
		track(task, root) {
			const run = {
				id: task.definition.runId, kind: 'test', operation: task.definition.operation, root, label: task.name,
				reports: [], started: Date.now(), status: vscode.l10n.t('Queued'), icon: 'clock'
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

module.exports = { registerTestResults };
