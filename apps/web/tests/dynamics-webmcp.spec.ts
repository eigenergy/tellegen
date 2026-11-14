import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { expect, test } from './fixtures/page-errors.js';
import { callTool, installWebMcpHarness } from './fixtures/planning-case';

const example = (name: string) =>
	readFileSync(
		fileURLToPath(new URL(`../static/examples/wecc240/${name}`, import.meta.url)),
		'utf8'
	);

test.setTimeout(180_000);

test('an agent attaches .dyr text, runs a trip and reads a compact verdict', async ({ page }) => {
	await installWebMcpHarness(page);
	await page.route('**/api/cases', (route) => void route.fulfill({ json: [] }));
	await page.goto('/');
	await expect(page.getByText('no default cases loaded')).toBeVisible();
	await expect(page.locator('html')).toHaveAttribute('data-webmcp', 'ready');

	const opened = await callTool(page, 'import_case', {
		file_name: 'wecc240.raw',
		text: example('case.raw')
	});
	expect(opened.ok).toBe(true);
	const attached = await callTool(page, 'import_case', {
		file_name: 'wecc240.dyr',
		text: example('case.dyr')
	});
	expect(attached).toMatchObject({ ok: true, data: { renewable_plants: 37 } });
	// A second .dyr never opens a modal mid-call: it is refused unless the
	// agent says to replace, and then the new file is what is attached.
	const again = await callTool(page, 'import_case', {
		file_name: 'other.dyr',
		text: example('case.dyr')
	});
	expect(again).toMatchObject({ ok: false, error: { code: 'DYNAMICS_ATTACHED' } });
	const replaced = await callTool(page, 'import_case', {
		file_name: 'wecc240.dyr',
		text: example('case.dyr'),
		replace_dynamics: true
	});
	expect(replaced).toMatchObject({ ok: true, data: { renewable_plants: 37 } });

	const inspected = await callTool(page, 'inspect_case', {});
	if (!inspected.ok) throw new Error(inspected.error.message);
	const { case_id, revision } = inspected.data as { case_id: string; revision: string };

	const unknown = await callTool(page, 'simulate_dynamics', {
		case_id,
		expected_revision: revision,
		events: [{ kind: 'branch_trip', branch: '1101-9999', at_s: 1 }]
	});
	expect(unknown).toMatchObject({ ok: false, error: { code: 'INVALID_INPUT' } });

	const r = await callTool(page, 'simulate_dynamics', {
		case_id,
		expected_revision: revision,
		events: [{ kind: 'generator_trip', unit: '1431#N', at_s: 1 }],
		t_end_s: 15,
		channels: ['coi:f'],
		trace_points: 20
	});
	if (!r.ok) throw new Error(r.error.message);
	expect(JSON.stringify(r).length).toBeLessThanOrEqual(1450);
	const data = r.data as {
		verdict: string;
		frequency: { coi_nadir_hz: number; nominal_hz: number };
		traces: Record<string, number[]>;
	};
	expect(['stable', 'not_settled']).toContain(data.verdict);
	expect(data.frequency.coi_nadir_hz).toBeLessThan(data.frequency.nominal_hz);
	expect(data.traces['coi:f']).toHaveLength(20);
	await expect(page.getByTestId('dynamics-panel').getByText('Run by the agent')).toBeVisible();
});
