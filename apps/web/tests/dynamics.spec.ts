import { expect, test } from './fixtures/page-errors.js';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

// The WECC 240 dynamics demo, end to end: open it, run a fault, read the
// verdict, play it back. Dynamics is experimental, so these tests turn on
// Settings → Experimental features first; dynamics-off.spec.ts covers the
// default, with it off.
// Screenshots land in DYNAMICS_SHOTS when set.
const SHOTS = process.env.DYNAMICS_SHOTS;
const shot = async (page: import('@playwright/test').Page, name: string) => {
	if (SHOTS) await page.screenshot({ path: `${SHOTS}/${name}.png` });
};
const example = (name: string) =>
	readFileSync(fileURLToPath(new URL(`../static/examples/wecc240/${name}`, import.meta.url)));

test.setTimeout(180_000);

test.beforeEach(async ({ page }) => {
	await page.addInitScript(() =>
		localStorage.setItem(
			'tellegen.preferences.v1',
			JSON.stringify({ version: 1, values: { experimental: true } })
		)
	);
});

async function openDemo(page: import('@playwright/test').Page) {
	await page.route('**/api/cases', (route) => void route.fulfill({ json: [] }));
	await page.goto('/');
	await expect(page.getByText('No cases', { exact: true })).toBeVisible();
	// The Dynamics toggle opens the panel and switches the map to dynamics.
	await page.locator('header').getByRole('button', { name: 'Dynamics', exact: true }).click();
	const panel = page.getByTestId('dynamics-panel');
	await panel.getByTestId('dyn-open-demo').click();
	const counts = panel.getByTestId('dyn-counts');
	await expect(counts.locator('.stat').filter({ hasText: 'Renewables' })).toContainText('37', {
		timeout: 90_000
	});
	await expect(counts.locator('.stat').filter({ hasText: 'Machines' })).toContainText(/\d+/);
	await expect(panel.getByTestId('dyn-flat')).toHaveText('Yes', { timeout: 60_000 });
	// The steady state / dynamics switch waits for a simulation to show.
	await expect(panel.getByTestId('mode-dynamics')).toHaveCount(0);
	return panel;
}

test('the WECC 240 demo opens, a bus fault runs, plays back and screens stable', async ({
	page
}) => {
	const panel = await openDemo(page);
	// Before a run there is nothing to explain: no dynamics legend.
	await expect(page.getByRole('complementary', { name: 'Dynamics map legend' })).toHaveCount(0);
	// The Network panel keeps the steady-state numbers, as on every other case, and has no
	// empty placeholder before a bus or line is picked.
	// The demo case has no generator costs, so its heaviest line shows instead of a $0 cost.
	await expect(page.locator('[data-panel="network"] dl.stats')).toContainText('Max loading', {
		timeout: 60_000
	});
	await expect(page.locator('[data-panel="network"] dl.stats')).not.toContainText('$0');
	await expect(page.locator('[data-panel="network"]')).not.toContainText('Select a bus or line');
	await shot(page, '01-demo-loaded');

	// Pick bus 1101 with the keyboard path (the bus search), then fault it.
	const search = page.getByRole('combobox', { name: 'Bus lookup' });
	await search.fill('1101');
	await search.press('Enter');
	await page.getByRole('button', { name: /Apply a fault here/ }).click();
	const fault = panel.getByRole('listitem', { name: 'Fault at bus 1101' });
	await expect(fault.getByLabel('Start')).toHaveValue('1');
	await expect(fault.getByLabel('Clear after')).toHaveValue('6');
	await shot(page, '02-fault-chosen');

	await panel.getByRole('button', { name: /Run 20 s simulation/ }).click();
	const word = panel.getByTestId('dyn-verdict-word');
	await expect(word).toHaveText('Stable', { timeout: 120_000 });
	// Key results are labelled numbers, with no notation or sentence fragments.
	const lowest = panel.getByTestId('dyn-verdict').locator('.stat', { hasText: 'Lowest frequency' });
	await expect(lowest.locator('.value')).toHaveText(/^59\.\d{3} Hz$/);
	await expect(panel.getByTestId('dyn-verdict')).not.toContainText(/COI|δ|RoCoF|nadir/);
	// With a simulation to show, the switch appears and returns the map to steady state.
	await expect(panel.getByTestId('mode-steady')).toBeVisible();
	// The engine's notes are in the Log, not the panel.
	await expect(panel.getByText(/RmsOptions|REGCA1/)).toHaveCount(0);

	// Traces render (a canvas with an accessible summary) and the timeline plays.
	const traces = page.getByTestId('dyn-traces');
	await expect(traces.getByRole('img').first()).toHaveAttribute(
		'aria-label',
		/^Frequency, lowest 59\.\d+ Hz$/
	);
	await page.waitForTimeout(1500);
	await shot(page, '03-playback');

	// The demo has a name, not its RAW header sentence.
	await expect(page.locator('header .case-button')).toContainText('WECC 240 (dynamics demo)');
	// The legend box holds its ring key (the global .legend strip is 6 px).
	const legend = page.getByRole('complementary', { name: 'Dynamics map legend' });
	expect((await legend.boundingBox())!.height).toBeGreaterThan(40);
	// The scrubber keeps its place as the readout gains an "after the fault" line.
	const scrubBox = async () => (await page.getByTestId('dyn-scrubber').boundingBox())!.x;
	await page.getByTestId('dyn-scrubber').evaluate((el: HTMLInputElement) => {
		el.value = '0.5';
		el.dispatchEvent(new Event('input', { bubbles: true }));
	});
	const before = await scrubBox();
	await page.getByTestId('dyn-scrubber').evaluate((el: HTMLInputElement) => {
		el.value = '3';
		el.dispatchEvent(new Event('input', { bubbles: true }));
	});
	await expect(page.getByTestId('dyn-time')).toContainText('after clearing');
	expect(await scrubBox()).toBe(before);

	// The map's controls stay usable above the dock, beside the panels.
	await page.getByRole('button', { name: 'Zoom in' }).click({ timeout: 5_000 });

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
	// The single-line diagram draws no dynamics layers: no legend over it.
	await page.getByRole('button', { name: 'Show diagram', exact: true }).click();
	await expect(page.getByRole('complementary', { name: 'Dynamics map legend' })).toHaveCount(0);
});

test('a validated scenario carries its badge', async ({ page }) => {
	const panel = await openDemo(page);
	await panel.getByRole('combobox', { name: 'Scenario' }).selectOption('trip-1431-N');
	await panel.getByRole('button', { name: /Run 20 s simulation/ }).click();
	await expect(panel.getByTestId('dyn-verdict-word')).toHaveText('Stable', { timeout: 120_000 });
	await expect(panel.getByTestId('dyn-badge')).toContainText('Validated');
	await expect(panel.getByTestId('dyn-verdict')).toContainText(/59\.\d{2}\d? Hz/);
});

test('stopping a run returns to idle and the static solve still works', async ({ page }) => {
	const panel = await openDemo(page);
	await panel.getByRole('button', { name: '120' }).count();
	await panel.getByLabel('Duration', { exact: true }).fill('120');
	await panel.getByLabel('Duration', { exact: true }).blur();
	await panel.getByRole('button', { name: /Run 120 s simulation/ }).click();
	await expect(panel.getByText(/Simulating…/)).toBeVisible();
	await panel.getByRole('button', { name: 'Stop' }).click();
	await expect(panel.getByText('Stopped', { exact: true })).toBeVisible();
	await expect(panel.getByRole('button', { name: /Run 120 s simulation/ })).toBeEnabled();
	// Closing the panel returns the map to steady state: the solver still answers.
	await page.locator('header').getByRole('button', { name: 'Dynamics', exact: true }).click();
	await expect(page.locator('[data-panel="network"] dl.stats')).toContainText(
		/Voltage range|Operational cost|Max loading/,
		{ timeout: 60_000 }
	);
});

test('a dropped .raw and .dyr attach', async ({ page }) => {
	await page.route('**/api/cases', (route) => void route.fulfill({ json: [] }));
	await page.goto('/');
	await expect(page.getByText('No cases', { exact: true })).toBeVisible();
	await page.locator('input[type="file"]').setInputFiles([
		{ name: 'wecc.raw', mimeType: 'text/plain', buffer: example('case.raw') },
		{ name: 'wecc.dyr', mimeType: 'text/plain', buffer: example('case.dyr') },
		{ name: 'wecc.geo.csv', mimeType: 'text/csv', buffer: example('case.geo.csv') }
	]);
	await expect(page.getByTestId('dyn-counts')).toContainText('Machines', { timeout: 90_000 });
});

test('with experimental features on, the Modes tab computes and stops', async ({ page }) => {
	const panel = await openDemo(page);
	await shot(page, '07-experimental-settings');
	await panel.getByRole('tab', { name: /Modes/ }).click();
	// WECC 240 is too large for a quick dense eigenanalysis in the browser:
	// the tab says so, and Stop ends the analysis without losing the case.
	await expect(panel.getByTestId('dyn-modes-size')).toHaveText('May take a few minutes');
	await panel.getByRole('button', { name: 'Compute modes' }).click();
	await expect(panel.getByText('Computing modes…')).toBeVisible();
	await shot(page, '08-modes');
	await panel.getByRole('button', { name: 'Stop' }).click();
	await expect(panel.getByText('Stopped', { exact: true })).toBeVisible();
	await panel.getByRole('tab', { name: 'Simulation' }).click();
	await expect(panel.getByRole('button', { name: /Run 20 s simulation/ })).toBeEnabled({
		timeout: 30_000
	});
});

test('reduced motion parks the playhead instead of playing', async ({ page }) => {
	await page.emulateMedia({ reducedMotion: 'reduce' });
	const panel = await openDemo(page);
	await panel.getByRole('combobox', { name: 'Scenario' }).selectOption('trip-1101-1401');
	await panel.getByRole('button', { name: /Run 20 s simulation/ }).click();
	await expect(panel.getByTestId('dyn-verdict-word')).toHaveText(/Stable|Not settled/, {
		timeout: 120_000
	});
	await expect(page.getByTestId('dyn-play')).toHaveAttribute('aria-label', 'Play');
	await expect(page.getByTestId('dyn-time')).toContainText('Time 1.017 s');
});

test('on a phone the timeline stays above the drawer', async ({ page }) => {
	await page.setViewportSize({ width: 390, height: 844 });
	await page.route('**/api/cases', (route) => void route.fulfill({ json: [] }));
	await page.goto('/');
	await expect(page.getByText('No cases', { exact: true })).toBeVisible();
	// The Dynamics tab opens the panel and switches the map to dynamics.
	await page.locator('.panel-toolbar').getByRole('button', { name: 'Dynamics' }).click();
	await page.getByTestId('dyn-open-demo').click();
	await expect(page.getByTestId('dyn-counts')).toBeVisible({ timeout: 90_000 });
	await page.getByRole('combobox', { name: 'Scenario' }).selectOption('fault-1101');
	await page.getByRole('button', { name: /Run 20 s simulation/ }).click();
	await expect(page.getByTestId('dyn-verdict-word')).toHaveText('Stable', { timeout: 120_000 });
	await expect(page.getByTestId('dyn-scrubber')).toBeVisible();
	await shot(page, '09-phone');
});
