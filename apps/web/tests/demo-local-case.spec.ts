import { expect, test } from '@playwright/test';
import { CASE14 } from '../../../examples/browser-minimal/src/case14';
import { CASE14_COORDS } from './fixtures/local-case';

test('selected local case reaches the browser solve path', async ({ page }) => {
	await page.route('**/api/cases', (route) => {
		void route.fulfill({ json: [] });
	});

	await page.goto('/');

	await page.locator('input[type="file"]').setInputFiles([
		{
			name: 'case14-coords.csv',
			mimeType: 'text/csv',
			buffer: Buffer.from(CASE14_COORDS)
		},
		{
			name: 'case14synthetic.m',
			mimeType: 'text/plain',
			buffer: Buffer.from(CASE14)
		}
	]);

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

test('slider drag shows the step-scaled ΔLMP preview legend', async ({ page }) => {
	await page.route('**/api/cases', (route) => {
		void route.fulfill({ json: [] });
	});

	await page.goto('/');

	await page.locator('input[type="file"]').setInputFiles([
		{
			name: 'case14-coords.csv',
			mimeType: 'text/csv',
			buffer: Buffer.from(CASE14_COORDS)
		},
		{
			name: 'case14synthetic.m',
			mimeType: 'text/plain',
			buffer: Buffer.from(CASE14)
		}
	]);
	await expect(page.locator('.solvecard')).toContainText('OPF solve', { timeout: 60_000 });

	// Select a bus through the keyboard path and wait for the browser column.
	const lookup = page.locator('#bus-lookup-input');
	await lookup.fill('3');
	await lookup.press('Enter');
	await expect(page.locator('.chip', { hasText: '∂LMP/∂d' })).toBeVisible({ timeout: 30_000 });

	// Drive a preview without committing: set the slider value and fire only an
	// input event (pointerup/change would trigger the exact re-solve).
	const slider = page.getByLabel('demand delta at selected bus');
	await slider.evaluate((el) => {
		const input = el as HTMLInputElement;
		input.value = input.max;
		input.dispatchEvent(new Event('input', { bubbles: true }));
	});

	await expect(page.getByText('First order LMP preview')).toBeVisible();
	// case14 is uncongested, so the ∂LMP/∂d column is flat and the preview legend
	// shows the uniform predicted shift. The value must scale with the step — the
	// regression this guards is the step cancelling out of the preview display.
	const labels = page.locator('.sensitivity-readout .legend-labels');
	await expect(labels).toContainText(/uniform [+−-][\d.]+e[+-]\d+ \$\/MWh/);
	const atFull = parseShift(await labels.innerText());

	await slider.evaluate((el) => {
		const input = el as HTMLInputElement;
		input.value = String(Number(input.max) / 2);
		input.dispatchEvent(new Event('input', { bubbles: true }));
	});
	await expect.poll(async () => parseShift(await labels.innerText()) / atFull).toBeGreaterThan(0.4);
	expect(parseShift(await labels.innerText()) / atFull).toBeLessThan(0.6);
});

function parseShift(text: string): number {
	const m = text.replace('−', '-').match(/(-?[\d.]+e[+-]\d+)/);
	if (!m) throw new Error(`no shift value in ${JSON.stringify(text)}`);
	return Number(m[1]);
}
