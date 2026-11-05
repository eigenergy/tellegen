import { expect, test } from './fixtures/page-errors.js';

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
 1 2 0.001 0.05 0 300 300 300 1 0 1 -360 360;
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

for (const touch of [false, true]) {
	test.describe(touch ? 'phone touch' : 'desktop mouse', () => {
		test.use(
			touch ? { viewport: { width: 390, height: 844 }, hasTouch: true, isMobile: true } : {}
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
			const colour = toolbar.getByLabel('Colour by');
			await expect(colour).toBeEnabled({ timeout: 60_000 });
			await page.screenshot({ path: testInfo.outputPath('transmission-voltage.png') });
			await colour.selectOption('results');
			const toolbarBox = (await toolbar.boundingBox())!;
			const legendBox = (await page.locator('.tx-stage .legend').boundingBox())!;
			expect(legendBox.y).toBeGreaterThan(toolbarBox.y + toolbarBox.height);
			if (!touch) {
				const solverBox = (await page.locator('.solvecard').boundingBox())!;
				expect(legendBox.x + legendBox.width).toBeLessThanOrEqual(solverBox.x);
			}
			await expect(page.locator('.tx-stage .legend')).toContainText('Loading');
			await expect(page.locator('.tx-stage .legend')).toContainText('amber 90–100 %');
			await expect(page.locator('.tx-stage .legend')).toContainText('red above 100 %');
			expect(legendBox.x + legendBox.width).toBeLessThanOrEqual(touch ? 390 : 1280);
			await expect(key).toHaveCount(0);

			await canvas.focus();
			await canvas.press('+');
			await canvas.press('f');
			await page.screenshot({ path: testInfo.outputPath('transmission-diagram.png') });

			// The substation overview is laid out in the engine and shares the frame of the geography.
			const schematic = toolbar.getByRole('button', { name: 'Schematic', exact: true });
			await schematic.click();
			await expect(schematic).toHaveAttribute('aria-pressed', 'true');
			await expect(page.locator('.tx-stage .status')).toHaveCount(0);
			await page.screenshot({ path: testInfo.outputPath('transmission-schematic.png') });
			await toolbar.getByRole('button', { name: 'Geographic', exact: true }).click();
			await expect(schematic).toHaveAttribute('aria-pressed', 'false');

			await toolbar.getByRole('button', { name: 'Close', exact: true }).click();
			await expect(canvas).toHaveCount(0);
			await expect(page.locator('.maplibregl-canvas')).toBeVisible();
			if (touch) await page.getByRole('button', { name: 'Network', exact: true }).tap();
			await expect(page.getByRole('button', { name: 'Show diagram', exact: true })).toBeVisible();

			// AC results retain a fixed per-unit key rather than an autoscaled or uniform-grey key.
			const calculation = page.getByRole('combobox', { name: 'Calculation', exact: true });
			await calculation.selectOption('acpf');
			await expect(calculation).toBeEnabled();
			await page.getByRole('button', { name: 'Show diagram', exact: true }).click();
			if (touch) await page.getByRole('button', { name: 'Close network panel' }).tap();
			await toolbar.getByLabel('Colour by').selectOption('results');
			await expect(page.locator('.tx-stage .legend')).toContainText('0.90');
			await expect(page.locator('.tx-stage .legend')).toContainText('1.10 pu');
			await expect(page.locator('.tx-stage .legend')).toContainText('case limits apply');
			await toolbar.getByLabel('Basemap').uncheck();
			await toolbar.getByRole('button', { name: 'Fit', exact: true }).click();
			await page.evaluate(
				() =>
					new Promise<void>((resolve) =>
						requestAnimationFrame(() => requestAnimationFrame(() => resolve()))
					)
			);
			await page.screenshot({ path: testInfo.outputPath('transmission-ac-results.png') });
		});
	});
}
