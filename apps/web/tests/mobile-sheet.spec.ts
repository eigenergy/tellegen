import { devices, expect, test } from './fixtures/page-errors.js';

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
	await expect(page.getByText('No cases', { exact: true })).toBeVisible();
	await page.locator('input[type="file"]').setInputFiles([
		{ name: 'case3-coords.csv', mimeType: 'text/csv', buffer: Buffer.from(CASE3_COORDS) },
		{ name: 'case3congested.m', mimeType: 'text/plain', buffer: Buffer.from(CASE3) }
	]);
	// The solver's result is part of the Network panel.
	const networkTab = page.getByRole('button', { name: 'Network', exact: true });
	if ((await networkTab.getAttribute('aria-pressed')) !== 'true') await networkTab.click();
	await expect(page.locator('.solvecard')).toHaveAttribute('data-calculation', 'dcopf', {
		timeout: 60_000
	});

	// Compact panels share one drawer.
	const sheet = page.locator('[data-panel="network"].mobile');
	await expect(sheet).toBeVisible();

	// The grab bar has to be worth aiming a thumb at.
	const headBox = await page.locator('[data-panel="network"] > header').boundingBox();
	expect(headBox!.height).toBeGreaterThanOrEqual(44);

	// Basemap attribution is a licensing requirement: on screen, opaque, and on
	// the map above the sheet, never over the sheet's content. The sheet runs to
	// the bottom edge, and the zoom buttons step aside while it is open.
	await expect
		.poll(async () => {
			const sheetBox = (await sheet.boundingBox())!;
			const attribBox = (await page.locator('.maplibregl-ctrl-attrib').boundingBox())!;
			const opacity = await page
				.locator('.maplibregl-ctrl-bottom-right')
				.evaluate((el) => getComputedStyle(el).opacity);
			return (
				opacity === '1' &&
				attribBox.y >= 0 &&
				attribBox.y + attribBox.height <= sheetBox.y &&
				Math.round(sheetBox.y + sheetBox.height) === 844
			);
		})
		.toBe(true);
	await expect(page.locator('.maplibregl-ctrl-group')).toBeHidden();

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

	const chip = page.getByTestId('sensitivity-title');
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
	const sliderBox = (await page.getByLabel('Demand delta at selected bus').boundingBox())!;
	const ruleTop = await page
		.locator('.panel-body .section')
		.first()
		.evaluate((el) => el.getBoundingClientRect().top);
	expect(sliderBox.y).toBeLessThan(ruleTop);
});

/** How many distinct colours a screenshot holds, decoded in the page: a blank map is one or two. */
async function colourCount(page: import('@playwright/test').Page, png: Buffer): Promise<number> {
	return page.evaluate(async (data) => {
		const img = new Image();
		img.src = `data:image/png;base64,${data}`;
		await img.decode();
		const canvas = document.createElement('canvas');
		canvas.width = img.width;
		canvas.height = img.height;
		const ctx = canvas.getContext('2d')!;
		ctx.drawImage(img, 0, 0);
		const px = ctx.getImageData(0, 0, img.width, img.height).data;
		const seen = new Set<number>();
		for (let i = 0; i < px.length; i += 16) seen.add((px[i] << 16) | (px[i + 1] << 8) | px[i + 2]);
		return seen.size;
	}, png.toString('base64'));
}

test('the map draws again after the diagram and the other tabs', async ({ page }) => {
	await page.route('**/api/cases', (route) => void route.fulfill({ json: [] }));
	await page.goto('/');
	await expect(page.getByText('No cases', { exact: true })).toBeVisible();
	await page.locator('input[type="file"]').setInputFiles([
		{ name: 'case3-coords.csv', mimeType: 'text/csv', buffer: Buffer.from(CASE3_COORDS) },
		{ name: 'case3congested.m', mimeType: 'text/plain', buffer: Buffer.from(CASE3) }
	]);
	await expect(page.locator('.solvecard')).toHaveAttribute('data-calculation', 'dcopf', {
		timeout: 60_000
	});
	const strip = page.locator('.panel-toolbar');
	// Settings, the diagram's Limits view, back to the map, then Studies and Agent.
	await page.locator('header').getByRole('button', { name: 'Menu', exact: true }).click();
	await page.getByRole('menuitem', { name: 'Settings', exact: true }).click();
	await strip.getByRole('button', { name: 'Back' }).click();
	await page.getByRole('button', { name: 'Show diagram', exact: true }).click();
	const view = page.getByRole('combobox').filter({ hasText: 'Limits' });
	if (await view.count()) await view.first().selectOption({ label: 'Limits' });
	else await page.getByRole('radio', { name: 'Limits' }).click();
	await strip.getByRole('button', { name: 'Network', exact: true }).click();
	await page.getByRole('button', { name: 'Show map', exact: true }).first().click();
	await strip.getByRole('button', { name: /^Studies/ }).click();
	await strip.getByRole('button', { name: /^Agent/ }).click();
	// The map fills the screen behind the sheet and has drawn its basemap and network.
	const canvas = page.locator('.maplibregl-canvas');
	await expect(canvas).toBeVisible();
	const box = (await canvas.boundingBox())!;
	const viewport = page.viewportSize()!;
	expect(box.width).toBe(viewport.width);
	expect(box.height).toBeGreaterThan(viewport.height / 2);
	const sheetTop = (await strip.boundingBox())!.y;
	await expect
		.poll(
			async () =>
				colourCount(
					page,
					await page.screenshot({
						clip: { x: 0, y: 80, width: viewport.width, height: sheetTop - 100 }
					})
				),
			{ timeout: 20_000 }
		)
		.toBeGreaterThan(20);
});
