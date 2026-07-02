import type { Page } from '@playwright/test';

// One mocked two-bus backend case shared by the server fallback specs. The
// /case route answers 500 so the browser solver has nothing to solve and the
// controller reconciles selections via the server.

export const CASE_SUMMARY = [{ id: 'case1', name: 'Case One', n_bus: 2, n_branch: 1, n_gen: 1 }];

export const NETWORK = {
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

export const SOLUTION = {
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

export function sensitivityColumn(bus: number) {
	return {
		case: 'case1',
		operand: 'lmp',
		parameter: 'd',
		bus,
		units: '($/MWh)/MW',
		values: [
			{ bus: 1, value: 1.7e-3 },
			{ bus: 2, value: -0.9e-3 }
		]
	};
}

export async function lookupBus(page: Page, bus: number) {
	const input = page.locator('#bus-lookup-input');
	await input.fill(String(bus));
	await input.press('Enter');
}

/** Mock the data routes; sensitivity routes stay with the caller. Compute is
 * reported enabled so specs are independent of whatever real server the
 * preview proxy reaches; the compute-disabled spec overrides this route. */
export async function mockDataRoutes(page: Page, opts: { onCases?: () => void } = {}) {
	await page.route('**/api/compute', (route) => {
		void route.fulfill({ json: { enabled: true } });
	});
	await page.route('**/api/cases', (route) => {
		opts.onCases?.();
		void route.fulfill({ json: CASE_SUMMARY });
	});
	await page.route('**/api/cases/case1/network', (route) => {
		void route.fulfill({ json: NETWORK });
	});
	await page.route('**/api/cases/case1/solution', (route) => {
		void route.fulfill({ json: SOLUTION });
	});
	await page.route('**/api/cases/case1/case', (route) => {
		void route.fulfill({ status: 500, json: { error: 'case text unavailable' } });
	});
}
