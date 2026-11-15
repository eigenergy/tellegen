import { readFileSync } from 'node:fs';
import { openSettings } from './fixtures/chrome.js';
import { fileURLToPath } from 'node:url';
import { expect, test } from './fixtures/page-errors.js';
import { callTool, installWebMcpHarness } from './fixtures/planning-case';

// Dynamics (RMS) simulation is experimental. With Settings → Experimental
// features off (the default) the page shows none of it and the agent tool is
// not registered; turning the setting on brings it back.
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
	await expect(page.getByText('no default cases loaded')).toBeVisible();
	await expect(page.locator('html')).toHaveAttribute('data-webmcp', 'ready');
}

async function expectNoDynamics(page: import('@playwright/test').Page) {
	await expect(page.getByTestId('mode-dynamics')).toHaveCount(0);
	await expect(page.getByRole('group', { name: 'Analysis mode' })).toHaveCount(0);
	await expect(page.getByTestId('dynamics-panel')).toHaveCount(0);
	await expect(
		page.locator('.panel-toolbar').getByRole('button', { name: 'Dynamics' })
	).toHaveCount(0);
	await page.getByRole('button', { name: /^Commands/ }).click();
	const dialog = page.getByRole('dialog', { name: 'Command palette' });
	await dialog.getByRole('combobox').fill('dynamics');
	await expect(dialog.getByRole('option', { name: /dynamics/i })).toHaveCount(0);
	await page.keyboard.press('Escape');
	await expect(dialog).toHaveCount(0);
}

test('dynamics is hidden by default and its agent tool is not registered', async ({ page }) => {
	await openPage(page);
	await expectNoDynamics(page);
	expect(await toolList(page)).not.toContain('simulate_dynamics');

	// An agent's .dyr import is refused with a plain reason.
	const opened = await callTool(page, 'import_case', {
		file_name: 'wecc240.raw',
		text: example('case.raw')
	});
	expect(opened.ok).toBe(true);
	const refused = await callTool(page, 'import_case', {
		file_name: 'wecc240.dyr',
		text: example('case.dyr')
	});
	expect(refused).toMatchObject({ ok: false, error: { code: 'DYNAMICS_OFF' } });
	await expectNoDynamics(page);
});

test('turning on experimental features brings dynamics back, and off hides it again', async ({
	page
}) => {
	await openPage(page);
	await openSettings(page);
	const setting = page.getByLabel('Experimental features');
	await expect(setting).not.toBeChecked();
	await expect(
		page.getByText('Adds dynamics (RMS) simulation, which is still being validated.')
	).toBeVisible();
	await setting.check();

	// Dynamics comes back as a panel tab; its steady state / dynamics switch is inside it.
	const tab = page.locator('.panel-toolbar').getByRole('button', { name: 'Dynamics' });
	await expect(tab).toBeVisible();
	await expect.poll(() => toolList(page)).toContain('simulate_dynamics');
	await tab.click();
	await expect(page.getByTestId('dynamics-panel')).toBeVisible();
	await expect(page.getByTestId('mode-dynamics')).toHaveAttribute('aria-pressed', 'true');

	// The setting is saved: it survives a reload.
	await page.reload();
	await expect(tab).toBeVisible();

	await openSettings(page);
	await page.getByLabel('Experimental features').uncheck();
	await expectNoDynamics(page);
	await expect.poll(() => toolList(page)).not.toContain('simulate_dynamics');
});
