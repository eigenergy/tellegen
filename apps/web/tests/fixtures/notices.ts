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

/** The details of the latest message: from its toast while it is on screen and
 * has more to say than its title, else from the Log, which keeps every message. */
export async function noticeDetails(page: Page) {
	const toast = page.getByTestId('notification-toast');
	const showing = await toast
		.waitFor({ state: 'visible', timeout: 5_000 })
		.then(() => true)
		.catch(() => false);
	const toggle = toast.getByRole('button', { name: 'Details', exact: true });
	if (!showing || !(await toggle.count())) return openLog(page);
	if ((await toggle.getAttribute('aria-expanded')) !== 'true') await toggle.click();
	return toast.getByTestId('notification-details');
}
