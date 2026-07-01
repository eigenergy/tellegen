import { expect, test } from '@playwright/test';

// End-to-end branch rating flow on a congested local case: select the binding
// line from the panel list, see a non-flat ∂LMP/∂rating column, relax the
// rating, and confirm the exact re-solve lowers the cost.

// The engine crate's 3-bus fixture with the bus2-bus3 line tightened to 40 MW
// (the congested_case3 operating point): that line binds without shedding.
const CASE3_CONGESTED = `function mpc = case3test
mpc.version = '2';
mpc.baseMVA = 100;
mpc.bus = [
 1 3 0  0  0 0 1 1 0 230 1 1.1 0.9;
 2 1 90 30 0 0 1 1 0 230 1 1.1 0.9;
 3 2 0  0  0 0 1 1 0 230 1 1.1 0.9;
];
mpc.gen = [
 1 0  0 300 -300 1 100 1 250 10 0 0 0 0 0 0 0 0 0 0 0;
 3 60 0 300 -300 1 100 1 270 10 0 0 0 0 0 0 0 0 0 0 0;
];
mpc.branch = [
 1 2 0.01 0.1 0 250 250 250 0 0 1 -360 360;
 1 3 0.01 0.1 0 250 250 250 0 0 1 -360 360;
 2 3 0.01 0.1 0 40 40 40 0 0 1 -360 360;
];
mpc.gencost = [
 2 0 0 3 0.11  5   0;
 2 0 0 3 0.085 1.2 0;
];
`;

const CASE3_COORDS = `bus_i,lat,lon
1,34.0,-81.1
2,34.1,-81.0
3,34.0,-80.9
`;

test('binding line: select from the list, non-flat column, rating commit lowers cost', async ({
	page
}) => {
	await page.route('**/api/cases', (route) => {
		void route.fulfill({ json: [] });
	});

	await page.goto('/');
	await page.locator('input[type="file"]').setInputFiles([
		{ name: 'case3-coords.csv', mimeType: 'text/csv', buffer: Buffer.from(CASE3_COORDS) },
		{ name: 'case3congested.m', mimeType: 'text/plain', buffer: Buffer.from(CASE3_CONGESTED) }
	]);
	await expect(page.locator('.solvecard')).toContainText('OPF solve', { timeout: 60_000 });

	// The tightened line binds, so the panel lists it as the selection target.
	const bindingLine = page.locator('.binding-lines button');
	await expect(bindingLine).toHaveCount(1);
	await expect(bindingLine).toContainText('line 2');
	await expect(bindingLine).toContainText('100%');

	await bindingLine.click();
	await expect(page.locator('.chip', { hasText: '∂LMP/∂rating' })).toBeVisible({
		timeout: 30_000
	});
	await expect(page.getByText(/LMP response per MVA of rating on line/)).toBeVisible();
	// A binding line's column is structured, not the flat/uniform tint.
	const labels = page.locator('.sensitivity-readout .legend-labels');
	await expect(labels).not.toContainText('uniform');
	await expect(labels).toContainText(/[+−-]\d\.\de[+-]\d+/);

	// Relax the rating to the slider max and commit (change fires the exact solve).
	const slider = page.getByLabel('rating delta at selected line');
	await slider.evaluate((el) => {
		const input = el as HTMLInputElement;
		input.value = input.max;
		input.dispatchEvent(new Event('input', { bubbles: true }));
		input.dispatchEvent(new Event('change', { bubbles: true }));
	});

	// Relaxing a binding limit lowers the objective: the committed slider shows
	// the +8 MW rating with a negative gradient/exact score pair, and the exact
	// value comes from the re-solve (a local case has no server fallback).
	const panel = page.locator('.panel');
	await expect(panel).toContainText('+8 MW', { timeout: 30_000 });
	await expect(panel).toContainText(/gradient −[\d.,]+ · exact −[\d.,]+/, {
		timeout: 30_000
	});
});
