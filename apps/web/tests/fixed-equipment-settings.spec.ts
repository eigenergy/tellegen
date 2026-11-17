import { expect, test } from './fixtures/page-errors.js';

const source = `new circuit.test basekv=11
new transformer.t phases=3 windings=2 buses=[sourcebus load] conns=[wye wye] kvs=[11 .4] kvas=[100 100] taps=[1 1.025] xhl=4
new load.l bus1=load phases=3 kv=.4 kw=10 kvar=2
new regcontrol.r transformer=t winding=2 vreg=120 band=2 ptratio=1
`;

test('written equipment settings create an independent solvable case and survive saving', async ({
	page
}) => {
	await page.route('**/api/compute', (route) => route.fulfill({ json: { enabled: false } }));
	await page.route('**/api/cases', (route) => route.fulfill({ json: [] }));
	await page.goto('/');
	await expect(page.getByText('No cases', { exact: true })).toBeVisible();
	await page
		.locator('header input[type="file"][accept*=".dss"]')
		.setInputFiles({ name: 'controls.dss', mimeType: 'text/plain', buffer: Buffer.from(source) });
	const network = page.getByRole('complementary', { name: 'Network', exact: true });
	await expect(network.getByRole('button', { name: 'Run power flow', exact: true })).toBeDisabled();
	await network
		.getByRole('button', { name: 'Use written equipment settings', exact: true })
		.click();
	await expect(network.getByRole('heading', { name: /fixed settings/ })).toBeVisible();
	await expect(network).toContainText('Using written equipment settings');
	await expect(network.getByRole('button', { name: 'Run power flow', exact: true })).toBeEnabled();
	await network.getByRole('button', { name: 'Run power flow', exact: true }).click();
	const studies = page.getByRole('complementary', { name: 'Studies', exact: true });
	await expect(studies.getByRole('button', { name: 'Save result', exact: true })).toBeEnabled();
	await studies.getByRole('button', { name: 'Save result', exact: true }).click();
	const saved = page.getByLabel('Saved study');
	await expect(saved.locator('option[value^="mc:"]')).toHaveCount(1);
	const savedId = await saved.locator('option[value^="mc:"]').getAttribute('value');
	await page.reload();
	await page.getByRole('button', { name: 'Studies', exact: true }).click();
	await page.getByLabel('Saved study').selectOption(savedId!);
	await page.getByRole('button', { name: 'Network', exact: true }).click();
	await expect(network).toContainText('Using written equipment settings');
	await expect(
		network.getByRole('button', { name: 'Use written equipment settings', exact: true })
	).toHaveCount(0);
});
