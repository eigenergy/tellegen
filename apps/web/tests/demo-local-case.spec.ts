import { expect, test } from '@playwright/test';
import { CASE14 } from '../../../examples/browser-minimal/src/case14';
import { CASE14_COORDS } from './fixtures/local-case';

test('dropped local case reaches the browser solve path', async ({ page }) => {
	await page.route('**/api/cases', (route) => {
		void route.fulfill({ json: [] });
	});

	await page.goto('/');

	await page.evaluate(
		({ caseText, coordsText }) => {
			const transfer = new DataTransfer();
			transfer.items.add(new File([coordsText], 'case14-coords.csv', { type: 'text/csv' }));
			transfer.items.add(new File([caseText], 'case14synthetic.m', { type: 'text/plain' }));

			for (const type of ['dragenter', 'dragover', 'drop']) {
				window.dispatchEvent(
					new DragEvent(type, {
						bubbles: true,
						cancelable: true,
						dataTransfer: transfer
					})
				);
			}
		},
		{ caseText: CASE14, coordsText: CASE14_COORDS }
	);

	await expect(page.getByRole('heading', { name: /case14synthetic/i })).toBeVisible({
		timeout: 30_000
	});
	await expect(
		page.getByText('parsed in your browser by powerio (wasm); never uploaded')
	).toBeVisible();

	const solveCard = page.locator('.solvecard');
	await expect(solveCard).toContainText('OPF solve', { timeout: 60_000 });
	await expect(solveCard).toContainText('DC OPF');
	await expect(solveCard).toContainText(/\d+ iterations/);
	await expect(solveCard.getByLabel('residual by solver iteration')).toBeVisible();
	await expect(solveCard).toContainText(/\d+ ms/);
	await expect(solveCard).not.toContainText('server solve');
	await expect(solveCard.locator('.fallback-reason')).toHaveCount(0);
});
