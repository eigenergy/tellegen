import type { Page } from '@playwright/test';
import { expect } from './page-errors.js';
export async function noticeDetails(page: Page) {
	const section = page.getByRole('region', { name: 'Notifications', exact: true });
	await expect(section).toBeVisible();
	const toggle = section.getByRole('button', { name: /^(Details|Messages)/ });
	if ((await toggle.getAttribute('aria-expanded')) !== 'true') await toggle.click();
	const first = section.locator('details').first();
	if (!(await first.evaluate((element) => (element as HTMLDetailsElement).open)))
		await first.locator('summary').click();
	return first.getByTestId('notification-details');
}
