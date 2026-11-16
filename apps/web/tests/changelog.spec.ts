import { expect, test } from './fixtures/page-errors.js';

// The changelog is reached from the header menu, and its back link returns to the map.
test("Menu > What's new opens the changelog, and Back to the map returns", async ({ page }) => {
	await page.route('**/api/cases', (route) => route.fulfill({ json: [] }));
	await page.goto('/');
	await expect(page.getByText('No cases', { exact: true })).toBeVisible();
	await page.locator('header').getByRole('button', { name: 'Menu', exact: true }).click();
	await page.getByRole('menuitem', { name: "What's new", exact: true }).click();
	await expect(page).toHaveURL(/\/changelog\/?$/);
	await expect(
		page.getByRole('heading', { level: 1, name: "What's new in Tellegen" })
	).toBeVisible();
	await page.getByRole('link', { name: 'Back to the map', exact: true }).click();
	await expect(page).toHaveURL(/\/$/);
	await expect(page.getByText('No cases', { exact: true })).toBeVisible();
});
