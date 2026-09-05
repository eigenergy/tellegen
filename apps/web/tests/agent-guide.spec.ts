import { expect, test } from './fixtures/page-errors.js';
import { congestCase, installWebMcpHarness, callTool } from './fixtures/planning-case.js';

test('first-visit introduction dismisses persistently and agent help stays available', async ({
	page
}) => {
	await installWebMcpHarness(page);
	await page.route('**/api/cases', (route) => route.fulfill({ json: [] }));
	await page.goto('/');
	await expect(page.getByRole('complementary', { name: 'New to Tellegen' })).toBeVisible();
	await page.getByRole('button', { name: 'Dismiss introduction' }).click();
	await page.reload();
	await expect(page.getByRole('complementary', { name: 'New to Tellegen' })).toHaveCount(0);
	await page.getByRole('button', { name: 'Agent', exact: true }).click();
	await expect(page.getByRole('region', { name: 'Use Tellegen with an agent' })).toBeVisible();
	await expect(page.getByLabel('Agent prompt')).toHaveValue(
		/Use the WebMCP tools in this Tellegen tab/
	);
	await expect(page.getByText('WebMCP is ready in this tab.', { exact: false })).toBeVisible();
	await page.getByRole('link', { name: 'Changelog', exact: true }).click();
	await expect(page.getByRole('heading', { name: "What's new in Tellegen" })).toBeVisible();
	await page.getByRole('link', { name: 'Back to Tellegen' }).click();
	await expect(page.getByRole('complementary', { name: 'New to Tellegen' })).toHaveCount(0);
});

test('Studies, footer and map controls have separate space on desktop and phone', async ({
	page
}) => {
	await installWebMcpHarness(page);
	await congestCase(page);
	await expect(page.getByRole('complementary', { name: 'WebMCP activity' })).toBeVisible();
	for (let i = 0; i < 12; i++) await callTool(page, 'inspect_case', {});
	for (const viewport of [
		{ width: 1380, height: 1042 },
		{ width: 390, height: 844 }
	]) {
		await page.setViewportSize(viewport);
		const studies = await page.locator('.study-workspace').boundingBox();
		const footer = await page.locator('footer').boundingBox();
		expect(studies).not.toBeNull();
		expect(footer).not.toBeNull();
		expect(studies!.y + studies!.height).toBeLessThan(footer!.y - 6);
		const attribution = page.locator('.maplibregl-ctrl-attrib');
		await expect(attribution).toBeVisible();
		const zoom = page.getByRole('button', { name: 'Zoom in', exact: true });
		if (await zoom.isVisible()) {
			const a = (await attribution.boundingBox())!,
				z = (await zoom.boundingBox())!;
			expect(a.y + a.height).toBeLessThan(z.y - 6);
			expect(viewport.height - z.y).toBeLessThan(90);
			await zoom.click();
			const panel = (await page.locator('.activity-panel').boundingBox())!;
			expect(panel.y + panel.height).toBeLessThan(a.y);
			const heading = (await page.locator('.activity-panel h2').boundingBox())!;
			const exportButton = (await page.getByTestId('export-experiment-journal').boundingBox())!;
			expect(
				Math.abs(heading.y + heading.height / 2 - exportButton.y - exportButton.height / 2)
			).toBeLessThan(2);
		}
	}
	await page.getByRole('button', { name: /^Studies/ }).click();
	const attribution = page.locator('.maplibregl-ctrl-attrib');
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
});
