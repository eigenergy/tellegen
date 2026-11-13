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

test('Firefox is pointed at Chrome or Edge, and the setup routes stay visible', async ({
	page
}) => {
	const status = await asBrowser(
		page,
		'Mozilla/5.0 (Macintosh; Intel Mac OS X 10.15; rv:140.0) Gecko/20100101 Firefox/140.0'
	);
	await expect(status).toContainText("Firefox doesn't speak WebMCP yet");
	await expect(status).toContainText('Open this page in Chrome on a computer');
	// Nothing to prompt yet: the prompt and asks wait until the tools load.
	await expect(page.getByLabel('Agent prompt')).toHaveCount(0);
	await expect(page.getByRole('heading', { name: 'Connect your agent' })).toBeVisible();
	await expect(page.getByText('Do I need to turn on remote debugging?')).toBeVisible();
	await expect(page.getByText('Do I need to add an MCP server?')).toBeVisible();
	await expect(page.locator('html')).not.toHaveAttribute('data-webmcp', /.*/);
});

test('recent Chrome without WebMCP gets the flag to copy', async ({ page, context }) => {
	await context.grantPermissions(['clipboard-read', 'clipboard-write']);
	const status = await asBrowser(page, MAC_CHROME(157));
	await expect(status).toContainText('One switch away');
	await expect(status).toContainText('Chrome 157 has WebMCP');
	// The test server is not tellegen.dev, so the hint says the trial origin needs no switch.
	await expect(status).toContainText('Chrome needs no switch on tellegen.dev');
	await expect(status.locator('code')).toHaveText('chrome://flags/#enable-webmcp-testing');
	await status.getByRole('button', { name: 'Copy flag address' }).click();
	await expect(status.getByRole('button', { name: 'Copied flag address' })).toHaveText('Copied');
	expect(await page.evaluate(() => navigator.clipboard.readText())).toBe(
		'chrome://flags/#enable-webmcp-testing'
	);
});

test('Edge gets its own flag scheme and hears that its trial is separate', async ({ page }) => {
	const status = await asBrowser(page, `${MAC_CHROME(150)} Edg/150.0.0.0`);
	await expect(status.locator('code')).toHaveText('edge://flags/#enable-webmcp-testing');
	await expect(status).toContainText('Edge runs its own WebMCP trial');
});

test('Edge 149 is asked for 150, where its trial starts', async ({ page }) => {
	const status = await asBrowser(page, `${MAC_CHROME(149)} Edg/149.0.0.0`);
	await expect(status).toContainText('Almost there');
	await expect(status).toContainText('Update to version 150 or later');
});

test('Chrome before the trial is asked to update', async ({ page }) => {
	const status = await asBrowser(page, MAC_CHROME(140));
	await expect(status).toContainText('Almost there');
	await expect(status).toContainText('Update to version 149 or later');
});

test('a phone hears that agents work from a computer', async ({ page }) => {
	const status = await asBrowser(
		page,
		'Mozilla/5.0 (iPhone; CPU iPhone OS 19_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/19.0 Mobile/15E148 Safari/604.1',
		true
	);
	await expect(status).toContainText('Agents work from a computer');
});

test('the setup routes copy their exact commands', async ({ page, context }) => {
	await context.grantPermissions(['clipboard-read', 'clipboard-write']);
	await installWebMcpHarness(page);
	await page.route('**/api/cases', (route) => route.fulfill({ json: [] }));
	await page.goto('/');
	await page.getByRole('button', { name: 'Agent', exact: true }).click();
	await page.getByText('Claude Code, Codex CLI or Cursor with Chrome DevTools MCP').click();
	await page.getByRole('button', { name: 'Copy Claude Code DevTools command' }).click();
	expect(await page.evaluate(() => navigator.clipboard.readText())).toBe(
		'claude mcp add chrome-devtools --scope user -- npx chrome-devtools-mcp@latest --categoryExperimentalWebmcp=true --chromeArg=--enable-features=WebMCP'
	);
	await page.getByText('Claude Code with Claude in Chrome').click();
	await page.getByRole('button', { name: 'Copy prompt for Claude' }).click();
	expect(await page.evaluate(() => navigator.clipboard.readText())).toMatch(
		/^Open http:\/\/127\.0\.0\.1:\d+ in Chrome\. .*`const tools = await document\.modelContext\.getTools\(\)`.*executeTool\(tools\.find\(/
	);
	// The test host has registerTool only, so the Claude route says it can't call tools here.
	await expect(page.getByTestId('page-tool-calls-missing')).toContainText('Chrome DevTools MCP');
});

test('the introduction opens Connect and the explainer remembers being closed', async ({
	page
}) => {
	await installWebMcpHarness(page);
	await page.route('**/api/cases', (route) => route.fulfill({ json: [] }));
	await page.goto('/');
	const welcome = page.getByRole('complementary', { name: 'New to Tellegen' });
	await expect(welcome).toContainText('New: AI agents can drive this map');
	await welcome.getByRole('button', { name: 'Show me' }).press('Enter');
	await expect(welcome).toHaveCount(0);
	// Focus follows the invitation into the panel it opened.
	await expect(page.getByRole('button', { name: 'Connect', exact: true })).toBeFocused();
	await expect(page.getByTestId('webmcp-status')).toContainText('Ready for agents');
	const intro = page.locator('details.intro');
	await expect(intro).toHaveAttribute('open', '');
	await expect(intro).toContainText('hand your AI assistant a toolbox');
	await intro.locator('summary').press('Enter');
	await expect(intro).not.toHaveAttribute('open', '');
	await page.reload();
	await expect(welcome).toHaveCount(0);
	await page.getByRole('button', { name: 'Agent', exact: true }).click();
	await expect(page.locator('details.intro')).not.toHaveAttribute('open', '');
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
	await expect(status).toContainText('Getting the tools ready');
	await expect(status).toContainText('Ready for agents');
	// Connect comes first, then the prompt.
	const headings = await page
		.getByRole('region', { name: 'Use Tellegen with an agent' })
		.locator('h3')
		.allTextContents();
	expect(headings.slice(0, 3)).toEqual([
		'1. Connect your agent',
		'2. Give it this prompt',
		'3. Things to ask'
	]);
});

test('a refused registration shows the error card', async ({ page }) => {
	await installWebMcpHarness(page, 'inspect_case');
	await page.route('**/api/cases', (route) => route.fulfill({ json: [] }));
	await page.goto('/');
	await page.getByRole('button', { name: 'Agent', exact: true }).click();
	const status = page.getByTestId('webmcp-status');
	await expect(status).toContainText("The tools didn't load");
	await expect(status).toContainText('registration refused for inspect_case');
	await expect(page.getByLabel('Agent prompt')).toHaveCount(0);
});

test('a finished call turns the card into Agent connected', async ({ page }) => {
	await installWebMcpHarness(page);
	await congestCase(page);
	await callTool(page, 'inspect_case', {});
	await page.getByRole('button', { name: 'Agent', exact: true }).click();
	await page.getByRole('button', { name: 'Connect', exact: true }).click();
	const status = page.getByTestId('webmcp-status');
	await expect(status).toContainText('Agent connected');
	await expect(status).toContainText(/Your agent has made \d+ calls?/);
});
