import { expect, test } from './fixtures/page-errors.js';

test('the production map worker renders geographic background features', async ({ page }) => {
	await page.route('**/api/cases', (route) => route.fulfill({ json: [] }));
	await page.route('**/api/compute', (route) => route.fulfill({ json: { enabled: false } }));
	await page.route('https://basemaps.cartocdn.com/**/style.json', (route) =>
		route.fulfill({
			json: {
				version: 8,
				sources: {
					land: {
						type: 'geojson',
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
					{ id: 'land', type: 'fill', source: 'land', paint: { 'fill-color': '#14a0c8' } }
				]
			}
		})
	);

	await page.goto('/');
	const canvas = page.locator('.maplibregl-canvas');
	await expect(canvas).toBeVisible();
	// GeoJSON features require the map worker, unlike the background and grid overlay.
	await expect
		.poll(async () => {
			const png = await canvas.screenshot();
			return page.evaluate(async (base64) => {
				const bytes = Uint8Array.from(atob(base64), (character) => character.charCodeAt(0));
				const image = await createImageBitmap(new Blob([bytes], { type: 'image/png' }));
				const pixels = new OffscreenCanvas(image.width, image.height);
				const context = pixels.getContext('2d')!;
				context.drawImage(image, 0, 0);
				const { data } = context.getImageData(0, 0, image.width, image.height);
				let painted = 0;
				for (let i = 0; i < data.length; i += 4) {
					if (
						Math.abs(data[i] - 20) < 3 &&
						Math.abs(data[i + 1] - 160) < 3 &&
						Math.abs(data[i + 2] - 200) < 3
					)
						painted++;
				}
				image.close();
				return painted / (data.length / 4);
			}, png.toString('base64'));
		})
		.toBeGreaterThan(0.25);
});
