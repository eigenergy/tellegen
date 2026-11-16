import { expect, test } from './fixtures/page-errors.js';
import { congestCase, installWebMcpHarness, callTool } from './fixtures/planning-case.js';

test('first-visit introduction dismisses persistently and agent help stays available', async ({
	page
}) => {
	await installWebMcpHarness(page);
	await page.route('**/api/cases', (route) => route.fulfill({ json: [] }));
	await page.goto('/');
	await expect(page.getByRole('complementary', { name: 'About AI agents' })).toBeVisible();
	await page.getByRole('button', { name: 'Dismiss introduction' }).click();
	await page.reload();
	await expect(page.getByRole('complementary', { name: 'About AI agents' })).toHaveCount(0);
	await page.getByRole('button', { name: 'Agent', exact: true }).click();
	await expect(page.getByRole('region', { name: 'Use Tellegen with an agent' })).toBeVisible();
	await expect(page.getByRole('button', { name: 'Copy prompt', exact: true })).toBeVisible();
	const status = page.getByTestId('webmcp-status');
	await expect(status).toContainText('Ready');
	await expect(status.getByRole('definition').first()).toHaveText(/^[1-9]\d*$/);
	await page.reload();
	await expect(page.getByRole('complementary', { name: 'About AI agents' })).toHaveCount(0);
});

test('default docks keep Studies, Agent, solver and map controls separate', async ({
	page
}, testInfo) => {
	await installWebMcpHarness(page);
	await congestCase(page);
	for (let i = 0; i < 12; i++) await callTool(page, 'inspect_case', {});
	await page.getByRole('button', { name: 'Agent', exact: true }).click();
	await page.getByRole('button', { name: 'Studies', exact: true }).click();
	for (const viewport of [
		{ width: 1380, height: 1042 },
		{ width: 390, height: 844 }
	]) {
		await page.setViewportSize(viewport);
		if (viewport.width < 960)
			await page.getByRole('button', { name: 'Agent', exact: true }).click();
		const panels = await page
			.locator('[data-panel]:visible')
			.evaluateAll((elements) =>
				elements.map((element) => element.getBoundingClientRect().toJSON())
			);
		for (let i = 0; i < panels.length; i++)
			for (let j = i + 1; j < panels.length; j++) {
				const a = panels[i],
					b = panels[j];
				expect(
					a.right <= b.left || b.right <= a.left || a.bottom <= b.top || b.bottom <= a.top
				).toBe(true);
			}
		const attribution = page.locator('.maplibregl-ctrl-attrib');
		await expect(attribution).toBeVisible();
		await expect
			.poll(() =>
				attribution.evaluate((element) => {
					const box = element.getBoundingClientRect();
					return element.contains(
						document.elementFromPoint(box.x + box.width / 2, box.y + box.height / 2)
					);
				})
			)
			.toBe(true);
		const zoom = page.getByRole('button', { name: 'Zoom in', exact: true });
		await expect(zoom).toBeVisible();
		{
			const a = (await attribution.boundingBox())!,
				z = (await zoom.boundingBox())!;
			expect(a.x + a.width).toBeLessThan(z.x - 6);
			if (viewport.width < 960) {
				// With the sheet open, the zoom sits just above it rather than under it.
				const sheet = (await page.locator('[data-panel-drawer]').boundingBox())!;
				expect(z.y + z.height).toBeLessThanOrEqual(sheet.y);
				expect(sheet.y + sheet.height).toBeGreaterThan(viewport.height - 2);
			} else expect(viewport.height - z.y).toBeLessThan(90);
			await zoom.click();
		}
		await page.screenshot({
			path: testInfo.outputPath(`panels-${viewport.width}x${viewport.height}.png`)
		});
		// One panel shows at a time; the front one (Studies) clears the attribution: above it on a
		// wide screen, below it on a phone, where the sheet reaches the bottom edge.
		const panel = (await page.locator('[data-panel]:visible').first().boundingBox())!;
		const a = (await attribution.boundingBox())!;
		if (viewport.width < 960) expect(a.y + a.height).toBeLessThanOrEqual(panel.y);
		else expect(panel.y + panel.height).toBeLessThan(a.y);
	}
});
