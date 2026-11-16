import { expect, test } from './fixtures/page-errors.js';
import { openCaseMenu } from './fixtures/chrome.js';
import { CASE_SUMMARY, NETWORK, SOLUTION, mockDataRoutes } from './fixtures/backend-case.js';

test('Cmd+K opens the palette and zen mode hides the panels until Escape', async ({ page }) => {
	await page.route('**/api/cases', (route) => route.fulfill({ json: [] }));
	await page.goto('/');
	const dialog = page.getByRole('dialog', { name: 'Command palette' });
	const pill = page.getByRole('button', { name: 'Exit zen mode', exact: true });
	const welcome = page.getByRole('complementary', { name: 'About AI agents' });
	const toolbar = page.locator('.panel-toolbar');
	const zoom = page.getByRole('button', { name: 'Zoom in' });
	const attribution = page.locator('.maplibregl-ctrl-attrib');

	// The introduction note is chrome too: zen hides it and brings it back.
	await expect(welcome).toBeVisible();
	await expect(zoom).toBeVisible();
	await expect(attribution).toHaveClass(/maplibregl-compact-show/);
	const info = attribution.locator('.maplibregl-ctrl-attrib-button');
	await expect(info).toBeVisible();
	const infoBefore = await info.boundingBox();
	expect(infoBefore).not.toBeNull();
	// Toggling the text by hand must not move the button either.
	await info.click();
	await expect(attribution).not.toHaveClass(/maplibregl-compact-show/);
	await expect.poll(async () => await info.boundingBox()).toEqual(infoBefore);
	await info.click();
	await expect(attribution).toHaveClass(/maplibregl-compact-show/);
	await page.keyboard.press('ControlOrMeta+k');
	await dialog.getByRole('combobox').fill('zen');
	await page.keyboard.press('Enter');
	await expect(welcome).toHaveCount(0);
	await expect(toolbar).toHaveCount(0);
	await expect(zoom).toBeHidden();
	await expect(attribution).not.toHaveClass(/maplibregl-compact-show/);
	// The (i) button must not move when the attribution text collapses.
	await expect.poll(async () => await info.boundingBox()).toEqual(infoBefore);
	await page.keyboard.press('Escape');
	await expect(welcome).toBeVisible();
	await expect(toolbar).toBeVisible();
	await expect(zoom).toBeVisible();
	await expect(attribution).toHaveClass(/maplibregl-compact-show/);

	await page.getByRole('button', { name: 'Agent', exact: true }).click();
	const agent = page.locator('[data-panel="agent"]');
	await expect(agent).toBeVisible();

	await page.keyboard.press('ControlOrMeta+k');
	await expect(dialog).toBeVisible();
	const input = dialog.getByRole('combobox');
	await expect(input).toBeFocused();

	await input.fill('zen');
	await expect(dialog.getByRole('option', { name: /Enter zen mode/ })).toBeVisible();
	await page.keyboard.press('Enter');
	await expect(dialog).toHaveCount(0);
	await expect(toolbar).toHaveCount(0);
	await expect(agent).toHaveCount(0);
	await expect(pill).toBeVisible();

	await page.keyboard.press('ControlOrMeta+k');
	await expect(dialog.getByRole('option', { name: /^Exit zen mode/ })).toBeVisible();
	await page.keyboard.press('Escape');
	await expect(dialog).toHaveCount(0);
	await expect(pill).toBeVisible();

	await page.keyboard.press('Escape');
	await expect(pill).toHaveCount(0);
	await expect(toolbar).toBeVisible();
	await expect(agent).toBeVisible();

	await page.keyboard.press('ControlOrMeta+k');
	await expect(dialog).toBeVisible();
	await page.locator('.backdrop').click({ position: { x: 10, y: 10 } });
	await expect(dialog).toHaveCount(0);
});

test('zen mode keeps the case menu, which still lists every case', async ({ page }) => {
	await mockDataRoutes(page);
	await page.route('**/api/cases', (route) =>
		route.fulfill({
			json: [...CASE_SUMMARY, { ...CASE_SUMMARY[0], id: 'case2', name: 'Case Two' }]
		})
	);
	await page.route('**/api/cases/case2/network', (route) =>
		route.fulfill({ json: { ...NETWORK, id: 'case2', name: 'Case Two' } })
	);
	await page.route('**/api/cases/case2/solution', (route) => route.fulfill({ json: SOLUTION }));
	await page.route('**/api/cases/case2/case', (route) => route.fulfill({ status: 500, json: {} }));
	await page.goto('/');
	const button = page.locator('header .case-button');
	await expect(button).toContainText('Case One');

	await page.keyboard.press('ControlOrMeta+k');
	await page.getByRole('dialog', { name: 'Command palette' }).getByRole('combobox').fill('zen');
	await page.keyboard.press('Enter');
	await expect(page.getByRole('button', { name: 'Exit zen mode', exact: true })).toBeVisible();
	await expect(button).toBeVisible();

	const menu = await openCaseMenu(page);
	const list = menu.getByRole('navigation', { name: 'Cases' });
	await expect(list.getByRole('button', { name: /Case One/ }).first()).toBeVisible();
	await expect(list.getByRole('button', { name: /Case Two/ }).first()).toBeVisible();
	await expect(menu.getByRole('button', { name: /^Open a file/ })).toBeVisible();
	await list
		.getByRole('button', { name: /Case Two/ })
		.first()
		.click();
	await expect(menu).toBeHidden();
	await expect(button).toContainText('Case Two');
});

test('the palette can reset the panel layout', async ({ page }) => {
	await page.route('**/api/cases', (route) => route.fulfill({ json: [] }));
	await page.goto('/');
	await page.getByRole('button', { name: 'Agent', exact: true }).click();
	await page.getByRole('button', { name: 'Agent panel options' }).click();
	await page.getByRole('button', { name: 'Dock right', exact: true }).click();
	await expect(page.locator('[data-panel-dock="right"] [data-panel="agent"]')).toBeVisible();
	await page.keyboard.press('ControlOrMeta+k');
	const dialog = page.getByRole('dialog', { name: 'Command palette' });
	await dialog.getByRole('combobox').fill('reset');
	await expect(dialog.getByRole('option', { name: /Reset panel layout/ })).toBeVisible();
	await page.keyboard.press('Enter');
	await expect(dialog).toHaveCount(0);
	await expect(page.locator('[data-panel-dock="left"] [data-panel="agent"]')).toBeVisible();
});

test('zen mode hides the compact drawer and restores it on exit', async ({ page }) => {
	await page.setViewportSize({ width: 390, height: 844 });
	await page.route('**/api/cases', (route) => route.fulfill({ json: [] }));
	await page.goto('/');
	await page.getByRole('button', { name: 'Studies', exact: true }).click();
	await expect(page.locator('[data-panel]:visible')).toHaveCount(1);
	await page.keyboard.press('ControlOrMeta+k');
	const dialog = page.getByRole('dialog', { name: 'Command palette' });
	await dialog.getByRole('combobox').fill('zen');
	await page.keyboard.press('Enter');
	await expect(page.locator('[data-panel]:visible')).toHaveCount(0);
	await page.getByRole('button', { name: 'Exit zen mode', exact: true }).click();
	await expect(page.locator('[data-panel="studies"]')).toBeVisible();
});

test('the palette switches cases, opens panels and is reachable from the header', async ({
	page
}) => {
	await mockDataRoutes(page);
	await page.route('**/api/cases', (route) =>
		route.fulfill({
			json: [...CASE_SUMMARY, { ...CASE_SUMMARY[0], id: 'case2', name: 'Case Two' }]
		})
	);
	await page.route('**/api/cases/case2/network', (route) =>
		route.fulfill({ json: { ...NETWORK, id: 'case2', name: 'Case Two' } })
	);
	await page.route('**/api/cases/case2/solution', (route) => route.fulfill({ json: SOLUTION }));
	await page.route('**/api/cases/case2/case', (route) => route.fulfill({ status: 500, json: {} }));
	await page.goto('/');
	const button = page.locator('header .case-button');
	await expect(page.locator('#tellegen-case-list .cname', { hasText: 'Case Two' })).toBeAttached();

	await page.getByRole('button', { name: /^Commands/ }).click();
	const dialog = page.getByRole('dialog', { name: 'Command palette' });
	await expect(dialog.getByText('Cases', { exact: true })).toBeVisible();
	await expect(dialog.getByText('Panels', { exact: true })).toBeVisible();
	await dialog.getByRole('combobox').fill('case two');
	await page.keyboard.press('Enter');

	await expect(dialog).toHaveCount(0);
	await expect(button).toContainText('Case Two');
	await expect(page.locator('#tellegen-case-list .case-chip.active')).toContainText('Case Two');

	await page.keyboard.press('ControlOrMeta+k');
	await dialog.getByRole('combobox').fill('agent');
	await expect(dialog.getByRole('option', { name: 'Open Agent panel' })).toBeVisible();
	await page.keyboard.press('Enter');
	await expect(page.locator('[data-panel="agent"]')).toBeVisible();
	await page.keyboard.press('ControlOrMeta+k');
	await dialog.getByRole('combobox').fill('agent');
	await expect(dialog.getByRole('option', { name: 'Close Agent panel' })).toBeVisible();
});

test('zen mode survives a reload until it is exited', async ({ page }) => {
	await page.route('**/api/cases', (route) => route.fulfill({ json: [] }));
	await page.goto('/');
	const toolbar = page.locator('.panel-toolbar');
	await expect(toolbar).toBeVisible();
	await page.keyboard.press('ControlOrMeta+k');
	await page.getByRole('dialog', { name: 'Command palette' }).getByRole('combobox').fill('zen');
	await page.keyboard.press('Enter');
	await expect(toolbar).toHaveCount(0);
	await page.reload();
	await expect(page.getByRole('button', { name: 'Exit zen mode', exact: true })).toBeVisible();
	await expect(toolbar).toHaveCount(0);
	await page.keyboard.press('Escape');
	await expect(toolbar).toBeVisible();
	await page.reload();
	await expect(toolbar).toBeVisible();
});
