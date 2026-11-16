import type { Page } from '@playwright/test';
import { expect } from '@playwright/test';

/** Open Settings from the header menu. */
export async function openSettings(page: Page) {
	await page.locator('header').getByRole('button', { name: 'Menu', exact: true }).click();
	await page.getByRole('menuitem', { name: 'Settings', exact: true }).click();
}

/** Open the header's case menu, which lists every case and the file actions. */
export async function openCaseMenu(page: Page) {
	const button = page.locator('header .case-button');
	if ((await button.getAttribute('aria-expanded')) !== 'true') await button.click();
	await expect(page.locator('#tellegen-case-list')).toBeVisible();
	return page.locator('#tellegen-case-list');
}

/** Bring the Network panel to the front, opening it if it is closed. Its tab
 * toggles, so a second click would close it. */
export async function showNetwork(page: Page) {
	const tab = page.locator('.panel-toolbar').getByRole('button', { name: 'Network', exact: true });
	if ((await tab.getAttribute('aria-pressed')) !== 'true') await tab.click();
	await expect(tab).toHaveAttribute('aria-pressed', 'true');
}
