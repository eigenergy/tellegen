import { expect, test } from './fixtures/page-errors.js';
import { CASE3_COORDS, CASE3_PLANNING } from './fixtures/planning-case.js';

test.skip(!process.env.TELLEGEN_TEST_ACOPF, 'requires the explicit development AC OPF asset');

test('the capability-gated selector runs AC OPF in the isolated WASI worker', async ({ page }) => {
	test.setTimeout(180_000);
	await page.route('**/api/compute', (route) => route.fulfill({ json: { enabled: false } }));
	await page.route('**/api/cases', (route) => route.fulfill({ json: [] }));
	await page.goto('/');
	await expect(page.getByText('no default cases loaded')).toBeVisible();
	await page.locator('input[type="file"][multiple]').setInputFiles([
		{ name: 'case3-coords.csv', mimeType: 'text/csv', buffer: Buffer.from(CASE3_COORDS) },
		{ name: 'case3opf.m', mimeType: 'text/plain', buffer: Buffer.from(CASE3_PLANNING) }
	]);
	const calculation = page.getByRole('combobox', { name: 'Calculation', exact: true });
	await expect(calculation).toBeEnabled({ timeout: 60_000 });
	await expect(calculation.locator('option[value="acopf"]')).toHaveJSProperty('disabled', false);
	await calculation.selectOption('acopf');
	await expect(page.locator('.solvecard')).toContainText('AC OPF');
	await expect(page.locator('.solvecard')).toContainText('NLP iterations', { timeout: 120_000 });
	await expect(calculation).toBeEnabled();
	await expect(page.getByRole('button', { name: '|V|', exact: true })).toHaveAttribute(
		'aria-pressed',
		'true'
	);
	await expect(page.getByRole('button', { name: 'LMP', exact: true })).toHaveCount(0);
	await expect(page.locator('.error')).toHaveCount(0);
});
