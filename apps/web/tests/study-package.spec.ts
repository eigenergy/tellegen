import { readFileSync } from 'node:fs';
import { expect, test } from '@playwright/test';
import { CASE14 } from '../../../examples/browser-minimal/src/case14';
import { CASE14_COORDS } from './fixtures/local-case';

// A saved study is a `.pio.json` package. Its `.json` extension also matches the
// geographic-file path, so this guards that a dropped package is restored as a
// study rather than misread as coordinate data.
test('a saved study package downloads and restores when dropped back in', async ({ page }) => {
	await page.route('**/api/cases', (route) => {
		void route.fulfill({ json: [] });
	});

	await page.goto('/');
	// The prerendered input exists before hydration attaches its listener; the
	// empty-cases panel renders only after load.
	await expect(page.getByText('no default cases loaded')).toBeVisible();

	// Coordinates place the case so it solves (the demo path); without them a case
	// stays in synthetic-placement mode.
	await page.locator('input[type="file"]').setInputFiles([
		{ name: 'case14-coords.csv', mimeType: 'text/csv', buffer: Buffer.from(CASE14_COORDS) },
		{ name: 'case14synthetic.m', mimeType: 'text/plain', buffer: Buffer.from(CASE14) }
	]);
	await expect(page.locator('.solvecard')).toContainText('OPF solve', { timeout: 60_000 });

	// Save the study; the case never leaves the browser, so this is a local download.
	const downloadPromise = page.waitForEvent('download');
	await page.getByRole('button', { name: /save study/i }).click();
	const download = await downloadPromise;
	expect(download.suggestedFilename()).toMatch(/\.pio\.json$/);
	const text = readFileSync(await download.path(), 'utf8');
	// Guards against an empty or aborted blob: a real powerio package envelope.
	expect(text).toContain('powerio.dev/schema/pio-package');

	// Drop the saved package back in. Before the fix this fell into the geo-file
	// path and failed with "no bus coordinates or branch paths found"; now it
	// restores as a coordinate-less local case that asks to be placed.
	await page
		.locator('input[type="file"]')
		.setInputFiles([
			{ name: 'restored.pio.json', mimeType: 'application/json', buffer: Buffer.from(text) }
		]);
	await expect(page.getByText('click the map to place the synthetic topology')).toBeVisible({
		timeout: 30_000
	});
	await expect(page.locator('p.error')).toHaveCount(0);
});
