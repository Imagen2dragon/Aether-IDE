/* Copyright (c) Microsoft Corporation. Licensed under the MIT License. */
const { randomUUID } = require('node:crypto');
const vscode = require('vscode');

const storageKey = 'aether.team.cards';
const stages = ['Plan', 'Implement', 'Verify', 'Review'];

async function readRoles(extension) {
	const roles = new Map();
	for (const entry of extension.packageJSON.contributes.chatAgents) {
		if (!/^\.\/agents\/[a-z]+\.agent\.md$/.test(entry.path)) {
			continue;
		}
		const bytes = await vscode.workspace.fs.readFile(vscode.Uri.joinPath(extension.extensionUri, entry.path));
		const match = new TextDecoder().decode(bytes).match(/^---\r?\nname: Aether (.+)\r?\ndescription: (.+)\r?\n[\s\S]*?\r?\n---\r?\n([\s\S]*)$/);
		if (match) {
			roles.set(match[1].trim(), { description: match[2].trim(), instructions: match[3].trim() });
		}
	}
	return roles;
}

function activate(context) {
	const roleDefinitions = readRoles(context.extension).catch(error => {
		console.error('Unable to load Aether specialist definitions', error);
		return new Map();
	});
	let panel;
	let updates = Promise.resolve();
	const load = () => context.workspaceState.get(storageKey, []);
	const draft = (role, text = '') => vscode.commands.executeCommand('aether.assistant.prepareDraft', { role, text });

	context.subscriptions.push(vscode.window.registerTreeDataProvider('aether.team.specialists', {
		getChildren: async () => [{ board: true }, ...[...(await roleDefinitions)].map(([role, definition]) => ({ role, ...definition }))],
		getTreeItem: entry => {
			const label = entry.board ? vscode.l10n.t('Open Engineering Board') : entry.role;
			const item = new vscode.TreeItem(label);
			item.iconPath = new vscode.ThemeIcon(entry.board ? 'tasklist' : 'person');
			item.tooltip = entry.description;
			item.command = entry.board
				? { command: 'aether.team.open', title: label }
				: { command: 'aether.assistant.prepareDraft', title: label, arguments: [{ role: entry.role }] };
			return item;
		}
	}));

	context.subscriptions.push(vscode.commands.registerCommand('aether.team.open', () => {
		if (panel) {
			panel.reveal();
			return;
		}
		const current = panel = vscode.window.createWebviewPanel('aether.engineeringBoard', vscode.l10n.t('Engineering Board'), vscode.ViewColumn.Active, {
			enableScripts: true,
			retainContextWhenHidden: true,
			localResourceRoots: [context.extensionUri]
		});
		const sendState = async () => current.webview.postMessage({
			type: 'state', cards: load(), stages, roles: [...(await roleDefinitions).keys()], hasWorkspace: Boolean(vscode.workspace.workspaceFolders?.length)
		});
		const subscriptions = [vscode.workspace.onDidChangeWorkspaceFolders(() => void sendState())];
		subscriptions.push(current.webview.onDidReceiveMessage(message => {
			updates = updates.then(async () => {
				if (panel !== current || !message) {
					return;
				}
				if (message.type === 'ready') {
					await sendState();
					return;
				}
				if (!vscode.workspace.workspaceFolders?.length) {
					return;
				}
				const cards = load();
				const card = cards.find(item => item.id === message.id);
				switch (message.type) {
					case 'create': {
						const title = typeof message.title === 'string' ? message.title.trim() : '';
						const notes = typeof message.notes === 'string' ? message.notes.trim() : '';
						if (cards.length >= 100 || !title || title.length > 100 || notes.length > 1000 || !(await roleDefinitions).has(message.role)) {
							throw new Error(vscode.l10n.t('Could not add the task. Confirm its fields and keep the board under 100 tasks.'));
						}
						await context.workspaceState.update(storageKey, [...cards, { id: randomUUID(), title, notes, role: message.role, stage: stages[0] }]);
						break;
					}
					case 'move':
						if (card && stages.includes(message.stage)) {
							await context.workspaceState.update(storageKey, cards.map(item => item === card ? { ...item, stage: message.stage } : item));
						}
						break;
					case 'draft':
						if (card) {
							await draft(card.role, [card.title, card.notes].filter(Boolean).join('\n\n'));
						}
						break;
					case 'remove':
						if (card) {
							const remove = vscode.l10n.t('Remove Task');
							const picked = await vscode.window.showWarningMessage(vscode.l10n.t('Remove “{0}” from the board?', card.title), { modal: true }, remove);
							if (picked === remove) {
								await context.workspaceState.update(storageKey, cards.filter(item => item !== card));
							}
						}
						break;
				}
				await sendState();
			}).catch(error => {
				void current.webview.postMessage({ type: 'error', message: error.message });
			});
		}));
		current.onDidDispose(() => {
			panel = undefined;
			subscriptions.forEach(subscription => subscription.dispose());
		});
		const resource = name => current.webview.asWebviewUri(vscode.Uri.joinPath(context.extensionUri, name));
		current.webview.html = `<!doctype html><html><head><meta charset="utf-8">
<meta http-equiv="Content-Security-Policy" content="default-src 'none'; style-src ${current.webview.cspSource}; script-src ${current.webview.cspSource}">
<meta name="viewport" content="width=device-width, initial-scale=1"><link rel="stylesheet" href="${resource('board.css')}">
<title>Engineering Board</title></head><body><main>
<header><h1>Engineering Board</h1><p>Track tasks in this workspace. Prepare a draft to ask a specialist in Aether Assistant.</p></header>
<form id="task"><label>Task<input id="title" maxlength="100" required></label><label>Specialist<select id="role" required></select></label>
<label class="notes">Task notes<textarea id="notes" maxlength="1000" rows="2"></textarea></label><button id="add" type="submit" disabled>Add Task</button></form>
<p id="status" role="status"></p><div id="board" aria-label="Task stages"></div>
</main><script src="${resource('board.js')}"></script></body></html>`;
	}));
	return { roles: roleDefinitions };
}

module.exports = { activate };
