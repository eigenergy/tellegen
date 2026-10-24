import { readFile } from 'node:fs/promises';
import type { Page } from '@playwright/test';
import { expect, test } from './fixtures/page-errors.js';

// The engineer tasks from the workbench usability protocol (#177), as regression checks. Each task
// must stay completable within a small interaction budget. This is a proxy for, not a replacement of,
// sessions with engineers: it catches regressions and buried actions, not misreadings.
const fixture = new URL('./fixtures/four-wire-limits.bmopf.json', import.meta.url);

async function solvedOverview(page: Page) {
	await page.route('**/api/cases', (route) => route.fulfill({ json: [] }));
	await page.goto('/');
	// The prerendered input exists before hydration attaches its listener; a file set earlier is
	// lost. The empty-cases panel renders only after load.
	await expect(page.getByText('no default cases loaded')).toBeVisible();
	await page.locator('input[type=file]').setInputFiles({
		name: 'four-wire-limits.bmopf.json',
		mimeType: 'application/json',
		buffer: await readFile(fixture)
	});
	const solve = page.getByRole('button', { name: 'Solve AC power flow', exact: true });
	await expect(solve).toBeEnabled({ timeout: 60_000 });
	await solve.click();
	const overview = page.getByRole('region', { name: 'Operating point overview' });
	await expect(overview.locator('.issues button').first()).toBeVisible({ timeout: 60_000 });
	return overview;
}

test('protocol tasks stay within their interaction budgets', async ({ page }) => {
	const overview = await solvedOverview(page);
	const issues = overview.locator('.issues button');

	await test.step('1. the most serious problem is visible without interaction', async () => {
		await expect(issues.first()).toContainText('lb');
		await expect(overview.getByRole('button', { name: /Violations/ })).toContainText('1');
	});
	await test.step('2. its size and phase are visible without interaction', async () => {
		await expect(issues.first()).toContainText('c–n below 207.00 V by 1.72 %');
	});
	await test.step('3. every phase at it against its band: one click', async () => {
		await issues.first().click();
		await expect(overview.getByRole('heading', { name: 'Phases at lb' })).toBeVisible();
		await expect(
			overview
				.getByRole('list', { name: 'Pair voltages against declared limits' })
				.getByRole('listitem')
		).toHaveCount(3);
	});
	await test.step('4. the most heavily loaded line: one click', async () => {
		await overview.getByRole('button', { name: /Max loading/ }).click();
		await expect(overview.getByRole('heading', { name: 'Conductors on l1' })).toBeVisible();
	});
	await test.step('5. why it is loaded: visible without further interaction', async () => {
		await expect(
			overview.getByRole('list', { name: 'From end conductor currents' }).getByRole('listitem')
		).toHaveCount(4);
		await expect(overview).toContainText('neutral carries');
	});
	await test.step('6. find a named bus: type and Enter', async () => {
		await overview.getByRole('combobox', { name: 'Find a bus or branch' }).fill('src');
		await page.keyboard.press('Enter');
		await expect(overview.getByRole('heading', { name: 'Phases at src' })).toBeVisible();
	});
	await test.step('9. results for a report: one click', async () => {
		const download = page.waitForEvent('download');
		await overview.getByRole('button', { name: 'Branch CSV' }).click();
		expect((await download).suggestedFilename()).toMatch(/branches\.csv$/);
	});
});

test('7. switch to the schematic and between layouts, keeping the selection', async ({ page }) => {
	const overview = await solvedOverview(page);
	await overview.locator('.issues button').first().click();
	await overview.getByRole('button', { name: 'Show schematic' }).click();
	const schematic = page.getByRole('application', { name: 'Schematic diagram' });
	await expect(schematic).toBeVisible();
	const toolbar = page.getByRole('toolbar', { name: 'Schematic' });
	// It opens where the user was: the case's own positions, then switches to the schematic.
	const own = toolbar.getByRole('button', { name: /Geographic|Source positions/ });
	await expect(own).toHaveAttribute('aria-pressed', 'true');
	await toolbar.getByRole('button', { name: 'Schematic', exact: true }).click();
	await toolbar.getByRole('combobox', { name: 'Schematic arrangement' }).selectOption('leaf-order');
	await toolbar.getByRole('combobox', { name: 'Schematic arrangement' }).selectOption('tidy');
	await toolbar.getByRole('combobox', { name: 'Colour by' }).selectOption('loading-heat');
	await expect(page.getByText('≥ 100 %')).toBeVisible();
	await own.click();
	await expect(own).toHaveAttribute('aria-pressed', 'true');
	await expect(overview.getByRole('heading', { name: 'Phases at lb' })).toBeVisible();
	for (const [name, extension] of [
		['PNG', 'png'],
		['SVG', 'svg']
	] as const) {
		const download = page.waitForEvent('download');
		await toolbar.getByRole('button', { name, exact: true }).click();
		expect((await download).suggestedFilename()).toMatch(new RegExp(`schematic\\.${extension}$`));
	}
	await toolbar.getByRole('button', { name: 'Close' }).click();
	await expect(schematic).toHaveCount(0);
});

test('the schematic draws equipment symbols with a legend, and hides them when too small', async ({
	page
}) => {
	const overview = await solvedOverview(page);
	await overview.getByRole('button', { name: 'Show schematic' }).click();
	const legend = page.getByRole('list', { name: 'Symbols' });
	await expect(legend).toBeVisible();
	await expect(legend.getByRole('listitem')).toHaveText([
		'phases, ○ neutral',
		'active power flow',
		'source',
		'load',
		'neutral earth'
	]);
	// Zoomed far out, symbols would be specks: the plain lines carry the drawing instead.
	await page.getByRole('application', { name: 'Schematic diagram' }).focus();
	for (let i = 0; i < 8; i++) await page.keyboard.press('-');
	await expect(legend).toHaveCount(0);
	// Zoomed out, the source stays findable as a fixed-size landmark, and landmarks can be turned off.
	const landmarks = page.getByRole('list', { name: 'Landmarks' });
	await expect(landmarks.getByRole('listitem').first()).toHaveText('source, always shown');
	await page.getByRole('checkbox', { name: 'Landmarks' }).uncheck();
	await expect(landmarks).toHaveCount(0);
	await page.getByRole('checkbox', { name: 'Landmarks' }).check();

	// Selecting from the overview flies to the selection and zooms until symbols are legible.
	await overview.locator('.issues button', { hasText: 'l1' }).click();
	await expect(legend).toBeVisible();
	// Clicking the selected issue again brings it back into view rather than clearing it.
	await page.getByRole('application', { name: 'Schematic diagram' }).focus();
	for (let i = 0; i < 8; i++) await page.keyboard.press('-');
	await expect(legend).toHaveCount(0);
	await overview.locator('.issues button', { hasText: 'l1' }).click();
	await expect(legend).toBeVisible();
	await expect(overview.getByText(/Conductors on l1/)).toBeVisible();
});

test('the schematic can be walked bus by bus from the keyboard', async ({ page }) => {
	const overview = await solvedOverview(page);
	await overview.getByRole('button', { name: 'Show schematic' }).click();
	const schematic = page.getByRole('application', { name: 'Schematic diagram' });
	await expect(page.getByRole('list', { name: 'Symbols' })).toBeVisible();
	await schematic.focus();
	const spoken = schematic.locator('[aria-live]');
	await page.keyboard.press('ArrowRight');
	await expect(spoken).toHaveText(/^src: source bus\. 1 branch onward\. 1 source, 1 neutral earth/);
	await page.keyboard.press('ArrowRight');
	await expect(spoken).toHaveText(
		/^lb: 1 step from src, fed by line l1\. 0 branches onward\. 3 loads\. Worst voltage c–n 203\.4 V, outside 207\.0 V to 253\.0 V/
	);
	await page.keyboard.press('Enter');
	await expect(overview.getByRole('heading', { name: 'Phases at lb' })).toBeVisible();
	await page.keyboard.press('ArrowLeft');
	await expect(spoken).toHaveText(/^src: /);
	await page.keyboard.press('Escape');
	await expect(spoken).toHaveText('');
});

test('hovering a table row marks it as the linked hover, and leaving clears it', async ({
	page
}) => {
	const overview = await solvedOverview(page);
	await overview.getByText('Results table').click();
	const row = overview.getByRole('row').filter({ hasText: /^src/ }).first();
	await row.hover();
	await expect(row).toHaveClass(/hovered/);
	await overview.getByRole('heading', { name: /^Issues/ }).hover();
	await expect(overview.locator('[role=row].hovered')).toHaveCount(0);
});

test('8. a link reopens the saved study with its selection and view', async ({ page, context }) => {
	const overview = await solvedOverview(page);
	await overview.locator('.issues button').first().click();
	await overview.getByRole('button', { name: 'Show schematic' }).click();
	await page.getByRole('button', { name: 'Save result' }).click();
	await expect(page.getByRole('button', { name: 'Copy view link' })).toBeVisible({
		timeout: 30_000
	});
	await expect(page).toHaveURL(/[?&]study=[^&]+.*sel=bus%3Alb|sel=bus%3Alb.*study=/);
	await expect(page).toHaveURL(/view=schematic/);

	const reopened = await context.newPage();
	await reopened.route('**/api/cases', (route) => route.fulfill({ json: [] }));
	await reopened.goto(page.url());
	const restored = reopened.getByRole('region', { name: 'Operating point overview' });
	await expect(restored.getByRole('heading', { name: 'Phases at lb' })).toBeVisible({
		timeout: 60_000
	});
	await expect(reopened.getByRole('application', { name: 'Schematic diagram' })).toBeVisible();
});

test('the overview works from the keyboard alone', async ({ page }) => {
	const overview = await solvedOverview(page);
	const search = overview.getByRole('combobox', { name: 'Find a bus or branch' });
	await search.focus();
	await page.keyboard.type('lb');
	await page.keyboard.press('ArrowDown');
	await page.keyboard.press('Enter');
	await expect(overview.getByRole('heading', { name: 'Phases at lb' })).toBeVisible();

	// The table grid follows the shared selection and moves it with arrow keys.
	await overview.getByText('Results table').click();
	const grid = overview.getByRole('grid', { name: 'Results by bus' });
	await grid.focus();
	await page.keyboard.press('Home');
	await expect(grid.locator('[role=row][aria-selected=true]')).toHaveCount(1);
	await page.keyboard.press('ArrowDown');
	await expect(overview.getByRole('heading', { name: /^Phases at / })).toBeVisible();
});

test('every overview control has an accessible name', async ({ page }) => {
	const overview = await solvedOverview(page);
	await overview.locator('.issues button').first().click();
	await overview.getByText('Results table').click();
	const unnamed = await overview.evaluate((root) =>
		[
			...root.querySelectorAll<HTMLElement | SVGElement>(
				'button, input, select, summary, [role=button], [role=tab], [role=grid], [tabindex="0"]'
			)
		]
			.filter((el) => {
				const label = (el as HTMLInputElement).labels?.length;
				const text = el.textContent?.trim();
				return (
					!el.getAttribute('aria-label') && !el.getAttribute('aria-labelledby') && !label && !text
				);
			})
			.map((el) => el.outerHTML.slice(0, 120))
	);
	expect(unnamed).toEqual([]);
});

test('the schematic layout file loads back with the case as its diagram', async ({
	page,
	context
}) => {
	const overview = await solvedOverview(page);
	await overview.getByRole('button', { name: 'Show schematic' }).click();
	const toolbar = page.getByRole('toolbar', { name: 'Schematic' });
	await toolbar.getByRole('button', { name: 'Schematic', exact: true }).click();
	const download = page.waitForEvent('download');
	await toolbar.getByRole('button', { name: 'Layout', exact: true }).click();
	const file = await download;
	expect(file.suggestedFilename()).toMatch(/schematic\.geo\.json$/);
	const layer = await readFile(await file.path(), 'utf8');
	const parsed = JSON.parse(layer);
	expect(parsed.powerio_geo).toEqual({ space: 'diagram', kind: 'synthetic' });
	const lb = parsed.features.find(
		(f: { properties: { bus?: string } }) => f.properties.bus === 'lb'
	);

	// A fresh tab loads the case with the layout as its drawing, through the existing reader.
	const fresh = await context.newPage();
	await fresh.route('**/api/cases', (route) => route.fulfill({ json: [] }));
	await fresh.goto('/');
	// The prerendered input exists before hydration attaches its listener; a file set earlier is
	// lost. The empty-cases panel renders only after load.
	await expect(fresh.getByText('no default cases loaded')).toBeVisible();
	await fresh.locator('input[type=file]').setInputFiles([
		{
			name: 'four-wire-limits.bmopf.json',
			mimeType: 'application/json',
			buffer: await readFile(fixture)
		},
		{
			name: 'four-wire-limits.schematic.geo.json',
			mimeType: 'application/json',
			buffer: Buffer.from(layer)
		}
	]);
	await expect(
		fresh.getByRole('application', { name: 'Network diagram', exact: true })
	).toBeVisible({
		timeout: 60_000
	});
	const circles = fresh.locator('[data-bus-id] circle');
	await expect(circles.first()).toBeVisible();
	const cx = await circles.evaluateAll((els) => els.map((el) => Number(el.getAttribute('cx'))));
	expect(cx).toContain(lb.geometry.coordinates[0]);
});

test('a selected line shows every conductor in the multi-wire view', async ({ page }) => {
	const overview = await solvedOverview(page);
	await overview.getByRole('combobox', { name: 'Find a bus or branch' }).fill('l1');
	await page.keyboard.press('Enter');
	await expect(overview.getByRole('heading', { name: 'Multi-wire view' })).toBeVisible();
	await expect(
		overview.getByRole('img', { name: /line l1: 4 conductor segments between src and lb/ })
	).toBeVisible();
});

test('a balanced case without limits does not break the overview', async ({ page }) => {
	// Regression: the shipped example declares no bus voltage limits and its source bus is balanced.
	// Equal values once made a value-keyed list throw `each_key_duplicate` (a 500 page).
	await page.route('**/api/cases', (route) => route.fulfill({ json: [] }));
	await page.goto('/');
	// The prerendered input exists before hydration attaches its listener; a file set earlier is
	// lost. The empty-cases panel renders only after load.
	await expect(page.getByText('no default cases loaded')).toBeVisible();
	await page.locator('input[type=file]').setInputFiles({
		name: 'four-wire.bmopf.json',
		mimeType: 'application/json',
		buffer: await readFile(new URL('../static/examples/four-wire.bmopf.json', import.meta.url))
	});
	const solve = page.getByRole('button', { name: 'Solve AC power flow', exact: true });
	await expect(solve).toBeEnabled({ timeout: 60_000 });
	await solve.click();
	const overview = page.getByRole('region', { name: 'Operating point overview' });
	await expect(overview.getByRole('heading', { name: /^Issues/ })).toBeVisible({ timeout: 60_000 });
	for (const id of ['src', 'lb', 'l1']) {
		await overview.getByRole('combobox', { name: 'Find a bus or branch' }).fill(id);
		await page.keyboard.press('Enter');
		await expect(
			overview.getByRole('heading', { name: new RegExp(`^(Phases at|Conductors on) ${id}$`) })
		).toBeVisible();
	}
	await overview.getByRole('button', { name: 'Show schematic' }).click();
	await expect(page.getByRole('application', { name: 'Schematic diagram' })).toBeVisible();
	await expect(page.getByText('Internal Error')).toHaveCount(0);
});

test('a center-tap transformer (one id, several graph edges) does not break the results', async ({
	page
}) => {
	// Regression: a three-winding transformer projects to one graph edge per winding pair, all with
	// its id; the equipment list was keyed by id and threw `each_key_duplicate` (a 500 page).
	await page.route('**/api/cases', (route) => route.fulfill({ json: [] }));
	await page.goto('/');
	await expect(page.getByText('no default cases loaded')).toBeVisible();
	await page.locator('input[type=file]').setInputFiles({
		name: 'center-tap.bmopf.json',
		mimeType: 'application/json',
		buffer: await readFile(
			new URL(
				'../../../crates/tellegen/tests/data/mc_pf/oracle_inputs/pf_center_tap_loaded.json',
				import.meta.url
			)
		)
	});
	const solve = page.getByRole('button', { name: 'Solve AC power flow', exact: true });
	await expect(solve).toBeEnabled({ timeout: 60_000 });
	await solve.click();
	const overview = page.getByRole('region', { name: 'Operating point overview' });
	await expect(overview.getByRole('heading', { name: /^Issues/ })).toBeVisible({ timeout: 60_000 });
	await page.getByText('Equipment currents and power').click();
	const equipment = page.getByLabel('Branch result');
	await expect(equipment.locator('option', { hasText: 't1' })).toHaveCount(1);
	await overview.getByRole('button', { name: 'Show schematic' }).click();
	await expect(page.getByRole('application', { name: 'Schematic diagram' })).toBeVisible();
	await expect(page.getByText('Internal Error')).toHaveCount(0);
});

test('the diagram opens before a solve, without results', async ({ page }) => {
	// Cases that do not solve yet (generators, IBRs) can still be inspected as a diagram.
	await page.route('**/api/cases', (route) => route.fulfill({ json: [] }));
	await page.goto('/');
	await expect(page.getByText('no default cases loaded')).toBeVisible();
	await page.locator('input[type=file]').setInputFiles({
		name: 'four-wire-limits.bmopf.json',
		mimeType: 'application/json',
		buffer: await readFile(fixture)
	});
	await page.getByRole('button', { name: 'Show diagram' }).click();
	const toolbar = page.getByRole('toolbar', { name: 'Schematic' });
	await expect(page.getByRole('application', { name: 'Schematic diagram' })).toBeVisible();
	await expect(toolbar.getByRole('combobox', { name: 'Colour by' })).toBeDisabled();
	await toolbar.getByRole('button', { name: 'Schematic', exact: true }).click();
	await expect(page.getByRole('list', { name: 'Symbols' })).toBeVisible();
	await page.getByRole('button', { name: 'Show map' }).click();
	await expect(page.getByRole('application', { name: 'Schematic diagram' })).toHaveCount(0);
});

test('a second case opens its own diagram while the first one is showing', async ({ page }) => {
	// Regression: the first diagram's teardown wrote app state, leaving the map's choice of
	// diagram stale, so the second case's Show diagram did nothing.
	await page.route('**/api/cases', (route) => route.fulfill({ json: [] }));
	await page.goto('/');
	await expect(page.getByText('no default cases loaded')).toBeVisible();
	const load = async (name: string) =>
		page
			.locator('input[type=file]')
			.first()
			.setInputFiles({
				name,
				mimeType: 'application/json',
				buffer: await readFile(fixture)
			});
	await load('first.bmopf.json');
	await page.getByRole('button', { name: 'Show diagram' }).click();
	const diagram = page.getByRole('application', { name: 'Schematic diagram' });
	await expect(diagram).toBeVisible();
	await load('second.bmopf.json');
	await expect(diagram).toHaveCount(0);
	await page.getByRole('button', { name: 'Show diagram' }).click();
	await expect(diagram).toBeVisible();
});

test('switching between two open diagrams draws each case with its own layout', async ({
	page
}) => {
	// Regression: the diagram component outlived a switch between two cases whose diagrams were
	// both open, and served the other case's cached schematic for this one.
	await page.route('**/api/cases', (route) => route.fulfill({ json: [] }));
	await page.goto('/');
	await expect(page.getByText('no default cases loaded')).toBeVisible();
	const centreTap = new URL(
		'../../../crates/tellegen/tests/data/mc_pf/oracle_inputs/pf_center_tap_loaded.json',
		import.meta.url
	);
	const load = async (name: string, file: URL) =>
		page
			.locator('input[type=file]')
			.first()
			.setInputFiles({ name, mimeType: 'application/json', buffer: await readFile(file) });
	const diagram = page.getByRole('application', { name: 'Schematic diagram' });
	const toolbar = page.getByRole('toolbar', { name: 'Schematic' });
	const schematicLayout = async () => {
		await toolbar.getByRole('button', { name: 'Schematic', exact: true }).click();
		const download = page.waitForEvent('download');
		await toolbar.getByRole('button', { name: 'Layout', exact: true }).click();
		return JSON.parse(await readFile(await (await download).path(), 'utf8')).features;
	};

	await load('first.bmopf.json', fixture);
	await page.getByRole('button', { name: 'Show diagram' }).click();
	await expect(diagram).toBeVisible();
	const first = await schematicLayout();

	await load('centre-tap.bmopf.json', centreTap);
	await page.getByRole('button', { name: 'Show diagram' }).click();
	await expect(diagram).toBeVisible();
	await schematicLayout();

	await page.getByRole('button', { name: 'pf_3ph_line multiconductor' }).click();
	await expect(diagram).toBeVisible();
	expect(await schematicLayout()).toEqual(first);
});

test('perfectly grounded terminals show as a solid earth', async ({ page }) => {
	await page.route('**/api/cases', (route) => route.fulfill({ json: [] }));
	await page.goto('/');
	await expect(page.getByText('no default cases loaded')).toBeVisible();
	await page.locator('input[type=file]').setInputFiles({
		name: 'pf_1ph_line.bmopf.json',
		mimeType: 'application/json',
		buffer: await readFile(
			new URL(
				'../../../crates/tellegen/tests/data/mc_pf/oracle_inputs/pf_1ph_line.json',
				import.meta.url
			)
		)
	});
	await page.getByRole('button', { name: 'Show diagram' }).click();
	await page
		.getByRole('toolbar', { name: 'Schematic' })
		.getByRole('button', { name: 'Schematic', exact: true })
		.click();
	await expect(
		page
			.getByRole('list', { name: 'Symbols' })
			.getByRole('listitem', { name: undefined })
			.filter({ hasText: 'solid earth' })
	).toHaveCount(1);
});

test('buses at the same point spread apart for the keyboard cursor', async ({ page }) => {
	// Geographic data often puts an MV and an LV bus at one pole; drawn on top of each other, one
	// hides the other. The cursor on either opens the stack: its buses spread and a card lists them.
	const at = { latitude: -34.9285, longitude: 138.6007 };
	const data = JSON.parse(await readFile(fixture, 'utf8'));
	for (const bus of Object.values(data.bus) as object[]) Object.assign(bus, at);
	await page.route('**/api/cases', (route) => route.fulfill({ json: [] }));
	await page.goto('/');
	await expect(page.getByText('no default cases loaded')).toBeVisible();
	await page.locator('input[type=file]').setInputFiles({
		name: 'stacked.bmopf.json',
		mimeType: 'application/json',
		buffer: Buffer.from(JSON.stringify(data))
	});
	await page.getByRole('button', { name: 'Show diagram' }).click();
	const schematic = page.getByRole('application', { name: 'Schematic diagram' });
	await expect(schematic).toBeVisible();
	await schematic.focus();
	await page.keyboard.press('ArrowRight');
	await expect(schematic.locator('[aria-live]')).toContainText('One of 2 buses at this point');
	const card = page.getByRole('list', { name: 'Buses at this point' });
	await expect(card.getByRole('listitem')).toHaveCount(2);
	await expect(card).toContainText('src');
	await expect(card).toContainText('lb');
	await page.keyboard.press('Escape');
	await expect(card).toHaveCount(0);
});
