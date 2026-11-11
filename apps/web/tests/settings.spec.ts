import { expect, test } from './fixtures/page-errors.js';
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
	await expect(page.getByText('no default cases loaded')).toBeVisible();
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
	await expect(page.getByText('Multiconductor', { exact: true }).first()).toBeVisible();
	await page.locator('header').getByRole('button', { name: 'Settings', exact: true }).click();
	const panel = page.getByRole('region', { name: 'Common settings' });
	await panel.getByLabel('Maximum iterations', { exact: true }).fill('150');
	await expect(panel.getByRole('button', { name: 'Run power flow', exact: true })).toBeEnabled();
	await panel.getByRole('button', { name: 'Apply settings', exact: true }).click();
	await expect(panel.getByRole('button', { name: 'Run power flow', exact: true })).toBeEnabled();
	await expect(page.getByRole('region', { name: 'AC power flow results' })).toHaveCount(0);
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
	await expect(panel.getByLabel('Maximum iterations', { exact: true })).toHaveValue('200');
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
	await panel.getByLabel('Maximum iterations', { exact: true }).fill('1');
	await panel.getByRole('button', { name: 'Apply settings', exact: true }).click();
	await panel.getByRole('button', { name: 'Run power flow', exact: true }).click();
	await expect(panel.getByRole('alert')).toContainText('did not converge');
	inspected = await tool(page, 'inspect_settings');
	expect(inspected.data.accepted).toMatchObject({ max_iterations: 200 });
	expect(inspected.data.pending).toBe(true);
	await panel.getByRole('combobox', { name: 'Theme', exact: true }).selectOption('dark');
	await panel.getByRole('combobox', { name: 'Bus sizing', exact: true }).selectOption('uniform');
	const preferences = await tool(page, 'inspect_settings', { scope: 'display' });
	expect(preferences.data.selected).toMatchObject({ theme: 'dark', busSizing: 'uniform' });
	await page.reload();
	await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');
});

test('an explicit run applies the calculation draft and agent edits retain its options', async ({
	page
}) => {
	await page.locator('header input[type=file]').setInputFiles([
		{ name: 'case14.m', mimeType: 'text/plain', buffer: Buffer.from(CASE14) },
		{ name: 'case14-coords.csv', mimeType: 'text/csv', buffer: Buffer.from(CASE14_COORDS) }
	]);
	const result = page.locator('.solvecard');
	await expect(result).toContainText('DC OPF', { timeout: 60000 });
	await page.locator('header').getByRole('button', { name: 'Settings', exact: true }).click();
	const panel = page.getByRole('region', { name: 'Common settings' });
	await panel.getByRole('combobox', { name: 'Calculation', exact: true }).selectOption('acpf');
	await panel.getByRole('combobox', { name: 'Initial voltages', exact: true }).selectOption('case');
	await expect(result).toContainText('DC OPF');
	await panel.getByRole('button', { name: 'Run calculation', exact: true }).click();
	await expect(result).toContainText('AC power flow', { timeout: 60000 });
	const inspected = await tool(page, 'inspect_settings');
	expect(inspected.ok).toBe(true);
	expect(inspected.data.accepted_calculation).toBe('acpf');
	expect(inspected.data.accepted).toEqual({ start: 'case' });
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
	await expect(page.locator('.solvecard')).toContainText('DC OPF', { timeout: 60000 });
	const inspected = await tool(page, 'inspect_settings');
	const updated = await tool(page, 'update_settings', {
		case_id: inspected.data.case_id,
		expected_revision: inspected.data.revision,
		settings_revision: inspected.data.settings_revision,
		patch: { lazy: { near_binding: 0.9 } }
	});
	expect(updated.ok).toBe(true);
	expect(updated.data.selected).toMatchObject({ lazy: { near_binding: 0.9, max_rounds: 20 } });
	await page.locator('header').getByRole('button', { name: 'Settings', exact: true }).click();
	const panel = page.getByRole('region', { name: 'Common settings' });
	await expect(panel.getByLabel('Maximum enforcement rounds')).toHaveValue('20');
	await expect(panel.getByRole('button', { name: 'Apply settings', exact: true })).toBeDisabled();
	await panel.getByRole('button', { name: 'Run calculation', exact: true }).click();
	await expect(panel.getByRole('button', { name: 'Run calculation', exact: true })).toBeEnabled();
	const accepted = await tool(page, 'inspect_settings');
	expect(accepted.data.pending).toBe(false);
	expect(accepted.data.selected).toEqual(accepted.data.accepted);
	await panel.getByRole('combobox', { name: 'Calculation', exact: true }).selectOption('acpf');
	await panel.getByRole('combobox', { name: 'Calculation', exact: true }).selectOption('dcopf');
	await expect(panel.getByRole('button', { name: 'Apply settings', exact: true })).toBeDisabled();
	await expect(panel.getByRole('button', { name: 'Run calculation', exact: true })).toBeEnabled();
});

test('DC conventions share common settings and can return to the input default', async ({
	page
}) => {
	await page.locator('header input[type=file]').setInputFiles([
		{ name: 'case14.m', mimeType: 'text/plain', buffer: Buffer.from(CASE14) },
		{ name: 'case14-coords.csv', mimeType: 'text/csv', buffer: Buffer.from(CASE14_COORDS) }
	]);
	await expect(page.locator('.solvecard')).toContainText('DC OPF', { timeout: 60000 });
	await page.locator('header').getByRole('button', { name: 'Settings', exact: true }).click();
	const panel = page.getByRole('region', { name: 'Common settings' });
	const convention = panel.getByRole('combobox', { name: 'DC convention', exact: true });
	await expect(convention).toHaveValue('null');
	for (const [selected, resolved] of [
		['reactance_only', 'reactance_only'],
		['null', 'series_susceptance']
	]) {
		await convention.selectOption(selected);
		await panel.getByRole('button', { name: 'Run calculation', exact: true }).click();
		await expect
			.poll(async () => (await tool(page, 'inspect_case')).data.solution)
			.toMatchObject({
				branch_susceptance_formula: resolved
			});
		const settings = await tool(page, 'inspect_settings');
		expect(settings.data.accepted).toMatchObject({
			branch_susceptance_formula: selected === 'null' ? null : selected
		});
	}
});
