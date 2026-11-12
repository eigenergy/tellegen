import type { Page } from '@playwright/test';
import { expect } from '@playwright/test';
import { schemaErrors } from '@tellegen/webmcp';
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
	'export_case',
	'focus_network',
	'import_case',
	'inspect_case',
	'inspect_webmcp_registration',
	'inspect_settings',
	'update_settings',
	'reset_settings',
	'run_calculation',
	'list_cases',
	'select_case',
	'solve_multiconductor_pf',
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
	'revise_study_goal',
	'edit_demand',
	'restore_base_case'
].sort();
export const PLANNING_TOOLS = [...BASE_TOOLS, 'propose_capacity_plan'].sort();
export const PROPOSAL_TOOLS = [...PLANNING_TOOLS, 'apply_capacity_plan'].sort();

/** A test host that follows the WebMCP algorithms tellegen depends on: names
 * are validated and unique, both schemas are serialized at registration, input
 * and output cross as JSON text, and a `toolchange` event follows each change. */
export async function installWebMcpHarness(page: Page, failTool: string | null = null) {
	await page.addInitScript((rejectedTool) => {
		type Tool = {
			name: string;
			inputSchema?: object;
			outputSchema?: object;
			annotations?: Record<string, boolean>;
			execute(input: Record<string, unknown>, options?: { signal: AbortSignal }): Promise<unknown>;
		};
		type Entry = { tool: Tool; outputSchema: string; exposedTo: string[] };
		const tools = new Map<string, Entry>();
		const roundTrip = async (pending: Promise<unknown>) => {
			// The browser serializes the resolved value to JSON text for the agent.
			const text = JSON.stringify(await pending);
			if (text === undefined) throw new TypeError('tool result is not JSON serializable');
			return JSON.parse(text);
		};
		const modelContext = new (class extends EventTarget {
			async registerTool(tool: Tool, options: { signal?: AbortSignal; exposedTo?: string[] } = {}) {
				if (tool.name === rejectedTool) throw new Error(`registration refused for ${tool.name}`);
				if (!/^[A-Za-z0-9_.-]{1,128}$/.test(tool.name))
					throw new DOMException('invalid tool name', 'InvalidStateError');
				if (tools.has(tool.name)) throw new DOMException('duplicate tool', 'InvalidStateError');
				JSON.stringify(tool.inputSchema);
				tools.set(tool.name, {
					tool,
					outputSchema: JSON.stringify(tool.outputSchema ?? {}),
					exposedTo: [...(options.exposedTo ?? [])]
				});
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
		const lookup = (name: string) => {
			const entry = tools.get(name);
			if (!entry) throw new Error(`tool ${name} is not registered`);
			return entry;
		};
		Object.defineProperty(window, '__tellegenWebMcpTest', {
			value: {
				list: () => [...tools.keys()].sort(),
				describe: (name: string) => {
					const entry = lookup(name);
					return {
						outputSchema: JSON.parse(entry.outputSchema),
						annotations: entry.tool.annotations ?? {},
						exposedTo: entry.exposedTo
					};
				},
				execute: async (name: string, input: Record<string, unknown>) =>
					roundTrip(lookup(name).tool.execute(JSON.parse(JSON.stringify(input)))),
				executeAborted: async (name: string, input: Record<string, unknown>, delayMs: number) => {
					const { tool } = lookup(name);
					const controller = new AbortController();
					const pending = tool.execute(input, { signal: controller.signal });
					setTimeout(() => controller.abort(), delayMs);
					return roundTrip(pending);
				}
			}
		});
	}, failTool);
}

export function describeTool(
	page: Page,
	name: string
): Promise<{
	outputSchema: Record<string, unknown>;
	annotations: Record<string, boolean>;
	exposedTo: string[];
}> {
	return page.evaluate(
		(toolName) =>
			(
				window as unknown as {
					__tellegenWebMcpTest: {
						describe(name: string): {
							outputSchema: Record<string, unknown>;
							annotations: Record<string, boolean>;
							exposedTo: string[];
						};
					};
				}
			).__tellegenWebMcpTest.describe(toolName),
		name
	);
}

export function listTools(page: Page): Promise<string[]> {
	return page.evaluate(() =>
		(
			window as unknown as { __tellegenWebMcpTest: { list(): string[] } }
		).__tellegenWebMcpTest.list()
	);
}

/** Call a tool the way an agent does and check the response against the
 * output schema the tool declared when it registered. */
export async function callTool(
	page: Page,
	name: string,
	input: Record<string, unknown>
): Promise<ToolResponse> {
	// Read the schema first: a call can unregister its own tool.
	const { outputSchema } = await describeTool(page, name);
	const response = await page.evaluate(
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
	expect(schemaErrors(outputSchema, response), `${name} output schema`).toEqual([]);
	return response;
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
	await expect(page.locator('.solvecard')).toContainText('DC OPF', { timeout: 60_000 });

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
