import { readFileSync } from 'node:fs';
import { expect, test } from './fixtures/page-errors.js';
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
	// The markers are the ones powerio's own classifier and lineage gate read —
	// `model_kind` beside `model`, and the version that wrote the document. The
	// `schema` URL field this used to check left `.pio.json` in powerio 0.8.0.
	const saved = JSON.parse(text);
	expect(saved.model_kind).toBe('balanced');
	expect(typeof saved.powerio_version).toBe('string');

	// The applied coordinates live on the network payload, so the layout also
	// downloads as a canonical `.geo.json` layer.
	const layoutPromise = page.waitForEvent('download');
	await page.getByRole('button', { name: /download layout/i }).click();
	const layout = await layoutPromise;
	expect(layout.suggestedFilename()).toMatch(/\.geo\.json$/);
	expect(readFileSync(await layout.path(), 'utf8')).toContain('powerio_geo');

	// Drop the saved package back in. Its `.json` extension also matches the
	// geographic-file path, so this guards the content sniff; and the package
	// payload carries the applied coordinates, so the case restores placed and
	// solves without a placement step.
	await page
		.locator('input[type="file"]')
		.setInputFiles([
			{ name: 'restored.pio.json', mimeType: 'application/json', buffer: Buffer.from(text) }
		]);
	await expect(page.locator('.solvecard')).toContainText('OPF solve', { timeout: 60_000 });
	await expect(page.getByText('click the map to place the topology layout')).toHaveCount(0);
	await expect(page.locator('p.error')).toHaveCount(0);
});
