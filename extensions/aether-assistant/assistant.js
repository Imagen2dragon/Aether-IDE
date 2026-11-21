/* Copyright (c) Microsoft Corporation. Licensed under the MIT License. */
(() => {
	const host = acquireVsCodeApi();
	const get = id => document.getElementById(id);
	const messages = get('messages');
	const composer = get('composer');
	const connection = get('connection');
	const model = get('model');
	const role = get('role');
	const prompt = get('prompt');
	const status = get('status');
	const send = get('send');
	const planMode = get('plan-mode');
	const stop = get('stop');
	const latest = get('latest');
	const empty = get('empty');
	const teamDraft = get('team-draft');
	let busy = false;
	let discovering = true;
	let reply;
	let draft;

	function updateControls() {
		send.textContent = planMode.checked ? 'Create Plan' : 'Send';
		send.disabled = busy || discovering || !model.value || !prompt.value.trim();
		get('refresh').disabled = busy || discovering;
		get('clear').disabled = busy;
		stop.hidden = !busy;
		model.disabled = busy || discovering;
		role.disabled = busy || discovering;
		composer.querySelector('.mode').disabled = busy;
		teamDraft.hidden = !draft;
		teamDraft.disabled = busy || discovering || ![...role.options].some(option => option.value === draft?.role);
	}

	function setConnection(state, label) {
		connection.dataset.state = state;
		connection.textContent = label;
	}

	function fill(select, values) {
		const previous = select.value;
		select.replaceChildren(...values.map(([value, label]) => new Option(label, value)));
		if (values.some(([value]) => value === previous)) {
			select.value = previous;
		}
	}

	function isAtLatest() {
		return messages.scrollHeight - messages.scrollTop - messages.clientHeight < 48;
	}

	function updateScrollAction() {
		latest.hidden = isAtLatest();
	}

	function scrollToLatest() {
		messages.scrollTop = messages.scrollHeight;
		updateScrollAction();
	}

	function make(tag, text, className) {
		const node = document.createElement(tag);
		node.textContent = text;
		if (className) {
			node.className = className;
		}
		return node;
	}

	function append(label, text, kind) {
		empty.remove();
		const item = make('article', '', kind);
		const body = make('div', text, 'message-body');
		item.append(make('strong', label, 'speaker'), body);
		messages.append(item);
		scrollToLatest();
		return body;
	}

	function copyButton(label, text) {
		const button = make('button', label, 'quiet copy');
		button.type = 'button';
		button.addEventListener('click', () => host.postMessage({ type: 'copy', text }));
		return button;
	}

	function renderPlan(body, plan) {
		const card = make('section', '', 'proposed-plan');
		const header = make('div', '', 'plan-header');
		const plainText = [plan.summary, '', 'Steps', ...plan.steps.map((text, index) => `${index + 1}. ${text}`),
			'', 'Checks to Run', ...plan.checks.map((text, index) => `${index + 1}. ${text}`)].join('\n');
		header.append(make('h2', 'Proposed Plan'), copyButton('Copy Plan', plainText));
		card.append(header, make('p', plan.summary, 'plan-summary'));
		for (const [label, items] of [['Steps', plan.steps], ['Checks to Run', plan.checks]]) {
			const list = make('ol', '', 'plan-list');
			list.append(...items.map(text => make('li', text)));
			card.append(make('h3', label), list);
		}
		body.replaceWith(card);
		return card;
	}

	function formatCodeBlocks(body) {
		const text = body.textContent;
		const fences = /^\x60\x60\x60([\w.+-]*)[^\S\r\n]*\r?\n([\s\S]*?)^\x60\x60\x60[^\S\r\n]*(?:\r?\n|$)/gm;
		let offset = 0;
		const content = document.createDocumentFragment();
		for (const match of text.matchAll(fences)) {
			const [, language, code] = match;
			if (match.index > offset) {
				content.append(make('p', text.slice(offset, match.index).trimEnd()));
			}
			const block = make('section', '', 'code-block');
			const bar = make('div', '', 'code-header');
			bar.append(make('span', language || 'Code'));
			if (code.length && code.length <= 64000) {
				bar.append(copyButton('Copy Code', code));
			}
			const pre = document.createElement('pre');
			pre.append(make('code', code));
			block.append(bar, pre);
			content.append(block);
			offset = match.index + match[0].length;
		}
		if (offset) {
			if (offset < text.length) {
				content.append(make('p', text.slice(offset).trimStart()));
			}
			body.replaceChildren(content);
		}
	}

	function discover() {
		discovering = true;
		setConnection('checking', 'Checking Ollama');
		status.textContent = '';
		updateControls();
		host.postMessage({ type: 'models' });
	}

	get('refresh').addEventListener('click', discover);
	get('clear').addEventListener('click', () => {
		host.postMessage({ type: 'clear' });
		messages.replaceChildren(empty);
		updateScrollAction();
		prompt.focus();
	});
	stop.addEventListener('click', () => host.postMessage({ type: 'stop' }));
	latest.addEventListener('click', scrollToLatest);
	messages.addEventListener('scroll', updateScrollAction);
	window.addEventListener('resize', updateScrollAction);
	prompt.addEventListener('input', updateControls);
	model.addEventListener('change', updateControls);
	composer.querySelector('.mode').addEventListener('change', updateControls);
	prompt.addEventListener('keydown', event => {
		if (event.key === 'Enter' && (event.ctrlKey || event.metaKey) && !event.isComposing) {
			event.preventDefault();
			composer.requestSubmit(send);
		}
	});

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
		updateControls();
		prompt.focus();
	});

	composer.addEventListener('submit', event => {
		event.preventDefault();
		const text = prompt.value.trim();
		if (send.disabled || !text) {
			return;
		}
		append('You', text, 'user');
		reply = append(role.value || 'Aether', '', 'assistant');
		const mode = planMode.checked ? 'plan' : 'chat';
		busy = true;
		prompt.value = '';
		status.textContent = mode === 'plan' ? 'Preparing a proposed plan…' : 'Generating locally…';
		updateControls();
		prompt.focus();
		host.postMessage({ type: 'send', model: model.value, role: role.value, text, mode });
	});

	window.addEventListener('message', ({ data }) => {
		if (data.type === 'draft') {
			draft = data.draft;
			teamDraft.textContent = draft.text ? 'Use Team Draft' : `Select ${draft.role}`;
			updateControls();
		} else if (data.type === 'models') {
			fill(model, data.models.map(name => [name, name]));
			fill(role, [['', 'General'], ...data.roles.map(name => [name, name])]);
			setConnection(data.unavailable ? 'offline' : data.models.length ? 'ready' : 'empty',
				data.unavailable ? 'Ollama Offline' : data.models.length ? 'Local Model' : 'No Models');
			status.textContent = data.unavailable ? 'Start Ollama, then refresh.' :
				data.models.length ? '' : 'Install a model in Ollama, then refresh.';
			discovering = false;
			updateControls();
		} else if (data.type === 'token' && reply) {
			const pinned = isAtLatest();
			reply.append(document.createTextNode(data.text));
			if (pinned) {
				scrollToLatest();
			} else {
				updateScrollAction();
			}
		} else if (data.type === 'complete') {
			const pinned = isAtLatest();
			if (data.plan && reply) {
				const card = renderPlan(reply, data.plan);
				if (pinned) {
					messages.scrollTop += card.getBoundingClientRect().top - messages.getBoundingClientRect().top;
				}
			} else if (data.error && reply && !reply.textContent) {
				reply.textContent = data.error;
			} else if (reply && !data.error) {
				formatCodeBlocks(reply);
				if (pinned) {
					scrollToLatest();
				}
			}
			status.textContent = data.error ?? '';
			reply = undefined;
			busy = false;
			updateScrollAction();
			updateControls();
		} else if (data.type === 'copied' || data.type === 'copyFailed') {
			status.textContent = data.type === 'copied' ? 'Copied to clipboard.' : 'Could not copy to clipboard.';
		}
	});
	discover();
})();
