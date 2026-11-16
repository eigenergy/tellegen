import { expect, test } from './fixtures/page-errors.js';
import { showNetwork } from './fixtures/chrome.js';
import { runCalculation } from './fixtures/calculation.js';
import { readFile } from 'node:fs/promises';

// Two voltage levels: buses 1 (345 kV) and 2 (138 kV) share a substation and a transformer;
// 3 and 4 are 138 kV substations of their own. Line 2–4 is out of service.
const CASE = `function mpc = txdiagram
mpc.version = '2';
mpc.baseMVA = 100;
mpc.bus = [
 1 3 0  0  0 0 1 1 0 345 1 1.1 0.9;
 2 1 0  0  0 0 1 1 0 138 1 1.1 0.9;
 3 1 90 30 0 0 1 1 0 138 1 1.1 0.9;
 4 2 20 5  0 0 1 1 0 138 1 1.1 0.9;
];
mpc.gen = [
 1 0  0 300 -300 1 100 1 250 10 0 0 0 0 0 0 0 0 0 0 0;
 4 20 0 300 -300 1 100 1 100 10 0 0 0 0 0 0 0 0 0 0 0;
];
mpc.branch = [
 1 2 0.001 0.05 0 35 35 35 1 0 1 -360 360;
 2 3 0.01 0.1 0 120 120 120 0 0 1 -360 360;
 3 4 0.01 0.1 0 120 120 120 0 0 1 -360 360;
 2 4 0.01 0.1 0 120 120 120 0 0 0 -360 360;
];
mpc.gencost = [
 2 0 0 3 0.11  5   0;
 2 0 0 3 0.085 9 0;
];
`;
const COORDS = `bus_i,lat,lon
1,34.0,-81.1
2,34.0,-81.1
3,34.1,-81.0
4,34.0,-80.9
`;

const voltageCase = (uniform: boolean) => `function mpc = stationlimits
mpc.version = '2';
mpc.baseMVA = 100;
mpc.bus = [
 1 3 0 0 0 0 1 1.08 0 138 1 1.05 0.95;
 2 2 0 0 0 0 1 ${uniform ? 1.08 : 1.02} 0 138 1 1.05 0.95;
];
mpc.gen = [
 1 0 0 300 -300 1.08 100 1 100 0;
 2 0 0 300 -300 ${uniform ? 1.08 : 1.02} 100 1 100 0;
];
mpc.branch = [1 2 0.01 0.1 0 1000 1000 1000 0 0 1 -360 360;];
mpc.gencost = [2 0 0 3 0.1 5 0; 2 0 0 3 0.1 5 0;];
`;

/** The results panel starts collapsed on a phone; open it where a test needs the full panel. */
async function expandResults(page: import('@playwright/test').Page) {
	const more = page.locator('.tx-stage .legend').getByRole('button', { name: 'Show details' });
	if (await more.isVisible()) await more.click();
	await expect(
		page.locator('.tx-stage .legend').getByRole('button', { name: 'Hide details' })
	).toBeVisible();
}

async function selectView(toolbar: import('@playwright/test').Locator, label: string) {
	const select = toolbar.getByRole('combobox', { name: 'Diagram view', exact: true });
	if (await select.isVisible()) await select.selectOption({ label });
	else await toolbar.getByRole('radio', { name: label, exact: true }).click();
}
async function expectView(toolbar: import('@playwright/test').Locator, label: string) {
	const select = toolbar.getByRole('combobox', { name: 'Diagram view', exact: true });
	if (await select.isVisible()) await expect(select.locator('option:checked')).toHaveText(label);
	else
		await expect(toolbar.getByRole('radio', { name: label, exact: true })).toHaveAttribute(
			'aria-checked',
			'true'
		);
}
async function selectLayout(toolbar: import('@playwright/test').Locator, label: string) {
	await displayOptions(toolbar, true);
	const select = toolbar.getByRole('combobox', { name: 'Diagram layout', exact: true });
	if (await select.isVisible()) {
		await select.selectOption({ label });
		await expect(select.locator('option:checked')).toHaveText(label);
	} else {
		const button = toolbar.getByRole('button', { name: label, exact: true });
		await button.click();
		await expect(button).toHaveAttribute('aria-pressed', 'true');
	}
	await displayOptions(toolbar, false);
}
/** On a phone the drawing controls (layout, fit, zoom) are behind Display. */
async function fit(toolbar: import('@playwright/test').Locator) {
	await displayOptions(toolbar, true);
	await toolbar.getByRole('button', { name: 'Fit', exact: true }).click();
	await displayOptions(toolbar, false);
}
async function displayOptions(toolbar: import('@playwright/test').Locator, open: boolean) {
	const toggle = toolbar.getByRole('button', { name: 'Display', exact: true });
	if (
		(await toggle.isVisible()) &&
		((await toggle.getAttribute('aria-expanded')) === 'true') !== open
	)
		await toggle.click();
}
async function hideBasemap(toolbar: import('@playwright/test').Locator) {
	await displayOptions(toolbar, true);
	await toolbar.getByLabel('Basemap').uncheck();
	await displayOptions(toolbar, false);
}
async function exportPng(toolbar: import('@playwright/test').Locator) {
	await displayOptions(toolbar, true);
	await toolbar.getByRole('button', { name: 'Save image', exact: true }).click();
}

for (const touch of [false, true]) {
	test.describe(touch ? 'phone touch' : 'desktop mouse', () => {
		test.use(
			touch ? { viewport: { width: 390, height: 844 }, hasTouch: true, isMobile: true } : {}
		);
		test('utilization at the rating is red and selectable', async ({ page }, testInfo) => {
			test.setTimeout(120_000);
			const limited = voltageCase(false)
				.replace('2 2 0 0 0 0 1', '2 2 90 0 0 0 1')
				.replace('1000 1000 1000', '80 80 80')
				.replace('0.1 5 0; 2 0 0 3 0.1 5 0;', '0.1 5 0; 2 0 0 3 0.1 500 0;');
			await page.route('**/api/cases', (route) => route.fulfill({ json: [] }));
			await page.route('**/api/compute', (route) => route.fulfill({ json: { enabled: false } }));
			await page.goto('/');
			await page.emulateMedia({ reducedMotion: 'reduce' });
			await expect(page.getByText('No cases', { exact: true })).toBeVisible();
			await page.locator('input[type="file"]').setInputFiles([
				{
					name: 'limited-coords.csv',
					mimeType: 'text/csv',
					buffer: Buffer.from('bus_i,lat,lon\n1,34,-81\n2,34,-80.9\n')
				},
				{ name: 'limited.m', mimeType: 'text/plain', buffer: Buffer.from(limited) }
			]);
			await expect(page.locator('.solvecard')).toHaveAttribute('data-calculation', 'dcopf', {
				timeout: 60000
			});
			if (touch) await showNetwork(page);
			await page.getByRole('button', { name: 'Show diagram', exact: true }).click();
			const toolbar = page.getByRole('toolbar', { name: 'Single-line diagram' });
			await hideBasemap(toolbar);
			await fit(toolbar);
			const panel = page.getByRole('region', { name: 'Limits', exact: true });
			// A line at its rating counts as over the limit, in red as on the map.
			const over = panel.locator('.stat').filter({ hasText: 'Over limit' });
			await expect(over).toContainText('1');
			await expect(over).toHaveClass(/failure/);
			// Counts are listed only when they are not zero.
			await expect(panel.locator('.stat').filter({ hasText: '≥90%' })).toHaveCount(0);
			const gauge = page.getByRole('button', { name: /Branch 1, .*percent, at rating/ });
			// At rating is red (a heavy outline), not amber; only an overload is filled.
			await expect(gauge).toHaveClass(/at-limit/);
			await expect(gauge).not.toHaveClass(/warning/);
			await expect(gauge).not.toHaveClass(/violation/);
			await page.screenshot({ path: testInfo.outputPath('at-rating-gauge.png') });
			if (touch) await gauge.tap();
			else {
				await gauge.focus();
				await gauge.press('Enter');
			}
			if (touch) {
				await expect(
					page.getByRole('slider', { name: 'Rating delta at selected line' })
				).toBeVisible();
				await page.getByRole('button', { name: 'Close network panel' }).tap();
			}
			await fit(toolbar);
			await expect(gauge).toHaveAttribute('aria-pressed', 'true');
			const diagram = page.locator('.tx-stage');
			const inspector = diagram.getByRole('region', { name: 'Selected equipment' });
			await expect(inspector).toContainText('+80 MW');
			await expect(inspector).toContainText('100%');
			await expect(inspector).toContainText('0 MW');
			await inspector.getByRole('button', { name: 'Clear selection' }).click();
			await expect(inspector).toHaveCount(0);
			const exceptions = diagram.getByText('Flagged (1)', { exact: true });
			if (touch) await exceptions.tap();
			else await exceptions.press('Enter');
			const search = diagram.getByRole('searchbox', { name: 'Search' });
			await search.fill('branch 99');
			await expect(diagram.getByText('No matches', { exact: true })).toBeVisible();
			await search.fill('branch 1');
			const row = diagram.getByRole('button', { name: /Line 1 \(1–2\)/ });
			if (touch) await row.tap();
			else await row.press('Enter');
			await expect(inspector.getByRole('heading', { name: 'Line 1' })).toBeFocused();
			await expect(inspector.locator('dl div').filter({ hasText: /^Rating/ })).toContainText(
				'80 MW'
			);
			// DC reports no limiting end, so the row is left out rather than marked unavailable.
			await expect(inspector.locator('dl div').filter({ hasText: 'Limiting end' })).toHaveCount(0);
			await inspector.getByRole('button', { name: 'Frame', exact: true }).click();
			if (touch) await inspector.getByRole('button', { name: 'Clear selection' }).tap();
			else await inspector.getByRole('button', { name: 'Frame', exact: true }).press('Escape');
			await expect(inspector).toHaveCount(0);

			await selectView(toolbar, 'Prices');
			const economics = diagram.getByRole('region', { name: 'Prices' });
			await expect(
				economics
					.locator('.stat')
					.filter({ hasText: /^(Lowest|Price)/ })
					.first()
			).toContainText('$/MWh');
			// The price scale fits the prices by default, as the Network panel's legend does.
			await expect(economics.getByLabel('Scale')).toHaveValue('-1');
			await economics.getByLabel('Scale').selectOption('3');
			await expect(economics).toContainText('200 $/MWh');
			await economics.getByText('Lines at limit (1)', { exact: true }).click();
			const binding = economics.getByRole('button', { name: /Line 1.*\/MW/ });
			if (touch) await binding.tap();
			else await binding.press('Enter');
			const heading = inspector.getByRole('heading', { name: 'Line 1' });
			await expect(heading).toBeFocused();
			await expect(heading).toBeInViewport({ ratio: 1 });
			await expect(economics.locator('.pick div').filter({ hasText: 'Line 1' })).toContainText(
				'At limit'
			);
			await expect(
				economics.locator('.pick div').filter({ hasText: 'Value of rating' })
			).toContainText(/[1-9]/);
			await expect(diagram.getByLabel('Branch utilization gauges')).toHaveCount(0);
			await page.screenshot({ path: testInfo.outputPath('opf-economics.png') });

			await selectView(toolbar, 'Flows');
			const transfers = diagram.getByRole('region', { name: 'Flows' });
			const widthScale = transfers.getByLabel('Width scale');
			await expect(widthScale).toHaveValue('500');
			await widthScale.selectOption('100');
			await expect(widthScale).toHaveValue('100');
			await expect(inspector.getByRole('row', { name: /From bus 1/ })).toContainText('+80 MW');
			await expect(inspector.getByRole('row', { name: /To bus 2/ })).toContainText('-80 MW');
			await expect(inspector.getByRole('columnheader', { name: 'Reactive' })).toHaveCount(0);
			await expect(inspector).toContainText('from bus 1 to bus 2');
			// No overloads: the list is left out rather than shown empty.
			await expect(diagram.getByText(/^Overloads/)).toHaveCount(0);
			await inspector.getByRole('button', { name: 'Clear selection' }).click();
			await transfers.getByText('Largest flows', { exact: true }).click();
			const ranked = transfers.getByRole('button', { name: /Line 1.*80 MW.*1 to 2/ });
			if (touch) await ranked.tap();
			else await ranked.press('Enter');
			await expect(inspector.getByRole('heading', { name: 'Line 1' })).toBeVisible();
			await expect(inspector.getByRole('heading', { name: 'Line 1' })).toBeFocused();
			await page.screenshot({ path: testInfo.outputPath('power-transfers-dc.png') });
			if (touch) {
				// Record the text painted into the exported PNG, including lines wrapped on a phone.
				await page.evaluate(() => {
					const text: string[] = [];
					Reflect.set(window, 'exportedCanvasText', text);
					const fillText = CanvasRenderingContext2D.prototype.fillText;
					CanvasRenderingContext2D.prototype.fillText = function (line, x, y, maxWidth) {
						if (!this.canvas.isConnected) text.push(line);
						fillText.call(this, line, x, y, maxWidth);
					};
				});
				const pendingDownload = page.waitForEvent('download');
				await exportPng(toolbar);
				const exportPath = testInfo.outputPath('power-transfers-export.png');
				await (await pendingDownload).saveAs(exportPath);
				const text = await page.evaluate(() =>
					(Reflect.get(window, 'exportedCanvasText') as string[]).join(' ')
				);
				expect(text).toContain('Flows, DC OPF');
				expect(text).toContain('Width 0–100 MW');
				expect(text).not.toContain('Limits,');
			}

			const closeNetwork = page.getByRole('button', { name: 'Close network panel', exact: true });
			if (!(await closeNetwork.isVisible()))
				await page.getByRole('button', { name: 'Network', exact: true }).click();
			const lookup = page.getByRole('combobox', { name: 'Bus lookup', exact: true });
			await lookup.fill('2');
			await lookup.press('Enter');
			if (touch && (await closeNetwork.isVisible())) await closeNetwork.tap();
			await expect(inspector.getByRole('heading', { name: 'Bus 2' })).toBeVisible();
			await expect(inspector.locator('dl div').filter({ hasText: 'Generation' })).toContainText(
				'10 MW'
			);
			await expect(inspector.locator('dl div').filter({ hasText: 'Demand' })).toContainText(
				'90 MW'
			);
			await expect(inspector.locator('dl div').filter({ hasText: 'Capacity' })).toContainText(
				'100 MW'
			);
		});
		for (const uniform of [false, true])
			test(
				uniform
					? 'uniform voltage violations remain emphasized'
					: 'station ranges retain the violating bus',
				async ({ page }, testInfo) => {
					test.setTimeout(120_000);
					await page.route('**/api/compute', (route) =>
						route.fulfill({ json: { enabled: false } })
					);
					await page.route('**/api/cases', (route) => route.fulfill({ json: [] }));
					await page.goto('/');
					await page.emulateMedia({ reducedMotion: 'reduce' });
					await expect(page.getByText('No cases', { exact: true })).toBeVisible();
					await page.locator('input[type="file"]').setInputFiles([
						{
							name: 'stationlimits.m',
							mimeType: 'text/plain',
							buffer: Buffer.from(voltageCase(uniform))
						},
						{
							name: 'stationlimits-coords.csv',
							mimeType: 'text/csv',
							buffer: Buffer.from('bus_i,lat,lon\n1,34,-81\n2,34,-81\n')
						}
					]);
					await expect(page.locator('.solvecard')).toHaveAttribute('data-calculation', 'dcopf', {
						timeout: 60_000
					});
					if (touch) await showNetwork(page);
					const calculation = page.getByRole('combobox', { name: 'Calculation', exact: true });
					await expect(calculation).toBeEnabled({ timeout: 60000 });
					await runCalculation(page, 'acpf');
					await expect(calculation).toBeEnabled();
					await page.getByRole('button', { name: 'Show diagram', exact: true }).click();
					const toolbar = page.getByRole('toolbar', { name: 'Single-line diagram' });
					await expectView(toolbar, 'Limits');
					await hideBasemap(toolbar);
					await selectLayout(toolbar, 'Schematic');
					await expect(page.locator('.tx-stage .status')).toHaveCount(0);
					const panel = page.getByRole('region', { name: 'Limits', exact: true });
					await expect(panel.locator('.stat').filter({ hasText: 'High voltage' })).toContainText(
						uniform ? '2' : '1'
					);
					await expandResults(page);
					await expect(panel).toContainText(uniform ? '1.0800–1.0800 pu' : '1.0200–1.0800 pu');
					const diagram = page.locator('.tx-stage');
					const exceptions = diagram.getByText(`Flagged (${uniform ? 2 : 1})`, { exact: true });
					await exceptions.click();
					await expect(
						diagram.getByRole('listitem').filter({ hasText: 'High' }).first()
					).toContainText('Bus 1');
					await exceptions.click();
					await panel.locator('details.station summary').click();
					await expect(panel.getByRole('button', { name: /pu, bus 1$/ }).last()).toBeVisible();
					await expect(
						panel.getByRole('button', { name: /^1\.0800 pu, bus \d+$/ }).first()
					).toBeVisible();
					await fit(toolbar);
					await page.evaluate(
						() =>
							new Promise<void>((resolve) =>
								requestAnimationFrame(() => requestAnimationFrame(() => resolve()))
							)
					);
					await page.screenshot({
						path: testInfo.outputPath(
							uniform ? 'uniform-voltage-violation.png' : 'station-voltage-range.png'
						)
					});
					await panel.getByRole('button', { name: /^\+[\d.]+ pu, bus 1$/ }).click();
					await expect(
						page.getByRole('region', { name: 'Power-flow equipment results' })
					).toContainText('Voltage (pu) 1.0800');
					if (!touch && !uniform) {
						const pending = page.waitForEvent('download');
						await exportPng(toolbar);
						await (await pending).saveAs(testInfo.outputPath('station-limits-export.png'));
						await page.getByRole('button', { name: 'Show map', exact: true }).click();
						await runCalculation(page, 'socwr');
						await expect(calculation).toBeEnabled();
						await page.getByRole('button', { name: 'Show diagram', exact: true }).click();
						await expect(page.getByRole('region', { name: 'Limits', exact: true })).toBeVisible();
						await selectView(toolbar, 'Prices');
						const economics = page.getByRole('region', { name: 'Prices' });
						await expect(
							economics
								.locator('.stat')
								.filter({ hasText: /^(Lowest|Price)/ })
								.first()
						).toBeVisible();
						// The Prices card shows the price range; lines at limit are in the Network panel.
						await expect(
							economics.locator('.stat').filter({ hasText: 'Lines at limit' })
						).toHaveCount(0);
					}
				}
			);
		test('a balanced case opens as a single-line diagram and returns to the map', async ({
			page
		}, testInfo) => {
			test.setTimeout(120_000);
			await page.route('**/api/compute', (route) => route.fulfill({ json: { enabled: false } }));
			await page.route('**/api/cases', (route) => route.fulfill({ json: [] }));
			await page.goto('/');
			await expect(page.getByText('No cases', { exact: true })).toBeVisible();
			await page.locator('input[type="file"]').setInputFiles([
				{ name: 'txdiagram-coords.csv', mimeType: 'text/csv', buffer: Buffer.from(COORDS) },
				{ name: 'txdiagram.m', mimeType: 'text/plain', buffer: Buffer.from(CASE) }
			]);
			await expect(page.locator('.solvecard')).toHaveAttribute('data-calculation', 'dcopf', {
				timeout: 60_000
			});

			if (touch) await showNetwork(page);
			await page.getByRole('button', { name: 'Show diagram', exact: true }).click();
			const canvas = page.getByRole('application', { name: 'Single-line diagram' });
			await expect(canvas).toBeVisible();
			await expect(page.locator('.maplibregl-ctrl-attrib')).toHaveCount(0);
			const toolbar = page.getByRole('toolbar', { name: 'Single-line diagram' });
			await displayOptions(toolbar, true);
			await expect(toolbar).toContainText('Geographic');
			await page.emulateMedia({ reducedMotion: 'reduce' });
			if (touch) {
				expect(await page.evaluate(() => matchMedia('(pointer: coarse)').matches)).toBe(true);
				for (const name of ['Zoom in', 'Zoom out', 'Fit', 'Show map', 'Display']) {
					const button = toolbar.getByRole('button', { name, exact: true });
					const bounds = (await button.boundingBox())!;
					expect(bounds.height).toBeGreaterThanOrEqual(44);
					expect(bounds.x).toBeGreaterThanOrEqual(0);
					expect(bounds.x + bounds.width).toBeLessThanOrEqual(390);
				}
			}
			await displayOptions(toolbar, false);

			await expectView(toolbar, 'Limits');
			// DC solves no voltage magnitudes, so the limits view counts lines only.
			const limits = page.getByRole('region', { name: 'Limits', exact: true });
			await expect(limits.locator('.stat').first()).toBeVisible();
			await expect(limits).not.toContainText('voltage');
			// Every view is named in the mobile selector; desktop keeps keyboard-operable radios.
			const viewNames = ['Voltage', 'Limits', 'Prices', 'Flows', 'Map colours'];
			if (touch) {
				const chooser = toolbar.getByRole('combobox', { name: 'Diagram view', exact: true });
				await expect(chooser).toBeVisible();
				await expect(chooser.locator('option')).toHaveText(viewNames);
				expect((await toolbar.boundingBox())!.height).toBeLessThan(70);
				await expect(toolbar.getByLabel('Basemap')).toHaveCount(0);
				await selectView(toolbar, 'Prices');
				const economics = page.getByRole('region', { name: 'Prices', exact: true });
				await expect(economics.getByLabel('Scale')).toHaveCount(0);
				await expect(
					page.locator('.tx-stage .legend').getByRole('button', { name: 'Show details' })
				).toBeVisible();
				await page.screenshot({ path: testInfo.outputPath('mobile-economics-summary.png') });
				await expandResults(page);
				await expect(economics.getByLabel('Scale')).toBeVisible();
				// The explanations live in the docs, one link away.
				await expect(economics.getByRole('link', { name: 'Prices docs' })).toHaveAttribute(
					'href',
					/transmission-diagrams\.html#opf-economics$/
				);
				await page.screenshot({ path: testInfo.outputPath('mobile-economics-expanded.png') });
				await page.locator('.tx-stage .legend').evaluate((el) => {
					el.scrollTop = 0;
				});
				await page
					.locator('.tx-stage .legend')
					.getByRole('button', { name: 'Hide details' })
					.click();
				await expect(economics.getByLabel('Scale')).toHaveCount(0);
				await page.setViewportSize({ width: 320, height: 740 });
				for (const control of await toolbar.locator('button, select').all()) {
					const box = (await control.boundingBox())!;
					expect(box.x).toBeGreaterThanOrEqual(0);
					expect(box.x + box.width).toBeLessThanOrEqual(320);
					expect(box.height).toBeGreaterThanOrEqual(44);
				}
				await page.setViewportSize({ width: 390, height: 844 });
			} else {
				await expect(toolbar.getByRole('radio')).toHaveText(viewNames);
				// The views fit on one row: none is clipped.
				for (const radio of await toolbar.getByRole('radio').all())
					expect(await radio.evaluate((el) => el.scrollWidth <= el.clientWidth)).toBe(true);
				await toolbar.getByRole('radio', { name: 'Limits', exact: true }).focus();
				await page.keyboard.press('ArrowRight');
				await expect(toolbar.getByRole('radio', { name: 'Prices', exact: true })).toBeFocused();
				await expectView(toolbar, 'Prices');
			}

			await selectView(toolbar, 'Voltage');
			// The key sits in the right-hand card, as on every other view. It names the levels
			// drawn, the transformer, and the engine's substation grouping: buses 1 and 2 share a
			// coordinate, so four buses make three substations.
			const key = page.locator('.tx-stage .legend').getByRole('region', { name: 'Voltage' });
			await expect(key.getByRole('list', { name: 'Voltage levels' })).toContainText('345 kV');
			await expect(key.getByRole('list', { name: 'Voltage levels' })).toContainText('138 kV');
			await expect(key).toContainText('Transformer');
			await expect(key).toContainText('Out of service');
			// Levels are grouped under their tier, named in sentence case.
			await expect(key.getByText('Extra high voltage', { exact: true })).toBeVisible();
			await expect(key.getByText('High voltage', { exact: true })).toBeVisible();
			await expect(key.locator('.stat').filter({ hasText: 'Substations' })).toContainText('3');
			await expect(key.locator('.stat').filter({ hasText: 'Buses' })).toContainText('4');
			// Each symbol is listed once.
			await expect(key.getByText('Generator', { exact: true })).toHaveCount(1);
			// A level in the key highlights on hover and stays highlighted when clicked.
			const level = key.getByRole('button', { name: /345 kV/ });
			await level.click();
			await expect(level).toHaveAttribute('aria-pressed', 'true');
			await level.click();
			await expect(level).toHaveAttribute('aria-pressed', 'false');
			await displayOptions(toolbar, true);
			await toolbar.getByRole('button', { name: 'Fit', exact: true }).focus();
			await displayOptions(toolbar, false);
			await page.mouse.move(0, 0);

			// Once solved, the drawing can colour by results: the bus series and branch loading.
			await page.screenshot({ path: testInfo.outputPath('transmission-voltage.png') });
			await selectView(toolbar, 'Limits');
			await expandResults(page);
			const toolbarBox = (await toolbar.boundingBox())!;
			const legendBox = (await page.locator('.tx-stage .legend').boundingBox())!;
			// The summary never covers the toolbar: below it on a phone, docked beside it on a
			// wide screen.
			if (touch) expect(legendBox.y).toBeGreaterThan(toolbarBox.y + toolbarBox.height);
			else
				await expect
					.poll(async () => {
						const bar = (await toolbar.boundingBox())!;
						const summary = (await page.locator('.tx-stage .legend').boundingBox())!;
						return summary.x - (bar.x + bar.width);
					})
					.toBeGreaterThanOrEqual(0);
			if (!touch) {
				// The key stays clear of the panel, which holds the solver's result.
				const panelBox = (await page.locator('[data-panel-dock="left"]').boundingBox())!;
				expect(
					legendBox.x >= panelBox.x + panelBox.width || legendBox.y >= panelBox.y + panelBox.height
				).toBe(true);
			}
			await page.locator('.tx-stage details.settings summary').click();
			const warning = page.getByRole('spinbutton', { name: 'Warn at (%)' });
			await warning.fill('10');
			await warning.press('Tab');
			await expect(page.locator('.tx-stage .legend .key')).toContainText('<10%');
			await expect(page.locator('.tx-stage .legend .key')).toContainText('≥10%');
			await page.locator('.tx-stage details.settings summary').click();
			await canvas.press('f');
			await expect(
				page.getByRole('button', { name: /Branch \d+, .* percent, warning/ }).first()
			).toBeVisible();
			const gauge = page.getByRole('button', { name: /Branch \d+, .* percent, warning/ }).first();
			if (touch) expect((await gauge.boundingBox())!.height).toBeGreaterThanOrEqual(44);
			expect(legendBox.x + legendBox.width).toBeLessThanOrEqual(touch ? 390 : 1280);
			await expect(key).toHaveCount(0);

			await canvas.focus();
			await canvas.press('+');
			await canvas.press('f');
			await page.screenshot({ path: testInfo.outputPath('transmission-diagram.png') });

			// The substation overview is laid out in the engine and shares the frame of the geography.
			await selectLayout(toolbar, 'Schematic');
			await expect(page.locator('.tx-stage .status')).toHaveCount(0);
			await page.screenshot({ path: testInfo.outputPath('transmission-schematic.png') });
			await selectLayout(toolbar, 'Geographic');

			await page.getByRole('button', { name: 'Show map', exact: true }).click();
			await expect(canvas).toHaveCount(0);
			await expect(page.locator('.maplibregl-canvas')).toBeVisible();
			// The network panel's Show diagram is the one diagram toggle; the map has none of its own.
			await expect(page.getByRole('button', { name: 'Single-line diagram' })).toHaveCount(0);
			if (touch) await page.getByRole('button', { name: 'Network', exact: true }).tap();
			await expect(page.getByRole('button', { name: 'Show diagram', exact: true })).toBeVisible();

			// AC results retain a fixed per-unit key rather than an autoscaled or uniform-grey key.
			const calculation = page.getByRole('combobox', { name: 'Calculation', exact: true });
			await runCalculation(page, 'acpf');
			await expect(calculation).toBeEnabled();
			await page.getByRole('button', { name: 'Show diagram', exact: true }).click();
			await expectView(toolbar, 'Limits');
			await expandResults(page);
			await page.locator('.tx-stage details.settings summary').click();
			await expect(page.getByRole('spinbutton', { name: 'Warn at (%)' })).toHaveValue('10');
			await page.getByRole('spinbutton', { name: 'Warn at (%)' }).fill('90');
			await page.getByRole('spinbutton', { name: 'Warn at (%)' }).press('Tab');
			await expect(page.locator('.tx-stage .legend')).toContainText('0.90');
			await expect(page.locator('.tx-stage .legend')).toContainText('1.10 pu');
			// AC results key the voltage markers too.
			await expect(page.locator('.tx-stage .legend .key')).toContainText('Low voltage');
			await expect(page.locator('.tx-stage .legend .key')).toContainText('High voltage');
			await page.locator('.tx-stage details.settings summary').click();
			await hideBasemap(toolbar);
			await fit(toolbar);
			await page.evaluate(
				() =>
					new Promise<void>((resolve) =>
						requestAnimationFrame(() => requestAnimationFrame(() => resolve()))
					)
			);
			await page.screenshot({ path: testInfo.outputPath('transmission-ac-results.png') });
			const pendingDownload = page.waitForEvent('download');
			await exportPng(toolbar);
			const imagePath = testInfo.outputPath('operating-limits-export.png');
			await (await pendingDownload).saveAs(imagePath);
			const png = await readFile(imagePath);
			expect(png.subarray(1, 4).toString()).toBe('PNG');
			expect(png.readUInt32BE(16)).toBeGreaterThanOrEqual(page.viewportSize()!.width);
			expect(png.readUInt32BE(20)).toBeGreaterThan(page.viewportSize()!.height);

			await selectView(toolbar, 'Flows');
			const transfer = page.getByRole('region', { name: 'Flows' });
			await expect(transfer.locator('.stat').filter({ hasText: 'Largest flow' })).toBeVisible();
			const overloads = page.getByText('Overloads (1)', { exact: true });
			await expect(overloads).toBeVisible();
			await overloads.click();
			await expect(page.getByRole('button', { name: /^Line 1 \(/ }).first()).toBeVisible();
			await overloads.click();
			await transfer.getByText('Largest flows', { exact: true }).click();
			await transfer
				.getByRole('button', { name: /^Line / })
				.first()
				.click();
			const inspector = page.getByRole('region', { name: 'Selected equipment' });
			await expect(inspector.getByRole('columnheader', { name: 'Reactive' })).toBeVisible();
			await expect(inspector.getByRole('cell', { name: /[+-]?[0-9.]+ MVAr/ })).toHaveCount(2);
			await expect(inspector.locator('dl div').filter({ hasText: /^Flow/ })).toContainText('MW');
			await page.screenshot({ path: testInfo.outputPath('power-transfers-ac.png') });
		});
	});
}

// Legacy and partial backend results must not become healthy or zero-flow summaries.
for (const previousFormulation of [false, true]) {
	test(`partial backend data preserves unknowns${previousFormulation ? ' while awaiting the selected calculation' : ''}`, async ({
		page
	}, testInfo) => {
		const { mockDataRoutes, SOLUTION } = await import('./fixtures/backend-case.js');
		await mockDataRoutes(page);
		await page.route('**/api/cases/case1/solution', (route) =>
			route.fulfill({
				json: {
					...SOLUTION,
					formulation: previousFormulation ? 'acpf' : 'dcopf',
					flows: []
				}
			})
		);
		await page.goto('/');
		await page.getByRole('button', { name: 'Show diagram', exact: true }).click();
		const toolbar = page.getByRole('toolbar', { name: 'Single-line diagram' });
		const operating = page.getByRole('region', { name: 'Limits', exact: true });
		// No line could be checked: the count is a dash, not a healthy zero.
		await expect(operating.locator('.stat').filter({ hasText: 'Over limit' })).toContainText('—');
		await expect(operating.locator('.stat').filter({ hasText: 'No flow' })).toContainText('1');
		await expect(operating.locator('.stat.ok')).toHaveCount(0);
		await selectView(toolbar, 'Flows');
		const transfer = page.getByRole('region', { name: 'Flows', exact: true });
		if (previousFormulation) await expect(transfer).toContainText('No result');
		else await expect(transfer.locator('.stat').filter({ hasText: 'No flow' })).toContainText('1');
		await expect(transfer).not.toContainText('Zero flow');
		if (!previousFormulation) {
			await selectView(toolbar, 'Prices');
			const economics = page.getByRole('region', { name: 'Prices', exact: true });
			// No thermal binding data: no count of lines at limit is claimed.
			await expect(economics.locator('.stat').filter({ hasText: 'Lines at limit' })).toHaveCount(0);
			await page.screenshot({ path: testInfo.outputPath('unknown-thermal-status.png') });
		}
	});
}
