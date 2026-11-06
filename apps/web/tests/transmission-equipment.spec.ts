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
			await expect(page.getByText('no default cases loaded')).toBeVisible();
			await page.locator('input[type="file"]').setInputFiles([fixture]);
			await expect(page.getByText('no default cases loaded')).toBeHidden();
			if (width < 600) {
				const networkTab = page.getByRole('button', { name: 'Network', exact: true });
				if ((await networkTab.getAttribute('aria-pressed')) !== 'true') await networkTab.click();
			}
			await page.getByRole('button', { name: 'Show diagram', exact: true }).click();
			if (width < 600) await page.getByRole('button', { name: 'Close network panel' }).click();
			const toolbar = page.getByRole('toolbar', { name: 'Single-line diagram' });
			await toolbar.getByRole('button', { name: 'Equipment', exact: true }).click();
			const panel = page.getByRole('region', { name: 'Equipment models' });
			await expect(panel).toBeVisible();
			await panel.getByRole('button', { name: /PST · branch 1/ }).click();
			await expect(panel.getByRole('article')).toContainText('Phase setting 0°');
			await expect(panel.getByRole('article')).toContainText('does not adjust');
			await panel.getByRole('button', { name: /Capacitor bank 1/ }).click();
			await expect(panel.getByRole('article')).toContainText('2 sections connected');
			await expect(panel.getByRole('article')).toContainText('voltage squared');
			await panel.getByLabel('Find equipment').fill('HVDC');
			await panel.getByRole('button', { name: /HVDC link 1/ }).click();
			await expect(panel.getByRole('article')).toContainText('Net input loss: 0.5 MW');
			await expect(panel.getByRole('article')).toContainText('not an HVDC-network simulation');
			const bounds = await panel.boundingBox();
			expect(bounds!.x).toBeGreaterThanOrEqual(0);
			expect(bounds!.x + bounds!.width).toBeLessThanOrEqual(width);
			await page.screenshot({ path: testInfo.outputPath('equipment-models.png') });
			await panel.getByRole('button', { name: 'Close equipment models' }).click();
			if (width < 600) await toolbar.getByRole('button', { name: 'Display', exact: true }).click();
			const download = page.waitForEvent('download');
			await toolbar.getByRole('button', { name: 'PNG', exact: true }).click();
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
			await expect(page.getByText('no default cases loaded')).toBeVisible();
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
			await expect(page.getByText('no default cases loaded')).toBeHidden();
			if (width < 600) {
				const tab = page.getByRole('button', { name: 'Network', exact: true });
				if ((await tab.getAttribute('aria-pressed')) !== 'true') await tab.click();
			}
			await page.getByRole('button', { name: 'Show diagram', exact: true }).click();
			if (width < 600) await page.getByRole('button', { name: 'Close network panel' }).click();
			await page
				.getByRole('toolbar', { name: 'Single-line diagram' })
				.getByRole('button', { name: 'Equipment', exact: true })
				.click();
			const panel = page.getByRole('region', { name: 'Equipment models' });
			await panel.getByText('Detailed DC network · 4 nodes', { exact: true }).click();
			await expect(
				panel.getByRole('img', { name: 'Detailed DC nodes, branches and converters' })
			).toBeVisible();
			await panel.getByRole('button', { name: 'Solve DC network', exact: true }).click();
			await expect(panel.getByRole('region', { name: 'DC network results' })).toContainText(
				'0 limit violations'
			);
			await expect(panel.getByRole('region', { name: 'DC network results' })).toContainText(
				'a: 320 kV'
			);
			await panel.getByRole('button', { name: 'Solve with AC power flow', exact: true }).click();
			await expect(panel.getByRole('region', { name: 'AC/DC study results' })).toContainText(
				'AC ACPF calculation completed'
			);
			await expect(panel.getByRole('region', { name: 'AC/DC study results' })).toContainText(
				'AC bus 2:'
			);
			await panel.getByRole('button', { name: 'Dispatch AC generation', exact: true }).click();
			await expect(panel.getByRole('region', { name: 'AC/DC study results' })).toContainText(
				'AC DCOPF calculation completed'
			);
			await expect(panel.getByRole('region', { name: 'AC/DC study results' })).toContainText(
				'AC generation objective:'
			);
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
