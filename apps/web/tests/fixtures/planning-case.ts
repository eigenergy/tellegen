import type { Page } from '@playwright/test';
import { expect } from '@playwright/test';
export type ToolResponse =
	| { ok: true; data: Record<string, unknown> }
	| { ok: false; error: { code: string; message: string } };

export const CASE3_PLANNING = `function mpc = case3test
mpc.version = '2';
mpc.baseMVA = 100;
mpc.bus = [
 1 3 0  0  0 0 1 1 0 230 1 1.1 0.9;
 2 1 90 30 0 0 1 1 0 230 1 1.1 0.9;
 3 2 0  0  0 0 1 1 0 230 1 1.1 0.9;
];
mpc.gen = [
 1 0  0 300 -300 1 100 1 250 10 0 0 0 0 0 0 0 0 0 0 0;
 3 60 0 300 -300 1 100 1 270 10 0 0 0 0 0 0 0 0 0 0 0;
];
mpc.branch = [
 1 2 0.01 0.1 0 250 250 250 0 0 1 -360 360;
 1 3 0.01 0.1 0 250 250 250 0 0 1 -360 360;
 2 3 0.01 0.1 0 60 60 60 0 0 1 -360 360;
];
mpc.gencost = [
 2 0 0 3 0.11  5   0;
 2 0 0 3 0.085 1.2 0;
];
`;

export const CASE3_COORDS = `bus_i,lat,lon
1,34.0,-81.1
2,34.1,-81.0
3,34.0,-80.9
`;

export const BASE_TOOLS = [
	'analyze_sensitivity',
	'focus_network',
	'inspect_case',
	'preview_case_update',
	'query_network',
	'reset_case',
	'update_case',
	'branch_study',
	'compare_study_states',
	'create_study',
	'inspect_study',
	'propose_study',
	'record_study_evidence',
	'revise_study_goal'
].sort();
export const PLANNING_TOOLS = [...BASE_TOOLS, 'propose_capacity_plan'].sort();
export const PROPOSAL_TOOLS = [...PLANNING_TOOLS, 'apply_capacity_plan'].sort();

export async function installWebMcpHarness(page: Page) {
	await page.addInitScript(() => {
		type Tool = {
			name: string;
			execute(input: Record<string, unknown>, options?: { signal: AbortSignal }): Promise<unknown>;
		};
		const tools = new Map<string, Tool>();
		const modelContext = new (class extends EventTarget {
			async registerTool(tool: Tool, options: { signal?: AbortSignal } = {}) {
				if (tools.has(tool.name)) throw new DOMException('duplicate tool', 'InvalidStateError');
				tools.set(tool.name, tool);
				options.signal?.addEventListener(
					'abort',
					() => {
						tools.delete(tool.name);
						this.dispatchEvent(new Event('toolchange'));
					},
					{ once: true }
				);
				this.dispatchEvent(new Event('toolchange'));
			}
		})();
		Object.defineProperty(Document.prototype, 'modelContext', {
			configurable: true,
			get: () => modelContext
		});
		Object.defineProperty(window, '__tellegenWebMcpTest', {
			value: {
				list: () => [...tools.keys()].sort(),
				execute: async (name: string, input: Record<string, unknown>) => {
					const tool = tools.get(name);
					if (!tool) throw new Error(`tool ${name} is not registered`);
					return tool.execute(input);
				},
				executeAborted: async (name: string, input: Record<string, unknown>, delayMs: number) => {
					const tool = tools.get(name);
					if (!tool) throw new Error(`tool ${name} is not registered`);
					const controller = new AbortController();
					const pending = tool.execute(input, { signal: controller.signal });
					setTimeout(() => controller.abort(), delayMs);
					return pending;
				}
			}
		});
	});
}

export function listTools(page: Page): Promise<string[]> {
	return page.evaluate(() =>
		(
			window as unknown as { __tellegenWebMcpTest: { list(): string[] } }
		).__tellegenWebMcpTest.list()
	);
}

export async function callTool(
	page: Page,
	name: string,
	input: Record<string, unknown>
): Promise<ToolResponse> {
	return page.evaluate(
		async ({ toolName, args }) => {
			const harness = (
				window as unknown as {
					__tellegenWebMcpTest: {
						execute(name: string, input: Record<string, unknown>): Promise<ToolResponse>;
					};
				}
			).__tellegenWebMcpTest;
			return harness.execute(toolName, args);
		},
		{ toolName: name, args: input }
	);
}

/** Load the congested 3-bus case and cut the loaded line's rating so it binds.
 * Returns the ids the planning calls need. */
export async function congestCase(page: Page): Promise<{
	caseId: string;
	revision: string;
	sourceDigest: string;
	weightBusId: string;
	candidateIds: string[];
	bindingBranchId: string;
}> {
	await page.route('**/api/compute', (route) => route.fulfill({ json: { enabled: false } }));
	await page.route('**/api/cases', (route) => {
		void route.fulfill({ json: [] });
	});
	await page.goto('/');
	await expect(page.getByText('no default cases loaded')).toBeVisible();
	await expect.poll(() => listTools(page)).toEqual(BASE_TOOLS);

	await page.locator('input[type="file"]').setInputFiles([
		{ name: 'case3-coords.csv', mimeType: 'text/csv', buffer: Buffer.from(CASE3_COORDS) },
		{ name: 'case3planning.m', mimeType: 'text/plain', buffer: Buffer.from(CASE3_PLANNING) }
	]);
	await expect(page.locator('.solvecard')).toContainText('OPF solve', { timeout: 60_000 });

	// Propose follows the case; apply needs a staged proposal.
	await expect.poll(() => listTools(page)).toEqual(PLANNING_TOOLS);

	const inspected = await callTool(page, 'inspect_case', {});
	expect(inspected.ok).toBe(true);
	if (!inspected.ok) throw new Error('inspect_case failed');
	const caseId = String(inspected.data.case_id);

	const branches = await callTool(page, 'query_network', {
		case_id: caseId,
		element_kind: 'branch',
		sort_by: 'loading',
		direction: 'desc',
		limit: 3
	});
	expect(branches.ok).toBe(true);
	if (!branches.ok) throw new Error('query_network failed');
	const rows = branches.data.elements as Array<{ element_id: string; rating_mw: number }>;
	expect(rows).toHaveLength(3);
	const binding = rows[0];
	expect(binding.rating_mw).toBe(60);

	const congested = await callTool(page, 'update_case', {
		case_id: caseId,
		expected_revision: String(inspected.data.revision),
		mode: 'increment',
		ratings: [{ branch_id: binding.element_id, delta_mw: -12 }]
	});
	expect(congested).toMatchObject({ ok: true, data: { rating_edit_count: 1 } });
	if (!congested.ok) throw new Error('update_case failed');

	const buses = await callTool(page, 'query_network', {
		case_id: caseId,
		element_kind: 'bus',
		sort_by: 'price',
		direction: 'desc',
		limit: 1
	});
	expect(buses.ok).toBe(true);
	if (!buses.ok) throw new Error('query_network failed');
	const topBus = (buses.data.elements as Array<{ element_id: string; legacy_id: number }>)[0];
	expect(topBus.legacy_id).toBe(2);

	return {
		caseId,
		revision: String(congested.data.revision),
		sourceDigest: String(inspected.data.source_digest),
		weightBusId: topBus.element_id,
		candidateIds: rows.map((row) => row.element_id),
		bindingBranchId: binding.element_id
	};
}
