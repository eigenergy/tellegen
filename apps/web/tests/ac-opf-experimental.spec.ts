import { expect, test } from './fixtures/page-errors.js';
import type { Route } from '@playwright/test';
import { CASE3_COORDS, CASE3_PLANNING } from './fixtures/planning-case.js';

test.skip(!process.env.TELLEGEN_TEST_ACOPF, 'requires the explicit development AC OPF asset');

test('the capability-gated selector runs AC OPF in the isolated WASI worker', async ({ page }) => {
	test.setTimeout(180_000);
	const workers: { closed: boolean }[] = [];
	page.on('worker', (worker) => {
		if (!worker.url().includes('acopf-worker')) return;
		const state = { closed: false };
		workers.push(state);
		worker.on('close', () => {
			state.closed = true;
		});
	});
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
	await expect(
		page.getByRole('button', { name: 'save PowerIO module (.json)', exact: true })
	).toBeDisabled();
	await expect(
		page.getByRole('button', { name: 'export committed state…', exact: true })
	).toBeDisabled();
	await expect(
		page.getByText('Saving and exporting AC OPF cases is not supported yet.', { exact: true })
	).toBeVisible();
	await expect.poll(() => workers.length).toBe(2); // probe plus first solve
	await expect.poll(() => workers.every((worker) => worker.closed)).toBe(true);

	// A second solve must use and dispose a new worker, not retain its WASI memory.
	await calculation.selectOption('dcopf');
	await expect(calculation).toBeEnabled();
	await calculation.selectOption('acopf');
	await expect(page.locator('.solvecard')).toContainText('NLP iterations', { timeout: 120_000 });
	await expect.poll(() => workers.length).toBe(3);
	await expect.poll(() => workers.every((worker) => worker.closed)).toBe(true);

	// Hold the next real worker's fetch so removal deterministically happens
	// while it is pending, without relying on how fast a three-bus solve runs.
	await calculation.selectOption('dcopf');
	await expect(calculation).toBeEnabled();
	let held: Route | undefined;
	await page.context().route('**/experimental-acopf/tellegen_acopf_wasi.wasm', (route) => {
		held = route;
	});
	await calculation.selectOption('acopf');
	await expect.poll(() => !!held).toBe(true);
	await expect.poll(() => workers.length).toBe(4);
	await page.locator('.case-chip.local .case-remove').click();
	await expect.poll(() => workers.every((worker) => worker.closed)).toBe(true);
	await held!.abort();
	await expect(page.locator('.solvecard')).toHaveCount(0);
	await expect(page.locator('.error')).toHaveCount(0);
});

test('imports and solves a retained canonical AC OPF instance', async ({ page }) => {
	test.setTimeout(180_000);
	const port = Number(
		process.env.TELLEGEN_MC_PREVIEW_PORT ?? Number(process.env.TELLEGEN_PREVIEW_PORT ?? 4173) + 1
	);
	await page.goto(`http://127.0.0.1:${port}/acopf.html`);
	await page.waitForFunction(() => typeof window.prepareAcOpfInstance === 'function');
	const moduleJson = await page.evaluate(
		(text) => window.prepareAcOpfInstance(text),
		CASE3_PLANNING
	);
	await page.route('**/api/compute', (route) => route.fulfill({ json: { enabled: false } }));
	await page.route('**/api/cases', (route) => route.fulfill({ json: [] }));
	await page.goto('/');
	await expect(page.getByText('no default cases loaded')).toBeVisible();
	await page.locator('input[type="file"][multiple]').setInputFiles([
		{ name: 'canonical.pio.json', mimeType: 'application/json', buffer: Buffer.from(moduleJson) },
		{ name: 'case3-coords.csv', mimeType: 'text/csv', buffer: Buffer.from(CASE3_COORDS) }
	]);
	const calculation = page.getByRole('combobox', { name: 'Calculation', exact: true });
	await expect(calculation).toHaveValue('acopf');
	await expect(page.locator('.solvecard')).toContainText('NLP iterations', { timeout: 120_000 });
	await expect(calculation.locator('option[value="dcopf"]')).toHaveJSProperty('disabled', true);
	await expect(page.locator('.error')).toHaveCount(0);
});
