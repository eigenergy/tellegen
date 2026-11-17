import { expect, test } from './fixtures/page-errors.js';
import { mockDataRoutes, SOLUTION } from './fixtures/backend-case.js';
for (const kind of ['feasibility', 'cost'] as const) {
	test(`zero objective with ${kind} metadata has the correct calculation label`, async ({
		page
	}) => {
		await mockDataRoutes(page);
		await page.route('**/api/cases/case1/solution', (route) =>
			route.fulfill({ json: { ...SOLUTION, objective: 0, objective_kind: kind } })
		);
		await page.goto('/');
		await expect(page.getByRole('heading', { name: 'Case One' })).toBeVisible();
		await expect(
			page.getByRole('combobox', { name: 'Calculation', exact: true }).locator('option:checked')
		).toHaveText(kind === 'feasibility' ? 'DC feasibility' : 'DC OPF');
		if (kind === 'feasibility') {
			await expect(page.getByText('Max loading', { exact: true })).toBeVisible();
			await expect(page.getByText('Operational cost', { exact: true })).toHaveCount(0);
			await expect(page.getByText('Lines at limit', { exact: true })).toHaveCount(0);
		} else {
			await expect(page.getByText('Operational cost', { exact: true })).toBeVisible();
			await expect(page.getByText('$0.00/h', { exact: true })).toBeVisible();
		}
	});
}
