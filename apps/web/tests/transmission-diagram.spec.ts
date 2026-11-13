import { expect, test } from './fixtures/page-errors.js';
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
	const more = page.locator('.tx-stage .legend').getByRole('button', { name: 'Details ▾' });
	if (await more.isVisible()) await more.click();
	await expect(
		page.locator('.tx-stage .legend').getByRole('button', { name: 'Summary ▴' })
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
	const select = toolbar.getByRole('combobox', { name: 'Diagram layout', exact: true });
	if (await select.isVisible()) {
		await select.selectOption({ label });
		await expect(select.locator('option:checked')).toHaveText(label);
	} else {
		const button = toolbar.getByRole('button', { name: label, exact: true });
		await button.click();
		await expect(button).toHaveAttribute('aria-pressed', 'true');
	}
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
	await toolbar.getByRole('button', { name: 'PNG', exact: true }).click();
}

for (const touch of [false, true]) {
	test.describe(touch ? 'phone touch' : 'desktop mouse', () => {
		test.use(
			touch ? { viewport: { width: 390, height: 844 }, hasTouch: true, isMobile: true } : {}
		);
		test('utilization at the rating remains amber and selectable', async ({ page }, testInfo) => {
			test.setTimeout(120_000);
			const limited = voltageCase(false)
				.replace('2 2 0 0 0 0 1', '2 2 90 0 0 0 1')
				.replace('1000 1000 1000', '80 80 80')
				.replace('0.1 5 0; 2 0 0 3 0.1 5 0;', '0.1 5 0; 2 0 0 3 0.1 500 0;');
			await page.route('**/api/cases', (route) => route.fulfill({ json: [] }));
			await page.route('**/api/compute', (route) => route.fulfill({ json: { enabled: false } }));
			await page.goto('/');
			await page.emulateMedia({ reducedMotion: 'reduce' });
			await expect(page.getByText('no default cases loaded')).toBeVisible();
			await page.locator('input[type="file"]').setInputFiles([
				{
					name: 'limited-coords.csv',
					mimeType: 'text/csv',
					buffer: Buffer.from('bus_i,lat,lon\n1,34,-81\n2,34,-80.9\n')
				},
				{ name: 'limited.m', mimeType: 'text/plain', buffer: Buffer.from(limited) }
			]);
			if (touch) await page.getByRole('button', { name: 'Solver', exact: true }).tap();
			await expect(page.locator('.solvecard')).toContainText('DC OPF', { timeout: 60000 });
			if (touch) await page.getByRole('button', { name: 'Network', exact: true }).tap();
			await page.getByRole('button', { name: 'Show diagram', exact: true }).click();
			if (touch) await page.getByRole('button', { name: 'Close network panel' }).tap();
			const toolbar = page.getByRole('toolbar', { name: 'Single-line diagram' });
			await hideBasemap(toolbar);
			await toolbar.getByRole('button', { name: 'Fit', exact: true }).click();
			const panel = page.getByRole('region', { name: 'Operating limits', exact: true });
			await expect(panel).toContainText('1 at rating');
			await expect(panel).toContainText('0 overloads');
			const gauge = page.getByRole('button', { name: /Branch 1, .*percent, at rating/ });
			await expect(gauge).toHaveClass(/warning/);
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
			await toolbar.getByRole('button', { name: 'Fit', exact: true }).click();
			await expect(gauge).toHaveAttribute('aria-pressed', 'true');
			const diagram = page.locator('.tx-stage');
			const inspector = diagram.getByRole('region', { name: 'Selected equipment' });
			await expect(inspector).toContainText('+80 MW');
			await expect(inspector).toContainText('100%');
			await expect(inspector).toContainText('0 MW');
			await inspector.getByRole('button', { name: 'Clear selection' }).click();
			await expect(inspector).toHaveCount(0);
			const exceptions = diagram.getByText('Exceptions (1)', { exact: true });
			if (touch) await exceptions.tap();
			else await exceptions.press('Enter');
			const search = diagram.getByRole('searchbox', { name: 'Search exceptions' });
			await search.fill('branch 99');
			await expect(diagram.getByText('No matching exceptions.')).toBeVisible();
			await search.fill('branch 1');
			const row = diagram.getByRole('button', { name: /Branch 1, 1 → 2/ });
			if (touch) await row.tap();
			else await row.press('Enter');
			await expect(inspector.getByRole('heading', { name: 'Branch 1' })).toBeFocused();
			await expect(inspector).toContainText('Effective rating');
			await expect(inspector).toContainText('Unavailable');
			await inspector.getByRole('button', { name: 'Frame', exact: true }).click();
			if (touch) await inspector.getByRole('button', { name: 'Clear selection' }).tap();
			else await inspector.getByRole('button', { name: 'Frame', exact: true }).press('Escape');
			await expect(inspector).toHaveCount(0);

			await selectView(toolbar, 'OPF economics');
			const economics = diagram.getByRole('region', { name: 'OPF economics' });
			await expect(economics).toContainText('Total LMP');
			await expect(economics).toContainText('objective units/MW');
			await economics.getByLabel('Price scale').selectOption('3');
			await expect(economics).toContainText('200 objective units/MW');
			await economics.getByText('Binding thermal limits (1)', { exact: true }).click();
			const binding = economics.getByRole('button', { name: /Branch 1.*marginal relief/ });
			if (touch) await binding.tap();
			else await binding.press('Enter');
			const heading = inspector.getByRole('heading', { name: 'Branch 1' });
			await expect(heading).toBeFocused();
			await expect(heading).toBeInViewport({ ratio: 1 });
			await expect(economics).toContainText('Binding, with positive marginal relief');
			await expect(economics).toContainText(/Marginal rating relief [1-9]/);
			await expect(diagram.getByLabel('Branch utilization gauges')).toHaveCount(0);
			await page.screenshot({ path: testInfo.outputPath('opf-economics.png') });

			await selectView(toolbar, 'Power transfers');
			const transfers = diagram.getByRole('region', { name: 'Power transfers' });
			await expect(transfers).toContainText('0 to 500 MW');
			await transfers.getByLabel('Width scale').selectOption('100');
			await expect(transfers).toContainText('0 to 100 MW');
			await expect(inspector.getByRole('row', { name: /From bus 1/ })).toContainText('+80 MW');
			await expect(inspector.getByRole('row', { name: /To bus 2/ })).toContainText('-80 MW');
			await expect(inspector.getByRole('columnheader', { name: 'Reactive' })).toHaveCount(0);
			await expect(inspector).toContainText('Bus 1 → bus 2');
			await expect(diagram.getByText('Overloads (0)', { exact: true })).toBeVisible();
			await inspector.getByRole('button', { name: 'Clear selection' }).click();
			await transfers.getByText('Largest active transfers', { exact: true }).click();
			const ranked = transfers.getByRole('button', { name: /Branch 1.*Bus 1 → bus 2.*80 MW/ });
			if (touch) await ranked.tap();
			else await ranked.press('Enter');
			await expect(inspector.getByRole('heading', { name: 'Branch 1' })).toBeVisible();
			await expect(inspector.getByRole('heading', { name: 'Branch 1' })).toBeFocused();
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
				expect(text).toContain('Power transfers · dcopf · Result available');
				expect(text).toContain('Blue line width: 0–100 MW');
				expect(text).toContain('Red gauges show overload in percent of rating');
				expect(text).not.toContain('Operating limits');
			}

			const closeNetwork = page.getByRole('button', { name: 'Close network panel', exact: true });
			if (!(await closeNetwork.isVisible()))
				await page.getByRole('button', { name: 'Network', exact: true }).click();
			const lookup = page.getByRole('combobox', { name: 'Bus lookup', exact: true });
			await lookup.fill('2');
			await lookup.press('Enter');
			if (touch && (await closeNetwork.isVisible())) await closeNetwork.tap();
			await expect(inspector.getByRole('heading', { name: 'Bus 2' })).toBeVisible();
			await expect(
				inspector.locator('dl div').filter({ hasText: 'Solved generation' })
			).toContainText('10 MW');
			await expect(inspector.locator('dl div').filter({ hasText: 'Model demand' })).toContainText(
				'90 MW'
			);
			await expect(
				inspector.locator('dl div').filter({ hasText: 'Installed capacity' })
			).toContainText('100 MW');
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
					await expect(page.getByText('no default cases loaded')).toBeVisible();
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
					if (touch) await page.getByRole('button', { name: 'Solver', exact: true }).tap();
					await expect(page.locator('.solvecard')).toContainText('DC OPF', { timeout: 60_000 });
					if (touch) await page.getByRole('button', { name: 'Network', exact: true }).tap();
					const calculation = page.getByRole('combobox', { name: 'Calculation', exact: true });
					await expect(calculation).toBeEnabled({ timeout: 60000 });
					await runCalculation(page, 'acpf');
					if (touch) await page.getByRole('button', { name: 'Network', exact: true }).tap();
					await expect(calculation).toBeEnabled();
					await page.getByRole('button', { name: 'Show diagram', exact: true }).click();
					if (touch) await page.getByRole('button', { name: 'Close network panel' }).tap();
					const toolbar = page.getByRole('toolbar', { name: 'Single-line diagram' });
					await expectView(toolbar, 'Operating limits');
					await hideBasemap(toolbar);
					await selectLayout(toolbar, 'Schematic');
					await expect(page.locator('.tx-stage .status')).toHaveCount(0);
					const panel = page.getByRole('region', { name: 'Operating limits', exact: true });
					await expect(panel).toContainText(uniform ? '↑ 2 high' : '↑ 1 high');
					await expandResults(page);
					await expect(panel).toContainText(uniform ? '1.0800–1.0800 pu' : '1.0200–1.0800 pu');
					const diagram = page.locator('.tx-stage');
					const exceptions = diagram.getByText(`Exceptions (${uniform ? 2 : 1})`, { exact: true });
					await exceptions.click();
					await expect(
						diagram.getByRole('listitem').filter({ hasText: 'above maximum' }).first()
					).toContainText('Bus 1');
					await exceptions.click();
					await panel.locator('details.station summary').click();
					await expect(panel.getByRole('button', { name: /Worst violation: bus 1/ })).toBeVisible();
					await expect(
						panel.getByRole('button', { name: /Maximum: bus 1, 1.0800 pu/ })
					).toBeVisible();
					await toolbar.getByRole('button', { name: 'Fit', exact: true }).click();
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
					await panel.getByRole('button', { name: /Worst violation: bus 1/ }).click();
					await expect(
						page.getByRole('region', { name: 'Power-flow equipment results' })
					).toContainText('Voltage (pu) 1.0800');
					if (!touch && !uniform) {
						const pending = page.waitForEvent('download');
						await exportPng(toolbar);
						await (await pending).saveAs(testInfo.outputPath('station-limits-export.png'));
						await toolbar.getByRole('button', { name: 'Close', exact: true }).click();
						await runCalculation(page, 'socwr');
						await expect(calculation).toBeEnabled();
						await page.getByRole('button', { name: 'Show diagram', exact: true }).click();
						await expect(
							page.getByRole('region', { name: 'Operating limits', exact: true })
						).toContainText('Relaxation');
						await selectView(toolbar, 'OPF economics');
						const economics = page.getByRole('region', { name: 'OPF economics' });
						await expect(economics).toContainText('Relaxation');
						await expect(economics).toContainText(
							'Thermal enforcement and marginal relief unavailable'
						);
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
			await expect(page.getByText('no default cases loaded')).toBeVisible();
			await page.locator('input[type="file"]').setInputFiles([
				{ name: 'txdiagram-coords.csv', mimeType: 'text/csv', buffer: Buffer.from(COORDS) },
				{ name: 'txdiagram.m', mimeType: 'text/plain', buffer: Buffer.from(CASE) }
			]);
			if (touch) await page.getByRole('button', { name: 'Solver', exact: true }).tap();
			await expect(page.locator('.solvecard')).toContainText('DC OPF', { timeout: 60_000 });

			if (touch) await page.getByRole('button', { name: 'Network', exact: true }).tap();
			await page.getByRole('button', { name: 'Show diagram', exact: true }).click();
			if (touch) await page.getByRole('button', { name: 'Close network panel' }).tap();
			const canvas = page.getByRole('application', { name: 'Single-line diagram' });
			await expect(canvas).toBeVisible();
			await expect(page.locator('.maplibregl-ctrl-attrib')).toHaveCount(0);
			const toolbar = page.getByRole('toolbar', { name: 'Single-line diagram' });
			await expect(toolbar).toContainText('Geographic');
			await page.emulateMedia({ reducedMotion: 'reduce' });
			if (touch) {
				expect(await page.evaluate(() => matchMedia('(pointer: coarse)').matches)).toBe(true);
				for (const name of ['Zoom in', 'Zoom out', 'Fit', 'Close']) {
					const button = toolbar.getByRole('button', { name, exact: true });
					const bounds = (await button.boundingBox())!;
					expect(bounds.height).toBeGreaterThanOrEqual(44);
					expect(bounds.x).toBeGreaterThanOrEqual(0);
					expect(bounds.x + bounds.width).toBeLessThanOrEqual(390);
				}
			}

			await expectView(toolbar, 'Operating limits');
			await expect(
				page.getByRole('region', { name: 'Operating limits', exact: true })
			).toContainText('Voltage not solved (DC)');
			// Every view is named in the mobile selector; desktop keeps keyboard-operable radios.
			const viewNames = [
				'Voltage levels',
				'Operating limits',
				'OPF economics',
				'Power transfers',
				'Map display'
			];
			if (touch) {
				const chooser = toolbar.getByRole('combobox', { name: 'Diagram view', exact: true });
				await expect(chooser).toBeVisible();
				await expect(chooser.locator('option')).toHaveText(viewNames);
				expect((await toolbar.boundingBox())!.height).toBeLessThan(125);
				await expect(toolbar.getByLabel('Basemap')).toHaveCount(0);
				await selectView(toolbar, 'OPF economics');
				const economics = page.getByRole('region', { name: 'OPF economics', exact: true });
				await expect(economics.getByLabel('Price scale')).toHaveCount(0);
				await expect(
					page.locator('.tx-stage .legend').getByRole('button', { name: 'Details ▾' })
				).toBeVisible();
				await page.screenshot({ path: testInfo.outputPath('mobile-economics-summary.png') });
				await expandResults(page);
				await expect(economics.getByLabel('Price scale')).toBeVisible();
				await economics
					.getByText('Loading, binding and marginal relief explained', { exact: true })
					.click();
				await expect(
					economics.getByText(/A binding limit can have zero marginal value/)
				).toBeVisible();
				await economics
					.locator('details.concepts')
					.evaluate((el) => el.scrollIntoView({ block: 'start' }));
				await page.screenshot({ path: testInfo.outputPath('mobile-economics-explained.png') });
				await page.locator('.tx-stage .legend').evaluate((el) => {
					el.scrollTop = 0;
				});
				await page.locator('.tx-stage .legend').getByRole('button', { name: 'Summary ▴' }).click();
				await expect(economics.getByLabel('Price scale')).toHaveCount(0);
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
				await toolbar.getByRole('radio', { name: 'Operating limits', exact: true }).focus();
				await page.keyboard.press('ArrowRight');
				await expect(
					toolbar.getByRole('radio', { name: 'OPF economics', exact: true })
				).toBeFocused();
				await expectView(toolbar, 'OPF economics');
			}

			await selectView(toolbar, 'Voltage levels');
			// The key names the levels drawn, the transformer, and the engine's substation grouping:
			// buses 1 and 2 share a coordinate, so four buses make three substations.
			const key = page.locator('.tx-stage details.key');
			await expect(key.getByRole('list', { name: 'Voltage levels' })).toContainText('345 kV');
			await expect(key.getByRole('list', { name: 'Voltage levels' })).toContainText('138 kV');
			await expect(key).toContainText('transformer');
			await expect(key).toContainText('out of service');
			// Levels are grouped under their tier.
			await expect(key).toContainText(/extra high voltage/i);
			await expect(key).toContainText(/high voltage/i);
			await expect(key).toContainText('3 substations, 4 buses');
			// A level in the key highlights on hover and stays highlighted when clicked.
			const level = key.getByRole('button', { name: /345 kV/ });
			await level.click();
			await expect(level).toHaveAttribute('aria-pressed', 'true');
			await level.click();
			await expect(level).toHaveAttribute('aria-pressed', 'false');
			await toolbar.getByRole('button', { name: 'Fit', exact: true }).focus();
			await page.mouse.move(0, 0);

			// Once solved, the drawing can colour by results: the bus series and branch loading.
			await page.screenshot({ path: testInfo.outputPath('transmission-voltage.png') });
			await selectView(toolbar, 'Operating limits');
			await expandResults(page);
			const toolbarBox = (await toolbar.boundingBox())!;
			const legendBox = (await page.locator('.tx-stage .legend').boundingBox())!;
			expect(legendBox.y).toBeGreaterThan(toolbarBox.y + toolbarBox.height);
			if (!touch) {
				const solverBox = (await page.locator('.solvecard').boundingBox())!;
				expect(legendBox.x + legendBox.width).toBeLessThanOrEqual(solverBox.x);
			}
			await page.locator('.tx-stage details.settings summary').click();
			await expect(page.locator('.tx-stage .legend')).toContainText('|P| / MW rating');
			const warning = page.getByRole('spinbutton', { name: 'Loading warning percent' });
			await warning.fill('10');
			await warning.press('Tab');
			await expect(page.locator('.tx-stage .legend')).toContainText(
				'Lines are grey below 10% of their rating'
			);
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

			await toolbar.getByRole('button', { name: 'Close', exact: true }).click();
			await expect(canvas).toHaveCount(0);
			await expect(page.locator('.maplibregl-canvas')).toBeVisible();
			// The map offers the diagram beside its zoom buttons, not only in the network panel.
			await expect(page.getByRole('button', { name: 'Single-line diagram' })).toBeVisible();
			if (touch) await page.getByRole('button', { name: 'Network', exact: true }).tap();
			await expect(page.getByRole('button', { name: 'Show diagram', exact: true })).toBeVisible();

			// AC results retain a fixed per-unit key rather than an autoscaled or uniform-grey key.
			const calculation = page.getByRole('combobox', { name: 'Calculation', exact: true });
			await runCalculation(page, 'acpf');
			if (touch) await page.getByRole('button', { name: 'Network', exact: true }).tap();
			await expect(calculation).toBeEnabled();
			await page.getByRole('button', { name: 'Show diagram', exact: true }).click();
			if (touch) await page.getByRole('button', { name: 'Close network panel' }).tap();
			await expectView(toolbar, 'Operating limits');
			await expandResults(page);
			await page.locator('.tx-stage details.settings summary').click();
			await expect(page.getByRole('spinbutton', { name: 'Loading warning percent' })).toHaveValue(
				'10'
			);
			await page.getByRole('spinbutton', { name: 'Loading warning percent' }).fill('90');
			await page.getByRole('spinbutton', { name: 'Loading warning percent' }).press('Tab');
			await expect(page.locator('.tx-stage .legend')).toContainText('0.90');
			await expect(page.locator('.tx-stage .legend')).toContainText('1.10 pu');
			await expect(page.locator('.tx-stage .legend')).toContainText('its case voltage limits');
			await expect(page.locator('.tx-stage .legend')).toContainText(
				'max(|S from|, |S to|) / MVA rating'
			);
			await page.locator('.tx-stage details.settings summary').click();
			await hideBasemap(toolbar);
			await toolbar.getByRole('button', { name: 'Fit', exact: true }).click();
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

			await selectView(toolbar, 'Power transfers');
			const transfer = page.getByRole('region', { name: 'Power transfers' });
			await expect(transfer).toContainText('AC power flow');
			const overloads = page.getByText('Overloads (1)', { exact: true });
			await expect(overloads).toBeVisible();
			await overloads.click();
			await expect(page.getByRole('button', { name: /Branch 1.*overload/ }).first()).toBeVisible();
			await overloads.click();
			await transfer.getByText('Largest active transfers', { exact: true }).click();
			await transfer
				.getByRole('button', { name: /^Branch / })
				.first()
				.click();
			const inspector = page.getByRole('region', { name: 'Selected equipment' });
			await expect(inspector.getByRole('columnheader', { name: 'Reactive' })).toBeVisible();
			await expect(inspector.getByRole('cell', { name: /[+-]?[0-9.]+ MVAr/ })).toHaveCount(2);
			await expect(inspector).toContainText('Active magnitude');
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
		const operating = page.getByRole('region', { name: 'Operating limits', exact: true });
		await expect(operating).toContainText('No line loading could be assessed.');
		await expect(operating).toContainText('1 line has no usable flow value.');
		await expect(operating.locator('.lead')).not.toHaveClass(/\bok\b/);
		await selectView(toolbar, 'Power transfers');
		const transfer = page.getByRole('region', { name: 'Power transfers', exact: true });
		await expect(transfer).toContainText(
			previousFormulation
				? 'Awaiting a result for this calculation.'
				: 'Flow values are unavailable for 1 in-service line.'
		);
		await expect(transfer).not.toContainText('No line carries power');
		if (!previousFormulation) {
			await selectView(toolbar, 'OPF economics');
			const economics = page.getByRole('region', { name: 'OPF economics', exact: true });
			await expect(economics).toContainText('Thermal binding information is unavailable');
			await expect(economics).not.toContainText('No reported thermal limit binds');
			await page.screenshot({ path: testInfo.outputPath('unknown-thermal-status.png') });
		}
	});
}
