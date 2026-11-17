import { expect, test } from '@playwright/test';
const port = Number(process.env.TELLEGEN_MC_PREVIEW_PORT ?? 4174);
test('generated areas and planned HVDC transfers reach the viewer and linked selection', async ({
	page
}, testInfo) => {
	test.setTimeout(120_000);
	await page.goto(`http://127.0.0.1:${port}/synchronous-areas.html`);
	await page.getByRole('button', { name: 'Generate USA fixture' }).click();
	await expect(page.getByTestId('generation-status')).toHaveText('Generated', { timeout: 90_000 });
	const panel = page.getByRole('region', { name: 'Synchronous areas', exact: true });
	await expect(panel).toBeVisible();
	for (const name of ['Western', 'Eastern', 'Texas']) {
		await expect(
			panel.getByRole('button', { name: `Inspect a bus in USA ${name}`, exact: true })
		).toBeVisible();
	}
	await expect(panel.getByText('planned transfer', { exact: false })).toHaveCount(3);
	const endpoints = panel.getByRole('button', { name: /^Inspect bus / });
	await expect(endpoints).toHaveCount(4);
	const label = await endpoints.first().innerText();
	await endpoints.first().click();
	await expect(page.getByTestId('selected-bus')).toHaveText(
		`Selected bus: ${label.replace('Inspect bus ', '')}`
	);
	await expect(panel.getByText('Selected bus’s area')).toBeVisible();
	await panel.getByRole('button', { name: 'Inspect a bus in USA Texas', exact: true }).click();
	await expect(panel.locator('li.selected')).toContainText('Texas');
	await page.screenshot({ path: testInfo.outputPath('synchronous-areas.png'), fullPage: true });
});
