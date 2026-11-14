import { expect, test } from './fixtures/page-errors.js';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

// The WECC 240 dynamics demo, end to end: open it, run a fault, read the
// verdict, play it back, and the experimental tools behind their switch.
// Screenshots land in DYNAMICS_SHOTS when set.
const SHOTS = process.env.DYNAMICS_SHOTS;
const shot = async (page: import('@playwright/test').Page, name: string) => {
	if (SHOTS) await page.screenshot({ path: `${SHOTS}/${name}.png` });
};
const example = (name: string) =>
	readFileSync(fileURLToPath(new URL(`../static/examples/wecc240/${name}`, import.meta.url)));

test.setTimeout(180_000);

async function openDemo(page: import('@playwright/test').Page) {
	await page.route('**/api/cases', (route) => void route.fulfill({ json: [] }));
	await page.goto('/');
	await expect(page.getByText('no default cases loaded')).toBeVisible();
	await page.getByTestId('mode-dynamics').click();
	const panel = page.getByTestId('dynamics-panel');
	await panel.getByTestId('dyn-open-demo').click();
	await expect(panel.getByTestId('dyn-counts')).toContainText(
		/\d+ machines, 37 renewable plants simulated/,
		{
			timeout: 90_000
		}
	);
	await expect(panel.getByTestId('dyn-flat')).toContainText('✓', { timeout: 60_000 });
	return panel;
}

test('the WECC 240 demo opens, a bus fault runs, plays back and screens stable', async ({
	page
}) => {
	const panel = await openDemo(page);
	await shot(page, '01-demo-loaded');

	// Pick bus 1101 with the keyboard path (the bus search), then fault it.
	const search = page.getByRole('combobox', { name: 'Bus lookup' });
	await search.fill('1101');
	await search.press('Enter');
	await page.getByRole('button', { name: /Apply a fault here/ }).click();
	await expect(
		panel.getByText(/Three-phase fault at bus 1101.*1\.000 s, cleared after 6 cycles/)
	).toBeVisible();
	await shot(page, '02-fault-chosen');

	await panel.getByRole('button', { name: /Run 20 s simulation/ }).click();
	const word = panel.getByTestId('dyn-verdict-word');
	await expect(word).toHaveText('Stable', { timeout: 120_000 });
	await expect(panel.getByTestId('dyn-verdict')).toContainText(/59\.\d{3} Hz at/);

	// Traces render (a canvas with an accessible summary) and the timeline plays.
	const traces = page.getByTestId('dyn-traces');
	await expect(traces.getByRole('img').first()).toHaveAttribute(
		'aria-label',
		/COI nadir 59\.\d+ Hz/
	);
	await page.waitForTimeout(1500);
	await shot(page, '03-playback');

	// Scrub to 1.05 s, during the fault: the lowest voltage is deep.
	const scrub = page.getByTestId('dyn-scrubber');
	await scrub.evaluate((el: HTMLInputElement) => {
		el.value = '1.05';
		el.dispatchEvent(new Event('input', { bubbles: true }));
	});
	await expect(scrub).toHaveAttribute(
		'aria-valuetext',
		/1\.050 seconds.*fault.*lowest voltage 0\.[0-4]\d pu/
	);
	await shot(page, '04-mid-fault');
	for (const tab of ['Rotor angles', 'Voltages']) {
		await traces.getByRole('tab', { name: tab }).click();
		await expect(traces.getByRole('img').first()).toBeVisible();
	}
	await shot(page, '05-traces');
	await panel.getByTestId('dyn-verdict').scrollIntoViewIfNeeded();
	await shot(page, '06-verdict');
});

test('a validated scenario carries its badge', async ({ page }) => {
	const panel = await openDemo(page);
	await panel.getByRole('combobox', { name: /Validated scenarios/ }).selectOption('trip-1431-N');
	await panel.getByRole('button', { name: /Run 20 s simulation/ }).click();
	await expect(panel.getByTestId('dyn-verdict-word')).toHaveText('Stable', { timeout: 120_000 });
	await expect(panel.getByTestId('dyn-badge')).toContainText('Validated scenario');
	await expect(panel.getByTestId('dyn-verdict')).toContainText(/59\.\d{2}\d? Hz/);
});

test('stopping a run returns to idle and the static solve still works', async ({ page }) => {
	const panel = await openDemo(page);
	await panel.getByRole('button', { name: '120' }).count();
	await panel.getByLabel('Duration in seconds').fill('120');
	await panel.getByLabel('Duration in seconds').blur();
	await panel.getByRole('button', { name: /Run 120 s simulation/ }).click();
	await expect(panel.getByText(/Simulating…/)).toBeVisible();
	await panel.getByRole('button', { name: 'Stop' }).click();
	await expect(panel.getByText('Run stopped.')).toBeVisible();
	await expect(panel.getByRole('button', { name: /Run 120 s simulation/ })).toBeEnabled();
	// Back to steady state: the solver still answers.
	await page.getByTestId('mode-steady').click();
	await expect(page.locator('.solvecard')).toContainText(
		/AC power flow|DC OPF|converged|cheapest/i,
		{
			timeout: 60_000
		}
	);
});

test('a dropped .raw and .dyr attach', async ({ page }) => {
	await page.route('**/api/cases', (route) => void route.fulfill({ json: [] }));
	await page.goto('/');
	await expect(page.getByText('no default cases loaded')).toBeVisible();
	await page.locator('input[type="file"]').setInputFiles([
		{ name: 'wecc.raw', mimeType: 'text/plain', buffer: example('case.raw') },
		{ name: 'wecc.dyr', mimeType: 'text/plain', buffer: example('case.dyr') },
		{ name: 'wecc.geo.csv', mimeType: 'text/csv', buffer: example('case.geo.csv') }
	]);
	await expect(page.getByTestId('dyn-counts')).toContainText('DYR records', { timeout: 90_000 });
});

test('the experimental switch shows the Modes tab', async ({ page }) => {
	const panel = await openDemo(page);
	await page
		.getByRole('button', { name: /^Settings/ })
		.first()
		.click();
	await page.getByLabel('Show experimental dynamics tools').check();
	await shot(page, '07-experimental-settings');
	await panel.getByRole('tab', { name: /Modes/ }).click();
	// WECC 240 is too large for a quick dense eigenanalysis in the browser:
	// the tab says so, and Stop ends the analysis without losing the case.
	await expect(panel.getByTestId('dyn-modes-size')).toContainText('many minutes');
	await panel.getByRole('button', { name: 'Compute modes' }).click();
	await expect(panel.getByText('Computing modes…')).toBeVisible();
	await shot(page, '08-modes');
	await panel.getByRole('button', { name: 'Stop' }).click();
	await expect(panel.getByText('Stopped.')).toBeVisible();
	await panel.getByRole('tab', { name: 'Run' }).click();
	await expect(panel.getByRole('button', { name: /Run 20 s simulation/ })).toBeEnabled({
		timeout: 30_000
	});
});

test('reduced motion parks the playhead instead of playing', async ({ page }) => {
	await page.emulateMedia({ reducedMotion: 'reduce' });
	const panel = await openDemo(page);
	await panel.getByRole('combobox', { name: /Validated scenarios/ }).selectOption('trip-1101-1401');
	await panel.getByRole('button', { name: /Run 20 s simulation/ }).click();
	await expect(panel.getByTestId('dyn-verdict-word')).toHaveText(/Stable|Inconclusive/, {
		timeout: 120_000
	});
	await expect(page.getByTestId('dyn-play')).toHaveAttribute('aria-label', 'Play');
	await expect(page.getByTestId('dyn-time')).toContainText('t = 1.017 s');
});

test('on a phone the timeline stays above the drawer', async ({ page }) => {
	await page.setViewportSize({ width: 390, height: 844 });
	await page.route('**/api/cases', (route) => void route.fulfill({ json: [] }));
	await page.goto('/');
	await expect(page.getByText('no default cases loaded')).toBeVisible();
	await page.getByTestId('mode-dynamics').click();
	await page.getByTestId('dyn-open-demo').click();
	await expect(page.getByTestId('dyn-counts')).toBeVisible({ timeout: 90_000 });
	await page.getByRole('combobox', { name: /Validated scenarios/ }).selectOption('fault-1101');
	await page.getByRole('button', { name: /Run 20 s simulation/ }).click();
	await expect(page.getByTestId('dyn-verdict-word')).toHaveText('Stable', { timeout: 120_000 });
	await expect(page.getByTestId('dyn-scrubber')).toBeVisible();
	await shot(page, '09-phone');
});
