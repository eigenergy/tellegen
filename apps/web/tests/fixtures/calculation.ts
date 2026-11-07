import type { Page } from '@playwright/test';
import { expect } from '@playwright/test';

/** Select the next calculation, then run it explicitly through common settings. */
export async function runCalculation(
	page: Page,
	formulation: string,
	{ closeSettings = true } = {}
) {
	const selection = page.locator('#formulation-select');
	await expect(selection).toBeEnabled();
	await selection.selectOption(formulation);
	await page.locator('header').getByRole('button', { name: 'Settings', exact: true }).click();
	const panel = page.getByRole('region', { name: 'Common settings' });
	await expect(panel.getByRole('combobox', { name: 'Calculation', exact: true })).toHaveValue(
		formulation
	);
	await panel.getByRole('button', { name: 'Run calculation', exact: true }).click();
	if (closeSettings)
		await page.getByRole('button', { name: 'Close settings panel', exact: true }).click();
	return panel;
}
