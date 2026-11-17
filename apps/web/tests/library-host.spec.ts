import { expect, test } from './fixtures/page-errors.js';
const port = Number(
	process.env.TELLEGEN_MC_PREVIEW_PORT ?? Number(process.env.TELLEGEN_PREVIEW_PORT ?? 4173) + 1
);
test('library styles and themes stay within each provider', async ({ page }) => {
	const requests: string[] = [];
	page.on('request', (request) => {
		if (request.url().includes('/api/')) requests.push(request.url());
	});
	await page.goto(`http://127.0.0.1:${port}/library-host.html`);
	await expect(page.locator('.tellegen')).toHaveCount(2);
	await expect(page.locator('html')).not.toHaveAttribute('data-theme');
	const colors = await page.evaluate(() => {
		const color = (id: string) =>
			getComputedStyle(document.querySelector(`[data-testid="${id}"]`)!).color;
		return {
			host: color('host-link'),
			light: color('light-link'),
			dark: color('dark-link'),
			margin: getComputedStyle(document.body).margin
		};
	});
	expect(colors.host).toBe('rgb(0, 0, 238)');
	expect(colors.light).toBe('rgb(39, 71, 184)');
	expect(colors.dark).toBe('rgb(143, 166, 255)');
	expect(colors.margin).toBe('8px');
	expect(requests).toEqual([]);
});

test('a custom app loads and solves distribution cases without the demo shell', async ({
	page
}) => {
	await page.goto(`http://127.0.0.1:${port}/custom-app.html`);
	await expect(page.getByRole('heading', { name: 'Distribution case explorer' })).toBeVisible();
	await page
		.getByLabel('Open a case')
		.setInputFiles('tests/fixtures/mc-pf-browser/public/mc-pf-three-phase.pio.json');
	await expect(page.getByRole('button', { name: 'Run power flow' })).toBeVisible();
	await page.getByRole('button', { name: 'Run power flow' }).click();
	await expect(page.getByText('Converged', { exact: true })).toBeVisible();
	await expect(page.getByRole('button', { name: 'Studies', exact: true })).toHaveCount(0);
});
