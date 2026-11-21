/* Copyright (c) Microsoft Corporation. Licensed under the MIT License. */
(() => {
	const host = acquireVsCodeApi();
	const form = document.getElementById('task');
	const add = document.getElementById('add');
	const role = document.getElementById('role');
	const title = document.getElementById('title');
	const notes = document.getElementById('notes');
	const board = document.getElementById('board');
	const status = document.getElementById('status');
	let creating = false;

	function element(tag, text, className) {
		const node = document.createElement(tag);
		node.textContent = text;
		if (className) {
			node.className = className;
		}
		return node;
	}

	function renderCard(card, stages) {
		const article = element('article', '', 'card');
		article.append(element('h3', card.title), element('p', card.role, 'specialist'));
		if (card.notes) {
			article.append(element('p', card.notes, 'task-notes'));
		}
		const stage = element('select', '', 'stage');
		stage.setAttribute('aria-label', `Stage for ${card.title}`);
		stage.dataset.taskId = card.id;
		stage.replaceChildren(...stages.map(value => new Option(value, value)));
		stage.value = card.stage;
		stage.addEventListener('change', () => host.postMessage({ type: 'move', id: card.id, stage: stage.value }));
		const draft = element('button', 'Prepare Draft', 'draft');
		draft.setAttribute('aria-label', `Prepare draft for ${card.title}`);
		draft.addEventListener('click', () => host.postMessage({ type: 'draft', id: card.id }));
		const remove = element('button', 'Remove', 'remove');
		remove.setAttribute('aria-label', `Remove ${card.title}`);
		remove.addEventListener('click', () => host.postMessage({ type: 'remove', id: card.id }));
		const actions = element('div', '', 'card-actions');
		actions.append(stage, draft, remove);
		article.append(actions);
		return article;
	}

	form.addEventListener('submit', event => {
		event.preventDefault();
		if (add.disabled || !title.value.trim()) {
			return;
		}
		creating = true;
		add.disabled = true;
		host.postMessage({ type: 'create', title: title.value, role: role.value, notes: notes.value });
	});
	window.addEventListener('message', ({ data }) => {
		if (data.type === 'error') {
			creating = false;
			add.disabled = !role.value;
			status.textContent = data.message;
			return;
		}
		if (data.type !== 'state') {
			return;
		}
		const focusedId = document.activeElement?.dataset.taskId;
		const selectedRole = role.value;
		role.replaceChildren(...data.roles.map(name => new Option(name, name)));
		if (data.roles.includes(selectedRole)) {
			role.value = selectedRole;
		}
		add.disabled = !data.hasWorkspace || !data.roles.length;
		status.textContent = !data.hasWorkspace ? 'Open a folder or workspace to create tasks.' :
			!data.roles.length ? 'Specialists could not be loaded.' : '';
		board.replaceChildren(...data.stages.map(name => {
			const lane = element('section', '', 'lane');
			const cards = data.cards.filter(card => card.stage === name);
			lane.append(element('h2', `${name} (${cards.length})`), ...cards.map(card => renderCard(card, data.stages)));
			return lane;
		}));
		if (focusedId) {
			[...board.querySelectorAll('select.stage')].find(select => select.dataset.taskId === focusedId)?.focus();
		}
		if (creating) {
			creating = false;
			title.value = '';
			notes.value = '';
			title.focus();
			status.textContent = 'Task added to Plan.';
		}
	});
	host.postMessage({ type: 'ready' });
})();
