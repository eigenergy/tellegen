import type { Page } from '@playwright/test';
import { expect } from '@playwright/test';

/** Choose a calculation in the Network panel. Choosing one runs it; the select
 * is disabled while it solves. */
export async function runCalculation(page: Page, formulation: string) {
	const selection = page.locator('#formulation-select');
	await expect(selection).toBeEnabled();
	await selection.selectOption(formulation);
}
