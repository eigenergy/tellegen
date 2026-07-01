import { expect, test } from '@playwright/test';

// A rate-limited (429) server sensitivity fallback must read as a rate limit,
// fire exactly one request per selection (no blind retry), honor the cooldown,
// and let the retry button re-run the failed selection rather than reload the
// case list.

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

function sensitivityColumn(bus: number) {
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

async function lookupBus(page: import('@playwright/test').Page, bus: number) {
	const input = page.locator('#bus-lookup-input');
	await input.fill(String(bus));
	await input.press('Enter');
}

test('429 sensitivity fallback: honest copy, one request, cooldown, working retry', async ({
	page
}) => {
	let casesFetches = 0;
	let sensitivityFetches = 0;
	let sensitivityStatus = 429;

	await page.route('**/api/cases', (route) => {
		casesFetches += 1;
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
		if (sensitivityStatus === 429) {
			void route.fulfill({ status: 429, json: { error: 'sensitivity rate limit exceeded' } });
		} else {
			const bus = Number(new URL(route.request().url()).pathname.split('/').pop());
			void route.fulfill({ json: sensitivityColumn(bus) });
		}
	});

	await page.goto('/');
	await expect(page.getByRole('heading', { name: /Case One/i })).toBeVisible({ timeout: 30_000 });

	// One selection, one 429: the rate limit copy, no "Error:" prefix, no retry
	// of the same doomed request.
	await lookupBus(page, 1);
	const error = page.locator('.panel .error');
	await expect(error).toHaveText('rate limited; wait a few seconds and try again');
	expect(sensitivityFetches).toBe(1);

	// The cooldown suppresses the server call for the next selection entirely.
	await lookupBus(page, 2);
	await expect(error).toHaveText('rate limited; wait a few seconds and try again');
	expect(sensitivityFetches).toBe(1);

	// Retry re-runs the failed bus selection (not a case list reload) and the
	// recovered column renders the sensitivity readout.
	sensitivityStatus = 200;
	await page.getByRole('button', { name: 'retry', exact: true }).click();
	await expect(page.locator('.chip', { hasText: '∂LMP/∂d' })).toBeVisible();
	await expect(error).toHaveCount(0);
	expect(sensitivityFetches).toBe(2);
	expect(casesFetches).toBe(1);
});
