import { expect, test } from '@playwright/test';
import { lookupBus, mockDataRoutes, sensitivityColumn } from './fixtures/backend-case';

// A rate-limited (429) server sensitivity fallback must read as a rate limit,
// fire exactly one request per selection (no blind retry), honor the cooldown,
// and let the retry button re-run the failed selection rather than reload the
// case list.

test('429 sensitivity fallback: honest copy, one request, cooldown, working retry', async ({
	page
}) => {
	let casesFetches = 0;
	let sensitivityFetches = 0;
	let sensitivityStatus = 429;

	await mockDataRoutes(page, { onCases: () => (casesFetches += 1) });
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
	// Wait for the settled readout (the chip also renders while loading) before
	// counting requests, so the assertion doesn't race the in-flight fetch.
	await expect(page.getByText('LMP response per MW of demand at bus 2')).toBeVisible();
	await expect(error).toHaveCount(0);
	await expect.poll(() => sensitivityFetches).toBe(2);
	expect(casesFetches).toBe(1);
});
