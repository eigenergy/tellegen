import type { Locator } from '@playwright/test';
import { expect, test } from './fixtures/page-errors.js';

async function box(locator: Locator) {
	const value = await locator.boundingBox();
	if (!value) throw new Error('Expected a visible panel');
	return value;
}

test('panels detach, resize, persist, redock and reset without leaving the viewport', async ({
	page
}) => {
	await page.route('**/api/cases', (route) => route.fulfill({ json: [] }));
	await page.goto('/');
	await expect(page.getByText('no default cases loaded')).toBeVisible();
	await page.getByRole('button', { name: 'Agent', exact: true }).click();
	const panel = page.locator('[data-panel="agent"]');
	const handle = page.getByRole('button', { name: 'Move Agent panel', exact: true });
	const start = await box(handle);
	await page.mouse.move(start.x + 30, start.y + 10);
	await page.mouse.down();
	await page.mouse.move(start.x - 170, start.y + 90, { steps: 8 });
	await page.mouse.up();
	await expect(panel).toHaveClass(/floating/);
	const floated = await box(panel);
	await page.getByRole('button', { name: 'Resize Agent panel' }).press('ArrowRight');
	await expect.poll(async () => (await box(panel)).width).toBeCloseTo(floated.width + 10);
	const resized = await box(panel);
	await page.reload();
	await expect(page.getByText('no default cases loaded')).toBeVisible();
	await page.getByRole('button', { name: 'Agent', exact: true }).click();
	await expect.poll(async () => await box(panel)).toEqual(resized);
	await page.setViewportSize({ width: 1024, height: 700 });
	await expect(async () => {
		const clamped = await box(panel);
		expect(clamped.x).toBeGreaterThanOrEqual(16);
		expect(clamped.y).toBeGreaterThanOrEqual(0);
		expect(clamped.x + clamped.width).toBeLessThanOrEqual(1008);
		expect(clamped.y + clamped.height).toBeLessThanOrEqual(612);
	}).toPass();
	await page.getByRole('button', { name: 'Agent panel options' }).click();
	await page.getByRole('button', { name: 'Dock left', exact: true }).click();
	await expect(page.locator('[data-panel-dock="left"] [data-panel="agent"]')).toBeVisible();
	await page.getByRole('button', { name: 'Reset layout', exact: true }).click();
	await expect(page.locator('[data-panel-dock="right"] [data-panel="agent"]')).toBeVisible();
	await page.getByRole('button', { name: 'Move Agent panel', exact: true }).press('ArrowLeft');
	await expect(panel).toHaveClass(/floating/);
	await expect(page.getByRole('button', { name: 'Move Agent panel', exact: true })).toBeFocused();
});

test('narrow screens show one drawer and keep panel controls reachable', async ({ page }) => {
	await page.setViewportSize({ width: 390, height: 844 });
	await page.route('**/api/cases', (route) => route.fulfill({ json: [] }));
	await page.goto('/');
	await expect(page.getByText('no default cases loaded')).toBeVisible();
	for (const title of ['Studies', 'Agent', 'Network']) {
		await page.getByRole('button', { name: title, exact: true }).click();
		await expect(page.locator('[data-panel]:visible')).toHaveCount(1);
		const active = page.locator(`[data-panel="${title.toLowerCase()}"]`);
		await expect(active).toBeVisible();
		const bounds = await box(active);
		expect(bounds.x).toBeGreaterThanOrEqual(0);
		expect(bounds.x + bounds.width).toBeLessThanOrEqual(390);
		expect(bounds.y + bounds.height).toBeLessThanOrEqual(756);
	}
	await page.getByRole('button', { name: 'Close network panel' }).click();
	await expect(page.locator('[data-panel]:visible')).toHaveCount(0);
	await expect(page.getByRole('button', { name: 'Network', exact: true })).toBeFocused();
});

test('a panel opened from outside the toolbar hands focus back to that control', async ({
	page
}) => {
	await page.route('**/api/cases', (route) => route.fulfill({ json: [] }));
	await page.goto('/');
	await expect(page.getByText('no default cases loaded')).toBeVisible();
	// A host shell that hides the toolbar and opens panels from its own rail.
	await page.locator('.panel-toolbar').evaluate((toolbar: HTMLElement) => {
		toolbar.style.display = 'none';
		const rail = document.createElement('button');
		rail.textContent = 'Rail agent';
		rail.addEventListener('click', () =>
			toolbar.querySelector<HTMLButtonElement>('button[aria-label="Agent"]')!.click()
		);
		document.body.append(rail);
	});
	const rail = page.getByRole('button', { name: 'Rail agent' });
	await rail.press('Enter');
	await expect(page.locator('[data-panel="agent"]')).toBeVisible();
	await page.getByRole('button', { name: 'Close agent panel' }).click();
	await expect(rail).toBeFocused();
});

test('a panel closed with no toolbar and no opener hands focus to its dock', async ({ page }) => {
	await page.route('**/api/cases', (route) => route.fulfill({ json: [] }));
	await page.goto('/');
	await expect(page.getByText('no default cases loaded')).toBeVisible();
	// What `PanelHost showToolbar={false}` renders: no toolbar at all. The
	// Network panel opened with the page, so no control opened it.
	await page.locator('.panel-toolbar').evaluate((toolbar) => toolbar.remove());
	await page.getByRole('button', { name: 'Close network panel' }).click();
	await expect(page.getByRole('region', { name: 'Left panels' })).toBeFocused();
});

test('panel content takes keyboard focus only while it overflows', async ({ page }) => {
	await page.route('**/api/cases', (route) => route.fulfill({ json: [] }));
	await page.goto('/');
	await expect(page.getByText('no default cases loaded')).toBeVisible();
	const content = page.locator('[data-panel="network"] .panel-content');
	await expect(content).not.toHaveAttribute('tabindex');
	const body = page.locator('#control-panel-body');
	await body.evaluate((element: HTMLElement) => (element.style.minHeight = '3000px'));
	await expect(content).toHaveAttribute('tabindex', '0');
	await expect(page.getByRole('region', { name: 'Network panel content' })).toBeVisible();
	await content.focus();
	await page.keyboard.press('PageDown');
	await expect.poll(() => content.evaluate((element) => element.scrollTop)).toBeGreaterThan(0);
	await body.evaluate((element: HTMLElement) => (element.style.minHeight = ''));
	await expect(content).not.toHaveAttribute('tabindex');
});

test('short desktop windows use one drawer without covering map controls', async ({ page }) => {
	await page.setViewportSize({ width: 1280, height: 450 });
	await page.route('**/api/cases', (route) => route.fulfill({ json: [] }));
	await page.goto('/');
	await expect(page.getByText('no default cases loaded')).toBeVisible();
	await page.getByRole('button', { name: 'Studies', exact: true }).click();
	await expect(page.locator('[data-panel]:visible')).toHaveCount(1);
	const panel = page.locator('[data-panel="studies"]');
	const bounds = await box(panel);
	const toolbar = await box(page.locator('.panel-toolbar'));
	expect(bounds.y).toBeGreaterThan(toolbar.y + toolbar.height + 40);
	expect(bounds.y + bounds.height).toBeLessThanOrEqual(362);
	// MapLibre adds the attribution control once the map loads, which can trail the panel.
	const credit = page.locator('.maplibregl-ctrl-attrib');
	await expect(credit).toBeVisible();
	const attribution = await box(credit);
	expect(bounds.y + bounds.height).toBeLessThan(attribution.y);
});

test('switching to a narrow window retains the most recently opened panel', async ({ page }) => {
	await page.route('**/api/cases', (route) => route.fulfill({ json: [] }));
	await page.goto('/');
	await expect(page.getByText('no default cases loaded')).toBeVisible();
	await page.getByRole('button', { name: 'Studies', exact: true }).click();
	await page.setViewportSize({ width: 390, height: 844 });
	await expect(page.locator('[data-panel]:visible')).toHaveCount(1);
	await expect(page.locator('[data-panel="studies"]')).toBeVisible();
});
