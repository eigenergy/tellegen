import { readFileSync } from 'node:fs';
import { openSettings } from './fixtures/chrome.js';
import { fileURLToPath } from 'node:url';
import { expect, test } from './fixtures/page-errors.js';
import { callTool, installWebMcpHarness } from './fixtures/planning-case';

// Dynamics (RMS simulation) is on by default: the page shows it and the agent
// tool is registered without any setting. Settings → Experimental features
// adds only the tools still being validated (the Live, Models and Modes tabs).
const example = (name: string) =>
	readFileSync(
		fileURLToPath(new URL(`../static/examples/wecc240/${name}`, import.meta.url)),
		'utf8'
	);
const toolList = (page: import('@playwright/test').Page) =>
	page.evaluate(() =>
		(
			window as unknown as { __tellegenWebMcpTest: { list(): string[] } }
		).__tellegenWebMcpTest.list()
	);

async function openPage(page: import('@playwright/test').Page) {
	await installWebMcpHarness(page);
	await page.route('**/api/cases', (route) => void route.fulfill({ json: [] }));
	await page.goto('/');
	await expect(page.getByText('No cases', { exact: true })).toBeVisible();
	await expect(page.locator('html')).toHaveAttribute('data-webmcp', 'ready');
}

test('dynamics shows by default and its agent tool is registered', async ({ page }) => {
	await openPage(page);
	const tab = page.locator('header').getByRole('button', { name: 'Dynamics', exact: true });
	await expect(tab).toBeVisible();
	await expect.poll(() => toolList(page)).toContain('simulate_dynamics');
	await tab.click();
	await expect(page.getByTestId('dynamics-panel')).toBeVisible();
	// With no simulation yet, there is no steady state / dynamics switch to show.
	await expect(page.getByTestId('mode-dynamics')).toHaveCount(0);
	await expect(page.getByTestId('dyn-open-demo')).toBeVisible();
	// An agent's .dyr import is taken.
	const opened = await callTool(page, 'import_case', {
		file_name: 'wecc240.raw',
		text: example('case.raw')
	});
	expect(opened.ok).toBe(true);
	const dyr = await callTool(page, 'import_case', {
		file_name: 'wecc240.dyr',
		text: example('case.dyr')
	});
	expect(dyr.ok).toBe(true);
});

test('experimental features add the Live, Models and Modes tabs, and only those', async ({
	page
}) => {
	await openPage(page);
	// The tabs belong to a case with dynamic data: open WECC 240 with its .dyr.
	for (const [file_name, file] of [
		['wecc240.raw', 'case.raw'],
		['wecc240.dyr', 'case.dyr']
	])
		expect((await callTool(page, 'import_case', { file_name, text: example(file) })).ok).toBe(true);
	const tab = page.locator('header').getByRole('button', { name: 'Dynamics', exact: true });
	const panel = page.getByTestId('dynamics-panel');
	// Settings opens in the panel dock over the Dynamics panel: close it, and
	// bring the Dynamics panel back, before looking at its tabs.
	const showPanel = async () => {
		const closeSettings = page.getByRole('button', { name: 'Close settings panel' });
		if (await closeSettings.isVisible()) await closeSettings.click();
		if (!(await panel.isVisible())) await tab.click();
		await expect(panel).toBeVisible();
	};
	await showPanel();
	const views = panel.getByRole('tablist', { name: 'Dynamics views' });
	await expect(panel.getByRole('button', { name: 'Run simulation' })).toBeVisible();
	await expect(views).toHaveCount(0);

	await openSettings(page);
	const setting = page.getByLabel('Experimental features');
	await expect(setting).not.toBeChecked();
	await setting.check();
	await showPanel();
	for (const name of ['Live', 'Models', 'Modes'])
		await expect(views.getByRole('tab', { name, exact: true })).toBeVisible();

	// Turning the setting off hides the tabs again; dynamics and its tool stay.
	await openSettings(page);
	await page.getByLabel('Experimental features').uncheck();
	await showPanel();
	await expect(panel.getByRole('button', { name: 'Run simulation' })).toBeVisible();
	await expect(views).toHaveCount(0);
	await expect(tab).toBeVisible();
	expect(await toolList(page)).toContain('simulate_dynamics');
});
