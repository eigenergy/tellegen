import type { Page } from '@playwright/test';
import { expect, test } from './fixtures/page-errors.js';
import { CASE14 } from '../../../examples/browser-minimal/src/case14';
import { CASE14_COORDS } from './fixtures/local-case';

type ToolResponse =
	| { ok: true; data: Record<string, unknown> }
	| { ok: false; error: { code: string; message: string } };

async function installWebMcpHarness(page: Page, failTool: string | null = null) {
	await page.addInitScript((rejectedTool) => {
		type Tool = {
			name: string;
			execute(input: Record<string, unknown>, options?: { signal: AbortSignal }): Promise<unknown>;
		};
		const tools = new Map<string, Tool>();
		const modelContext = new (class extends EventTarget {
			async registerTool(tool: Tool, options: { signal?: AbortSignal } = {}) {
				if (tool.name === rejectedTool) throw new Error(`registration refused for ${tool.name}`);
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
	}, failTool);
}

async function callTool(
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

test('headless tools inspect, query, focus, preview, mutate, and reject a stale replay', async ({
	page
}) => {
	await installWebMcpHarness(page);
	await page.route('**/api/cases', (route) => {
		void route.fulfill({ json: [] });
	});

	await page.goto('/');
	await expect(page.getByText('no default cases loaded')).toBeVisible();
	await expect
		.poll(() =>
			page.evaluate(() =>
				(
					window as unknown as { __tellegenWebMcpTest: { list(): string[] } }
				).__tellegenWebMcpTest.list()
			)
		)
		.toEqual([
			'analyze_sensitivity',
			'focus_network',
			'inspect_case',
			'preview_case_update',
			'query_network',
			'reset_case',
			'update_case'
		]);
	await expect(page.locator('html')).toHaveAttribute('data-webmcp', 'ready');

	await page.locator('input[type="file"]').setInputFiles([
		{
			name: 'case14-coords.csv',
			mimeType: 'text/csv',
			buffer: Buffer.from(CASE14_COORDS)
		},
		{
			name: 'case14synthetic.m',
			mimeType: 'text/plain',
			buffer: Buffer.from(CASE14)
		}
	]);
	await expect(page.locator('.solvecard')).toContainText('OPF solve', { timeout: 60_000 });

	const inspected = await callTool(page, 'inspect_case', {});
	expect(inspected.ok).toBe(true);
	if (!inspected.ok) return;
	const caseId = String(inspected.data.case_id);
	const sessionId = String(inspected.data.session_id);
	const sourceDigest = String(inspected.data.source_digest);
	const revision = String(inspected.data.revision);
	expect(sessionId).toMatch(/^session-/);
	expect(sourceDigest).toMatch(/^sha256:[0-9a-f]{64}$/);
	expect(revision.startsWith(`${sessionId}:r`)).toBe(true);
	expect(Number.parseInt(revision.slice(`${sessionId}:r`.length), 10)).toBeGreaterThan(0);
	expect(JSON.stringify(inspected).length).toBeLessThanOrEqual(1_450);
	await expect(page.locator('[data-webmcp-activity="open"]')).toBeVisible();
	await expect(page.getByText('Inspect active case')).toBeVisible();

	const queried = await callTool(page, 'query_network', {
		case_id: caseId,
		element_kind: 'bus',
		sort_by: 'demand_mw',
		limit: 1
	});
	expect(queried.ok).toBe(true);
	if (!queried.ok) return;
	const elements = queried.data.elements as Array<{ element_id: string }>;
	const busId = elements[0].element_id;

	const focused = await callTool(page, 'focus_network', {
		case_id: caseId,
		target: { kind: 'bus', element_id: busId }
	});
	expect(focused).toMatchObject({ ok: true, data: { sensitivity_loaded: true } });
	if (!focused.ok) return;
	const focusedRevision = String(focused.data.revision);
	expect(focusedRevision).not.toBe(revision);
	await expect(page.locator('.chip', { hasText: '∂value/∂d' })).toBeVisible();

	const previewed = await callTool(page, 'preview_case_update', {
		case_id: caseId,
		expected_revision: focusedRevision,
		mode: 'increment',
		demand: [{ bus_id: busId, delta_mw: 5 }]
	});
	expect(previewed).toMatchObject({
		ok: true,
		data: { committed: false, revision: focusedRevision }
	});
	if (!previewed.ok) return;
	expect(JSON.stringify(previewed).length).toBeLessThanOrEqual(1_450);
	await expect(page.getByText('predicted Δ objective')).toBeVisible();

	const cancelled = await page.evaluate(
		async ({ caseId, focusedRevision, busId }) =>
			(
				window as unknown as {
					__tellegenWebMcpTest: {
						executeAborted(
							name: string,
							input: Record<string, unknown>,
							delayMs: number
						): Promise<ToolResponse>;
					};
				}
			).__tellegenWebMcpTest.executeAborted(
				'update_case',
				{
					case_id: caseId,
					expected_revision: focusedRevision,
					mode: 'increment',
					demand: [{ bus_id: busId, delta_mw: 5 }]
				},
				10
			),
		{ caseId, focusedRevision, busId }
	);
	expect(cancelled).toMatchObject({ ok: false, error: { code: 'CANCELLED' } });
	const afterCancel = await callTool(page, 'inspect_case', {});
	expect(afterCancel).toMatchObject({
		ok: true,
		data: {
			session_id: sessionId,
			revision: focusedRevision,
			edits: { demand_count: 0, rating_count: 0 }
		}
	});

	const infeasible = await callTool(page, 'update_case', {
		case_id: caseId,
		expected_revision: focusedRevision,
		mode: 'set',
		demand: Array.from({ length: 14 }, (_, index) => ({
			bus_id: String(index + 1),
			delta_mw: 50
		}))
	});
	expect(infeasible).toMatchObject({ ok: false });
	const afterFailure = await callTool(page, 'inspect_case', {});
	expect(afterFailure).toMatchObject({
		ok: true,
		data: {
			session_id: sessionId,
			revision: focusedRevision,
			edits: { demand_count: 0, rating_count: 0 }
		}
	});

	const raced = await page.evaluate(
		async ({ caseId, revision, busId }) => {
			const harness = (
				window as unknown as {
					__tellegenWebMcpTest: {
						execute(name: string, input: Record<string, unknown>): Promise<ToolResponse>;
					};
				}
			).__tellegenWebMcpTest;
			const update = (deltaMw: number) =>
				harness.execute('update_case', {
					case_id: caseId,
					expected_revision: revision,
					mode: 'increment',
					demand: [{ bus_id: busId, delta_mw: deltaMw }]
				});
			return Promise.all([update(5), update(7)]);
		},
		{ caseId, revision: focusedRevision, busId }
	);
	const updated = raced.find((response) => response.ok);
	const refused = raced.find((response) => !response.ok);
	expect(refused).toMatchObject({ ok: false, error: { code: 'STALE_REVISION' } });
	expect(updated).toBeDefined();
	if (!updated) return;
	expect(updated).toMatchObject({ ok: true, data: { demand_edit_count: 1 } });
	if (!updated.ok) return;
	expect(String(updated.data.revision)).not.toBe(focusedRevision);
	expect(JSON.stringify(updated).length).toBeLessThanOrEqual(1_450);
	await expect(page.getByText('exact result')).toBeVisible();
	await expect(page.getByText('binding lines')).toBeVisible();

	// Tool deltas are exact values, not slider gestures. A valid update below
	// the UI slider's deadband must survive the transactional solve.
	const branchQuery = await callTool(page, 'query_network', {
		case_id: caseId,
		element_kind: 'branch',
		sort_by: 'loading',
		limit: 1
	});
	expect(branchQuery.ok).toBe(true);
	if (!branchQuery.ok) return;
	const branchId = String(
		(branchQuery.data.elements as Array<{ element_id: string }>)[0].element_id
	);
	const smallRating = await callTool(page, 'update_case', {
		case_id: caseId,
		expected_revision: String(updated.data.revision),
		mode: 'set',
		ratings: [{ branch_id: branchId, delta_mw: 0.1 }]
	});
	expect(smallRating).toMatchObject({ ok: true, data: { rating_edit_count: 1 } });

	const replay = await callTool(page, 'update_case', {
		case_id: caseId,
		expected_revision: revision,
		mode: 'increment',
		demand: [{ bus_id: busId, delta_mw: 5 }]
	});
	expect(replay).toMatchObject({ ok: false, error: { code: 'STALE_REVISION' } });
});

test('a full page reload creates a new WebMCP session id', async ({ page }) => {
	await installWebMcpHarness(page);
	await page.route('**/api/cases', (route) => {
		void route.fulfill({ json: [] });
	});
	await page.goto('/');
	await expect(page.getByText('no default cases loaded')).toBeVisible();
	const first = await callTool(page, 'inspect_case', {});
	expect(first.ok).toBe(true);
	if (!first.ok) return;
	expect(String(first.data.session_id)).toMatch(/^session-/);

	await page.reload();
	await expect(page.getByText('no default cases loaded')).toBeVisible();
	const second = await callTool(page, 'inspect_case', {});
	expect(second.ok).toBe(true);
	if (!second.ok) return;
	expect(String(second.data.session_id)).toMatch(/^session-/);
	expect(String(second.data.session_id)).not.toBe(String(first.data.session_id));
});

test('inspect_case refuses a snapshot whose revision changes while its source is read', async ({
	page
}) => {
	await installWebMcpHarness(page);
	await page.route('**/api/cases', (route) => {
		void route.fulfill({ json: [] });
	});
	await page.goto('/');
	await expect(page.getByText('no default cases loaded')).toBeVisible();
	await page.locator('input[type="file"]').setInputFiles([
		{
			name: 'case14-coords.csv',
			mimeType: 'text/csv',
			buffer: Buffer.from(CASE14_COORDS)
		},
		{
			name: 'case14synthetic.m',
			mimeType: 'text/plain',
			buffer: Buffer.from(CASE14)
		}
	]);
	await expect(page.locator('.solvecard')).toContainText('OPF solve', { timeout: 60_000 });

	const raced = await page.evaluate(async () => {
		const harness = (
			window as unknown as {
				__tellegenWebMcpTest: {
					execute(name: string, input: Record<string, unknown>): Promise<ToolResponse>;
				};
			}
		).__tellegenWebMcpTest;
		const pending = harness.execute('inspect_case', {});
		const formulation = document.querySelector<HTMLSelectElement>('#formulation-select');
		if (!formulation) throw new Error('formulation control is unavailable');
		formulation.value = 'socwr';
		formulation.dispatchEvent(new Event('change', { bubbles: true }));
		return pending;
	});
	expect(raced).toMatchObject({
		ok: false,
		error: { code: 'STALE_REVISION', message: expect.stringContaining('retry inspect_case') }
	});

	const retried = await callTool(page, 'inspect_case', {});
	expect(retried).toMatchObject({ ok: true, data: { formulation: 'socwr' } });
});

test('a dynamic planning registration failure is visible in the interface', async ({ page }) => {
	await installWebMcpHarness(page, 'propose_capacity_plan');
	await page.route('**/api/cases', (route) => {
		void route.fulfill({ json: [] });
	});
	await page.goto('/');
	await expect(page.getByText('no default cases loaded')).toBeVisible();

	await page.locator('input[type="file"]').setInputFiles([
		{
			name: 'case14-coords.csv',
			mimeType: 'text/csv',
			buffer: Buffer.from(CASE14_COORDS)
		},
		{
			name: 'case14synthetic.m',
			mimeType: 'text/plain',
			buffer: Buffer.from(CASE14)
		}
	]);
	await expect(page.locator('.solvecard')).toContainText('OPF solve', { timeout: 60_000 });

	await expect(page.locator('html')).toHaveAttribute('data-webmcp', 'error');
	await expect(page.getByTestId('webmcp-registration-error')).toContainText(
		'registration refused for propose_capacity_plan'
	);
	await expect
		.poll(() =>
			page.evaluate(() =>
				(
					window as unknown as { __tellegenWebMcpTest: { list(): string[] } }
				).__tellegenWebMcpTest.list()
			)
		)
		.toEqual([
			'analyze_sensitivity',
			'focus_network',
			'inspect_case',
			'preview_case_update',
			'query_network',
			'reset_case',
			'update_case'
		]);
});
