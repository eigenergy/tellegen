import { readFile, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import type { Page } from '@playwright/test';
import type { StudyBundle } from '@tellegen/engine';
import { expect, test } from './fixtures/page-errors.js';
import { callTool, installWebMcpHarness } from './fixtures/planning-case.js';

const dataset = process.env.TELLEGEN_NEW_ENGLAND_DIR;
test.skip(!dataset, 'Requires the local New England dataset directory');
test.use({ trace: 'off' });

async function downloadLayer(page: Page, label: string) {
	const pending = page.waitForEvent('download');
	await page.getByRole('button', { name: label, exact: true }).click();
	return readFile((await (await pending).path())!, 'utf8');
}

async function settledCase(page: Page) {
	await expect
		.poll(
			async () => {
				const result = await callTool(page, 'inspect_case', {});
				return result.ok && result.data.active && result.data.solving === false;
			},
			{ timeout: 90_000 }
		)
		.toBe(true);
	const result = await callTool(page, 'inspect_case', {});
	if (!result.ok) throw new Error(result.error.message);
	return result.data;
}

test('New England PWB geography and PWD drawing remain distinct through saving and RAW attachment', async ({
	page,
	request
}, testInfo) => {
	test.setTimeout(240_000);
	expect(await (await request.get('/_app/version.json')).json()).toEqual(
		JSON.parse(await readFile(new URL('../build/_app/version.json', import.meta.url), 'utf8'))
	);
	await installWebMcpHarness(page);
	await page.route('**/api/cases', (route) => route.fulfill({ json: [] }));
	await page.goto('/');
	await expect(page.locator('html')).toHaveAttribute('data-webmcp', 'ready');
	const input = page.locator('input[type="file"][accept*=".m,"]');
	const pwd = join(dataset!, 'PowerWorld Cases', 'New England One-Line.pwd');
	await input.setInputFiles([join(dataset!, 'PowerWorld Cases', 'New England Base Case.PWB'), pwd]);
	await expect(page.getByRole('button', { name: 'Diagram', exact: true })).toBeVisible();
	await expect(page.locator('.maplibregl-canvas')).toBeVisible();
	const pwb = await settledCase(page);
	const geographyText = await downloadLayer(page, 'Download geography (.geo.json)');
	const geography = JSON.parse(geographyText);
	const points = geography.features.filter(
		(feature: { geometry: { type: string } }) => feature.geometry.type === 'Point'
	);
	expect(points).toHaveLength(250);
	const firstBus = points.find(
		(feature: { properties: { id?: string; bus_i?: number } }) =>
			String(feature.properties.id ?? feature.properties.bus_i) === '1'
	);
	expect(firstBus.geometry.coordinates).toEqual([-75.91, 43.98]);
	expect(
		points.every((feature: { geometry: { coordinates: number[] } }) => {
			const [lon, lat] = feature.geometry.coordinates;
			return lon >= -80 && lon <= -60 && lat >= 35 && lat <= 50;
		})
	).toBe(true);
	await page.getByRole('button', { name: 'Diagram', exact: true }).click();
	const canvas = page.getByRole('application', { name: 'Network diagram' });
	await expect(canvas).toBeVisible();
	await expect(page.locator('.maplibregl-canvas')).toHaveCount(0);
	await expect(page.locator('.maplibregl-ctrl-attrib')).toHaveCount(0);
	const drawingText = await downloadLayer(page, 'Download drawing (.geo.json)');
	const drawing = JSON.parse(drawingText);
	expect(drawing.powerio_geo.space).toBe('diagram');
	expect(
		drawing.features.filter(
			(feature: { geometry: { type: string } }) => feature.geometry.type === 'Point'
		)
	).toHaveLength(250);
	expect(
		drawing.features.filter(
			(feature: { geometry: { type: string } }) => feature.geometry.type === 'LineString'
		)
	).toHaveLength(339);
	expect(
		drawing.features.some(
			(feature: { geometry: { type: string } }) => feature.geometry.type === 'LineString'
		)
	).toBe(true);
	const selected = await settledCase(page);
	const saved = await callTool(page, 'create_study', {
		case_id: selected.case_id,
		expected_case_revision: selected.revision,
		study: { title: 'New England drawing', formulation: 'dcopf' }
	});
	expect(saved.ok, JSON.stringify(saved)).toBe(true);
	if (!saved.ok) throw new Error(saved.error.message);
	const layers = await page.evaluate(async (id) => {
		const db = await new Promise<IDBDatabase>((resolve, reject) => {
			const req = indexedDB.open('tellegen-studies', 1);
			req.onsuccess = () => resolve(req.result);
			req.onerror = () => reject(req.error);
		});
		try {
			const bundle = await new Promise<StudyBundle>((resolve, reject) => {
				const req = db.transaction('studies', 'readonly').objectStore('studies').get(id);
				req.onsuccess = () => resolve(req.result);
				req.onerror = () => reject(req.error);
			});
			const display = bundle.document.display!;
			return [...new Set([display.geography, ...(display.layers ?? [])])].map((hash) => {
				const layer = JSON.parse(bundle.artifacts[hash].text);
				return { space: layer.powerio_geo?.space ?? 'geographic', features: layer.features.length };
			});
		} finally {
			db.close();
		}
	}, String(saved.data.id));
	expect(layers.some((layer) => layer.space === 'geographic' && layer.features > 0)).toBe(true);
	expect(layers.some((layer) => layer.space === 'diagram' && layer.features > 0)).toBe(true);
	await expect(canvas).toBeVisible();
	await callTool(page, 'select_case', { case_id: selected.case_id });
	await input.setInputFiles([
		{
			name: 'BaseCase.RAW',
			mimeType: 'text/plain',
			buffer: await readFile(join(dataset!, 'PSSE Cases', 'BaseCase.RAW'))
		},
		{
			name: 'New England geography.geo.json',
			mimeType: 'application/geo+json',
			buffer: Buffer.from(geographyText)
		},
		{
			name: 'New England One-Line.pwd',
			mimeType: 'application/octet-stream',
			buffer: await readFile(pwd)
		}
	]);
	await expect(page.getByRole('button', { name: 'Diagram', exact: true })).toBeVisible();
	await expect(page.locator('.maplibregl-canvas')).toBeVisible();
	const raw = await settledCase(page);
	expect(raw.case_id).not.toBe(selected.case_id);
	const rawGeography = JSON.parse(await downloadLayer(page, 'Download geography (.geo.json)'));
	const rawPoints = rawGeography.features.filter(
		(feature: { geometry: { type: string } }) => feature.geometry.type === 'Point'
	);
	expect(rawPoints.map((feature: { geometry: unknown }) => feature.geometry)).toEqual(
		points.map((feature: { geometry: unknown }) => feature.geometry)
	);
	await page.getByRole('button', { name: 'Diagram', exact: true }).click();
	await expect(canvas).toBeVisible();
	expect(JSON.parse(await downloadLayer(page, 'Download drawing (.geo.json)'))).toEqual(drawing);
	const evidence = JSON.stringify(
		{
			version: JSON.parse(
				await readFile(new URL('../build/_app/version.json', import.meta.url), 'utf8')
			),
			pwb: pwb.network,
			raw: raw.network,
			geographic_points: points.length,
			drawing_features: drawing.features.length,
			saved_layers: layers
		},
		null,
		2
	);
	const evidencePath = testInfo.outputPath('new-england-evidence.json');
	await writeFile(evidencePath, evidence);
	await testInfo.attach('New England coordinate evidence', {
		path: evidencePath,
		contentType: 'application/json'
	});
	await page.screenshot({ path: testInfo.outputPath('new-england-diagram.png') });
});
