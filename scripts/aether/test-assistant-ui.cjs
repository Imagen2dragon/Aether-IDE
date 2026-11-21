/* Copyright (c) Microsoft Corporation. Licensed under the MIT License. */
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');
const { chromium } = require('playwright');

const extension = path.resolve(__dirname, '../../extensions/aether-assistant');
const source = fs.readFileSync(path.join(extension, 'extension.js'), 'utf8');
const theme = JSON.parse(fs.readFileSync(path.resolve(__dirname, '../../extensions/theme-aether/aether-obsidian.json'), 'utf8'));
let provider;
const moduleMock = { exports: {} };
vm.runInNewContext(source, {
	module: moduleMock,
	require: () => ({
		commands: { registerCommand() {} },
		extensions: { getExtension() {} },
		window: { registerWebviewViewProvider(_, value) { provider = value; } },
		Uri: { joinPath: path.join }
	})
});
moduleMock.exports.activate({ subscriptions: [], extensionUri: extension });
const webview = {
	cspSource: "'self'",
	asWebviewUri: filename => `/${path.basename(filename)}`,
	onDidReceiveMessage() {}
};
provider.resolveWebviewView({ webview, onDidDispose() {} });

async function run() {
	const channel = process.env.AETHER_TEST_BROWSER_CHANNEL;
	const browser = await chromium.launch(channel ? { channel } : {});
	try {
		const page = await browser.newPage({ viewport: { width: 240, height: 440 } });
		await page.addInitScript(() => {
			window.requests = [];
			window.acquireVsCodeApi = () => ({ postMessage: message => window.requests.push(message) });
		});
		await page.route('http://aether.test/**', route => {
			const resource = new URL(route.request().url()).pathname;
			if (resource === '/') {
				return route.fulfill({ body: webview.html, contentType: 'text/html' });
			}
			if (resource === '/assistant.css') {
				const tokens = Object.entries(theme.colors).map(([key, value]) => `--vscode-${key.replaceAll('.', '-')}: ${value};`).join('\n');
				return route.fulfill({ body: `:root { ${tokens} --vscode-font-family: sans-serif; }\n${fs.readFileSync(path.join(extension, 'assistant.css'), 'utf8')}`, contentType: 'text/css' });
			}
			if (resource === '/assistant.js') {
				return route.fulfill({ path: path.join(extension, 'assistant.js'), contentType: 'application/javascript' });
			}
			return route.abort();
		});
		await page.goto('http://aether.test/');
		const deliver = value => page.evaluate(message => window.postMessage(message, '*'), value);
		const lastRequest = () => page.evaluate(() => window.requests.at(-1));
		const prompt = page.getByRole('textbox', { name: 'Message' });
		const send = page.locator('#send');
		await deliver({ type: 'models', unavailable: true, models: [], roles: ['Mobile Engineer'] });
		await page.getByText('Ollama Offline').waitFor();
		assert.ok(await send.isDisabled());
		await deliver({ type: 'models', models: ['local-model'], roles: ['Mobile Engineer'] });
		await page.getByText('Local Model', { exact: true }).waitFor();
		assert.ok(await send.isDisabled(), 'Blank messages keep Send disabled');

		await page.locator('label').filter({ hasText: /^Plan$/ }).click();
		await prompt.fill('Review a sample Android build');
		await send.click();
		assert.equal((await lastRequest()).mode, 'plan');
		await page.getByRole('button', { name: 'Stop' }).waitFor();
		await deliver({ type: 'complete', plan: { summary: 'Verify the debug APK.', steps: ['Find the APK.'], checks: ['Confirm its application ID.'] } });
		await page.getByRole('heading', { name: 'Proposed Plan' }).waitFor();
		await page.getByRole('button', { name: 'Copy Plan' }).click();
		assert.match((await lastRequest()).text, /Checks to Run\n1\. Confirm its application ID\./);
		await deliver({ type: 'copied' });
		await page.getByText('Copied to clipboard.').waitFor();

		for (const [width, height] of [[240, 440], [340, 600], [480, 760]]) {
			await page.setViewportSize({ width, height });
			const rectangle = await send.boundingBox();
			assert.ok(rectangle && rectangle.x >= 0 && rectangle.y >= 0 && rectangle.x + rectangle.width <= width && rectangle.y + rectangle.height <= height,
				`Composer must stay visible at ${width}×${height}`);
			assert.ok(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth), `No page overflow at ${width}px`);
		}

		await page.getByRole('button', { name: 'New Chat' }).click();
		await page.getByRole('heading', { name: 'Ask Aether' }).waitFor();
		await page.locator('label').filter({ hasText: /^Chat$/ }).click();
		await prompt.fill('Show a short snippet');
		await prompt.press('Control+Enter');
		assert.equal((await lastRequest()).mode, 'chat');
		await deliver({ type: 'token', text: 'Example:\n```js\nconst example = "<img src=x>";\n```\n' });
		await deliver({ type: 'complete' });
		await page.getByRole('button', { name: 'Copy Code' }).click();
		assert.equal((await lastRequest()).text, 'const example = "<img src=x>";\n');
		assert.equal(await page.locator('img').count(), 0, 'Model code is rendered as text');

		await deliver({ type: 'draft', draft: { role: 'Mobile Engineer', text: 'Verify the debug artifact' } });
		await prompt.fill('Existing draft');
		await page.getByRole('button', { name: 'Use Team Draft' }).click();
		assert.equal(await prompt.inputValue(), 'Existing draft\n\nVerify the debug artifact');
		assert.equal(await page.getByLabel('Specialist').inputValue(), 'Mobile Engineer');
		assert.equal((await lastRequest()).type, 'draftApplied');
		await prompt.press('Control+Enter');
		const transcript = page.getByRole('log', { name: 'Conversation' });
		await deliver({ type: 'token', text: 'Sample response line.\n'.repeat(90) });
		await transcript.getByText(/Sample response line/).waitFor();
		await transcript.evaluate(element => { element.scrollTop = 0; });
		const latest = page.getByRole('button', { name: 'Jump to Latest' });
		await latest.waitFor();
		await deliver({ type: 'token', text: 'A new line while reading older messages.' });
		await transcript.getByText(/A new line while reading/).waitFor();
		assert.equal(await transcript.evaluate(element => element.scrollTop), 0, 'Streaming does not interrupt reading older messages');
		await latest.click();
		await latest.waitFor({ state: 'hidden' });
		console.log('PASS: Assistant layouts at 240, 340, and 480 px, chat and plan modes, keyboard submission, copy controls, team draft handoff, and transcript scrolling.');
	} finally {
		await browser.close();
	}
}

run().catch(error => {
	console.error(error);
	process.exitCode = 1;
});
