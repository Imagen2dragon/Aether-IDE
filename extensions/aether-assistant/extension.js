/*---------------------------------------------------------------------------------------------
 *  Copyright (c) Microsoft Corporation. All rights reserved.
 *  Licensed under the MIT License. See License.txt in the project root for license information.
 *--------------------------------------------------------------------------------------------*/
const vscode = require('vscode');

const endpoint = 'http://127.0.0.1:11434';
const planItems = { type: 'array', minItems: 1, maxItems: 5, items: { type: 'string', minLength: 1, maxLength: 240 } };
const planSchema = {
	type: 'object',
	additionalProperties: false,
	properties: {
		summary: { type: 'string', minLength: 1, maxLength: 400 },
		steps: planItems,
		checks: planItems
	},
	required: ['summary', 'steps', 'checks']
};

function parsePlan(answer) {
	const plan = JSON.parse(answer);
	const isText = (value, limit) => typeof value === 'string' && value.length <= limit && Boolean(value.trim());
	const isList = values => Array.isArray(values) && values.length >= planItems.minItems && values.length <= planItems.maxItems &&
		values.every(value => isText(value, planItems.items.maxLength));
	if (!isText(plan?.summary, planSchema.properties.summary.maxLength) || !isList(plan.steps) || !isList(plan.checks)) {
		throw new Error('Invalid structured plan');
	}
	return { summary: plan.summary.trim(), steps: plan.steps.map(text => text.trim()), checks: plan.checks.map(text => text.trim()) };
}

async function request(route, options) {
	const response = await fetch(endpoint + route, { redirect: 'error', ...options });
	if (!response.ok) {
		throw new Error(vscode.l10n.t('Ollama returned HTTP {0}.', response.status));
	}
	return response;
}

async function readRoles() {
	const team = vscode.extensions.getExtension('aether.aether-engineering-team');
	return team ? (await team.activate()).roles : new Map();
}

function activate(context) {
	const roles = readRoles().catch(() => new Map());
	let draft;
	let publishDraft;
	context.subscriptions.push(vscode.commands.registerCommand('aether.assistant.open',
		() => vscode.commands.executeCommand('aether.assistant.view.focus')));
	context.subscriptions.push(vscode.commands.registerCommand('aether.assistant.prepareDraft', async selection => {
		if (!(await roles).has(selection?.role)) {
			return;
		}
		draft = { role: selection.role, text: typeof selection.text === 'string' ? selection.text.slice(0, 1200) : '' };
		await vscode.commands.executeCommand('aether.assistant.open');
		publishDraft?.();
	}));
	context.subscriptions.push(vscode.window.registerWebviewViewProvider('aether.assistant.view', {
		resolveWebviewView(view) {
			const webview = view.webview;
			webview.options = { enableScripts: true, localResourceRoots: [context.extensionUri] };
			const post = (type, payload = {}) => webview.postMessage({ type, ...payload });
			publishDraft = () => { if (draft) { void post('draft', { draft }); } };
			let models = [];
			let discovery = 0;
			let active;
			let history = [];
			let conversation;
			const messageListener = webview.onDidReceiveMessage(async message => {
				if (message?.type === 'draftApplied') {
					if (draft?.role === message.role && draft?.text === message.text) {
						draft = undefined;
					}
					return;
				}
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
					publishDraft?.();
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
				const wantsPlan = message.mode === 'plan';
				const key = JSON.stringify([message.model, message.role]);
				if (key !== conversation) {
					history = [];
					conversation = key;
				}
				let answer = '';
				try {
					const instructions = (await roles).get(message.role)?.instructions ??
						'You are Aether Assistant. Help the user with engineering questions. Use only the information they provide and be explicit about what you cannot verify.';
					const system = wantsPlan
						? `${instructions}\nReturn a short proposal with a summary, action steps, and checks for the user to perform. Only use supplied context. Do not claim that changes or checks were executed. Output JSON matching this schema: ${JSON.stringify(planSchema)}`
						: instructions;
					const response = await request('/api/chat', {
						method: 'POST',
						headers: { 'Content-Type': 'application/json' },
						body: JSON.stringify({ model: message.model, stream: true, think: false,
							...(wantsPlan ? { format: planSchema, options: { temperature: 0 } } : {}),
							messages: [{ role: 'system', content: system }, ...history, turn] }),
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
							if (!wantsPlan) {
								void post('token', { text });
							} else if (answer.length > 12000) {
								throw new Error(vscode.l10n.t('The proposed plan is too long. Try a narrower request.'));
							}
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
					controller.signal.throwIfAborted();
					let plan;
					if (wantsPlan) {
						try {
							plan = parsePlan(answer);
						} catch {
							throw new Error(vscode.l10n.t('The model did not return a usable plan. Try again or use Send for a chat reply.'));
						}
					}
					if (!controller.signal.aborted && conversation === key) {
						history = [...history, turn, { role: 'assistant', content: plan ? JSON.stringify(plan) : answer.slice(0, 16000) }].slice(-12);
					}
					void post('complete', plan ? { plan } : undefined);
				} catch (error) {
					void post('complete', { error: controller.signal.aborted
						? vscode.l10n.t('Response stopped.')
						: vscode.l10n.t('Local response failed: {0}', error.message) });
				} finally {
					active = undefined;
				}
			});
			view.onDidDispose(() => {
				publishDraft = undefined;
				++discovery;
				active?.abort();
				messageListener.dispose();
			});
			const resource = name => webview.asWebviewUri(vscode.Uri.joinPath(context.extensionUri, name));
			webview.html = `<!doctype html><html><head><meta charset="utf-8">
<meta http-equiv="Content-Security-Policy" content="default-src 'none'; style-src ${webview.cspSource}; script-src ${webview.cspSource}">
<meta name="viewport" content="width=device-width, initial-scale=1"><link rel="stylesheet" href="${resource('assistant.css')}">
<title>Aether Assistant</title></head><body>
<main><header><div class="choices"><label>Local model<select id="model"></select></label><label>Specialist<select id="role"><option value="">General</option></select></label></div>
<p id="status" role="status">Checking local models…</p><div class="actions"><button id="refresh" type="button">Refresh Models</button><button id="clear" type="button">New Conversation</button><button id="team-draft" type="button" hidden>Use Team Draft</button></div></header>
<section id="messages" role="log" aria-label="Conversation"><p id="empty">Ask a question or request a proposed plan. The assistant sees only what you share here.</p></section>
<form id="composer"><label for="prompt">Message</label><textarea id="prompt" rows="2" maxlength="8000" placeholder="Ask Aether…"></textarea>
<footer><span>Ollama</span><button id="stop" type="button" hidden>Stop</button><button id="plan" type="submit" disabled aria-label="Generate a proposed plan">Plan</button><button id="send" type="submit" disabled>Send</button></footer></form>
</main><script src="${resource('assistant.js')}"></script></body></html>`;
		}
	}));
}

module.exports = { activate };
