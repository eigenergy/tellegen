import { readFile } from 'node:fs/promises';
import { expect, test } from './fixtures/page-errors.js';
import { CASE3_COORDS, callTool, installWebMcpHarness } from './fixtures/planning-case.js';

test('DC variant selection changes the solved problem and links its compatibility matrix', async ({
	page
}) => {
	test.setTimeout(120_000);
	await installWebMcpHarness(page);
	await page.route('**/api/compute', (route) => route.fulfill({ json: { enabled: false } }));
	await page.route('**/api/cases', (route) => route.fulfill({ json: [] }));
	await page.goto('/');
	await expect(page.getByText('no default cases loaded')).toBeVisible();
	const data = await readFile(
		new URL('../../../crates/tellegen/tests/data/dc_variants/case3_variants.m', import.meta.url)
	);
	await page.locator('input[type="file"][multiple]').setInputFiles([
		{ name: 'case3-coords.csv', mimeType: 'text/csv', buffer: Buffer.from(CASE3_COORDS) },
		{ name: 'variants.m', mimeType: 'text/plain', buffer: data }
	]);
	const selector = page.getByRole('combobox', { name: 'DC variant', exact: true });
	await expect(selector).toBeEnabled({ timeout: 60_000 });
	await expect(selector).toHaveValue('series_susceptance');
	await expect(page.getByRole('link', { name: 'Compatibility matrix' })).toHaveAttribute(
		'href',
		/dc-compatibility.html$/
	);
	const baseline = await callTool(page, 'inspect_case', {});
	if (!baseline.ok) throw new Error('No base solve');
	for (const formula of [
		'series_susceptance_no_phase_shift',
		'tap_adjusted_reactance',
		'tap_adjusted_series_susceptance',
		'reactance_only'
	]) {
		await selector.selectOption(formula);
		await expect(selector).toBeEnabled();
		const result = await callTool(page, 'inspect_case', {});
		expect(result).toMatchObject({
			ok: true,
			data: { solution: { branch_susceptance_formula: formula } }
		});
		if (!result.ok) throw new Error('No variant solve');
		const edit = {
			case_id: result.data.case_id,
			expected_revision: result.data.revision,
			demand: [{ bus_id: '2', delta_mw: 0.001 }]
		};
		const preview = await callTool(page, 'preview_case_update', edit);
		expect(preview).toMatchObject({ ok: true });
		const updated = await callTool(page, 'update_case', edit);
		expect(updated).toMatchObject({
			ok: true,
			data: { solution: { branch_susceptance_formula: formula } }
		});
		if (!preview.ok || !updated.ok) throw new Error('No variant preview/update');
		expect((preview.data.prediction as { objective: number }).objective).toBeCloseTo(
			(updated.data.solution as { objective: number }).objective,
			5
		);
		const reset = await callTool(page, 'reset_case', {
			case_id: result.data.case_id,
			expected_revision: updated.data.revision
		});
		expect(reset).toMatchObject({ ok: true });
		const afterReset = await callTool(page, 'inspect_case', {});
		expect(afterReset).toMatchObject({
			ok: true,
			data: { solution: { branch_susceptance_formula: formula } }
		});
		expect((result.data.solution as { objective: number }).objective).not.toBe(
			(baseline.data.solution as { objective: number }).objective
		);
	}
});
