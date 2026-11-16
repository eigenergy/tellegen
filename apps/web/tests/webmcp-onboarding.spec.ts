import type { Page } from '@playwright/test';
import { expect, test } from './fixtures/page-errors.js';
import { callTool, congestCase, installWebMcpHarness } from './fixtures/planning-case.js';

const MAC_CHROME = (version: number) =>
	`Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/${version}.0.0.0 Safari/537.36`;

/** Pretend to be another browser: a user agent string, no client hints, and
 * no WebMCP, whatever the test Chromium supports. */
async function asBrowser(page: Page, userAgent: string, mobile = false) {
	await page.addInitScript(
		([ua, isMobile]) => {
			Object.defineProperty(Navigator.prototype, 'userAgent', { get: () => ua });
			Object.defineProperty(Navigator.prototype, 'userAgentData', {
				get: () => (isMobile ? { brands: [], mobile: true } : undefined)
			});
			Object.defineProperty(Document.prototype, 'modelContext', { get: () => undefined });
			delete (Navigator.prototype as { modelContext?: unknown }).modelContext;
		},
		[userAgent, mobile] as const
	);
	await page.route('**/api/cases', (route) => route.fulfill({ json: [] }));
	await page.goto('/');
	await page.getByRole('button', { name: 'Agent', exact: true }).click();
	return page.getByTestId('webmcp-status');
}

test('Firefox hears it is not available, with a link to the docs', async ({ page }) => {
	const status = await asBrowser(
		page,
		'Mozilla/5.0 (Macintosh; Intel Mac OS X 10.15; rv:140.0) Gecko/20100101 Firefox/140.0'
	);
	await expect(status).toHaveText('Not available in this browser');
	// Nothing to prompt yet: the prompt waits until the tools load.
	await expect(page.getByRole('button', { name: 'Copy prompt', exact: true })).toHaveCount(0);
	// Per-agent setup lives in the docs, not in the panel.
	await expect(page.getByRole('heading', { name: 'Agents', exact: true })).toHaveCount(0);
	await expect(page.getByRole('link', { name: 'Docs', exact: true }).last()).toBeVisible();
	await expect(page.locator('html')).not.toHaveAttribute('data-webmcp', /.*/);
});

test('recent Chrome without WebMCP gets the flag to copy', async ({ page, context }) => {
	await context.grantPermissions(['clipboard-read', 'clipboard-write']);
	const status = await asBrowser(page, MAC_CHROME(157));
	await expect(status).toContainText('Not available in this browser');
	// The flag sits in the status card, to copy.
	const card = page.getByRole('region', { name: 'Use Tellegen with an agent' });
	await expect(card.locator('code')).toHaveText('chrome://flags/#enable-webmcp-testing');
	// The flag address is shown in full, not cut off.
	await expect
		.poll(() => card.locator('code').evaluate((el) => el.scrollWidth <= el.clientWidth))
		.toBe(true);
	await card.getByRole('button', { name: 'Copy flag address' }).click();
	await expect(card.getByRole('button', { name: 'Copied flag address' })).toHaveText('Copied');
	expect(await page.evaluate(() => navigator.clipboard.readText())).toBe(
		'chrome://flags/#enable-webmcp-testing'
	);
});

test('the first-visit hint follows what this browser can do', async ({ page }) => {
	await page.addInitScript(() => {
		Object.defineProperty(Document.prototype, 'modelContext', { get: () => undefined });
		delete (Navigator.prototype as { modelContext?: unknown }).modelContext;
	});
	await page.route('**/api/cases', (route) => route.fulfill({ json: [] }));
	const welcome = page.getByRole('complementary', { name: 'About AI agents' });
	await page.addInitScript(
		(ua) => Object.defineProperty(Navigator.prototype, 'userAgent', { get: () => ua }),
		MAC_CHROME(157)
	);
	await page.addInitScript(() =>
		Object.defineProperty(Navigator.prototype, 'userAgentData', { get: () => undefined })
	);
	await page.goto('/');
	// Chrome with WebMCP off: the hint says so and leads to the setting.
	await expect(welcome).toContainText('Agent tools are off');
	await welcome.getByRole('button', { name: 'Set up' }).click();
	await expect(page.getByTestId('webmcp-status')).toContainText('Not available in this browser');
});

test('Firefox gets no first-visit hint', async ({ page }) => {
	await page.addInitScript(() => {
		Object.defineProperty(Navigator.prototype, 'userAgent', {
			get: () =>
				'Mozilla/5.0 (Macintosh; Intel Mac OS X 10.15; rv:140.0) Gecko/20100101 Firefox/140.0'
		});
		Object.defineProperty(Navigator.prototype, 'userAgentData', { get: () => undefined });
		Object.defineProperty(Document.prototype, 'modelContext', { get: () => undefined });
		delete (Navigator.prototype as { modelContext?: unknown }).modelContext;
	});
	await page.route('**/api/cases', (route) => route.fulfill({ json: [] }));
	await page.goto('/');
	await expect(page.getByText('No cases', { exact: true })).toBeVisible();
	await expect(page.getByRole('complementary', { name: 'About AI agents' })).toHaveCount(0);
});

test('Edge gets its own flag scheme', async ({ page }) => {
	const status = await asBrowser(page, `${MAC_CHROME(150)} Edg/150.0.0.0`);
	await expect(
		page.getByRole('region', { name: 'Use Tellegen with an agent' }).locator('code')
	).toHaveText('edge://flags/#enable-webmcp-testing');
	await expect(status).toContainText('Not available in this browser');
});

test('Edge 149 is asked for 150, where its trial starts', async ({ page }) => {
	const status = await asBrowser(page, `${MAC_CHROME(149)} Edg/149.0.0.0`);
	await expect(status).toHaveText('Needs Edge 150+');
});

test('Chrome before the trial is asked to update', async ({ page }) => {
	const status = await asBrowser(page, MAC_CHROME(140));
	await expect(status).toHaveText('Needs Chrome 149+');
});

test('a phone hears that agent tools are not available', async ({ page }) => {
	const status = await asBrowser(
		page,
		'Mozilla/5.0 (iPhone; CPU iPhone OS 19_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/19.0 Mobile/15E148 Safari/604.1',
		true
	);
	await expect(status).toHaveText('Not available in this browser');
	// No desktop setup follows on a phone.
	await expect(page.locator('[data-testid="webmcp-status"] ~ *').getByRole('button')).toHaveCount(
		0
	);
});

async function copyPrompt(page: Page) {
	await page.route('**/api/cases', (route) => route.fulfill({ json: [] }));
	await page.goto('/');
	await page.getByRole('button', { name: 'Agent', exact: true }).click();
	await expect(page.getByTestId('webmcp-status')).toContainText('Ready');
	await page.getByRole('button', { name: 'Copy prompt', exact: true }).click();
	await expect(page.getByRole('button', { name: 'Copied', exact: true })).toBeVisible();
	return page.evaluate(() => navigator.clipboard.readText());
}

test('with getTools and executeTool, the prompt says how to call the page tools', async ({
	page,
	context
}) => {
	await context.grantPermissions(['clipboard-read', 'clipboard-write']);
	await installWebMcpHarness(page, null, { pageToolCalls: true });
	const text = await copyPrompt(page);
	expect(text).toMatch(
		/^Open http:\/\/127\.0\.0\.1:\d+ in Chrome\. .*getTools\(\).*executeTool\(tools\.find\(/
	);
	expect(text).toContain('Start with inspect_case');
});

test('without getTools and executeTool, the prompt only names the tools', async ({
	page,
	context
}) => {
	await context.grantPermissions(['clipboard-read', 'clipboard-write']);
	await installWebMcpHarness(page);
	const text = await copyPrompt(page);
	expect(text).toMatch(/^Use the WebMCP tools in this Tellegen tab/);
	expect(text).not.toContain('getTools');
});

test('the introduction opens Connect and stays dismissed', async ({ page }) => {
	await installWebMcpHarness(page);
	await page.route('**/api/cases', (route) => route.fulfill({ json: [] }));
	await page.goto('/');
	const welcome = page.getByRole('complementary', { name: 'About AI agents' });
	await expect(welcome).toContainText('Agent tools available');
	await welcome.getByRole('button', { name: 'Connect' }).press('Enter');
	await expect(welcome).toHaveCount(0);
	// Focus follows the invitation into the panel it opened.
	await expect(page.getByRole('button', { name: 'Connect', exact: true })).toBeFocused();
	await expect(page.getByTestId('webmcp-status')).toContainText('Ready');
	await page.reload();
	await expect(welcome).toHaveCount(0);
});

test('a browser with WebMCP sees a loading card while the tools register', async ({ page }) => {
	await installWebMcpHarness(page);
	await page.addInitScript(() => {
		const modelContext = (document as unknown as { modelContext: { registerTool: Function } })
			.modelContext;
		const register = modelContext.registerTool.bind(modelContext);
		// Hold the first registration back, as a slow browser would.
		let first = true;
		modelContext.registerTool = (...args: unknown[]) => {
			const wait = first ? 1_500 : 0;
			first = false;
			return new Promise((done) => setTimeout(done, wait)).then(() => register(...args));
		};
	});
	await page.route('**/api/cases', (route) => route.fulfill({ json: [] }));
	await page.goto('/');
	await page.getByRole('button', { name: 'Agent', exact: true }).click();
	const status = page.getByTestId('webmcp-status');
	await expect(status).toContainText('Loading…');
	await expect(status).toContainText('Ready');
	// Once ready, the prompt to copy and the docs; nothing else.
	const guide = page.getByRole('region', { name: 'Use Tellegen with an agent' });
	await expect(guide.getByRole('button', { name: 'Copy prompt', exact: true })).toBeVisible();
	await expect(guide.getByRole('link', { name: 'Docs', exact: true })).toBeVisible();
	await expect(guide.locator('h3, details')).toHaveCount(0);
});

test('a refused registration shows the error card', async ({ page }) => {
	await installWebMcpHarness(page, 'inspect_case');
	await page.route('**/api/cases', (route) => route.fulfill({ json: [] }));
	await page.goto('/');
	await page.getByRole('button', { name: 'Agent', exact: true }).click();
	const status = page.getByTestId('webmcp-status');
	await expect(status).toContainText('Tools failed to load');
	await expect(status).toContainText('registration refused for inspect_case');
	await expect(page.getByRole('button', { name: 'Copy prompt', exact: true })).toHaveCount(0);
});

test('a finished call turns the card into Connected', async ({ page }) => {
	await installWebMcpHarness(page);
	await congestCase(page);
	await callTool(page, 'inspect_case', {});
	await page.getByRole('button', { name: 'Agent', exact: true }).click();
	await page.getByRole('button', { name: 'Connect', exact: true }).click();
	const status = page.getByTestId('webmcp-status');
	await expect(status).toContainText('Connected');
	await expect(status.getByRole('definition').last()).toHaveText(/^[1-9]\d*$/);
});
