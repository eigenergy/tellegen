import type { Page } from '@playwright/test';
import { expect } from './page-errors.js';

/** The Log, which keeps every message, including notes that never raise a notice. */
export async function openLog(page: Page) {
	const log = page.getByTestId('diagnostic-log');
	if (!(await log.isVisible()))
		await page
			.locator('.panel-toolbar')
			.getByRole('button', { name: /^Log\b/ })
			.click();
	await expect(log).toBeVisible();
	return log;
}

/** The details of the latest message: from its notice while it is on screen,
 * else from the Log, which keeps every message after the notice closes. */
export async function noticeDetails(page: Page) {
	const section = page.getByRole('region', { name: 'Notifications', exact: true });
	const toggle = section.getByRole('button', { name: /^(Details|Messages)/ });
	const showing = await toggle
		.waitFor({ state: 'visible', timeout: 5_000 })
		.then(() => true)
		.catch(() => false);
	if (!showing) return openLog(page);
	if ((await toggle.getAttribute('aria-expanded')) !== 'true') await toggle.click();
	// An open notice expands its own details; the list of past messages shows after it closes.
	const inline = section.getByTestId('notification-toast').getByTestId('notification-details');
	if (await inline.count()) return inline;
	// The open notice has only one message, so its title is not repeated in a list.
	const first = section.locator('details').first();
	if (!(await first.evaluate((element) => (element as HTMLDetailsElement).open)))
		await first.locator('summary').click();
	return first.getByTestId('notification-details');
}
