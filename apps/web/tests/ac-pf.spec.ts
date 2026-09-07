import { readFile } from 'node:fs/promises';
import type { StudyBundle } from '@tellegen/engine';
import { expect, test } from './fixtures/page-errors.js';
import {
	CASE3_COORDS,
	CASE3_PLANNING,
	callTool,
	installWebMcpHarness
} from './fixtures/planning-case.js';

test('AC power flow shows voltages, supports equipment inspection, and saves its result', async ({
	page
}, testInfo) => {
	test.setTimeout(120_000);
	await installWebMcpHarness(page);
	await page.route('**/api/compute', (route) => route.fulfill({ json: { enabled: false } }));
	await page.route('**/api/cases', (route) => route.fulfill({ json: [] }));
	await page.goto('/');
	await expect(page.getByText('no default cases loaded')).toBeVisible();
	await page.locator('input[type="file"]').setInputFiles([
		{ name: 'case3-coords.csv', mimeType: 'text/csv', buffer: Buffer.from(CASE3_COORDS) },
		{ name: 'case3pf.m', mimeType: 'text/plain', buffer: Buffer.from(CASE3_PLANNING) }
	]);
	await expect(page.locator('.solvecard')).toContainText('OPF solve', { timeout: 60_000 });
	const calculation = page.getByRole('combobox', { name: 'Calculation', exact: true });
	await expect(calculation).toBeEnabled();
	await expect(calculation.locator('option[value="acopf"]')).toHaveJSProperty('disabled', true);
	await calculation.selectOption('acpf');
	await expect(page.locator('.solvecard')).toContainText('Power flow');
	await expect(calculation).toBeEnabled();
	await expect(page.getByRole('button', { name: '|V|', exact: true })).toHaveAttribute(
		'aria-pressed',
		'true'
	);
	await expect(page.getByRole('button', { name: 'LMP', exact: true })).toHaveCount(0);
	await expect(page.getByText('OPF objective', { exact: true })).toHaveCount(0);
	const inspected = await callTool(page, 'inspect_case', {});
	expect(inspected).toMatchObject({
		ok: true,
		data: { formulation: 'acpf', solution: { objective: null } }
	});
	if (!inspected.ok) throw new Error('Case inspection failed');
	const caseId = String(inspected.data.case_id);
	const queried = await callTool(page, 'query_network', {
		case_id: caseId,
		element_kind: 'bus',
		sort_by: 'voltage_pu',
		direction: 'asc',
		limit: 3
	});
	expect(queried.ok).toBe(true);
	if (!queried.ok) throw new Error('Voltage query failed');
	const buses = queried.data.elements as Array<{
		legacy_id: number;
		voltage_pu: number;
		price: number | null;
	}>;
	expect(buses).toHaveLength(3);
	expect(buses.every((bus) => bus.price === null)).toBe(true);
	const loadBus = buses.find((bus) => bus.legacy_id === 2)!;
	expect(loadBus.voltage_pu).toBeGreaterThan(0.9);
	expect(loadBus.voltage_pu).toBeLessThan(1);
	for (const bus of buses.filter((bus) => bus.legacy_id !== 2))
		expect(bus.voltage_pu).toBeCloseTo(1, 8);
	await page.getByRole('combobox', { name: 'bus lookup', exact: true }).fill('2');
	await page.getByRole('combobox', { name: 'bus lookup', exact: true }).press('Enter');
	const equipment = page.getByRole('region', { name: 'Power-flow equipment results' });
	await expect(equipment).toContainText(`${loadBus.voltage_pu.toFixed(4)} pu`);
	await expect(equipment).toContainText('90 MW');
	await expect(equipment).toContainText('rad');
	await expect(page.locator('.error')).toHaveCount(0);
	await equipment.scrollIntoViewIfNeeded();
	await page.screenshot({ path: testInfo.outputPath('ac-power-flow.png') });
	await page.setViewportSize({ width: 390, height: 844 });
	await expect(equipment).toBeVisible();
	await page.screenshot({ path: testInfo.outputPath('ac-power-flow-mobile.png') });
	await page.setViewportSize({ width: 1280, height: 800 });
	await page.getByRole('button', { name: 'Studies', exact: true }).click();
	await page.getByLabel('Study name').fill('Three-bus power flow');
	await page.getByRole('button', { name: 'Save study', exact: true }).click();
	await expect(
		page.getByRole('heading', { name: 'Three-bus power flow', exact: true })
	).toBeVisible();
	await page.getByRole('button', { name: 'Show on map', exact: true }).click();
	await expect(page.getByRole('region', { name: 'Saved network details' })).toBeVisible();
	const saved = await callTool(page, 'query_network', {
		case_id: caseId,
		element_kind: 'bus',
		sort_by: 'voltage_pu',
		direction: 'asc',
		limit: 3
	});
	expect(saved).toMatchObject({ ok: true, data: { formulation: 'acpf' } });
	if (!saved.ok) throw new Error('Saved voltage query failed');
	expect(saved.data.state_id).toBeTruthy();
	expect(
		(saved.data.elements as typeof buses).map((bus) => [bus.legacy_id, bus.voltage_pu, bus.price])
	).toEqual(buses.map((bus) => [bus.legacy_id, bus.voltage_pu, bus.price]));
	await page.getByRole('button', { name: 'Return to live case', exact: true }).click();
	await calculation.selectOption('dcopf');
	await expect(calculation).toBeEnabled();
	await expect(page.getByRole('button', { name: 'LMP', exact: true })).toBeVisible();
	await expect(page.locator('.solvecard')).toContainText('OPF solve');
	await expect(page.locator('.error')).toHaveCount(0);
	const download = page.waitForEvent('download');
	await page.getByRole('button', { name: 'Export', exact: true }).click();
	const bundle: StudyBundle = JSON.parse(await readFile((await (await download).path())!, 'utf8'));
	const state = bundle.document.states[bundle.document.inspected_state!];
	await page.locator('input[type="file"]').setInputFiles({
		name: 'declared-ac-pf.pio.json',
		mimeType: 'application/json',
		buffer: Buffer.from(bundle.artifacts[state.input].text)
	});
	await expect(calculation).toHaveValue('acpf');
	await expect(calculation).toBeEnabled();
	await expect(page.locator('.solvecard')).toContainText('Power flow');
	await expect(page.getByRole('button', { name: '|V|', exact: true })).toHaveAttribute(
		'aria-pressed',
		'true'
	);
	const dropped = await callTool(page, 'inspect_case', {});
	expect(dropped).toMatchObject({
		ok: true,
		data: { formulation: 'acpf', available_formulations: ['acpf'], solution: { objective: null } }
	});
	await expect(calculation.locator('option[value="dcopf"]')).toHaveJSProperty('disabled', true);
	await expect(page.locator('.error')).toHaveCount(0);
});
