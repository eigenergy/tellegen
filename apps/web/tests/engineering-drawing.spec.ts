import { writeFile } from 'node:fs/promises';
import { expect, test } from './fixtures/page-errors.js';

test('node-breaker objects retain edits through saved drawings and SVG export', async ({
	page
}) => {
	const port = Number(
		process.env.TELLEGEN_MC_PREVIEW_PORT ?? Number(process.env.TELLEGEN_PREVIEW_PORT ?? 4173) + 1
	);
	await page.goto(`http://127.0.0.1:${port}/drawing.html`);
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
