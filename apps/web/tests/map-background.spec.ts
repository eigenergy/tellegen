import type { Page, Route } from '@playwright/test';
import { expect, test } from './fixtures/page-errors.js';

// A one-source stub style: a GeoJSON land polygon in a known colour. GeoJSON
// features need the map worker, so a painted polygon proves the production
// worker bundle runs, which the background and the grid overlay do not.
function stubStyle(fill: string, attribution?: string) {
	return {
		version: 8,
		sources: {
			land: {
				type: 'geojson',
				...(attribution ? { attribution } : {}),
				data: {
					type: 'Feature',
					properties: {},
					geometry: {
						type: 'Polygon',
						coordinates: [
							[
								[-120, 20],
								[-60, 20],
								[-60, 55],
								[-120, 55],
								[-120, 20]
							]
						]
					}
				}
			}
		},
		layers: [
			{ id: 'background', type: 'background', paint: { 'background-color': '#ffffff' } },
			{ id: 'land', type: 'fill', source: 'land', paint: { 'fill-color': fill } }
		]
	};
}

/** Fraction of the map canvas within a few units of `rgb`. */
async function paintedFraction(page: Page, rgb: [number, number, number]) {
	const canvas = page.locator('.maplibregl-canvas');
	const png = await canvas.screenshot();
	return page.evaluate(
		async ({ base64, rgb }) => {
			const bytes = Uint8Array.from(atob(base64), (character) => character.charCodeAt(0));
			const image = await createImageBitmap(new Blob([bytes], { type: 'image/png' }));
			const pixels = new OffscreenCanvas(image.width, image.height);
			const context = pixels.getContext('2d')!;
			context.drawImage(image, 0, 0);
			const { data } = context.getImageData(0, 0, image.width, image.height);
			let painted = 0;
			for (let i = 0; i < data.length; i += 4) {
				if (
					Math.abs(data[i] - rgb[0]) < 3 &&
					Math.abs(data[i + 1] - rgb[1]) < 3 &&
					Math.abs(data[i + 2] - rgb[2]) < 3
				)
					painted++;
			}
			image.close();
			return painted / (data.length / 4);
		},
		{ base64: png.toString('base64'), rgb }
	);
}

// The keyed dev server from playwright.config.ts.
const keyedUrl = `http://127.0.0.1:${
	process.env.TELLEGEN_KEYED_PORT ?? Number(process.env.TELLEGEN_PREVIEW_PORT ?? 4173) + 2
}/`;
const CARTO_STYLE = /^https:\/\/basemaps\.cartocdn\.com\/gl\/[^/]+\/style\.json/;

async function stubApi(page: Page) {
	await page.route('**/api/cases', (route) => route.fulfill({ json: [] }));
	await page.route('**/api/compute', (route) => route.fulfill({ json: { enabled: false } }));
}

test('the production map worker renders geographic background features', async ({ page }) => {
	await stubApi(page);
	await page.route('https://tiles.openfreemap.org/styles/*', (route: Route) =>
		route.fulfill({ json: stubStyle('#14a0c8') })
	);

	await page.goto('/');
	const canvas = page.locator('.maplibregl-canvas');
	await expect(canvas).toBeVisible();
	await expect.poll(() => paintedFraction(page, [20, 160, 200])).toBeGreaterThan(0.25);
});

test('the CARTO fallback paints, with its credit, when OpenFreeMap cannot be fetched', async ({
	page
}) => {
	await stubApi(page);
	await page.route('https://tiles.openfreemap.org/**', (route: Route) => route.abort('failed'));
	await page.route('https://basemaps.cartocdn.com/**/style.json', (route: Route) =>
		route.fulfill({
			json: stubStyle('#c81432', '© <a href="https://carto.com/attributions">CARTO</a>')
		})
	);

	await page.goto('/');
	const canvas = page.locator('.maplibregl-canvas');
	await expect(canvas).toBeVisible();
	await expect.poll(() => paintedFraction(page, [200, 20, 50])).toBeGreaterThan(0.25);
	// Attribution follows the style that is actually on screen.
	await expect(page.locator('.maplibregl-ctrl-attrib')).toContainText('CARTO');
});

test('with a CARTO key, the keyed CARTO style leads', async ({ page }) => {
	await stubApi(page);
	const requested: string[] = [];
	await page.route(CARTO_STYLE, (route: Route) => {
		requested.push(route.request().url());
		return route.fulfill({
			json: stubStyle('#2864c8', '© <a href="https://carto.com/attributions">CARTO</a>')
		});
	});
	await page.route('https://tiles.openfreemap.org/**', (route: Route) => route.abort('failed'));

	await page.goto(keyedUrl);
	await expect(page.locator('.maplibregl-canvas')).toBeVisible({ timeout: 60_000 });
	await expect
		.poll(() => paintedFraction(page, [40, 100, 200]), { timeout: 60_000 })
		.toBeGreaterThan(0.25);
	expect(requested.length).toBeGreaterThan(0);
	expect(new URL(requested[0]).searchParams.get('key')).toBe('e2e-key');
	await expect(page.locator('.maplibregl-ctrl-attrib')).toContainText('CARTO');
});

test('with a CARTO key, OpenFreeMap paints, with its credit, when CARTO cannot be fetched', async ({
	page
}) => {
	await stubApi(page);
	await page.route(CARTO_STYLE, (route: Route) => route.abort('failed'));
	await page.route('https://tiles.openfreemap.org/styles/*', (route: Route) =>
		route.fulfill({ json: stubStyle('#14c864', '© OpenFreeMap') })
	);

	await page.goto(keyedUrl);
	await expect(page.locator('.maplibregl-canvas')).toBeVisible({ timeout: 60_000 });
	await expect
		.poll(() => paintedFraction(page, [20, 200, 100]), { timeout: 60_000 })
		.toBeGreaterThan(0.25);
	await expect(page.locator('.maplibregl-ctrl-attrib')).toContainText('OpenFreeMap');
});
