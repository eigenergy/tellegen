import { expect, test } from './fixtures/page-errors.js';
import { lookupBus, mockDataRoutes } from './fixtures/backend-case.js';
import { openLog } from './fixtures/notices.js';

test('an error shows once in the toast and the Log, counts repeats, and holds while pointed at', async ({
	page
}, testInfo) => {
	await mockDataRoutes(page);
	let requests = 0;
	await page.route('**/api/cases/case1/sensitivity/**', (route) => {
		requests++;
		return route.fulfill({ status: 429, json: { error: 'sensitivity rate limit exceeded' } });
	});
	await page.goto('/');
	await expect(page.getByRole('heading', { name: /Case One/i })).toBeVisible();
	await lookupBus(page, 1);
	const toast = page.getByTestId('notification-toast');
	await expect(toast).toContainText('Rate limited');
	await expect(page.getByRole('complementary', { name: 'Network', exact: true })).not.toContainText(
		'Rate limited'
	);
	await toast.getByRole('button', { name: 'Retry', exact: true }).click();
	await expect.poll(() => requests).toBe(2);
	await expect(toast.getByLabel('2 occurrences')).toBeVisible();
	// The pointer on the toast holds it open.
	await toast.hover();
	await page.clock.install();
	await page.clock.fastForward(10_000);
	await expect(toast).toContainText('Rate limited');
	await page.setViewportSize({ width: 390, height: 844 });
	// Above the map's credit, which sits above the open sheet (the zoom buttons step aside).
	await expect
		.poll(async () => {
			const box = await toast.boundingBox();
			const credit = await page.locator('.maplibregl-ctrl-attrib').boundingBox();
			return !!box && !!credit && box.y + box.height < credit.y;
		})
		.toBe(true);
	await page.screenshot({ path: testInfo.outputPath('notification-mobile.png') });
	// Once it times out, the Log still has the entry, counted once with both reports.
	await page.mouse.move(2, 2);
	await page.clock.fastForward(7000);
	await expect(toast).toHaveCount(0);
	await expect(page.locator('.panel-toolbar').getByRole('button', { name: /^Log/ })).toContainText(
		'(1)'
	);
	const log = await openLog(page);
	await expect(log.getByTestId('log-entry')).toHaveCount(1);
	await expect(log).toContainText('Rate limited');
	await expect(log).toContainText('2 times');
});

test('local previews never request analytics and the preference persists', async ({ page }) => {
	let analyticsRequests = 0;
	await page.route(/https:\/\/(?:cloud|gateway)\.umami\.is\//, (route) => {
		analyticsRequests++;
		return route.abort();
	});
	await page.goto('/privacy/');
	const checkbox = page.getByRole('checkbox', { name: 'Disable usage analytics in this browser' });
	await expect(checkbox).toBeEnabled();
	await checkbox.check();
	await page.reload();
	await expect(checkbox).toBeChecked();
	expect(await page.evaluate(() => localStorage.getItem('umami.disabled'))).toBe('1');
	await checkbox.uncheck();
	await expect(page.getByRole('status')).toContainText('Analytics are enabled on the public site');
	expect(analyticsRequests).toBe(0);
});
