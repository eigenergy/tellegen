import { readFile } from 'node:fs/promises';
import { expect, test } from './fixtures/page-errors.js';

test('a geographic feeder opens at a useful size after zooming the map out', async ({
	page
}, info) => {
	await page.emulateMedia({ reducedMotion: 'reduce' });
	await page.route('**/api/cases', (route) => route.fulfill({ json: [] }));
	await page.route('**/api/compute', (route) => route.fulfill({ json: { enabled: false } }));
	const style = { version: 8, sources: {}, layers: [] };
	await page.route(/^https:\/\/(basemaps\.cartocdn\.com|tiles\.openfreemap\.org)\//, (route) =>
		route.fulfill({ json: style })
	);
	await page.goto('/');
	await expect(page.getByText('no default cases loaded')).toBeVisible();
	await page.locator('input[type=file]').setInputFiles([
		{
			name: 'four-wire-limits.bmopf.json',
			mimeType: 'application/json',
			buffer: await readFile(new URL('./fixtures/four-wire-limits.bmopf.json', import.meta.url))
		},
		{
			name: 'four-wire-limits.geo.json',
			mimeType: 'application/geo+json',
			buffer: Buffer.from(
				JSON.stringify({
					type: 'FeatureCollection',
					powerio_geo: { space: 'geographic' },
					features: ['src', 'lb'].map((bus, i) => ({
						type: 'Feature',
						properties: { bus },
						geometry: { type: 'Point', coordinates: [-83 + i * 0.01, 35] }
					}))
				})
			)
		}
	]);
	const open = page.getByRole('button', { name: 'Show diagram' });
	await expect(open).toBeEnabled({ timeout: 60_000 });
	// Reduced motion makes each camera step immediate, so the carried map view is deliberately
	// far below the feeder's fit even though it still includes both buses.
	for (let i = 0; i < 10; i++)
		await page.getByRole('button', { name: 'Zoom out', exact: true }).click();
	await open.click();
	const diagram = page.getByRole('application', { name: 'Schematic diagram' });
	await expect(diagram).toBeVisible();
	const toolbar = page.getByRole('toolbar', { name: 'Schematic' });
	await expect(toolbar.getByRole('button', { name: 'Geographic', exact: true })).toHaveAttribute(
		'aria-pressed',
		'true'
	);
	const downloaded = page.waitForEvent('download');
	await toolbar.getByRole('button', { name: 'SVG', exact: true }).click();
	const svg = await readFile(await (await downloaded).path(), 'utf8');
	const buses = [...svg.matchAll(/<circle cx="([\d.-]+)" cy="([\d.-]+)" r="2\.6"/g)];
	expect(buses).toHaveLength(2);
	const xs = buses.map((bus) => Number(bus[1]));
	expect(Math.max(...xs) - Math.min(...xs)).toBeGreaterThan(200);
	const panel = await page.locator('#control-panel-body').boundingBox();
	if (panel) expect(Math.min(...xs)).toBeGreaterThan(panel.x + panel.width);
	await page.screenshot({ path: info.outputPath('schematic-opening.png') });
});
