import { expect, test } from '@playwright/test';

// The JSON content classifier routes non-package objects to the BMOPF reader,
// which is liberal: an arbitrary object parses as an empty case. This guards
// that a stray `.json` object falls through to the geo sidecar path and its
// precise error, instead of landing as a phantom empty multiconductor case.
test('a stray JSON object is not swallowed as an empty multiconductor case', async ({ page }) => {
	await page.route('**/api/cases', (route) => {
		void route.fulfill({ json: [] });
	});

	await page.goto('/');
	// The prerendered input exists before hydration attaches its listener; a
	// drop fired earlier is lost. The empty-cases panel renders only after load.
	await expect(page.getByText('no default cases loaded')).toBeVisible();

	await page.locator('input[type="file"]').setInputFiles([
		{
			name: 'stray.json',
			mimeType: 'application/json',
			buffer: Buffer.from(JSON.stringify({ foo: 'bar', notes: [1, 2, 3] }))
		}
	]);

	await expect(page.locator('p.error')).toContainText('no bus coordinates', {
		timeout: 30_000
	});
	await expect(page.getByRole('heading', { name: /stray/i })).toHaveCount(0);
});
