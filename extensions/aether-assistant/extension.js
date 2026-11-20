/*---------------------------------------------------------------------------------------------
 *  Copyright (c) Microsoft Corporation. All rights reserved.
 *  Licensed under the MIT License. See License.txt in the project root for license information.
 *--------------------------------------------------------------------------------------------*/
const vscode = require('vscode');

const endpoint = 'http://127.0.0.1:11434';

async function request(route, options) {
	const response = await fetch(endpoint + route, { redirect: 'error', ...options });
	if (!response.ok) {
		throw new Error(vscode.l10n.t('Ollama returned HTTP {0}.', response.status));
	}
	return response;
}

async function readRoles() {
	const team = vscode.extensions.getExtension('aether.aether-engineering-team');
	const roles = new Map();
	for (const entry of team?.packageJSON.contributes?.chatAgents ?? []) {
		if (!/^\.\/agents\/[a-z]+\.agent\.md$/.test(entry.path)) {
			continue;
		}
		const bytes = await vscode.workspace.fs.readFile(vscode.Uri.joinPath(team.extensionUri, entry.path));
		const match = new TextDecoder().decode(bytes).match(/^---\r?\nname: Aether (.+)\r?\n[\s\S]*?\r?\n---\r?\n([\s\S]*)$/);
		if (match) {
			roles.set(match[1].trim(), match[2].trim());
		}
	}
	return roles;
}

function activate(context) {
	context.subscriptions.push(vscode.commands.registerCommand('aether.assistant.open',
		() => vscode.commands.executeCommand('aether.assistant.view.focus')));
	context.subscriptions.push(vscode.window.registerWebviewViewProvider('aether.assistant.view', {
		resolveWebviewView(view) {
			const webview = view.webview;
			webview.options = { enableScripts: true, localResourceRoots: [context.extensionUri] };
			const post = (type, payload = {}) => webview.postMessage({ type, ...payload });
			const roles = readRoles().catch(() => new Map());
			let models = [];
			let discovery = 0;
			let active;
			let history = [];
			let conversation;
			const messageListener = webview.onDidReceiveMessage(async message => {
				if (message?.type === 'stop') {
					active?.abort();
					return;
				}
				if (message?.type === 'clear') {
					active?.abort();
					history = [];
					conversation = undefined;
					return;
				}
				if (message?.type === 'models') {
					const generation = ++discovery;
					try {
						const response = await request('/api/tags', { signal: AbortSignal.timeout(4000) });
						const result = await response.json();
						const found = result.models.map(model => model.name).filter(name => typeof name === 'string');
						const names = [...(await roles).keys()];
						if (generation === discovery) {
							models = found;
							void post('models', { models, roles: names });
						}
					} catch {
						if (generation === discovery) {
							models = [];
							void post('models', { models, roles: [...(await roles).keys()], unavailable: true });
						}
					}
					return;
				}
				if (message?.type !== 'send' || active || !models.includes(message.model) ||
					typeof message.text !== 'string' || message.text.length > 8000 || !message.text.trim()) {
					return;
				}
				const controller = new AbortController();
				active = controller;
				const turn = { role: 'user', content: message.text.trim() };
				const key = JSON.stringify([message.model, message.role]);
				if (key !== conversation) {
					history = [];
					conversation = key;
				}
				let answer = '';
				try {
					const instructions = (await roles).get(message.role) ??
						'You are Aether Assistant. Help the user with engineering questions. Use only the information they provide and be explicit about what you cannot verify.';
					const response = await request('/api/chat', {
						method: 'POST',
						headers: { 'Content-Type': 'application/json' },
						body: JSON.stringify({ model: message.model, stream: true, think: false,
							messages: [{ role: 'system', content: instructions }, ...history, turn] }),
						signal: AbortSignal.any([controller.signal, AbortSignal.timeout(180000)])
					});
					let buffer = '';
					let done = false;
					const consume = line => {
						if (!line.trim()) {
							return;
						}
						const event = JSON.parse(line);
						if (event.error) {
							throw new Error(event.error);
						}
						const text = event.message?.content;
						if (typeof text === 'string' && text) {
							answer += text;
							void post('token', { text });
						}
						done ||= event.done === true;
					};
					for await (const chunk of response.body.pipeThrough(new TextDecoderStream())) {
						buffer += chunk;
						const lines = buffer.split('\n');
						buffer = lines.pop();
						for (const line of lines) {
							consume(line);
						}
					}
					consume(buffer);
					if (!done) {
						throw new Error(vscode.l10n.t('The response ended early. Try again.'));
					}
					if (!controller.signal.aborted && conversation === key) {
						history = [...history, turn, { role: 'assistant', content: answer.slice(0, 16000) }].slice(-12);
					}
					void post('complete');
				} catch (error) {
					void post('complete', { error: controller.signal.aborted
						? vscode.l10n.t('Response stopped.')
						: vscode.l10n.t('Local response failed: {0}', error.message) });
				} finally {
					active = undefined;
				}
			});
			view.onDidDispose(() => {
				++discovery;
				active?.abort();
				messageListener.dispose();
			});
			const resource = name => webview.asWebviewUri(vscode.Uri.joinPath(context.extensionUri, name));
			webview.html = `<!doctype html><html><head><meta charset="utf-8">
<meta http-equiv="Content-Security-Policy" content="default-src 'none'; style-src ${webview.cspSource}; script-src ${webview.cspSource}">
<meta name="viewport" content="width=device-width, initial-scale=1"><link rel="stylesheet" href="${resource('assistant.css')}">
<title>Aether Assistant</title></head><body>
<main><header><p id="status" role="status">Checking local models…</p><div class="actions"><button id="refresh" type="button">Refresh Models</button><button id="clear" type="button">New Conversation</button></div></header>
<section id="messages" role="log" aria-label="Conversation"><p id="empty">Ask a question or paste a snippet. The assistant sees only what you share here.</p></section>
<form id="composer"><div class="choices"><label>Local model<select id="model"></select></label><label>Specialist<select id="role"><option value="">General</option></select></label></div><label for="prompt">Message</label><textarea id="prompt" rows="3" maxlength="8000" placeholder="Ask Aether…"></textarea>
<footer><span>Ollama</span><button id="stop" type="button" hidden>Stop</button><button id="send" type="submit" disabled>Send</button></footer></form>
</main><script src="${resource('assistant.js')}"></script></body></html>`;
		}
	}));
}

module.exports = { activate };
