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

test('short desktop windows use one drawer without covering map controls', async ({ page }) => {
	await page.setViewportSize({ width: 1280, height: 450 });
	await page.route('**/api/cases', (route) => route.fulfill({ json: [] }));
	await page.goto('/');
	await page.getByRole('button', { name: 'Studies', exact: true }).click();
	await expect(page.locator('[data-panel]:visible')).toHaveCount(1);
	const panel = page.locator('[data-panel="studies"]');
	const bounds = await box(panel);
	const toolbar = await box(page.locator('.panel-toolbar'));
	expect(bounds.y).toBeGreaterThan(toolbar.y + toolbar.height + 40);
	expect(bounds.y + bounds.height).toBeLessThanOrEqual(362);
	const attribution = await box(page.locator('.maplibregl-ctrl-attrib'));
	expect(bounds.y + bounds.height).toBeLessThan(attribution.y);
});

test('switching to a narrow window retains the most recently opened panel', async ({ page }) => {
	await page.route('**/api/cases', (route) => route.fulfill({ json: [] }));
	await page.goto('/');
	await page.getByRole('button', { name: 'Studies', exact: true }).click();
	await page.setViewportSize({ width: 390, height: 844 });
	await expect(page.locator('[data-panel]:visible')).toHaveCount(1);
	await expect(page.locator('[data-panel="studies"]')).toBeVisible();
});
