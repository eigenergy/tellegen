import { expect, test } from './fixtures/page-errors.js';
import { CASE_SUMMARY, NETWORK, SOLUTION, mockDataRoutes } from './fixtures/backend-case.js';

test('Cmd+K opens the palette and zen mode hides the panels until Escape', async ({ page }) => {
	await page.route('**/api/cases', (route) => route.fulfill({ json: [] }));
	await page.goto('/');
	const dialog = page.getByRole('dialog', { name: 'Command palette' });
	const pill = page.getByRole('button', { name: 'Exit zen mode', exact: true });
	const welcome = page.getByRole('complementary', { name: 'New to Tellegen' });
	const toolbar = page.locator('.panel-toolbar');
	const footer = page.getByRole('link', { name: 'credits' });
	const zoom = page.getByRole('button', { name: 'Zoom in' });
	const attribution = page.locator('.maplibregl-ctrl-attrib');

	// The introduction banner and footer are chrome too: zen hides them and
	// brings them back.
	await expect(welcome).toBeVisible();
	await expect(footer).toBeVisible();
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
	await expect(footer).toHaveCount(0);
	await expect(zoom).toBeHidden();
	await expect(attribution).not.toHaveClass(/maplibregl-compact-show/);
	// The (i) button must not move when the attribution text collapses.
	await expect.poll(async () => await info.boundingBox()).toEqual(infoBefore);
	await page.keyboard.press('Escape');
	await expect(welcome).toBeVisible();
	await expect(toolbar).toBeVisible();
	await expect(footer).toBeVisible();
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
	await expect(
		dialog.getByRole('option', { name: 'Exit zen mode Bring the panels back' })
	).toBeVisible();
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

test('zen mode keeps only the active case tab until the strip is hovered', async ({ page }) => {
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
	const strip = page.getByRole('navigation', { name: 'networks' });
	const caseA = strip.getByRole('button', { name: /Case One/ }).first();
	const caseB = strip.getByRole('button', { name: /Case Two/ }).first();
	const file = strip.getByRole('button', { name: /case file/ });
	await expect(caseA).toBeVisible();
	await expect(caseB).toBeVisible();
	await expect(file).toBeVisible();

	await page.keyboard.press('ControlOrMeta+k');
	await page.getByRole('dialog', { name: 'Command palette' }).getByRole('combobox').fill('zen');
	await page.keyboard.press('Enter');
	const visible = strip.locator('.case-chip:visible');
	await expect(visible).toHaveCount(1);
	await expect(file).toBeHidden();

	await visible.hover();
	await expect(caseA).toBeVisible();
	await expect(caseB).toBeVisible();
	await expect(file).toBeVisible();
	await page.mouse.move(640, 400);
	await expect(strip.locator('.case-chip:visible')).toHaveCount(1);
	await expect(file).toBeHidden();

	await page.keyboard.press('Escape');
	await expect(caseA).toBeVisible();
	await expect(caseB).toBeVisible();
	await expect(file).toBeVisible();
});

test('the palette can reset the panel layout', async ({ page }) => {
	await page.route('**/api/cases', (route) => route.fulfill({ json: [] }));
	await page.goto('/');
	await page.getByRole('button', { name: 'Agent', exact: true }).click();
	await page.getByRole('button', { name: 'Agent panel options' }).click();
	await page.getByRole('button', { name: 'Dock left', exact: true }).click();
	await expect(page.locator('[data-panel-dock="left"] [data-panel="agent"]')).toBeVisible();
	await page.keyboard.press('ControlOrMeta+k');
	const dialog = page.getByRole('dialog', { name: 'Command palette' });
	await dialog.getByRole('combobox').fill('reset');
	await expect(dialog.getByRole('option', { name: /Reset panel layout/ })).toBeVisible();
	await page.keyboard.press('Enter');
	await expect(dialog).toHaveCount(0);
	await expect(page.locator('[data-panel-dock="right"] [data-panel="agent"]')).toBeVisible();
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
