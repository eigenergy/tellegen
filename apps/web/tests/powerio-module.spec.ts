import { readFileSync } from 'node:fs';
import { expect, test } from './fixtures/page-errors.js';
import { CASE14 } from '../../../examples/browser-minimal/src/case14';
import { CASE14_COORDS } from './fixtures/local-case';

test('a materialized PowerIO module downloads and loads as a fresh case', async ({ page }) => {
	await page.route('**/api/cases', (route) => {
		void route.fulfill({ json: [] });
	});
	await page.goto('/');
	await expect(page.getByText('no default cases loaded')).toBeVisible();

	await page.locator('input[type="file"]').setInputFiles([
		{ name: 'case14-coords.csv', mimeType: 'text/csv', buffer: Buffer.from(CASE14_COORDS) },
		{ name: 'case14synthetic.m', mimeType: 'text/plain', buffer: Buffer.from(CASE14) }
	]);
	await expect(page.locator('.solvecard')).toContainText('OPF solve', { timeout: 60_000 });

	const downloadPromise = page.waitForEvent('download');
	await page.getByRole('button', { name: /save PowerIO module/i }).click();
	const download = await downloadPromise;
	expect(download.suggestedFilename()).toMatch(/\.powerio\.json$/);
	const text = readFileSync(await download.path(), 'utf8');
	const saved = JSON.parse(text) as { schema?: unknown; version?: unknown; value?: unknown };
	expect(saved.schema).toBe('powerio.module');
	expect(saved.version).toBe(1);
	expect(saved.value).toBeTruthy();
	expect(text).not.toContain('tellegen.study');

	const solutionDownloadPromise = page.waitForEvent('download');
	await page.getByRole('button', { name: /save exact solution/i }).click();
	const solutionDownload = await solutionDownloadPromise;
	expect(solutionDownload.suggestedFilename()).toMatch(/\.solution\.powerio\.json$/);
	const solutionText = readFileSync(await solutionDownload.path(), 'utf8');
	const exact = JSON.parse(solutionText) as {
		schema?: unknown;
		version?: unknown;
		value?: { kind?: unknown; data?: { instance?: { network?: unknown } } };
	};
	expect(exact.schema).toBe('powerio.module');
	expect(exact.version).toBe(1);
	expect(exact.value?.kind).toBe('dc_opf_solution');
	expect(exact.value?.data?.instance?.network).toBeTruthy();

	await page
		.locator('input[type="file"]')
		.setInputFiles([
			{ name: 'saved.powerio.json', mimeType: 'application/json', buffer: Buffer.from(text) }
		]);
	await expect(page.locator('.solvecard')).toContainText('OPF solve', { timeout: 60_000 });
	await expect(page.locator('p.error')).toHaveCount(0);
});

test('the retired tellegen.study envelope is rejected', async ({ page }) => {
	await page.route('**/api/cases', (route) => {
		void route.fulfill({ json: [] });
	});
	await page.goto('/');
	await expect(page.getByText('no default cases loaded')).toBeVisible();

	await page.locator('input[type="file"]').setInputFiles([
		{
			name: 'retired.study.json',
			mimeType: 'application/json',
			buffer: Buffer.from(JSON.stringify({ schema: 'tellegen.study', version: 1, module: {} }))
		}
	]);
	await expect(page.locator('p.error')).toBeVisible();
	await expect(page.locator('.solvecard')).toHaveCount(0);
});
