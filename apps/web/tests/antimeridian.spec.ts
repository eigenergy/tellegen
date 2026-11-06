import type { Page } from '@playwright/test';
import { expect, test } from './fixtures/page-errors.js';

// Lines in Chukotka, across ±180. Stored longitudes stay in [-180, 180], so
// consecutive vertices of each route sit almost 360° apart at the seam.
// Branch 2 crosses it three times, branch 3 has a vertex on +180 and branch 4
// one on -180.
const NETWORK = {
	id: 'case1',
	name: 'Chukotka',
	base_mva: 100,
	synthetic_coords: false,
	buses: [
		{ id: 1, lon: 177.5, lat: 64.73, demand_mw: 50, gen_mw: 0 },
		{ id: 2, lon: -179.12, lat: 66.32, demand_mw: 0, gen_mw: 120 },
		{ id: 3, lon: 179.3, lat: 64.95, demand_mw: 30, gen_mw: 0 },
		{ id: 4, lon: -178.6, lat: 64.6, demand_mw: 40, gen_mw: 0 }
	],
	branches: [
		{
			id: 1,
			from: 1,
			to: 2,
			path: [
				[177.5, 64.73],
				[178.6, 65.2],
				[179.6, 65.7],
				[-179.7, 66.0],
				[-179.12, 66.32]
			]
		},
		{
			id: 2,
			from: 3,
			to: 2,
			path: [
				[179.3, 64.95],
				[-179.8, 65.25],
				[179.8, 65.6],
				[-179.5, 65.95],
				[-179.12, 66.32]
			]
		},
		{
			id: 3,
			from: 3,
			to: 4,
			path: [
				[179.3, 64.95],
				[180, 64.8],
				[-178.6, 64.6]
			]
		},
		{
			id: 4,
			from: 1,
			to: 4,
			path: [
				[177.5, 64.73],
				[-180, 64.6],
				[-178.6, 64.6]
			]
		}
	].map((branch) => ({ ...branch, rate_mw: 100, status: 1 }))
};

// Every line at 95% loading, so every route draws in the attention red.
const SOLUTION = {
	objective: 1,
	prices: [1, 2, 3, 4].map((bus) => ({ bus, value: 20 + bus })),
	va: [],
	w: [],
	flows: [1, 2, 3, 4].map((branch) => ({ branch, mw: 95, loading: 0.95 })),
	dispatch: [{ gen: 1, mw: 120 }]
};

// A white basemap, so anything painted red is a route.
const blankStyle = {
	version: 8,
	sources: {},
	layers: [{ id: 'background', type: 'background', paint: { 'background-color': '#ffffff' } }]
};

const port = Number(
	process.env.TELLEGEN_MC_PREVIEW_PORT ?? Number(process.env.TELLEGEN_PREVIEW_PORT ?? 4173) + 1
);

async function openCase(page: Page, projection: 'mercator' | 'globe') {
	// Camera moves land at once, so no fly-to is still running when the specs
	// aim the camera themselves.
	await page.emulateMedia({ reducedMotion: 'reduce' });
	await page.route('**/api/compute', (route) => route.fulfill({ json: { enabled: true } }));
	await page.route('**/api/cases', (route) =>
		route.fulfill({ json: [{ id: 'case1', name: 'Chukotka', n_bus: 4, n_branch: 4, n_gen: 1 }] })
	);
	await page.route('**/api/cases/case1/network', (route) => route.fulfill({ json: NETWORK }));
	await page.route('**/api/cases/case1/solution', (route) => route.fulfill({ json: SOLUTION }));
	await page.route('**/api/cases/case1/case', (route) =>
		route.fulfill({ status: 500, json: { error: 'case text unavailable' } })
	);
	await page.route('https://tiles.openfreemap.org/styles/*', (route) =>
		route.fulfill({ json: blankStyle })
	);
	await page.goto(`http://127.0.0.1:${port}/antimeridian.html?projection=${projection}`);
	await page.waitForFunction(() => window.antimeridian?.loaded() && !!window.antimeridian.map());
}

/** Point the camera at `center` and wait for the map to draw there. */
async function look(page: Page, center: [number, number], zoom: number) {
	await page.evaluate(
		([center, zoom]) =>
			new Promise<void>((resolve) => {
				const map = window.antimeridian.map()!;
				map.jumpTo({ center, zoom });
				map.once('idle', () => resolve());
				map.triggerRepaint();
				setTimeout(resolve, 5_000);
			}),
		[center, zoom] as const
	);
}

/** Horizontal extent of the route red on screen, in pixels, or 0 before any
 * route draws. The camera is re-aimed each time, since the case frames itself
 * once it loads. A route drawn the long way round crosses the whole map. */
async function routeSpan(page: Page) {
	await look(page, [180, 65.4], 5.2);
	const png = await page.screenshot();
	return page.evaluate(async (base64) => {
		const bytes = Uint8Array.from(atob(base64), (character) => character.charCodeAt(0));
		const image = await createImageBitmap(new Blob([bytes], { type: 'image/png' }));
		const pixels = new OffscreenCanvas(image.width, image.height);
		const context = pixels.getContext('2d')!;
		context.drawImage(image, 0, 0);
		const { data, width, height } = context.getImageData(0, 0, image.width, image.height);
		image.close();
		let count = 0;
		let minX = width;
		let maxX = -1;
		for (let y = 0; y < height; y++) {
			for (let x = 0; x < width; x++) {
				const i = (y * width + x) * 4;
				if (data[i] > 140 && data[i + 1] < 90 && data[i + 2] < 90) {
					count++;
					minX = Math.min(minX, x);
					maxX = Math.max(maxX, x);
				}
			}
		}
		// Ignore a stray pixel or two before the routes draw.
		return count < 200 ? 0 : (maxX - minX) / width;
	}, png.toString('base64'));
}

for (const projection of ['mercator', 'globe'] as const) {
	const where = projection === 'globe' ? 'on the globe' : 'on the flat map';

	test.describe(where, () => {
		// The harness server compiles the map from source on first use, and the
		// map draws in software under a full parallel run.
		test.describe.configure({ timeout: 120_000 });

		test('routes across ±180 draw as short paths', async ({ page }) => {
			await openCase(page, projection);
			// The four routes span about 1.4° of longitude, about a sixth of the
			// screen at this zoom. Drawn the long way, they cross all of it.
			await expect
				.poll(() => routeSpan(page), { message: 'route red spans a short arc', timeout: 60_000 })
				.toBeGreaterThan(0.05);
			expect(await routeSpan(page)).toBeLessThan(0.3);
		});

		test('the selection cues follow a case across ±180', async ({ page }) => {
			await openCase(page, projection);
			// Framing the selected route leaves the camera just east of the seam,
			// while the route starts west of it.
			await page.evaluate(() => window.antimeridian.selectBranch(2));
			expect(await page.evaluate(() => window.antimeridian.selected())).toBe(2);
			await expect(page.locator('.selected-branch-cue')).toBeVisible({ timeout: 30_000 });

			// Bus 3 sits west of the seam; look at it from the east.
			await page.evaluate(() => window.antimeridian.selectBus(3));
			await look(page, [-179.5, 65.4], 5.2);
			await expect(page.locator('.selected-bus-cue')).toBeVisible({ timeout: 30_000 });
		});
	});
}
