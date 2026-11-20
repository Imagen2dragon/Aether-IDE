/* Copyright (c) Microsoft Corporation. Licensed under the MIT License. */
(() => {
	const host = acquireVsCodeApi();
	const get = id => document.getElementById(id);
	const messages = get('messages');
	const model = get('model');
	const role = get('role');
	const prompt = get('prompt');
	const status = get('status');
	const send = get('send');
	const teamDraft = get('team-draft');
	let busy = false;
	let reply;
	let draft;
	function updateDraftAction() {
		teamDraft.hidden = !draft;
		teamDraft.disabled = busy || ![...role.options].some(option => option.value === draft?.role);
	}
	function setBusy(value) {
		busy = value;
		send.disabled = busy || !model.value;
		get('refresh').disabled = busy;
		get('clear').disabled = busy;
		get('stop').hidden = !busy;
		model.disabled = busy;
		role.disabled = busy;
		updateDraftAction();
	}
	function fill(select, values) {
		const previous = select.value;
		select.replaceChildren(...values.map(([value, label]) => new Option(label, value)));
		if (values.some(([value]) => value === previous)) {
			select.value = previous;
		}
	}
	function append(label, text, kind) {
		get('empty')?.remove();
		const item = document.createElement('article');
		item.className = kind;
		const heading = document.createElement('strong');
		heading.textContent = label;
		const body = document.createElement('p');
		body.textContent = text;
		item.append(heading, body);
		messages.append(item);
		messages.scrollTop = messages.scrollHeight;
		return body;
	}
	function discover() {
		get('refresh').disabled = true;
		send.disabled = true;
		status.textContent = 'Checking local models…';
		host.postMessage({ type: 'models' });
	}
	get('refresh').addEventListener('click', discover);
	get('clear').addEventListener('click', () => {
		host.postMessage({ type: 'clear' });
		messages.replaceChildren();
		prompt.focus();
	});
	get('stop').addEventListener('click', () => host.postMessage({ type: 'stop' }));
	teamDraft.addEventListener('click', () => {
		const combined = [prompt.value, draft.text].filter(Boolean).join('\n\n');
		if (combined.length > prompt.maxLength) {
			status.textContent = 'Shorten the current message before adding the team draft.';
			return;
		}
		role.value = draft.role;
		prompt.value = combined;
		host.postMessage({ type: 'draftApplied', ...draft });
		draft = undefined;
		updateDraftAction();
		prompt.focus();
	});
	get('composer').addEventListener('submit', event => {
		event.preventDefault();
		const text = prompt.value.trim();
		if (busy || send.disabled || !model.value || !text) {
			return;
		}
		append('You', text, 'user');
		reply = append(role.value || 'Aether', '', 'assistant');
		setBusy(true);
		prompt.value = '';
		status.textContent = 'Generating locally…';
		host.postMessage({ type: 'send', model: model.value, role: role.value, text });
	});
	window.addEventListener('message', ({ data }) => {
		if (data.type === 'draft') {
			draft = data.draft;
			teamDraft.textContent = draft.text ? 'Use Team Draft' : `Select ${draft.role}`;
			updateDraftAction();
		} else if (data.type === 'models') {
			fill(model, data.models.map(name => [name, name]));
			fill(role, [['', 'General'], ...data.roles.map(name => [name, name])]);
			status.textContent = data.unavailable ? 'Start Ollama, then refresh.' :
				data.models.length ? 'Local models ready' : 'Install a model in Ollama, then refresh.';
			setBusy(false);
		} else if (data.type === 'token' && reply) {
			const pinned = messages.scrollHeight - messages.scrollTop - messages.clientHeight < 48;
			reply.append(document.createTextNode(data.text));
			if (pinned) {
				messages.scrollTop = messages.scrollHeight;
			}
		} else if (data.type === 'complete') {
			if (data.error && reply && !reply.textContent) {
				reply.textContent = data.error;
			}
			status.textContent = data.error ?? 'Local models ready';
			reply = undefined;
			setBusy(false);
		}
	});
	discover();
})();
