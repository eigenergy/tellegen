import { expect, test } from './fixtures/page-errors.js';
import { openSettings } from './fixtures/chrome.js';
import { runCalculation } from './fixtures/calculation.js';
import type { Page } from '@playwright/test';
import { CASE14 } from '../../../examples/browser-minimal/src/case14';
import { CASE14_COORDS } from './fixtures/local-case';

type Answer = {
	ok: boolean;
	data: Record<string, unknown>;
	error?: { code: string; message: string };
};
async function tool(
	page: Page,
	name: string,
	input: Record<string, unknown> = {}
): Promise<Answer> {
	return page.evaluate(
		async ({ name, input }) => {
			const api = (
				window as unknown as {
					settingsTools: Map<string, { execute(input: Record<string, unknown>): Promise<Answer> }>;
				}
			).settingsTools;
			return api.get(name)!.execute(input);
		},
		{ name, input }
	);
}
test.beforeEach(async ({ page }) => {
	await page.addInitScript(() => {
		const tools = new Map<
			string,
			{ name: string; execute(input: Record<string, unknown>): Promise<unknown> }
		>();
		const context = new (class extends EventTarget {
			async registerTool(
				tool: { name: string; execute(input: Record<string, unknown>): Promise<unknown> },
				options: { signal?: AbortSignal } = {}
			) {
				tools.set(tool.name, tool);
				options.signal?.addEventListener('abort', () => tools.delete(tool.name), { once: true });
			}
		})();
		Object.defineProperty(Document.prototype, 'modelContext', {
			configurable: true,
			get: () => context
		});
		Object.defineProperty(window, 'settingsTools', { value: tools });
	});
	await page.route('**/api/cases', (route) => route.fulfill({ json: [] }));
	await page.route('**/api/compute', (route) => route.fulfill({ json: { enabled: false } }));
	await page.goto('/');
	await expect(page.getByText('No cases', { exact: true })).toBeVisible();
});

test('common settings stage changes, share them with WebMCP, reject stale writes and retain a failed result', async ({
	page
}, testInfo) => {
	await page
		.locator('header input[type=file]')
		.setInputFiles(
			new URL('./fixtures/mc-pf-browser/public/mc-pf-three-phase.pio.json', import.meta.url)
				.pathname
		);
	await expect(page.getByTestId('distribution-case-chip').first()).toBeAttached();
	await openSettings(page);
	const panel = page.getByRole('region', { name: 'Common settings' });
	const iterations = panel.getByLabel('Max iterations', { exact: true });
	// A change applies and reruns at once: there is no Apply or Run button.
	await expect(panel.getByRole('button', { name: /^(Apply|Run)/ })).toHaveCount(0);
	await iterations.fill('150');
	await iterations.press('Tab');
	await expect
		.poll(async () => (await tool(page, 'inspect_settings')).data.accepted, { timeout: 60_000 })
		.toMatchObject({ max_iterations: 150 });
	await expect(iterations).toBeEnabled({ timeout: 60_000 });
	let inspected = await tool(page, 'inspect_settings');
	expect(inspected.ok).toBe(true);
	expect(inspected.data.selected).toMatchObject({ max_iterations: 150 });
	const field = await tool(page, 'inspect_settings', { field: 'max_iterations' });
	expect(field.ok).toBe(true);
	expect(field.data.field).toMatchObject({ key: 'max_iterations', type: 'integer', default: 100 });
	const binding = {
		case_id: inspected.data.case_id,
		expected_revision: inspected.data.revision,
		settings_revision: inspected.data.settings_revision
	};
	const update = await tool(page, 'update_settings', {
		...binding,
		patch: { max_iterations: 200 }
	});
	expect(update.ok).toBe(true);
	await expect(iterations).toHaveValue('200');
	const stale = await tool(page, 'update_settings', { ...binding, patch: { max_iterations: 100 } });
	expect(stale.ok).toBe(false);
	expect(stale.error?.code).toBe('STALE_REVISION');
	inspected = await tool(page, 'inspect_settings');
	const run = await tool(page, 'run_calculation', {
		case_id: inspected.data.case_id,
		expected_revision: inspected.data.revision,
		settings_revision: inspected.data.settings_revision
	});
	expect(run.ok).toBe(true);
	expect(run.data.accepted).toMatchObject({ max_iterations: 200 });
	await page.screenshot({ path: testInfo.outputPath('settings-desktop.png'), fullPage: true });
	await expect(iterations).toBeEnabled({ timeout: 60_000 });
	await iterations.fill('1');
	await iterations.press('Tab');
	await expect(panel.getByRole('alert')).toHaveText('Did not converge', { timeout: 60_000 });
	inspected = await tool(page, 'inspect_settings');
	expect(inspected.data.accepted).toMatchObject({ max_iterations: 200 });
	expect(inspected.data.pending).toBe(true);
	await panel.getByRole('combobox', { name: 'Theme', exact: true }).selectOption('dark');
	await panel.getByRole('combobox', { name: 'Bus sizing', exact: true }).selectOption('uniform');
	const preferences = await tool(page, 'inspect_settings', { scope: 'display' });
	expect(preferences.data.selected).toMatchObject({ theme: 'dark', busSizing: 'uniform' });
	await page.reload();
	await expect(page.locator('.tellegen')).toHaveAttribute('data-theme', 'dark');
});

test('an explicit run applies the calculation draft and agent edits retain its options', async ({
	page
}) => {
	await page.locator('header input[type=file]').setInputFiles([
		{ name: 'case14.m', mimeType: 'text/plain', buffer: Buffer.from(CASE14) },
		{ name: 'case14-coords.csv', mimeType: 'text/csv', buffer: Buffer.from(CASE14_COORDS) }
	]);
	const result = page.locator('.solvecard');
	await expect(result).toHaveAttribute('data-calculation', 'dcopf', { timeout: 60000 });
	// Choosing a calculation in the Network panel runs it.
	await runCalculation(page, 'acpf');
	await expect(result).toHaveAttribute('data-calculation', 'acpf', { timeout: 60000 });
	await openSettings(page);
	const panel = page.getByRole('region', { name: 'Common settings' });
	// The Network panel chooses the calculation; Settings shows that calculation's options only.
	await expect(panel.getByRole('combobox', { name: 'Type', exact: true })).toHaveCount(0);
	await expect(panel.getByText('Line limits', { exact: true })).toHaveCount(0);
	// Choosing an option applies it and reruns.
	await panel
		.getByRole('combobox', { name: 'Starting voltages', exact: true })
		.selectOption('case');
	await expect
		.poll(async () => (await tool(page, 'inspect_settings')).data.accepted)
		.toEqual({ start: 'case' });
	const inspected = await tool(page, 'inspect_settings');
	expect(inspected.ok).toBe(true);
	expect(inspected.data.accepted_calculation).toBe('acpf');
	const current = await tool(page, 'inspect_case');
	const updated = await tool(page, 'update_case', {
		case_id: current.data.case_id,
		expected_revision: current.data.revision,
		demand: [{ bus_id: 'bus:2', delta_mw: 1 }]
	});
	expect(updated.ok).toBe(true);
	expect((await tool(page, 'inspect_settings')).data.accepted).toEqual({ start: 'case' });
	const edited = await tool(page, 'inspect_case');
	const reset = await tool(page, 'reset_case', {
		case_id: edited.data.case_id,
		expected_revision: edited.data.revision
	});
	expect(reset.ok).toBe(true);
	expect((await tool(page, 'inspect_settings')).data.accepted).toEqual({ start: 'case' });
});

test('partial lazy options agree with accepted settings and remain clean when switching drafts', async ({
	page
}) => {
	await page.locator('header input[type=file]').setInputFiles([
		{ name: 'case14.m', mimeType: 'text/plain', buffer: Buffer.from(CASE14) },
		{ name: 'case14-coords.csv', mimeType: 'text/csv', buffer: Buffer.from(CASE14_COORDS) }
	]);
	await expect(page.locator('.solvecard')).toHaveAttribute('data-calculation', 'dcopf', {
		timeout: 60000
	});
	const inspected = await tool(page, 'inspect_settings');
	const updated = await tool(page, 'update_settings', {
		case_id: inspected.data.case_id,
		expected_revision: inspected.data.revision,
		settings_revision: inspected.data.settings_revision,
		patch: { lazy: { near_binding: 0.9 } }
	});
	expect(updated.ok).toBe(true);
	expect(updated.data.selected).toMatchObject({ lazy: { near_binding: 0.9, max_rounds: 20 } });
	await openSettings(page);
	const panel = page.getByRole('region', { name: 'Common settings' });
	await expect(panel.getByLabel('Max rounds', { exact: true })).toHaveValue('20');
	await expect(panel.getByLabel('Add above (%)', { exact: true })).toHaveValue('90');
	// The agent's change is not applied yet: Settings says so and offers to run it.
	await expect(panel.getByText('Not applied', { exact: true })).toBeVisible();
	await panel.getByRole('button', { name: 'Run', exact: true }).click();
	await expect
		.poll(async () => (await tool(page, 'inspect_settings')).data.pending, { timeout: 60_000 })
		.toBe(false);
	const accepted = await tool(page, 'inspect_settings');
	expect(accepted.data.selected).toEqual(accepted.data.accepted);
	await expect(panel.getByText('Not applied', { exact: true })).toHaveCount(0);
});

test('the DC line model is chosen in one place, shared with WebMCP, and can return to the input default', async ({
	page
}) => {
	await page.locator('header input[type=file]').setInputFiles([
		{ name: 'case14.m', mimeType: 'text/plain', buffer: Buffer.from(CASE14) },
		{ name: 'case14-coords.csv', mimeType: 'text/csv', buffer: Buffer.from(CASE14_COORDS) }
	]);
	await expect(page.locator('.solvecard')).toHaveAttribute('data-calculation', 'dcopf', {
		timeout: 60000
	});
	// Settings holds the DC line model, under Calculation; the Network panel does not repeat it.
	await openSettings(page);
	const panel = page.getByRole('region', { name: 'Common settings' });
	await expect(panel.getByLabel('Angle limits', { exact: true })).toBeVisible();
	await expect(panel).not.toContainText('series_susceptance');
	await expect(page.getByRole('combobox', { name: 'DC line model', exact: true })).toHaveCount(1);
	const model = panel.getByRole('combobox', { name: 'DC line model', exact: true });
	await expect(model).toHaveValue('null');
	// One default entry, and plain names for the rest.
	await expect(model.locator('option')).toHaveText([
		'Default',
		'Series impedance',
		'Reactance with taps',
		'Reactance only',
		'Series impedance with taps',
		'Series impedance, no phase shift'
	]);
	await model.selectOption('reactance_only');
	await expect
		.poll(async () => (await tool(page, 'inspect_case')).data.solution)
		.toMatchObject({ branch_susceptance_formula: 'reactance_only' });
	expect((await tool(page, 'inspect_settings')).data.accepted).toMatchObject({
		branch_susceptance_formula: 'reactance_only'
	});
	// An agent can hand the choice back to the input, which resolves to the default.
	let settings = await tool(page, 'inspect_settings');
	const reset = await tool(page, 'update_settings', {
		case_id: settings.data.case_id,
		expected_revision: settings.data.revision,
		settings_revision: settings.data.settings_revision,
		patch: { branch_susceptance_formula: null }
	});
	expect(reset.ok).toBe(true);
	settings = await tool(page, 'inspect_settings');
	const run = await tool(page, 'run_calculation', {
		case_id: settings.data.case_id,
		expected_revision: settings.data.revision,
		settings_revision: settings.data.settings_revision
	});
	expect(run.ok).toBe(true);
	await expect
		.poll(async () => (await tool(page, 'inspect_case')).data.solution)
		.toMatchObject({ branch_susceptance_formula: 'series_susceptance' });
	expect((await tool(page, 'inspect_settings')).data.accepted).toMatchObject({
		branch_susceptance_formula: null
	});
	await expect(model).toHaveValue('null');
});
