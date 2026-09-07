import { readFile } from 'node:fs/promises';
import type { StudyBundle } from '@tellegen/engine';
import { expect, test } from './fixtures/page-errors.js';
import { congestCase, installWebMcpHarness } from './fixtures/planning-case.js';

const drawing = {
	type: 'FeatureCollection',
	powerio_geo: { space: 'diagram' },
	features: [
		...[1, 2, 3].map((id, i) => ({
			type: 'Feature',
			properties: { bus_i: id },
			geometry: {
				type: 'Point',
				coordinates: [
					[1200, 2500],
					[2200, 2500],
					[2200, 3600]
				][i]
			}
		})),
		{
			type: 'Feature',
			properties: { bus_from: 1, bus_to: 2 },
			geometry: {
				type: 'LineString',
				coordinates: [
					[1200, 2500],
					[1700, 2400],
					[2200, 2500]
				]
			}
		}
	]
};

test('drawing coordinates remain separate from geography and support navigation', async ({
	page,
	browser
}, testInfo) => {
	test.setTimeout(120_000);
	await installWebMcpHarness(page);
	await congestCase(page);
	await page.locator('input[type="file"]').setInputFiles({
		name: 'three-bus-drawing.geo.json',
		mimeType: 'application/geo+json',
		buffer: Buffer.from(JSON.stringify(drawing))
	});
	await expect(page.getByRole('button', { name: 'Diagram', exact: true })).toBeVisible();
	await page.getByRole('button', { name: 'Diagram', exact: true }).click();
	const canvas = page.getByRole('application', { name: 'Network diagram' });
	await expect(canvas).toBeVisible();
	await expect(page.locator('.maplibregl-canvas')).toHaveCount(0);
	await expect(page.locator('.maplibregl-ctrl-attrib')).toHaveCount(0);
	const bus = page.locator('[data-bus-id="1"] circle');
	await expect(bus).toHaveAttribute('cx', '1200');
	await expect(bus).toHaveAttribute('cy', '2500');
	await expect(page.locator('[data-branch-id="1"]')).toHaveAttribute(
		'd',
		'M1200,2500 L1700,2400 L2200,2500'
	);
	const drawingGroup = page.locator('[data-diagram-transform]');
	const initial = await drawingGroup.getAttribute('transform');
	await canvas.press('ArrowRight');
	await expect(drawingGroup).not.toHaveAttribute('transform', initial!);
	const panned = await drawingGroup.getAttribute('transform');
	await page.getByRole('button', { name: 'Zoom in', exact: true }).click();
	await expect(drawingGroup).not.toHaveAttribute('transform', panned!);
	await page.getByRole('button', { name: 'Fit diagram', exact: true }).click();
	await expect(drawingGroup).toHaveAttribute('transform', initial!);
	await page.screenshot({ path: testInfo.outputPath('diagram-desktop.png') });
	const downloadPending = page.waitForEvent('download');
	await page.getByRole('button', { name: 'Download drawing (.geo.json)', exact: true }).click();
	const downloaded = JSON.parse(await readFile((await (await downloadPending).path())!, 'utf8'));
	expect(downloaded.powerio_geo.space).toBe('diagram');
	expect(downloaded.features[0].geometry.coordinates).toEqual([1200, 2500]);
	await page.getByRole('button', { name: 'Map', exact: true }).click();
	await expect(page.locator('.maplibregl-canvas')).toBeVisible();
	await expect(canvas).toHaveCount(0);
	await page.getByRole('button', { name: 'Diagram', exact: true }).click();
	await expect(bus).toHaveAttribute('cx', '1200');
	await canvas.press('ArrowLeft');
	await canvas.press('+');
	const beforeSave = await drawingGroup.getAttribute('transform');
	await page.getByRole('button', { name: 'Studies', exact: true }).click();
	await page.getByLabel('Study name').fill('Drawing comparison');
	await page.getByRole('button', { name: 'Save study', exact: true }).click();
	await expect(
		page.getByRole('heading', { name: 'Drawing comparison', exact: true })
	).toBeVisible();
	await expect(canvas).toBeVisible();
	await expect(bus).toHaveAttribute('cx', '1200');
	await expect(drawingGroup).toHaveAttribute('transform', beforeSave!);
	await expect(page.getByRole('button', { name: 'Show diagram', exact: true })).toBeVisible();
	const bundleDownload = page.waitForEvent('download');
	await page.getByRole('button', { name: 'Export', exact: true }).click();
	const bundleText = await readFile((await (await bundleDownload).path())!, 'utf8');
	const bundle = JSON.parse(bundleText) as StudyBundle;
	expect(bundle.document.display?.camera ?? null).toBeNull();
	expect(bundle.document.display?.diagram_camera?.scale).toBeGreaterThan(0);
	const viewNumbers = () =>
		drawingGroup.evaluate((element) => {
			const m = (element as SVGGraphicsElement).transform.baseVal.consolidate()!.matrix;
			return [m.a, m.d, m.e, m.f];
		});
	const savedView = await viewNumbers();
	await page.reload();
	await page.getByRole('button', { name: 'Studies', exact: true }).click();
	await page
		.getByRole('combobox', { name: 'Saved study', exact: true })
		.selectOption(bundle.document.id);
	await expect(canvas).toBeVisible();
	await expect.poll(viewNumbers).toEqual(savedView);
	await canvas.press('ArrowRight');
	await canvas.press('-');
	await expect.poll(viewNumbers).not.toEqual(savedView);
	const changedView = await viewNumbers();
	await page.locator('.file-button input[type="file"]').setInputFiles({
		name: 'drawing-study.json',
		mimeType: 'application/json',
		buffer: Buffer.from(bundleText)
	});
	await expect(page.getByRole('alert')).toHaveText(
		'This study is already saved. Open it from Saved study.'
	);
	await expect.poll(viewNumbers).toEqual(changedView);
	await page
		.getByRole('combobox', { name: 'Saved study', exact: true })
		.selectOption(bundle.document.id);
	await expect(page.getByRole('alert')).toHaveCount(0);
	const context = await browser.newContext({ viewport: page.viewportSize()! });
	try {
		const imported = await context.newPage();
		const errors: string[] = [];
		imported.on('pageerror', (error) => errors.push(error.message));
		await imported.route('**/api/cases', (route) => route.fulfill({ json: [] }));
		await imported.goto(page.url());
		await imported.getByRole('button', { name: 'Studies', exact: true }).click();
		await imported.locator('.file-button input[type="file"]').setInputFiles({
			name: 'drawing-study.json',
			mimeType: 'application/json',
			buffer: Buffer.from(bundleText)
		});
		await expect(imported.getByRole('application', { name: 'Network diagram' })).toBeVisible();
		await expect
			.poll(() =>
				imported.locator('[data-diagram-transform]').evaluate((element) => {
					const m = (element as SVGGraphicsElement).transform.baseVal.consolidate()!.matrix;
					return [m.a, m.d, m.e, m.f];
				})
			)
			.toEqual(savedView);
		expect(errors).toEqual([]);
	} finally {
		await context.close();
	}
	await page.locator('[data-bus-id="1"]').press('Enter');
	await expect(page.getByRole('region', { name: 'Saved network details' })).toContainText(
		'Current demand'
	);
	await page.setViewportSize({ width: 390, height: 844 });
	await page.getByRole('button', { name: 'Fit diagram', exact: true }).click();
	await page.screenshot({ path: testInfo.outputPath('diagram-mobile.png') });
	const zoom = await page.getByRole('button', { name: 'Zoom in', exact: true }).boundingBox();
	const panel = await page.locator('[data-panel]:visible').boundingBox();
	expect(zoom!.y).toBeGreaterThan(panel!.y + panel!.height);
});
