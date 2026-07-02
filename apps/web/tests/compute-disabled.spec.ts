import { expect, test } from '@playwright/test';
import { lookupBus, mockDataRoutes } from './fixtures/backend-case';

// With server compute disabled, a selection that cannot solve in the browser
// shows the compute-off notice with the contact address. Detection has two
// paths: /api/compute reports the gate up front (no doomed request is ever
// made), and when that endpoint is unavailable the first 403 latches it.

test('compute reported off up front: notice shown, no server sensitivity request', async ({
	page
}) => {
	let sensitivityFetches = 0;
	await mockDataRoutes(page);
	await page.route('**/api/compute', (route) => {
		void route.fulfill({ json: { enabled: false } });
	});
	await page.route('**/api/cases/case1/sensitivity/**', (route) => {
		sensitivityFetches += 1;
		void route.fulfill({ status: 403, json: { error: 'server compute is disabled' } });
	});

	await page.goto('/');
	await expect(page.getByRole('heading', { name: /Case One/i })).toBeVisible({ timeout: 30_000 });

	await lookupBus(page, 1);
	const error = page.locator('.panel .error');
	await expect(error).toContainText('server side compute is disabled');
	await expect(error).toContainText('talks@umich.edu');
	expect(sensitivityFetches).toBe(0);
});

test('compute status unavailable: first 403 shows the notice and latches', async ({ page }) => {
	let sensitivityFetches = 0;
	await mockDataRoutes(page);
	await page.route('**/api/compute', (route) => {
		void route.fulfill({ status: 404, json: { error: 'unknown API route' } });
	});
	await page.route('**/api/cases/case1/sensitivity/**', (route) => {
		sensitivityFetches += 1;
		void route.fulfill({ status: 403, json: { error: 'server compute is disabled' } });
	});

	await page.goto('/');
	await expect(page.getByRole('heading', { name: /Case One/i })).toBeVisible({ timeout: 30_000 });

	await lookupBus(page, 1);
	const error = page.locator('.panel .error');
	await expect(error).toContainText('server side compute is disabled');
	await expect(error).toContainText('talks@umich.edu');
	expect(sensitivityFetches).toBe(1);

	// Latched: the next selection shows the notice without touching the server.
	await lookupBus(page, 2);
	await expect(error).toContainText('server side compute is disabled');
	expect(sensitivityFetches).toBe(1);
});
