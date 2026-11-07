import type { Page } from '@playwright/test';
import { BACKGROUND } from './fixtures/mc-pf-browser/declutter-background.js';
import { expect, test } from './fixtures/page-errors.js';

// About 5,000 buses over California, crowded where people live: most around
// Los Angeles and the Bay Area, fewer in the valley towns, and a thin scatter
// over the rest of the state. The map draws them all at the zoom that frames
// the state unless it thins them.
const BUS_COUNT = 5000;
const LAYER = 'buses-ca';
const SHOTS = process.env.TELLEGEN_DECLUTTER_SHOTS;

/** A deterministic generator in [0, 1), so the grid is the same every run. */
function random(seed: number) {
	let state = seed >>> 0;
	return () => {
		state = (state + 0x6d2b79f5) >>> 0;
		let t = state;
		t = Math.imul(t ^ (t >>> 15), t | 1);
		t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
		return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
	};
}

// California, roughly, as [lon, lat].
const CALIFORNIA: [number, number][] = [
	[-124.2, 42.0],
	[-120.0, 42.0],
	[-120.0, 39.0],
	[-114.6, 35.0],
	[-114.6, 32.7],
	[-117.1, 32.5],
	[-118.5, 34.0],
	[-120.6, 34.5],
	[-121.9, 36.6],
	[-122.5, 37.8],
	[-123.8, 39.5],
	[-124.4, 40.4]
];

function inCalifornia([lon, lat]: [number, number]) {
	let inside = false;
	for (let i = 0, j = CALIFORNIA.length - 1; i < CALIFORNIA.length; j = i++) {
		const [xi, yi] = CALIFORNIA[i];
		const [xj, yj] = CALIFORNIA[j];
		if (yi > lat !== yj > lat && lon < ((xj - xi) * (lat - yi)) / (yj - yi) + xi) inside = !inside;
	}
	return inside;
}

// [lon, lat, spread in degrees, share of the buses]
const TOWNS: [number, number, number, number][] = [
	[-118.25, 34.05, 0.35, 0.3],
	[-117.35, 34.0, 0.3, 0.07],
	[-117.1, 32.8, 0.2, 0.08],
	[-122.2, 37.65, 0.3, 0.18],
	[-121.45, 38.6, 0.25, 0.06],
	[-119.8, 36.75, 0.2, 0.04],
	[-119.0, 35.37, 0.2, 0.03],
	[-121.1, 37.8, 0.2, 0.03]
];

function californiaNetwork() {
	const next = random(5000);
	const normal = () => Math.sqrt(-2 * Math.log(1 - next())) * Math.cos(2 * Math.PI * next());
	const place = (): [number, number] => {
		for (;;) {
			const pick = next();
			let share = 0;
			const town = TOWNS.find(([, , , s]) => (share += s) > pick);
			const at: [number, number] = town
				? [town[0] + normal() * town[2], town[1] + normal() * town[2]]
				: [-124.5 + next() * 10.4, 32.5 + next() * 9.6];
			if (inCalifornia(at)) return at;
		}
	};
	const buses = Array.from({ length: BUS_COUNT }, (_, i) => {
		const [lon, lat] = place();
		const generator = next() < 0.12;
		return {
			id: i + 1,
			lon: Number(lon.toFixed(4)),
			lat: Number(lat.toFixed(4)),
			demand_mw: Number(Math.exp(2.3 + normal()).toFixed(1)),
			gen_mw: generator ? Number(Math.exp(4 + normal()).toFixed(1)) : 0
		};
	});
	return {
		id: 'ca',
		name: 'California',
		base_mva: 100,
		synthetic_coords: false,
		buses,
		branches: []
	};
}

const NETWORK = californiaNetwork();

// Prices that rise from the coast inland and to the south, so the markers
// take the whole price ramp.
const SOLUTION = {
	objective: 1,
	prices: NETWORK.buses.map((b) => ({
		bus: b.id,
		value: 30 + 4 * (b.lon + 124) - 2 * (b.lat - 32.5)
	})),
	va: [],
	w: [],
	flows: [],
	dispatch: []
};

const port = Number(
	process.env.TELLEGEN_MC_PREVIEW_PORT ?? Number(process.env.TELLEGEN_PREVIEW_PORT ?? 4173) + 1
);

type Options = {
	projection?: 'mercator' | 'globe';
	theme?: 'light' | 'dark';
	declutter?: boolean;
};

async function openCase(
	page: Page,
	{ projection = 'mercator', theme = 'light', declutter = true }: Options
) {
	await page.emulateMedia({ reducedMotion: 'reduce' });
	await page.route('**/api/compute', (route) => route.fulfill({ json: { enabled: true } }));
	await page.route('**/api/cases', (route) =>
		route.fulfill({
			json: [{ id: 'ca', name: 'California', n_bus: BUS_COUNT, n_branch: 0, n_gen: 600 }]
		})
	);
	await page.route('**/api/cases/ca/network', (route) => route.fulfill({ json: NETWORK }));
	await page.route('**/api/cases/ca/solution', (route) => route.fulfill({ json: SOLUTION }));
	await page.route('**/api/cases/ca/case', (route) =>
		route.fulfill({ status: 500, json: { error: 'case text unavailable' } })
	);
	await page.goto(
		`http://127.0.0.1:${port}/declutter.html?projection=${projection}&theme=${theme}&declutter=${declutter ? 1 : 0}`
	);
	await page.waitForFunction(() => window.declutter?.loaded());
	// The zoom buttons and the credit would count as markers.
	await page.addStyleTag({ content: '.maplibregl-control-container { display: none; }' });
}

/** Point the camera at `center` and wait for the map to draw there. */
async function look(page: Page, center: [number, number], zoom: number) {
	await page.evaluate(
		([center, zoom]) =>
			new Promise<void>((resolve) => {
				const map = window.declutter.map()!;
				map.jumpTo({ center, zoom });
				map.once('idle', () => resolve());
				map.triggerRepaint();
				setTimeout(resolve, 5_000);
			}),
		[center, zoom] as const
	);
	// Let the layer update for the new zoom land and draw.
	await page.evaluate(
		() =>
			new Promise<void>((resolve) =>
				requestAnimationFrame(() => requestAnimationFrame(() => resolve()))
			)
	);
}

type Coverage = {
	/** Marker pixels on screen. */
	pixels: number;
	/** Share of the California box that marker pixels cover. */
	share: number;
};

/** Count the pixels that differ from the plain background, on the whole
 * screen and inside the screen box of California. */
async function coverage(page: Page, theme: 'light' | 'dark', shot?: string): Promise<Coverage> {
	const box = await page.evaluate(() => {
		const map = window.declutter.map()!;
		const nw = map.project([-124.5, 42.1]);
		const se = map.project([-114.1, 32.4]);
		return { x0: nw.x, y0: nw.y, x1: se.x, y1: se.y };
	});
	const png = await page.screenshot(SHOTS && shot ? { path: `${SHOTS}/${shot}.png` } : {});
	return page.evaluate(
		async ({ base64, background, box }) => {
			const bytes = Uint8Array.from(atob(base64), (character) => character.charCodeAt(0));
			const image = await createImageBitmap(new Blob([bytes], { type: 'image/png' }));
			const canvas = new OffscreenCanvas(image.width, image.height);
			const context = canvas.getContext('2d')!;
			context.drawImage(image, 0, 0);
			const { data, width, height } = context.getImageData(0, 0, image.width, image.height);
			image.close();
			// Screenshot pixels to CSS pixels.
			const scale = width / window.innerWidth;
			const x0 = Math.max(0, Math.floor(box.x0 * scale));
			const x1 = Math.min(width, Math.ceil(box.x1 * scale));
			const y0 = Math.max(0, Math.floor(box.y0 * scale));
			const y1 = Math.min(height, Math.ceil(box.y1 * scale));
			let pixels = 0;
			let inBox = 0;
			for (let y = 0; y < height; y++) {
				for (let x = 0; x < width; x++) {
					const i = (y * width + x) * 4;
					const marker =
						Math.abs(data[i] - background[0]) > 40 ||
						Math.abs(data[i + 1] - background[1]) > 40 ||
						Math.abs(data[i + 2] - background[2]) > 40;
					if (!marker) continue;
					pixels++;
					if (x >= x0 && x < x1 && y >= y0 && y < y1) inBox++;
				}
			}
			const area = Math.max(1, (x1 - x0) * (y1 - y0));
			return { pixels: pixels / (scale * scale), share: inBox / area };
		},
		{ base64: png.toString('base64'), background: BACKGROUND[theme], box }
	);
}

const STATE: [number, number] = [-119.4, 37.2];
const LOS_ANGELES: [number, number] = [-118.25, 34.05];

// Measured on this grid at zoom 4.7: with every marker drawn, about 950
// markers show and cover 29% of the California box; one per crowded spot,
// about 210 show and cover 20%.
const THINNED_SHARE = 0.24;

test.describe('bus markers on a state-sized grid', () => {
	// The harness server compiles the map from source on first use, and the
	// map draws in software under a full parallel run.
	test.describe.configure({ timeout: 120_000 });

	for (const theme of ['light', 'dark'] as const) {
		test(`thin to one per crowded spot at low zoom, and fill in closer (${theme})`, async ({
			page
		}) => {
			await openCase(page, { theme });
			await look(page, STATE, 4.7);
			await expect
				.poll(() => page.evaluate((layer) => window.declutter.visible(layer).length, LAYER), {
					message: 'the bus markers draw',
					timeout: 60_000
				})
				.toBeGreaterThan(50);
			// The case frames itself once it loads, so aim the camera again.
			await look(page, STATE, 4.7);
			const state = await coverage(page, theme, `state-z4.7-${theme}`);
			const stateMarkers = await page.evaluate((layer) => window.declutter.visible(layer), LAYER);
			expect(state.share).toBeGreaterThan(0.02);
			expect(state.share).toBeLessThan(THINNED_SHARE);
			expect(stateMarkers.length).toBeLessThan(BUS_COUNT / 10);

			await look(page, LOS_ANGELES, 8);
			const city = await coverage(page, theme, `los-angeles-z8-${theme}`);
			const cityMarkers = await page.evaluate((layer) => window.declutter.visible(layer), LAYER);
			expect(city.pixels).toBeGreaterThan(state.pixels);
			expect(cityMarkers.length).toBeGreaterThan(stateMarkers.length);
		});
	}

	test('every marker draws with declutterMarkers off', async ({ page }) => {
		await openCase(page, { declutter: false });
		await look(page, STATE, 4.7);
		await expect
			.poll(() => page.evaluate((layer) => window.declutter.visible(layer).length, LAYER), {
				message: 'the bus markers draw',
				timeout: 60_000
			})
			.toBeGreaterThan(50);
		await look(page, STATE, 4.7);
		const state = await coverage(page, 'light', 'state-z4.7-light-every-marker');
		expect(state.share).toBeGreaterThan(THINNED_SHARE);
	});

	test('a selected bus that was thinned out draws again', async ({ page }) => {
		await openCase(page, {});
		await look(page, STATE, 4.7);
		await expect
			.poll(() => page.evaluate((layer) => window.declutter.visible(layer).length, LAYER), {
				message: 'the bus markers draw',
				timeout: 60_000
			})
			.toBeGreaterThan(50);
		await look(page, STATE, 4.7);
		// The smallest load in central Los Angeles: a crowded spot keeps a larger bus.
		const hidden = NETWORK.buses
			.filter((b) => Math.hypot(b.lon - LOS_ANGELES[0], b.lat - LOS_ANGELES[1]) < 0.1)
			.filter((b) => b.gen_mw === 0)
			.sort((a, b) => a.demand_mw - b.demand_mw)[0];
		expect(await page.evaluate((layer) => window.declutter.visible(layer), LAYER)).not.toContain(
			hidden.id
		);

		await page.evaluate((id) => window.declutter.selectBus(id), hidden.id);
		await expect(page.locator('.selected-bus-cue')).toBeVisible({ timeout: 30_000 });
		await expect
			.poll(() =>
				page.evaluate(([layer, lon, lat]) => window.declutter.topAt(layer, lon, lat), [
					LAYER,
					hidden.lon,
					hidden.lat
				] as const)
			)
			.toBe(hidden.id);
		if (SHOTS) await page.screenshot({ path: `${SHOTS}/state-z4.7-light-selected.png` });
	});

	test('thins on the globe too', async ({ page }) => {
		await openCase(page, { projection: 'globe' });
		await look(page, STATE, 4.7);
		await expect
			.poll(() => page.evaluate((layer) => window.declutter.visible(layer).length, LAYER), {
				message: 'the bus markers draw',
				timeout: 60_000
			})
			.toBeGreaterThan(50);
		await look(page, STATE, 4.7);
		const state = await coverage(page, 'light', 'state-z4.7-light-globe');
		expect(state.share).toBeGreaterThan(0.02);
		expect(state.share).toBeLessThan(THINNED_SHARE);
	});
});
