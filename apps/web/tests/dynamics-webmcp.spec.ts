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
	type Trace = { t_s: number[]; v: number[] };
	const data = r.data as {
		verdict: string;
		frequency: { coi_nadir_hz: number; nadir_t_s: number; nominal_hz: number };
		traces: Record<string, Trace>;
	};
	expect(['stable', 'not_settled']).toContain(data.verdict);
	expect(data.frequency.coi_nadir_hz).toBeLessThan(data.frequency.nominal_hz);
	const coi = data.traces['coi:f'];
	expect(coi.v).toHaveLength(20);
	expect(coi.t_s).toHaveLength(20);
	// The trace holds the nadir the summary reports, at its time.
	expect(Math.min(...coi.v)).toBe(data.frequency.coi_nadir_hz);
	expect(coi.t_s[coi.v.indexOf(data.frequency.coi_nadir_hz)]).toBe(data.frequency.nadir_t_s);
	await expect(page.getByTestId('dynamics-panel').getByText('Run by the agent')).toBeVisible();

	// A fault: the voltage trace of the lowest bus holds the dip min_pu reports.
	const fault = {
		case_id,
		expected_revision: revision,
		events: [{ kind: 'bus_fault', bus: 1101, at_s: 1, clear_after_s: 0.1 }],
		t_end_s: 4,
		show_in_app: false
	};
	const first = await callTool(page, 'simulate_dynamics', fault);
	if (!first.ok) throw new Error(first.error.message);
	const lowest = (first.data as { voltage: { min_bus: number } }).voltage.min_bus;
	const traced = await callTool(page, 'simulate_dynamics', {
		...fault,
		channels: [`vm:${lowest}`],
		trace_points: 24
	});
	if (!traced.ok) throw new Error(traced.error.message);
	expect(JSON.stringify(traced).length).toBeLessThanOrEqual(1450);
	const faulted = traced.data as { voltage: { min_pu: number }; traces: Record<string, Trace> };
	const vm = faulted.traces[`vm:${lowest}`];
	expect(Math.min(...vm.v)).toBe(faulted.voltage.min_pu);
	// The samples around the fault and its clearing are there.
	for (const t of [1, 1.1]) expect(vm.t_s).toContain(t);
	// A request past the cap is refused before the run.
	const tooMany = await callTool(page, 'simulate_dynamics', {
		...fault,
		channels: ['coi:f', `vm:${lowest}`],
		trace_points: 13
	});
	expect(tooMany).toMatchObject({ ok: false, error: { code: 'INVALID_INPUT' } });
});
