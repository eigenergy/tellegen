import { readFile } from 'node:fs/promises';
import type { StudyBundle } from '@tellegen/engine';
import type { Page } from '@playwright/test';
import { expect, test } from './fixtures/page-errors.js';
import { installWebMcpHarness, congestCase } from './fixtures/planning-case.js';

test.beforeEach(async ({ request }) => {
	const expectedVersion = JSON.parse(
		await readFile(new URL('../build/_app/version.json', import.meta.url), 'utf8')
	);
	const servedVersion = await request.get('/_app/version.json');
	expect(await servedVersion.json(), 'Preview must serve the current isolated build').toEqual(
		expectedVersion
	);
	const html = await (await request.get('/')).text();
	const assets = [...new Set(html.match(/_app\/immutable\/[A-Za-z0-9_.\/-]+/g))];
	expect(assets.length).toBeGreaterThan(0);
	for (const asset of assets) {
		expect((await request.get(`/${asset}`)).ok(), `Preview asset ${asset}`).toBe(true);
	}
});

async function exported(page: Page): Promise<StudyBundle> {
	const pending = page.waitForEvent('download');
	await page.getByRole('button', { name: 'Export', exact: true }).click();
	const file = await pending;
	return JSON.parse(await readFile((await file.path())!, 'utf8'));
}

async function saveCase(page: Page) {
	await installWebMcpHarness(page);
	await congestCase(page);
	await page.getByRole('button', { name: 'Studies', exact: true }).click();
	await page.getByLabel('Study name').fill('Demand comparison');
	await page.getByRole('button', { name: 'Save study', exact: true }).click();
	await expect(page.getByRole('heading', { name: 'Demand comparison' })).toBeVisible();
	return exported(page);
}

test('saving a case needs no objective and controls produce the current planning goal', async ({
	page
}, testInfo) => {
	test.setTimeout(120_000);
	const saved = await saveCase(page);
	await page.screenshot({ path: testInfo.outputPath('saved-case-desktop.png') });
	expect(saved.document.active_goal ?? null).toBeNull();
	expect(Object.keys(saved.document.goals)).toHaveLength(0);
	await expect(page.getByRole('button', { name: 'Reset to base case', exact: true })).toBeEnabled();
	await expect(page.getByText('Resolve equipment and weights', { exact: true })).toHaveCount(0);
	await expect(page.locator('.study-workspace textarea')).toHaveCount(0);
	await page.getByRole('button', { name: 'Plan', exact: true }).click();
	await page.getByRole('button', { name: 'Set a goal', exact: true }).click();
	await page.getByLabel('Change budget (MW)', { exact: true }).fill('40');
	await page.getByLabel('Increment (MW)', { exact: true }).fill('2');
	await page.screenshot({ path: testInfo.outputPath('goal-form-desktop.png') });
	await page.getByRole('button', { name: 'Save goal', exact: true }).click();
	await expect(page.getByLabel('Solve budget', { exact: true })).toBeVisible();
	const planned = await exported(page);
	const goal = planned.document.goals[planned.document.active_goal!];
	expect(goal.decisions.total_budget).toBe(40);
	expect(goal.decisions.variables).toHaveLength(3);
	expect(
		goal.decisions.variables.every((variable) => variable.upper === 40 && variable.increment === 2)
	).toBe(true);
	expect(planned.document.states).toEqual(saved.document.states);
	expect(planned.document.applied_state).toBe(saved.document.applied_state);
	await page.setViewportSize({ width: 390, height: 844 });
	await expect(page.getByLabel('Solve budget', { exact: true })).toBeVisible();
	await page.screenshot({ path: testInfo.outputPath('saved-goal-mobile.png') });
});

test('filtering selected equipment changes visibility without dropping the selection', async ({
	page
}) => {
	test.setTimeout(120_000);
	await saveCase(page);
	await page.getByRole('button', { name: 'Plan', exact: true }).click();
	await page.getByRole('button', { name: 'Set a goal', exact: true }).click();
	await page.getByRole('combobox', { name: 'Equipment', exact: true }).selectOption('selected');
	await page.getByRole('checkbox', { name: 'Bus 2', exact: true }).check();
	await page.getByText('Candidate lines', { exact: false }).click();
	await page.getByRole('checkbox', { name: 'Line 1, 1 to 2', exact: true }).check();
	await page.getByLabel('Find candidate', { exact: true }).fill('Line 3');
	await expect(page.getByRole('checkbox', { name: 'Line 1, 1 to 2', exact: true })).toHaveCount(0);
	await page.getByRole('button', { name: 'Save goal', exact: true }).click();
	await expect(page.getByLabel('Solve budget', { exact: true })).toBeVisible();
	const planned = await exported(page);
	const goal = planned.document.goals[planned.document.active_goal!];
	expect(goal.decisions.variables).toHaveLength(1);
	expect(goal.objective.kind).toBe('weighted_observable');
	if (goal.objective.kind === 'weighted_observable') expect(goal.objective.weights).toHaveLength(1);
});

test('saving preserves geographic coordinates and leaves the camera unchanged', async ({
	page
}) => {
	test.setTimeout(120_000);
	const first = await saveCase(page);
	expect(first.document.display?.camera).toBeTruthy();
	const geography = first.document.display!.geography;
	const geo = JSON.parse(first.artifacts[geography].text);
	const points = geo.features.filter(
		(feature: { geometry: { type: string } }) => feature.geometry.type === 'Point'
	);
	expect(
		points.map((feature: { geometry: { coordinates: number[] } }) => feature.geometry.coordinates)
	).toEqual([
		[-81.1, 34],
		[-81, 34.1],
		[-80.9, 34]
	]);
	await page.getByRole('button', { name: 'New study', exact: true }).click();
	await page.getByLabel('Study name', { exact: true }).fill('Same case, same view');
	await page.getByRole('button', { name: 'Save study', exact: true }).click();
	await expect(
		page.getByRole('heading', { name: 'Same case, same view', exact: true })
	).toBeVisible();
	const second = await exported(page);
	expect(second.document.display?.geography).toBe(geography);
	expect(second.artifacts[geography]).toEqual(first.artifacts[geography]);
	const before = first.document.display!.camera!;
	const after = second.document.display!.camera!;
	expect(after.center[0]).toBeCloseTo(before.center[0], 8);
	expect(after.center[1]).toBeCloseTo(before.center[1], 8);
	expect(after.zoom).toBeCloseTo(before.zoom, 8);
	expect(after.bearing).toBeCloseTo(before.bearing, 8);
	expect(after.pitch).toBeCloseTo(before.pitch, 8);
});

test('demand edits accumulate across buses and reset retains the saved history', async ({
	page
}) => {
	test.setTimeout(120_000);
	const original = await saveCase(page);
	await page.getByText('Bus demand', { exact: true }).click();
	const row = (bus: number) =>
		page.locator('.demand-editor tbody tr').filter({
			has: page.getByRole('rowheader', { name: String(bus), exact: true })
		});
	for (const [bus, change] of [
		[2, 5],
		[1, 3],
		[2, -2]
	]) {
		await page.getByLabel('Bus', { exact: true }).fill(String(bus));
		await page.getByLabel('Change (MW)', { exact: true }).fill(String(change));
		await page.getByRole('button', { name: 'Update demand', exact: true }).click();
		await expect(page.getByRole('button', { name: 'Update demand', exact: true })).toBeEnabled();
	}
	await expect(row(1).getByRole('cell')).toHaveText(['0', '3', '3']);
	await expect(row(2).getByRole('cell')).toHaveText(['90', '93', '3']);
	const changed = await exported(page);
	expect(changed.document.active_goal ?? null).toBeNull();
	expect(Object.keys(changed.document.states)).toHaveLength(4);
	await page.getByRole('button', { name: 'Reset to base case', exact: true }).click();
	await expect(row(1).getByRole('cell')).toHaveText(['0', '0', '0']);
	await expect(row(2).getByRole('cell')).toHaveText(['90', '90', '0']);
	const reset = await exported(page);
	expect(Object.keys(reset.document.states)).toHaveLength(5);
	for (const [id, state] of Object.entries(changed.document.states)) {
		expect(reset.document.states[id]).toEqual(state);
	}
	expect(reset.document.applied_state).toBe(reset.document.inspected_state);
	expect(reset.document.display?.geography).toBe(original.document.display?.geography);
});
