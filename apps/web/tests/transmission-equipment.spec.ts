import { expect, test } from './fixtures/page-errors.js';
import { fileURLToPath } from 'node:url';
const fixture = fileURLToPath(new URL('./fixtures/transmission-equipment.json', import.meta.url));
for (const width of [1280, 390, 320]) {
	test.describe(`equipment at ${width}px`, () => {
		test.use({ viewport: { width, height: 844 }, hasTouch: width < 600 });
		test('exposes source settings and fixed-transfer limitations', async ({ page }, testInfo) => {
			test.setTimeout(120000);
			await page.route('**/api/cases', (route) => route.fulfill({ json: [] }));
			await page.route('**/api/compute', (route) => route.fulfill({ json: { enabled: false } }));
			await page.goto('/');
			await page.emulateMedia({ reducedMotion: 'reduce' });
			await expect(page.getByText('No cases', { exact: true })).toBeVisible();
			await page.locator('input[type="file"]').setInputFiles([fixture]);
			await expect(page.getByText('No cases', { exact: true })).toBeHidden();
			if (width < 600) {
				const networkTab = page.getByRole('button', { name: 'Network', exact: true });
				if ((await networkTab.getAttribute('aria-pressed')) !== 'true') await networkTab.click();
			}
			await page.getByRole('button', { name: 'Show diagram', exact: true }).click();
			const toolbar = page.getByRole('toolbar', { name: 'Single-line diagram' });
			await toolbar.getByRole('button', { name: 'Equipment', exact: true }).click();
			const panel = page.getByRole('region', { name: 'Equipment models' });
			await expect(panel).toBeVisible();
			await panel.getByRole('button', { name: /PST on branch 1/ }).click();
			await expect(panel.getByRole('article')).toContainText('Phase: 0°');
			await expect(panel.getByRole('article')).toContainText('Model: Fixed tap');
			await panel.getByRole('button', { name: /Capacitor bank 1/ }).click();
			await expect(panel.getByRole('article')).toContainText('Sections on: 2');
			await expect(panel.getByRole('article')).toContainText('Model: Conductance only (DC)');
			await panel.getByLabel('Find equipment').fill('HVDC');
			await panel.getByRole('button', { name: /HVDC link 1/ }).click();
			await expect(panel.getByRole('article')).toContainText('Loss: 0.5 MW');
			await expect(panel.getByRole('article')).toContainText('Model: Fixed injection');
			const bounds = await panel.boundingBox();
			expect(bounds!.x).toBeGreaterThanOrEqual(0);
			expect(bounds!.x + bounds!.width).toBeLessThanOrEqual(width);
			await page.screenshot({ path: testInfo.outputPath('equipment-models.png') });
			await panel
				.locator('summary')
				.filter({ hasText: /^Regulators$/ })
				.click();
			await panel.getByRole('button', { name: 'Solve with regulators', exact: true }).click();
			await expect(panel.locator('.stat').filter({ hasText: 'Passes' })).toContainText(/\d+/);
			await expect(panel.getByRole('region', { name: 'Regulator results' })).toContainText(
				'transformer:1'
			);
			await panel.getByRole('region', { name: 'Regulator results' }).scrollIntoViewIfNeeded();
			await page.screenshot({ path: testInfo.outputPath('automatic-regulators.png') });
			await panel.getByRole('button', { name: 'Close equipment models' }).click();
			if (width < 600) await toolbar.getByRole('button', { name: 'Display', exact: true }).click();
			const download = page.waitForEvent('download');
			await toolbar.getByRole('button', { name: 'Save image', exact: true }).click();
			const exported = await download;
			expect(exported.suggestedFilename()).toMatch(/\.png$/);
			await exported.saveAs(testInfo.outputPath('equipment-export.png'));
			expect(await page.evaluate(() => window.scrollY)).toBe(0);
		});
	});
}

for (const width of [1280, 390, 320]) {
	test.describe(`detailed DC at ${width}px`, () => {
		test.use({ viewport: { width, height: 844 }, hasTouch: width < 600 });
		test('solves a real three-terminal DC graph and qualified AC coupling', async ({
			page
		}, testInfo) => {
			await page.route('**/api/cases', (route) => route.fulfill({ json: [] }));
			await page.route('**/api/compute', (route) => route.fulfill({ json: { enabled: false } }));
			await page.goto('/');
			await page.emulateMedia({ reducedMotion: 'reduce' });
			await expect(page.getByText('No cases', { exact: true })).toBeVisible();
			await page
				.locator('input[type="file"]')
				.setInputFiles([
					fileURLToPath(
						new URL(
							'../../../crates/tellegen/tests/data/transmission-mtdc.pio.json',
							import.meta.url
						)
					)
				]);
			await expect(page.getByText('No cases', { exact: true })).toBeHidden();
			if (width < 600) {
				const tab = page.getByRole('button', { name: 'Network', exact: true });
				if ((await tab.getAttribute('aria-pressed')) !== 'true') await tab.click();
			}
			await page.getByRole('button', { name: 'Show diagram', exact: true }).click();
			await page
				.getByRole('toolbar', { name: 'Single-line diagram' })
				.getByRole('button', { name: 'Equipment', exact: true })
				.click();
			const panel = page.getByRole('region', { name: 'Equipment models' });
			await panel.getByText('DC network (4 nodes)', { exact: true }).click();
			await expect(
				panel.getByRole('img', { name: 'Detailed DC nodes, branches and converters' })
			).toBeVisible();
			await panel.getByRole('button', { name: 'Solve DC network', exact: true }).click();
			await expect(
				panel
					.getByRole('region', { name: 'DC network results' })
					.locator('.stat')
					.filter({ hasText: 'Violations' })
			).toContainText('0');
			await expect(panel.getByRole('region', { name: 'DC network results' })).toContainText(
				'a: 320 kV'
			);
			await panel.getByRole('button', { name: 'AC power flow', exact: true }).click();
			await expect(
				panel.getByRole('region', { name: 'AC/DC study results' }).locator('.stat').first()
			).toContainText('Violations');
			await expect(panel.getByRole('region', { name: 'AC/DC study results' })).toContainText(
				'AC bus 2:'
			);
			await panel.getByRole('button', { name: 'DC OPF', exact: true }).click();
			await expect(
				panel
					.getByRole('region', { name: 'AC/DC study results' })
					.locator('.stat')
					.filter({ hasText: 'Cost' })
			).toBeVisible();
			await panel
				.getByRole('img', { name: 'Detailed DC nodes, branches and converters' })
				.scrollIntoViewIfNeeded();
			// Keep the SVG clear of the sticky close/header row in the evidence capture.
			await page.locator('.equipment-overlay').evaluate((el) => {
				el.scrollTop = Math.max(0, el.scrollTop - 60);
			});
			const bounds = await panel.boundingBox();
			expect(bounds!.x + bounds!.width).toBeLessThanOrEqual(width);
			expect(await page.evaluate(() => window.scrollY)).toBe(0);
			await page.screenshot({ path: testInfo.outputPath('mtdc-topology.png') });
		});
	});
}

for (const width of [1280, 390, 320]) {
	test.describe(`joint AC/DC at ${width}px`, () => {
		test.use({ viewport: { width, height: 844 }, hasTouch: width < 600 });
		test('regulates AC voltage and optimizes joint dispatch in the WASI worker', async ({
			page
		}, testInfo) => {
			test.setTimeout(120000);
			await page.route('**/api/cases', (route) => route.fulfill({ json: [] }));
			await page.route('**/api/compute', (route) => route.fulfill({ json: { enabled: false } }));
			await page.goto('/');
			await page.emulateMedia({ reducedMotion: 'reduce' });
			await expect(page.getByText('No cases', { exact: true })).toBeVisible();
			await page
				.locator('input[type="file"]')
				.setInputFiles(
					fileURLToPath(new URL('./fixtures/transmission-acdc-controls.json', import.meta.url))
				);
			await expect(page.getByText('No cases', { exact: true })).toBeHidden();
			if (width < 600) {
				const tab = page.getByRole('button', { name: 'Network', exact: true });
				if ((await tab.getAttribute('aria-pressed')) !== 'true') await tab.click();
			}
			await page.getByRole('button', { name: 'Show diagram', exact: true }).click();
			await page
				.getByRole('toolbar', { name: 'Single-line diagram' })
				.getByRole('button', { name: 'Equipment', exact: true })
				.click();
			const panel = page.getByRole('region', { name: 'Equipment models' });
			await panel.getByText('DC network (4 nodes)', { exact: true }).click();
			await panel.getByRole('button', { name: 'AC power flow', exact: true }).click();
			await expect(panel.getByRole('region', { name: 'Regulator results' })).toContainText(
				'regulated'
			);
			await expect(panel.getByRole('region', { name: 'AC/DC study results' })).toContainText(
				'AC bus 2: 1.02 pu'
			);
			await panel.getByRole('region', { name: 'Regulator results' }).scrollIntoViewIfNeeded();
			await page.screenshot({ path: testInfo.outputPath('converter-voltage.png') });
			const joint = panel.getByRole('button', {
				name: 'Joint AC/DC OPF',
				exact: true
			});
			await expect(joint).toBeEnabled({ timeout: 30000 });
			await joint.click();
			const result = panel.getByRole('region', { name: 'AC/DC study results' });
			await expect(result.locator('.stat').filter({ hasText: 'Cost' })).toBeVisible({
				timeout: 60000
			});
			await expect(result.locator('.stat').filter({ hasText: 'Violations' })).toContainText('0');
			await expect(result).toContainText('AC bus 2: 1.02 pu');
			const dcResult = panel.getByRole('region', { name: 'DC network results' });
			await expect(dcResult.locator('.stat').filter({ hasText: 'Bounds checked' })).toContainText(
				'12'
			);
			await expect(dcResult.locator('.stat').filter({ hasText: 'Unspecified' })).toContainText('6');
			await result.scrollIntoViewIfNeeded();
			await page.screenshot({ path: testInfo.outputPath('joint-acdc.png') });
			expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBeLessThanOrEqual(
				width
			);
			expect(await page.evaluate(() => window.scrollY)).toBe(0);
		});
	});
}
