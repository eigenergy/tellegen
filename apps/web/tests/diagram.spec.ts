import { noticeDetails } from './fixtures/notices.js';
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
	const canvas = page.getByRole('application', { name: 'Single-line diagram' });
	await expect(canvas).toBeVisible();
	await expect(page.locator('.maplibregl-canvas')).toHaveCount(0);
	await expect(page.locator('.maplibregl-ctrl-attrib')).toHaveCount(0);
	await expect(canvas).toHaveAttribute('data-renderer-ready', 'true');
	const drawingGroup = canvas;
	const viewNumbers = async () => {
		const camera = JSON.parse((await canvas.getAttribute('data-diagram-camera'))!);
		return {
			target: camera.target.map((value: number) => Math.round(value * 1e8) / 1e8),
			zoom: Math.round(camera.zoom * 1e8) / 1e8
		};
	};
	const initial = await drawingGroup.getAttribute('data-diagram-camera');
	await canvas.press('ArrowRight');
	await expect(drawingGroup).not.toHaveAttribute('data-diagram-camera', initial!);
	const panned = await drawingGroup.getAttribute('data-diagram-camera');
	await page.getByRole('button', { name: 'Zoom in', exact: true }).click();
	await expect(drawingGroup).not.toHaveAttribute('data-diagram-camera', panned!);
	await page.getByRole('button', { name: 'Fit', exact: true }).click();
	await expect(drawingGroup).toHaveAttribute('data-diagram-camera', initial!);
	await page.screenshot({ path: testInfo.outputPath('diagram-desktop.png') });
	const downloadPending = page.waitForEvent('download');
	await page.getByRole('button', { name: 'File', exact: true }).click();
	await page.getByRole('button', { name: 'Save drawing', exact: true }).click();
	const downloaded = JSON.parse(await readFile((await (await downloadPending).path())!, 'utf8'));
	expect(downloaded.powerio_geo.space).toBe('diagram');
	expect(downloaded.features[0].geometry.coordinates).toEqual([1200, 2500]);
	await page.getByRole('button', { name: 'Map', exact: true }).click();
	await expect(page.locator('.maplibregl-canvas')).toBeVisible();
	await expect(canvas).toHaveCount(0);
	await page.getByRole('button', { name: 'Diagram', exact: true }).click();
	await expect(canvas).toHaveAttribute('data-renderer-ready', 'true');
	await canvas.press('ArrowLeft');
	await canvas.press('+');
	await page.getByRole('button', { name: 'Studies', exact: true }).click();
	await page.getByLabel('Study name').waitFor();
	const beforeSave = await viewNumbers();
	await page.getByLabel('Study name').fill('Drawing comparison');
	await page.getByRole('button', { name: 'Save study', exact: true }).click();
	await expect(
		page.getByRole('heading', { name: 'Drawing comparison', exact: true })
	).toBeVisible();
	await expect(canvas).toBeVisible();
	await expect(canvas).toHaveAttribute('data-renderer-ready', 'true');
	await expect.poll(viewNumbers).toEqual(beforeSave);
	await expect(page.getByRole('button', { name: 'Show diagram', exact: true })).toBeVisible();
	const bundleDownload = page.waitForEvent('download');
	await page.getByRole('button', { name: 'Export', exact: true }).click();
	const bundleText = await readFile((await (await bundleDownload).path())!, 'utf8');
	const bundle = JSON.parse(bundleText) as StudyBundle;
	expect(bundle.document.display?.camera ?? null).toBeNull();
	expect(bundle.document.display?.diagram_camera?.scale).toBeGreaterThan(0);
	const savedView = await viewNumbers();
	await page.reload();
	await page.getByRole('button', { name: 'Studies', exact: true }).click();
	await page
		.getByRole('combobox', { name: 'Saved study', exact: true })
		.selectOption(bundle.document.id);
	await expect(canvas).toBeVisible();
	await expect.poll(viewNumbers).not.toEqual(savedView);
	const openedView = await viewNumbers();
	await canvas.press('ArrowRight');
	await canvas.press('-');
	await expect.poll(viewNumbers).not.toEqual(openedView);
	const changedView = await viewNumbers();
	await page.locator('.file-button input[type="file"]').setInputFiles({
		name: 'drawing-study.json',
		mimeType: 'application/json',
		buffer: Buffer.from(bundleText)
	});
	await expect(await noticeDetails(page)).toContainText('Already saved');
	await expect.poll(viewNumbers).toEqual(changedView);
	await page
		.getByRole('combobox', { name: 'Saved study', exact: true })
		.selectOption(bundle.document.id);
	await expect(page.getByRole('combobox', { name: 'Saved study', exact: true })).toHaveValue(
		bundle.document.id
	);
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
		await expect(imported.getByRole('application', { name: 'Single-line diagram' })).toBeVisible();
		await expect(
			imported.getByRole('application', { name: 'Single-line diagram' })
		).not.toHaveAttribute('data-diagram-camera', JSON.stringify(savedView));
		expect(errors).toEqual([]);
	} finally {
		await context.close();
	}
	await page.getByRole('button', { name: 'Network', exact: true }).click();
	await page.getByRole('combobox', { name: 'Bus lookup', exact: true }).fill('1');
	await page.getByRole('combobox', { name: 'Bus lookup', exact: true }).press('Enter');
	await expect(page.getByRole('region', { name: 'Saved network details' })).toContainText(
		'Current demand'
	);
	await page.setViewportSize({ width: 390, height: 844 });
	await page.getByRole('button', { name: 'Display', exact: true }).click();
	await page.getByRole('button', { name: 'Fit', exact: true }).click();
	await page.screenshot({ path: testInfo.outputPath('diagram-mobile.png') });
	const zoom = await page.getByRole('button', { name: 'Zoom in', exact: true }).boundingBox();
	const panel = await page.locator('[data-panel]:visible').boundingBox();
	// On a phone the controls sit on the drawing above the open sheet.
	expect(zoom!.y + zoom!.height).toBeLessThanOrEqual(panel!.y);
});
