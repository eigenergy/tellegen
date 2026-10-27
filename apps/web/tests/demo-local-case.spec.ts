import { expect, test } from './fixtures/page-errors.js';
import { CASE14 } from '../../../examples/browser-minimal/src/case14';
import { CASE14_COORDS } from './fixtures/local-case';

test('selected local case reaches the browser solve path', async ({ page }) => {
	await page.route('**/api/cases', (route) => {
		void route.fulfill({ json: [] });
	});

	await page.goto('/');
	// The prerendered input exists before hydration attaches its listener; a
	// drop fired earlier is lost. The empty-cases panel renders only after load.
	await expect(page.getByText('no default cases loaded')).toBeVisible();

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
	await expect(page.getByText('From case14-coords.csv', { exact: true })).toBeVisible();

	const solveCard = page.locator('.solvecard');
	await expect(solveCard).toContainText('DC OPF', { timeout: 60_000 });
	await expect(solveCard).toContainText(/Iterations\s*\d+/);
	await expect(solveCard.getByLabel('Residual by solver iteration')).toBeVisible();
	await expect(solveCard).toContainText(/\d+ ms/);
	await expect(solveCard).not.toContainText('Server');
	await expect(solveCard).not.toContainText('Fallback');
});

test('slider drag shows the scaled LMP preview legend', async ({ page }) => {
	await page.route('**/api/cases', (route) => {
		void route.fulfill({ json: [] });
	});

	await page.goto('/');
	// The prerendered input exists before hydration attaches its listener; a
	// drop fired earlier is lost. The empty-cases panel renders only after load.
	await expect(page.getByText('no default cases loaded')).toBeVisible();

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
	await expect(page.locator('.solvecard')).toContainText('DC OPF', { timeout: 60_000 });

	// Select a bus through the keyboard path and wait for the browser column.
	const lookup = page.locator('#bus-lookup-input');
	await lookup.fill('3');
	await lookup.press('Enter');
	await expect(page.locator('.chip', { hasText: '∂LMP/∂d' })).toBeVisible({ timeout: 30_000 });

	// Drive a preview without committing: set the slider value and fire only an
	// input event (pointerup/change would trigger the exact re-solve).
	const slider = page.getByLabel('Demand delta at selected bus');
	await slider.evaluate((el) => {
		const input = el as HTMLInputElement;
		input.value = input.max;
		input.dispatchEvent(new Event('input', { bubbles: true }));
	});

	await expect(page.getByText('First-order preview. Release to solve.')).toBeVisible();
	// case14 is uncongested, so the LMP/demand column is flat and the preview legend
	// shows the uniform predicted shift. The value must scale with the step — the
	// regression this guards is the step cancelling out of the preview display.
	const labels = page.locator('.sensitivity-readout .legend-labels');
	await expect(labels).toContainText(/Uniform [+−-][\d.]+e[+-]\d+\s+objective units\/MW/);
	const atFull = parseShift(await labels.innerText());

	await slider.evaluate((el) => {
		const input = el as HTMLInputElement;
		input.value = String(Number(input.max) / 2);
		input.dispatchEvent(new Event('input', { bubbles: true }));
	});
	await expect.poll(async () => parseShift(await labels.innerText()) / atFull).toBeGreaterThan(0.4);
	expect(parseShift(await labels.innerText()) / atFull).toBeLessThan(0.6);
});

test('arrow keys commit a demand step on a bus under 50 MW', async ({ page }) => {
	await page.route('**/api/cases', (route) => {
		void route.fulfill({ json: [] });
	});

	await page.goto('/');
	await expect(page.getByText('no default cases loaded')).toBeVisible();
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
	await expect(page.locator('.solvecard')).toContainText('DC OPF', { timeout: 60_000 });

	// Bus 11 carries 3.5 MW, so its nearby range spans 8 MW. The native 1%
	// arrow step of 0.08 MW stayed inside the 0.25 MW dead zone and committed
	// nothing.
	const lookup = page.locator('#bus-lookup-input');
	await lookup.fill('11');
	await lookup.press('Enter');
	await expect(page.locator('.chip', { hasText: '∂LMP/∂d' })).toBeVisible({ timeout: 30_000 });
	const slider = page.getByLabel('Demand delta at selected bus');
	await slider.press('ArrowRight');
	await expect(page.locator('.slider-head .val')).toHaveText('+0.5 MW');
	await expect(page.getByRole('button', { name: 'Reset demand' })).toBeVisible({
		timeout: 30_000
	});
	await expect(page.getByRole('group', { name: 'Demand range' })).toBeVisible();
	// The group names the range; each button's name is its visible text (WCAG 2.5.3).
	await expect(
		page.getByRole('group', { name: 'Demand range' }).getByRole('button', { name: 'Full range' })
	).toHaveText('Full range');
});

test('network and solver panel text is at least 12 px', async ({ page }) => {
	await page.route('**/api/cases', (route) => {
		void route.fulfill({ json: [] });
	});

	await page.goto('/');
	await expect(page.getByText('no default cases loaded')).toBeVisible();
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
	await expect(page.locator('.solvecard')).toContainText('DC OPF', { timeout: 60_000 });
	const lookup = page.locator('#bus-lookup-input');
	await lookup.fill('11');
	await lookup.press('Enter');
	await expect(page.locator('.chip', { hasText: '∂LMP/∂d' })).toBeVisible({ timeout: 30_000 });

	for (const id of ['network', 'solver']) {
		const smallest = await page.locator(`[data-panel="${id}"]`).evaluate((panel) => {
			const sizes: Array<[number, string]> = [];
			const walker = document.createTreeWalker(panel, NodeFilter.SHOW_TEXT);
			for (let node = walker.nextNode(); node; node = walker.nextNode()) {
				const element = node.parentElement;
				if (!node.textContent?.trim() || !element?.checkVisibility()) continue;
				sizes.push([parseFloat(getComputedStyle(element).fontSize), node.textContent.trim()]);
			}
			return sizes.sort((a, b) => a[0] - b[0])[0];
		});
		expect(smallest[0], `${id} panel text "${smallest[1]}"`).toBeGreaterThanOrEqual(12);
	}
});

function parseShift(text: string): number {
	const m = text.replace('−', '-').match(/(-?[\d.]+e[+-]\d+)/);
	if (!m) throw new Error(`no shift value in ${JSON.stringify(text)}`);
	return Number(m[1]);
}
