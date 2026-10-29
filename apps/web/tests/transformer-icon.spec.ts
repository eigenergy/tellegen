import { readFile } from 'node:fs/promises';
import type { Page } from '@playwright/test';
import { expect, test } from './fixtures/page-errors.js';

// One Dy transformer between two buses: the smallest multiconductor case that
// draws a transformer symbol.
const fixture = new URL(
	'../../../crates/tellegen/tests/data/mc_pf/oracle_inputs/pf_dy_xfmr.json',
	import.meta.url
);

// Both buses on one parallel, so the transformer edge runs east-west.
const coordinates = JSON.stringify({
	type: 'FeatureCollection',
	powerio_geo: { space: 'geographic' },
	features: [
		{
			type: 'Feature',
			properties: { bus: 'hv' },
			geometry: { type: 'Point', coordinates: [-83, 35] }
		},
		{
			type: 'Feature',
			properties: { bus: 'lv' },
			geometry: { type: 'Point', coordinates: [-82.9, 35] }
		}
	]
});

// A white basemap, so anything painted is the network.
const blankStyle = {
	version: 8,
	sources: {},
	layers: [{ id: 'background', type: 'background', paint: { 'background-color': '#ffffff' } }]
};

/** How many rows the transformer symbol stands proud of its edge. Finds the
 * edge as the longest run of steel (blue above red) pixels, then compares the
 * tallest steel column in the middle of the run with a typical one. Without
 * the symbol every column there is just the edge's width. It reads a page
 * screenshot: an element screenshot fails when the map canvas is replaced
 * while the case loads. */
async function symbolRise(page: Page) {
	const png = await page.screenshot();
	return page.evaluate(async (base64) => {
		const bytes = Uint8Array.from(atob(base64), (character) => character.charCodeAt(0));
		const image = await createImageBitmap(new Blob([bytes], { type: 'image/png' }));
		const pixels = new OffscreenCanvas(image.width, image.height);
		const context = pixels.getContext('2d')!;
		context.drawImage(image, 0, 0);
		const { data, width, height } = context.getImageData(0, 0, image.width, image.height);
		image.close();
		const steel = (x: number, y: number) => {
			const i = (y * width + x) * 4;
			return data[i + 2] - data[i] > 12;
		};
		let row = 0;
		let best = 0;
		for (let y = 0; y < height; y++) {
			let count = 0;
			for (let x = 0; x < width; x++) if (steel(x, y)) count++;
			if (count > best) [best, row] = [count, y];
		}
		let x0 = 0;
		let x1 = 0;
		for (let x = 0, start = 0; x <= width; x++) {
			if (x < width && steel(x, row)) continue;
			if (x - start > x1 - x0) [x0, x1] = [start, x];
			start = x + 1;
		}
		// No edge drawn yet.
		if (x1 - x0 < 200) return 0;
		const column = (x: number) => {
			let count = 0;
			for (let y = Math.max(0, row - 16); y <= Math.min(height - 1, row + 16); y++)
				if (steel(x, y)) count++;
			return count;
		};
		const middle = Math.round((x0 + x1) / 2);
		const half = Math.round((x1 - x0) / 5);
		const counts: number[] = [];
		for (let x = middle - half; x <= middle + half; x++) counts.push(column(x));
		const typical = [...counts].sort((a, b) => a - b)[counts.length >> 1];
		return Math.max(...counts) - typical;
	}, png.toString('base64'));
}

// The symbol is a canvas handed to deck.gl as its icon atlas. When it was a
// `data:` URL, deck.gl fetched it, the CSP's connect-src blocked the fetch,
// and the map drew without symbols. The page-errors fixture fails the spec on
// the CSP violation; this spec also checks that the symbol is drawn.
test('the multiconductor map draws transformer symbols under the CSP', async ({ page }) => {
	const deckErrors: string[] = [];
	page.on('console', (message) => {
		if (message.type() === 'error' && message.text().startsWith('deck:'))
			deckErrors.push(message.text());
	});
	await page.route('**/api/cases', (route) => route.fulfill({ json: [] }));
	await page.route('**/api/compute', (route) => route.fulfill({ json: { enabled: false } }));
	await page.route('https://tiles.openfreemap.org/styles/*', (route) =>
		route.fulfill({ json: blankStyle })
	);
	await page.goto('/');
	// The input exists before hydration attaches its listener; a file set earlier is lost.
	await expect(page.getByText('no default cases loaded')).toBeVisible();
	await page.locator('input[type=file]').setInputFiles([
		{ name: 'xfmr.json', mimeType: 'application/json', buffer: await readFile(fixture) },
		{ name: 'xfmr.geojson', mimeType: 'application/json', buffer: Buffer.from(coordinates) }
	]);
	await expect(page.locator('.maplibregl-canvas')).toBeVisible();
	await expect
		.poll(() => symbolRise(page), { message: 'transformer symbol drawn on its edge' })
		.toBeGreaterThanOrEqual(3);
	expect(deckErrors).toEqual([]);
});
