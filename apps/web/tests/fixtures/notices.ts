import type { Page } from '@playwright/test';
import { expect } from './page-errors.js';

/** The details of the latest message: from its notice while it is on screen,
 * else from the Log, which keeps every message after the notice closes. */
export async function noticeDetails(page: Page) {
	const section = page.getByRole('region', { name: 'Notifications', exact: true });
	const toggle = section.getByRole('button', { name: /^(Details|Messages)/ });
	const showing = await toggle
		.waitFor({ state: 'visible', timeout: 5_000 })
		.then(() => true)
		.catch(() => false);
	if (!showing) {
		const log = page.getByTestId('diagnostic-log');
		if (!(await log.isVisible()))
			await page
				.locator('.panel-toolbar')
				.getByRole('button', { name: /^Log\b/ })
				.click();
		await expect(log).toBeVisible();
		return log;
	}
	if ((await toggle.getAttribute('aria-expanded')) !== 'true') await toggle.click();
	const first = section.locator('details').first();
	if (!(await first.evaluate((element) => (element as HTMLDetailsElement).open)))
		await first.locator('summary').click();
	return first.getByTestId('notification-details');
}
