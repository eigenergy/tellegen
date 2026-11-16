import { expect, test } from './fixtures/page-errors.js';
import { openSettings } from './fixtures/chrome.js';

// Menu > Settings must show Settings even when it is already open behind
// another panel: on a phone the menu is the only way to reach it.

test('on a phone, Menu > Settings brings Settings back after another tab took the sheet', async ({
	page
}) => {
	await page.setViewportSize({ width: 390, height: 844 });
	await page.route('**/api/cases', (route) => route.fulfill({ json: [] }));
	await page.goto('/');
	await expect(page.getByText('No cases', { exact: true })).toBeVisible();
	await openSettings(page);
	const experimental = page.getByLabel('Experimental features');
	await expect(experimental).toBeVisible();
	// The sheet names Settings and offers a way back to the tabs.
	const strip = page.locator('.panel-toolbar');
	await expect(strip.getByRole('heading', { name: 'Settings', exact: true })).toBeVisible();
	await strip.getByRole('button', { name: 'Back', exact: true }).click();
	await expect(page.locator('[data-panel="network"]')).toBeVisible();
	await expect(experimental).toBeHidden();
	await openSettings(page);
	await expect(experimental).toBeVisible();
	await expect(page.locator('[data-panel="network"]')).toBeHidden();
});

test('on a wide screen, Menu > Settings brings Settings to the front of its dock', async ({
	page
}) => {
	await page.route('**/api/cases', (route) => route.fulfill({ json: [] }));
	await page.goto('/');
	await expect(page.getByText('No cases', { exact: true })).toBeVisible();
	await openSettings(page);
	const experimental = page.getByLabel('Experimental features');
	await expect(experimental).toBeVisible();
	// Dock another panel on the right, in front of Settings.
	await page.locator('.panel-toolbar').getByRole('button', { name: 'Log', exact: true }).click();
	await page.getByRole('button', { name: 'Log panel options' }).click();
	await page.getByRole('button', { name: 'Dock right', exact: true }).click();
	await expect(page.locator('[data-panel-dock="right"] [data-panel="log"]')).toBeVisible();
	await expect(experimental).toBeHidden();
	// The palette offers to show Settings, not to close a panel nobody can see.
	await page.keyboard.press('ControlOrMeta+k');
	const palette = page.getByRole('dialog', { name: 'Command palette' });
	await palette.getByRole('combobox').fill('settings panel');
	await expect(palette.getByRole('option', { name: /Open Settings panel/ })).toBeVisible();
	await page.keyboard.press('Escape');
	await openSettings(page);
	await expect(experimental).toBeVisible();
	// Escape closes Settings while it has focus.
	await experimental.focus();
	await page.keyboard.press('Escape');
	await expect(experimental).toBeHidden();
});
