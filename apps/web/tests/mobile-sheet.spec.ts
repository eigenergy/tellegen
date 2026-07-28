import { devices, expect, test } from '@playwright/test';

// The compact layout puts the control panel in a bottom sheet over the map.
// Two things that regress silently when the sheet's geometry changes:
//   - basemap attribution has to stay visible, which means above the sheet;
//   - what the last tap produced has to land above the fold, so a selection
//     leads the sheet body instead of sitting under the case stats.

// The device descriptor carries defaultBrowserType: 'webkit' and CI installs
// chromium only, so pin the browser and keep the phone's viewport, pixel ratio
// and touch input — which is what the coarse pointer rules key off.
test.use({ ...devices['iPhone 13'], browserName: 'chromium' });

const CASE3 = `function mpc = case3test
mpc.version = '2';
mpc.baseMVA = 100;
mpc.bus = [
 1 3 0  0  0 0 1 1 0 230 1 1.1 0.9;
 2 1 90 30 0 0 1 1 0 230 1 1.1 0.9;
 3 2 0  0  0 0 1 1 0 230 1 1.1 0.9;
];
mpc.gen = [
 1 0  0 300 -300 1 100 1 250 10 0 0 0 0 0 0 0 0 0 0 0;
 3 60 0 300 -300 1 100 1 270 10 0 0 0 0 0 0 0 0 0 0 0;
];
mpc.branch = [
 1 2 0.01 0.1 0 250 250 250 0 0 1 -360 360;
 1 3 0.01 0.1 0 250 250 250 0 0 1 -360 360;
 2 3 0.01 0.1 0 40 40 40 0 0 1 -360 360;
];
mpc.gencost = [
 2 0 0 3 0.11  5   0;
 2 0 0 3 0.085 1.2 0;
];
`;

const CASE3_COORDS = `bus_i,lat,lon
1,34.0,-81.1
2,34.1,-81.0
3,34.0,-80.9
`;

test('compact sheet: attribution stays clear and a selection leads the body', async ({ page }) => {
	await page.route('**/api/cases', (route) => {
		void route.fulfill({ json: [] });
	});

	await page.goto('/');
	// Touch sizing keys off the pointer, so a run without one silently skips half
	// of what this file covers.
	expect(await page.evaluate(() => matchMedia('(pointer: coarse)').matches)).toBe(true);
	await expect(page.getByText('no default cases loaded')).toBeVisible();
	await page.locator('input[type="file"]').setInputFiles([
		{ name: 'case3-coords.csv', mimeType: 'text/csv', buffer: Buffer.from(CASE3_COORDS) },
		{ name: 'case3congested.m', mimeType: 'text/plain', buffer: Buffer.from(CASE3) }
	]);
	await expect(page.locator('.solvecard')).toContainText('OPF solve', { timeout: 60_000 });

	// The panel is a sheet here, not the desktop card.
	const sheet = page.locator('aside.panel.sheet');
	await expect(sheet).toBeVisible();

	// The grab bar has to be worth aiming a thumb at.
	const headBox = await page.locator('.sheet-head').boundingBox();
	expect(headBox!.height).toBeGreaterThanOrEqual(44);

	// Basemap attribution is a licensing requirement: on screen, opaque, and
	// above the sheet rather than under it.
	await expect
		.poll(async () => {
			const sheetBox = (await sheet.boundingBox())!;
			const attribBox = (await page.locator('.maplibregl-ctrl-attrib').boundingBox())!;
			const opacity = await page
				.locator('.maplibregl-ctrl-bottom-right')
				.evaluate((el) => getComputedStyle(el).opacity);
			return (
				opacity === '1' && attribBox.y >= 0 && attribBox.y + attribBox.height <= sheetBox.y + 1
			);
		})
		.toBe(true);

	// Clear of the solve card too: a credit under an opaque panel is not a credit.
	const attribOnTop = await page.locator('.maplibregl-ctrl-attrib').evaluate((el) => {
		const b = el.getBoundingClientRect();
		return el.contains(document.elementFromPoint(b.x + b.width / 2, b.y + b.height / 2));
	});
	expect(attribOnTop).toBe(true);

	// Select a bus through the lookup the sheet mounts inline.
	const lookup = page.locator('.bus-lookup input');
	await lookup.tap();
	await lookup.fill('2');
	await page.locator('.bus-lookup li').first().tap();

	const chip = page.locator('.chip', { hasText: '∂LMP/∂d' });
	await expect(chip).toBeVisible({ timeout: 30_000 });

	// Above the fold: the readout sits inside the sheet's visible box with the
	// body still unscrolled.
	expect(await page.locator('.panel-body').evaluate((el) => el.scrollTop)).toBe(0);
	const sheetBox = (await sheet.boundingBox())!;
	const chipBox = (await chip.boundingBox())!;
	expect(chipBox.y).toBeGreaterThanOrEqual(sheetBox.y);
	expect(chipBox.y + chipBox.height).toBeLessThanOrEqual(sheetBox.y + sheetBox.height);

	// The demand control is the next thing a reader wants, so it comes before the
	// rule that separates the selection from the case stats below it.
	const sliderBox = (await page.getByLabel('demand delta at selected bus').boundingBox())!;
	const ruleTop = await page
		.locator('.panel-body hr')
		.first()
		.evaluate((el) => el.getBoundingClientRect().top);
	expect(sliderBox.y).toBeLessThan(ruleTop);
});
