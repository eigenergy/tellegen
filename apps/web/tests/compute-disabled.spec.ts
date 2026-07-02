import { expect, test } from '@playwright/test';

// With server compute disabled (403), a selection that cannot solve in the
// browser shows the WebAssembly notice with the contact address, and the
// disabled answer is latched: the next selection shows the notice without
// firing another server request.

const CASE_SUMMARY = [{ id: 'case1', name: 'Case One', n_bus: 2, n_branch: 1, n_gen: 1 }];

const NETWORK = {
	id: 'case1',
	name: 'Case One',
	base_mva: 100,
	synthetic_coords: true,
	buses: [
		{ id: 1, lon: -80.1, lat: 34.1, demand_mw: 50, gen_mw: 0 },
		{ id: 2, lon: -80.4, lat: 34.4, demand_mw: 0, gen_mw: 120 }
	],
	branches: [
		{
			id: 1,
			from: 1,
			to: 2,
			rate_mw: 100,
			status: 1,
			path: [
				[-80.1, 34.1],
				[-80.4, 34.4]
			]
		}
	]
};

const SOLUTION = {
	objective: 1234.5,
	lmp: [
		{ bus: 1, usd_per_mwh: 25.2 },
		{ bus: 2, usd_per_mwh: 17.6 }
	],
	va: [
		{ bus: 1, value: 0 },
		{ bus: 2, value: 0.02 }
	],
	w: [],
	flows: [{ branch: 1, mw: 50, loading: 0.5 }],
	dispatch: [{ gen: 1, mw: 50 }]
};

async function lookupBus(page: import('@playwright/test').Page, bus: number) {
	const input = page.locator('#bus-lookup-input');
	await input.fill(String(bus));
	await input.press('Enter');
}

test('server compute disabled: wasm notice with contact, one request, latched', async ({
	page
}) => {
	let sensitivityFetches = 0;

	await page.route('**/api/cases', (route) => {
		void route.fulfill({ json: CASE_SUMMARY });
	});
	await page.route('**/api/cases/case1/network', (route) => {
		void route.fulfill({ json: NETWORK });
	});
	await page.route('**/api/cases/case1/solution', (route) => {
		void route.fulfill({ json: SOLUTION });
	});
	// Force the server sensitivity path: without the case text the browser
	// solver has nothing to solve, so selectBus reconciles via the server.
	await page.route('**/api/cases/case1/case', (route) => {
		void route.fulfill({ status: 500, json: { error: 'case text unavailable' } });
	});
	await page.route('**/api/cases/case1/sensitivity/**', (route) => {
		sensitivityFetches += 1;
		void route.fulfill({ status: 403, json: { error: 'server compute is disabled' } });
	});

	await page.goto('/');
	await expect(page.getByRole('heading', { name: /Case One/i })).toBeVisible({ timeout: 30_000 });

	await lookupBus(page, 1);
	const error = page.locator('.panel .error');
	await expect(error).toContainText('WebAssembly engine');
	await expect(error).toContainText('talks@umich.edu');
	expect(sensitivityFetches).toBe(1);

	// Latched: the next selection shows the notice without touching the server.
	await lookupBus(page, 2);
	await expect(error).toContainText('WebAssembly engine');
	expect(sensitivityFetches).toBe(1);
});
