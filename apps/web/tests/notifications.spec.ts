import { expect, test } from './fixtures/page-errors.js';
import { lookupBus, mockDataRoutes } from './fixtures/backend-case.js';
import { noticeDetails } from './fixtures/notices.js';

test('temporary errors group repeats, pause during interaction, and retain full details', async ({
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
	await expect(toast).toContainText('Too many requests');
	await expect(page.getByRole('complementary', { name: 'Network', exact: true })).not.toContainText(
		'rate limited'
	);
	await toast.getByRole('button', { name: 'Retry', exact: true }).click();
	await expect.poll(() => requests).toBe(2);
	await expect(toast.getByLabel('2 occurrences')).toBeVisible();
	const details = await noticeDetails(page);
	await expect(details).toContainText('rate limited; wait a few seconds and try again');
	await page.clock.install();
	await page.clock.fastForward(10_000);
	await expect(toast).toContainText('Too many requests');
	await page.setViewportSize({ width: 390, height: 844 });
	const box = await toast.boundingBox();
	const zoom = await page.getByRole('button', { name: 'Zoom in', exact: true }).boundingBox();
	expect(box).not.toBeNull();
	expect(zoom).not.toBeNull();
	expect(box!.y + box!.height).toBeLessThan(zoom!.y);
	await page.screenshot({ path: testInfo.outputPath('notification-mobile.png') });
	await page
		.getByRole('region', { name: 'Notifications' })
		.getByRole('button', { name: 'Details', exact: true })
		.click();
	await page.getByRole('button', { name: 'Network', exact: true }).focus();
	await page.mouse.move(2, 2);
	await page.clock.fastForward(7000);
	await expect(toast.locator('.message')).toHaveCount(0);
	await expect(await noticeDetails(page)).toContainText('rate limited');
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
