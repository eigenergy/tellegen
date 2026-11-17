import { writeFile } from 'node:fs/promises';
import { expect, test } from './fixtures/page-errors.js';

test('node-breaker objects retain edits through saved drawings and SVG export', async ({
	page
}) => {
	const port = Number(
		process.env.TELLEGEN_MC_PREVIEW_PORT ?? Number(process.env.TELLEGEN_PREVIEW_PORT ?? 4173) + 1
	);
	await page.goto(`http://127.0.0.1:${port}/drawing.html?live=1`);
	const drawing = page.getByRole('region', { name: 'Engineering drawing' });
	await expect(drawing).toBeVisible();
	await expect(
		page.getByRole('button', { name: 'disconnector DSC1 open', exact: true })
	).toBeVisible();
	await page.getByRole('button', { name: 'busbar BB1', exact: true }).click();
	await page.getByLabel('X', { exact: true }).first().fill('555');
	await page.getByLabel('X', { exact: true }).first().press('Tab');
	await expect(page.getByLabel('X', { exact: true }).first()).toHaveValue('560');
	await page.getByRole('button', { name: 'Undo', exact: true }).click();
	await expect(page.getByLabel('X', { exact: true }).first()).not.toHaveValue('560');
	await page.getByRole('button', { name: 'Redo', exact: true }).click();
	await page.getByLabel('Keep position during layout', { exact: true }).check();
	await page.getByRole('button', { name: 'Arrange unlocked objects', exact: true }).click();
	await expect(page.getByLabel('X', { exact: true }).first()).toHaveValue('560');
	await page.getByRole('button', { name: 'Save drawing', exact: true }).click();
	await page.reload();
	await page.getByRole('button', { name: 'List saved drawings', exact: true }).click();
	await page.getByLabel('Saved drawing', { exact: true }).selectOption({ label: 'Station' });
	await page.getByRole('button', { name: 'busbar BB1', exact: true }).click();
	await expect(page.getByLabel('X', { exact: true }).first()).toHaveValue('560');
	await expect(page.getByLabel('Keep position during layout', { exact: true })).toBeChecked();
	await page
		.getByRole('button', { name: 'Change switch states', exact: true, includeHidden: true })
		.evaluate((button: HTMLButtonElement) => button.click());
	await expect(
		page.getByRole('button', { name: 'disconnector DSC1 closed', exact: true })
	).toBeVisible();
	await expect(page.getByLabel('X', { exact: true }).first()).toHaveValue('560');
	const download = page.waitForEvent('download');
	await page.getByRole('button', { name: 'Export drawing SVG', exact: true }).click();
	const file = await download;
	expect(file.suggestedFilename()).toBe('Station.svg');
	const stream = await file.createReadStream();
	let text = '';
	for await (const chunk of stream!) text += chunk.toString();
	expect(text).toContain('DSC1');
	expect(text).toContain('translate(560');
	expect(text).not.toContain('data-edit-only');
});

for (const topology of ['radial', 'meshed'])
	test(`large ${topology} drawings keep selection, edits and SVG export responsive`, async ({
		page
	}, testInfo) => {
		const port = Number(
			process.env.TELLEGEN_MC_PREVIEW_PORT ?? Number(process.env.TELLEGEN_PREVIEW_PORT ?? 4173) + 1
		);
		const started = Date.now();
		await page.goto(
			`http://127.0.0.1:${port}/drawing.html?count=1000${topology === 'meshed' ? '&mesh' : ''}`
		);
		await expect(page.getByRole('region', { name: 'Engineering drawing' })).toBeVisible();
		const renderMs = Date.now() - started;
		const selecting = Date.now();
		await page.getByRole('combobox', { name: 'Object', exact: true }).selectOption('bus:500');
		await expect(page.getByLabel('Label', { exact: true })).toHaveValue('Bus 500');
		const selectionMs = Date.now() - selecting;
		const editing = Date.now();
		await page.getByLabel('X', { exact: true }).first().fill('1500');
		await page.getByLabel('X', { exact: true }).first().press('Tab');
		const editMs = Date.now() - editing;
		const layingOut = Date.now();
		await page.getByRole('button', { name: 'Arrange unlocked objects', exact: true }).click();
		const layoutMs = Date.now() - layingOut;
		const results = Date.now();
		await page.getByLabel('Show active power and arrows', { exact: true }).check();
		await expect(page.locator('svg text').filter({ hasText: '10.00 MW' }).first()).toBeVisible();
		const resultMs = Date.now() - results;
		const exporting = Date.now();
		const pending = page.waitForEvent('download');
		await page.getByRole('button', { name: 'Export drawing SVG', exact: true }).click();
		await (await pending).path();
		const exportMs = Date.now() - exporting;
		const timingFile = testInfo.outputPath('drawing-timings.json');
		await writeFile(
			timingFile,
			JSON.stringify(
				{ buses: 1000, topology, renderMs, selectionMs, editMs, layoutMs, resultMs, exportMs },
				null,
				2
			)
		);
		await testInfo.attach('drawing-timings.json', {
			path: timingFile,
			contentType: 'application/json'
		});
		expect(selectionMs).toBeLessThan(5000);
		expect(editMs).toBeLessThan(5000);
		expect(layoutMs).toBeLessThan(5000);
		expect(resultMs).toBeLessThan(5000);
		expect(exportMs).toBeLessThan(10000);
	});

test('invalid numeric edits leave a usable drawing and full export ignores interactive zoom', async ({
	page
}) => {
	const port = Number(
		process.env.TELLEGEN_MC_PREVIEW_PORT ?? Number(process.env.TELLEGEN_PREVIEW_PORT ?? 4173) + 1
	);
	await page.goto(`http://127.0.0.1:${port}/drawing.html`);
	const grid = page.getByLabel('Grid', { exact: true });
	const originalGrid = await grid.inputValue();
	for (const invalid of ['0', '-1', '']) {
		await grid.fill(invalid);
		await grid.press('Tab');
		await expect(grid).toHaveValue(originalGrid);
	}
	await page.getByRole('button', { name: 'busbar BB1', exact: true }).click();
	for (const input of [
		page.getByLabel('X', { exact: true }).first(),
		page.getByLabel('Label X', { exact: true }),
		page.locator('fieldset').first().getByLabel('X', { exact: true })
	]) {
		const before = await input.inputValue();
		for (const invalid of ['', '1000000000000']) {
			await input.fill(invalid);
			await input.press('Tab');
			await expect(input).toHaveValue(before);
		}
	}
	await page.getByRole('button', { name: 'Add frame', exact: true }).click();
	await page.getByLabel('width', { exact: true }).fill('2500');
	await page.getByLabel('width', { exact: true }).press('Tab');
	await page.getByLabel('rotation', { exact: true }).fill('45');
	await page.getByLabel('rotation', { exact: true }).press('Tab');
	await page.getByLabel('Label X', { exact: true }).fill('3000');
	await page.getByLabel('Label X', { exact: true }).press('Tab');
	await page.locator('polyline[role="button"]').first().focus();
	await page.locator('polyline[role="button"]').first().press('Enter');
	await page.getByRole('button', { name: 'Add route bend', exact: true }).click();
	const bend = page
		.locator('fieldset')
		.filter({ hasText: 'Bend 1' })
		.getByLabel('X', { exact: true });
	await bend.fill('');
	await bend.press('Tab');
	await expect(bend).toHaveValue('200');
	await bend.fill('-5000');
	await bend.press('Tab');
	await page.getByRole('button', { name: 'Save drawing', exact: true }).click();
	await expect(page.getByRole('status')).toHaveText('Drawing saved');
	await page.getByRole('button', { name: 'Undo', exact: true }).click();
	await page.getByRole('button', { name: 'Redo', exact: true }).click();
	await page.getByRole('button', { name: 'Fit drawing', exact: true }).click();
	for (let i = 0; i < 3; i++)
		await page.getByRole('button', { name: 'Zoom in', exact: true }).click();
	const pending = page.waitForEvent('download');
	await page.getByRole('button', { name: 'Export drawing SVG', exact: true }).click();
	const stream = await (await pending).createReadStream();
	let svg = '';
	for await (const chunk of stream!) svg += chunk.toString();
	const result = await page.evaluate((text) => {
		const exported = new DOMParser().parseFromString(text, 'image/svg+xml').documentElement;
		const host = document.createElement('div');
		host.innerHTML = new XMLSerializer().serializeToString(exported);
		document.body.append(host);
		const svg = host.querySelector('svg')!,
			content = svg.querySelector<SVGGElement>('[data-drawing-content]')!;
		const box = content.getBBox(),
			view = svg.viewBox.baseVal;
		const result = {
			transform: content.getAttribute('transform'),
			fits:
				box.x >= view.x &&
				box.y >= view.y &&
				box.x + box.width <= view.x + view.width &&
				box.y + box.height <= view.y + view.height,
			left: view.x
		};
		host.remove();
		return result;
	}, svg);
	expect(result.transform).toBeNull();
	expect(result.fits).toBe(true);
	expect(result.left).toBeLessThan(-5000);
});

for (const viewport of [
	{ width: 1280, height: 900 },
	{ width: 390, height: 844 }
]) {
	for (const scenario of ['station', 'radial', 'meshed']) {
		test(`drawing visual check ${scenario} ${viewport.width}`, async ({ browser }, testInfo) => {
			const context = await browser.newContext({
				viewport,
				isMobile: viewport.width < 700,
				hasTouch: viewport.width < 700
			});
			const page = await context.newPage();
			const port = Number(
				process.env.TELLEGEN_MC_PREVIEW_PORT ??
					Number(process.env.TELLEGEN_PREVIEW_PORT ?? 4173) + 1
			);
			const query =
				scenario === 'station' ? '' : `?count=12${scenario === 'meshed' ? '&mesh' : ''}`;
			await page.goto(`http://127.0.0.1:${port}/drawing.html${query}`);
			await expect(page.getByRole('region', { name: 'Engineering drawing' })).toBeVisible();
			if (viewport.width < 700) await page.getByText('Drawing controls', { exact: true }).click();
			if (scenario !== 'station')
				await page.getByLabel('Show active power and arrows', { exact: true }).check();
			await page.getByRole('button', { name: 'Fit drawing', exact: true }).click();
			if (viewport.width < 700) await page.getByText('Drawing controls', { exact: true }).click();
			await page.screenshot({
				path: testInfo.outputPath(`drawing-${scenario}-${viewport.width}.png`),
				fullPage: true
			});
			expect(
				await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)
			).toBe(true);
			await context.close();
		});
	}
}
