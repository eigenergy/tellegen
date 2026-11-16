import { expect, test } from './fixtures/page-errors.js';

// A case list the server cannot send: the Network panel says so once, with Retry and a
// way to open a file, and the Log keeps the message. No toast repeats it over the map.
test('a failed case list shows once in the panel and is kept in the Log', async ({ page }) => {
	await page.route('**/api/compute', (route) => route.fulfill({ json: { enabled: false } }));
	let fail = true;
	await page.route('**/api/cases', (route) =>
		fail ? route.fulfill({ status: 502, body: 'Bad gateway' }) : route.fulfill({ json: [] })
	);
	await page.goto('/');
	const panel = page.getByRole('complementary', { name: 'Network', exact: true });
	await expect(panel.getByRole('alert')).toHaveText('Could not load the case list');
	await expect(panel.getByRole('button', { name: 'Open case file', exact: true })).toBeVisible();
	// The raw error waits behind Details.
	await panel.getByText('Details', { exact: true }).click();
	await expect(panel.locator('.load-details pre')).toContainText('502');
	await expect(page.getByTestId('notification-toast')).toBeHidden();
	// The header does not claim there are no cases when the list failed to load.
	await expect(page.locator('header .case-button')).toHaveText('Open a case');
	await expect(page.getByRole('button', { name: /^Log, 1 new/ })).toBeVisible();
	// The map fills the page below the top bar: no band of background at the bottom.
	const canvas = page.locator('.map canvas').first();
	await expect
		.poll(async () => {
			const box = await canvas.boundingBox();
			return box ? Math.round(box.y + box.height) : 0;
		})
		.toBe(page.viewportSize()!.height);
	fail = false;
	await panel.getByRole('button', { name: 'Retry', exact: true }).click();
	await expect(panel.getByText('No cases', { exact: true })).toBeVisible();
});
