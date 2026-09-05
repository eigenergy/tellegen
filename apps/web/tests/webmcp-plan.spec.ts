import type { Page } from '@playwright/test';
import {
	capacityPlanFirstOrderError,
	capacityPlanPredictedPhiDelta
} from '../src/lib/webmcp/capacity-plan-metrics.js';
import { expect, test } from './fixtures/page-errors.js';
import type { CapacityPlanOutcomeJson } from '@tellegen/svelte';

type ToolResponse =
	| { ok: true; data: Record<string, unknown> }
	| { ok: false; error: { code: string; message: string } };

test('capacity plan metrics compare aggregate predicted and exact deltas', () => {
	const outcome = {
		baseline_phi: 20,
		final_phi: 15,
		iterations: [
			{ accepted: true, predicted_phi_delta: -2, first_order_error: 0.25 },
			{ accepted: false, predicted_phi_delta: -10, first_order_error: 9 },
			{ accepted: true, predicted_phi_delta: -4, first_order_error: 0.5 }
		]
	} as CapacityPlanOutcomeJson;

	expect(capacityPlanPredictedPhiDelta(outcome)).toBe(-6);
	expect(capacityPlanFirstOrderError(outcome)).toBe(1);
});

import {
	installWebMcpHarness,
	callTool,
	listTools,
	congestCase,
	PLANNING_TOOLS,
	PROPOSAL_TOOLS,
	CASE3_COORDS,
	CASE3_PLANNING
} from './fixtures/planning-case.js';

test('capacity planning stages a proposal that applies only after a human approves it', async ({
	page
}) => {
	await installWebMcpHarness(page);
	const { caseId, revision, sourceDigest, weightBusId, candidateIds, bindingBranchId } =
		await congestCase(page);

	const planned = await callTool(page, 'propose_capacity_plan', {
		case_id: caseId,
		expected_revision: revision,
		objective: {
			kind: 'weighted_lmp',
			weights: [{ bus_id: weightBusId, weight: 1 }]
		},
		candidates: candidateIds,
		max_increase_per_branch_mw: 15,
		budget_mw: 20,
		increment_mw: 5,
		max_changed_lines: 1,
		exact_solve_budget: 6
	});
	expect(planned.ok).toBe(true);
	if (!planned.ok) return;
	expect(sourceDigest).toMatch(/^sha256:[0-9a-f]{64}$/);
	expect(planned.data.source_digest).toBe(sourceDigest);
	expect(Number(planned.data.exact_solves)).toBeLessThanOrEqual(6);
	const study = await callTool(page, 'inspect_study', {});
	expect(study).toMatchObject({
		ok: true,
		data: { id: planned.data.study_id, experiment_count: 2 }
	});
	expect(JSON.stringify(planned).length).toBeLessThanOrEqual(1_450);
	const proposalId = String(planned.data.proposal_id);
	expect(proposalId).not.toBe('null');
	const proposal = planned.data.proposal as Array<{ branch_id: string; delta_mw: number }>;
	expect(proposal.length).toBeGreaterThan(0);
	expect(proposal.map((row) => row.branch_id)).toContain(bindingBranchId);
	const exactPhiDelta = Number(planned.data.exact_phi_delta);
	const predictedPhiDelta = Number(planned.data.predicted_phi_delta);
	expect(exactPhiDelta).toBeLessThan(0);
	expect(Number(planned.data.first_order_error)).toBeCloseTo(
		Math.abs(exactPhiDelta - predictedPhiDelta),
		3
	);

	// The staged proposal registers apply and renders a reviewable card.
	await expect.poll(() => listTools(page)).toEqual(PROPOSAL_TOOLS);
	const card = page.locator('[data-testid="capacity-plan-card"]').first();
	await expect(card).toBeVisible();
	await expect(card.locator('[data-testid="capacity-plan-status"]')).toHaveText('pending');
	for (const change of proposal) {
		await expect(card.locator('.changes')).toContainText(change.branch_id);
	}

	const cancelledPlan = await page.evaluate(
		async ({ caseId, revision, weightBusId, candidateIds }) =>
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
				'propose_capacity_plan',
				{
					case_id: caseId,
					expected_revision: revision,
					objective: {
						kind: 'weighted_lmp',
						weights: [{ bus_id: weightBusId, weight: 1 }]
					},
					candidates: candidateIds,
					max_increase_per_branch_mw: 15,
					budget_mw: 20,
					increment_mw: 5,
					max_changed_lines: 1,
					exact_solve_budget: 32
				},
				0
			),
		{ caseId, revision, weightBusId, candidateIds }
	);
	expect(cancelledPlan).toMatchObject({ ok: false, error: { code: 'CANCELLED' } });
	const afterCancelledPlan = await callTool(page, 'inspect_case', {});
	expect(afterCancelledPlan).toMatchObject({
		ok: true,
		data: {
			revision,
			staged_proposal: { proposal_id: proposalId, approved: false }
		}
	});
	await expect.poll(() => listTools(page)).toEqual(PROPOSAL_TOOLS);

	const unknown = await callTool(page, 'apply_capacity_plan', {
		case_id: caseId,
		expected_revision: revision,
		proposal_id: 'prop-nonexistent'
	});
	expect(unknown).toMatchObject({ ok: false, error: { code: 'UNKNOWN_PROPOSAL' } });

	const unapproved = await callTool(page, 'apply_capacity_plan', {
		case_id: caseId,
		expected_revision: revision,
		proposal_id: proposalId
	});
	expect(unapproved).toMatchObject({ ok: false, error: { code: 'APPROVAL_REQUIRED' } });

	await card.locator('[data-testid="capacity-plan-approve"]').click();
	await expect(card.locator('[data-testid="capacity-plan-approved"]')).toBeVisible();

	const cancelled = await page.evaluate(
		async ({ caseId, revision, proposalId }) =>
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
				'apply_capacity_plan',
				{ case_id: caseId, expected_revision: revision, proposal_id: proposalId },
				10
			),
		{ caseId, revision, proposalId }
	);
	expect(cancelled).toMatchObject({ ok: false, error: { code: 'CANCELLED' } });
	const afterCancel = await callTool(page, 'inspect_case', {});
	expect(afterCancel).toMatchObject({
		ok: true,
		data: {
			revision,
			staged_proposal: { proposal_id: proposalId, approved: true }
		}
	});
	if (!afterCancel.ok) return;
	await expect.poll(() => listTools(page)).toEqual(PROPOSAL_TOOLS);
	const branchesBeforeFailure = await callTool(page, 'query_network', {
		case_id: caseId,
		element_kind: 'branch',
		sort_by: 'id',
		limit: 10
	});
	expect(branchesBeforeFailure.ok).toBe(true);

	// Fail the isolated exact solve after its Study is created. The retained
	// interactive Study remains intact, and the same approval must still work.
	const failedApply = await page.evaluate(
		async ({ caseId, revision, proposalId }) => {
			const harness = (
				window as unknown as {
					__tellegenWebMcpTest: {
						execute(name: string, input: Record<string, unknown>): Promise<ToolResponse>;
					};
				}
			).__tellegenWebMcpTest;
			const NativeWorker = window.Worker;
			let injected = false;
			const FaultingWorker = new Proxy(NativeWorker, {
				construct(Target, args) {
					const worker = Reflect.construct(Target, args) as Worker;
					const postMessage = worker.postMessage.bind(worker);
					worker.postMessage = ((message: unknown) => {
						const request = message as { id?: number; op?: string };
						if (!injected && request.op === 'study_replace_edits') {
							injected = true;
							queueMicrotask(() => {
								worker.onmessage?.(
									new MessageEvent('message', {
										data: {
											id: request.id,
											ok: false,
											error: 'injected exact solve failure'
										}
									})
								);
							});
							return;
						}
						postMessage(message);
					}) as typeof worker.postMessage;
					return worker;
				}
			});
			Object.defineProperty(window, 'Worker', {
				configurable: true,
				writable: true,
				value: FaultingWorker
			});
			try {
				return await harness.execute('apply_capacity_plan', {
					case_id: caseId,
					expected_revision: revision,
					proposal_id: proposalId
				});
			} finally {
				Object.defineProperty(window, 'Worker', {
					configurable: true,
					writable: true,
					value: NativeWorker
				});
			}
		},
		{ caseId, revision, proposalId }
	);
	expect(failedApply).toMatchObject({
		ok: false,
		error: { code: 'TOOL_FAILED', message: expect.stringContaining('exact solve failure') }
	});
	const afterFailedApply = await callTool(page, 'inspect_case', {});
	expect(afterFailedApply).toMatchObject({
		ok: true,
		data: {
			revision,
			edits: afterCancel.data.edits,
			solution: afterCancel.data.solution,
			staged_proposal: { proposal_id: proposalId, approved: true }
		}
	});
	const branchesAfterFailure = await callTool(page, 'query_network', {
		case_id: caseId,
		element_kind: 'branch',
		sort_by: 'id',
		limit: 10
	});
	expect(branchesAfterFailure).toEqual(branchesBeforeFailure);
	await expect.poll(() => listTools(page)).toEqual(PROPOSAL_TOOLS);
	await expect(card.locator('[data-testid="capacity-plan-approved"]')).toBeVisible();

	const applied = await callTool(page, 'apply_capacity_plan', {
		case_id: caseId,
		expected_revision: revision,
		proposal_id: proposalId
	});
	expect(applied.ok).toBe(true);
	if (!applied.ok) return;
	expect(applied.data.source_digest).toBe(sourceDigest);
	expect(applied.data.case_updated).toBe(true);
	const savedStudy = await callTool(page, 'inspect_study', {});
	expect(savedStudy.ok).toBe(true);
	if (!savedStudy.ok) return;
	expect(savedStudy.data.applied_state).toBe(savedStudy.data.recommended_state);

	expect(String(applied.data.revision)).not.toBe(revision);
	const before = applied.data.before as { objective: number };
	const after = applied.data.after as { objective: number };
	expect(after.objective).toBeLessThan(before.objective);

	// The proposal is consumed: apply deregisters and the card resolves.
	await expect.poll(() => listTools(page)).toEqual(PLANNING_TOOLS);
	await expect(card.locator('[data-testid="capacity-plan-status"]')).toHaveText('applied');
	await page.reload();
	const resumedStudy = await callTool(page, 'inspect_study', { study_id: planned.data.study_id });
	expect(resumedStudy).toMatchObject({
		ok: true,
		data: { applied_state: savedStudy.data.applied_state }
	});
});

test('a case edit expires the staged proposal and drops apply_capacity_plan', async ({ page }) => {
	await installWebMcpHarness(page);
	const { caseId, revision, weightBusId, candidateIds } = await congestCase(page);

	const planned = await callTool(page, 'propose_capacity_plan', {
		case_id: caseId,
		expected_revision: revision,
		objective: {
			kind: 'weighted_lmp',
			weights: [{ bus_id: weightBusId, weight: 1 }]
		},
		candidates: candidateIds,
		max_increase_per_branch_mw: 15,
		budget_mw: 20,
		increment_mw: 5,
		max_changed_lines: 1,
		exact_solve_budget: 6
	});
	expect(planned.ok).toBe(true);
	if (!planned.ok) return;
	const proposalId = String(planned.data.proposal_id);
	await expect.poll(() => listTools(page)).toEqual(PROPOSAL_TOOLS);

	// Even an approval granted before the edit must not survive it.
	const card = page.locator('[data-testid="capacity-plan-card"]').first();
	await card.locator('[data-testid="capacity-plan-approve"]').click();

	const edited = await callTool(page, 'update_case', {
		case_id: caseId,
		expected_revision: revision,
		mode: 'increment',
		demand: [{ bus_id: weightBusId, delta_mw: 2 }]
	});
	expect(edited.ok).toBe(true);
	if (!edited.ok) return;
	expect(String(edited.data.revision)).not.toBe(revision);

	// The revision change expires the proposal, resolves the card, and
	// deregisters apply; a replay attempt no longer finds the tool.
	await expect.poll(() => listTools(page)).toEqual(PLANNING_TOOLS);
	await expect(card.locator('[data-testid="capacity-plan-status"]')).toHaveText('expired');
	const replay = await page.evaluate(
		async ({ args }) => {
			const harness = (
				window as unknown as {
					__tellegenWebMcpTest: {
						execute(name: string, input: Record<string, unknown>): Promise<unknown>;
					};
				}
			).__tellegenWebMcpTest;
			try {
				return { outcome: 'responded', value: await harness.execute('apply_capacity_plan', args) };
			} catch (error) {
				return { outcome: 'threw', message: String(error) };
			}
		},
		{ args: { case_id: caseId, expected_revision: revision, proposal_id: proposalId } }
	);
	expect(replay.outcome).toBe('threw');
	expect(String(replay.message)).toContain('not registered');
});

test('switching cases expires the staged proposal and its approval', async ({ page }) => {
	await installWebMcpHarness(page);
	const { caseId, revision, weightBusId, candidateIds } = await congestCase(page);
	const planned = await callTool(page, 'propose_capacity_plan', {
		case_id: caseId,
		expected_revision: revision,
		objective: {
			kind: 'weighted_lmp',
			weights: [{ bus_id: weightBusId, weight: 1 }]
		},
		candidates: candidateIds,
		max_increase_per_branch_mw: 15,
		budget_mw: 20,
		increment_mw: 5,
		max_changed_lines: 1,
		exact_solve_budget: 6
	});
	expect(planned.ok).toBe(true);
	if (!planned.ok) return;
	const proposalId = String(planned.data.proposal_id);
	await expect.poll(() => listTools(page)).toEqual(PROPOSAL_TOOLS);
	const card = page.locator('[data-testid="capacity-plan-card"]').first();
	await card.locator('[data-testid="capacity-plan-approve"]').click();
	await expect(card.locator('[data-testid="capacity-plan-approved"]')).toBeVisible();

	await page.locator('input[type="file"]').setInputFiles([
		{ name: 'case3-copy-coords.csv', mimeType: 'text/csv', buffer: Buffer.from(CASE3_COORDS) },
		{ name: 'case3-copy.m', mimeType: 'text/plain', buffer: Buffer.from(CASE3_PLANNING) }
	]);
	await expect(page.locator('.case-chip.local')).toHaveCount(2);
	await expect
		.poll(async () => {
			const tools = await listTools(page);
			return tools.includes('apply_capacity_plan');
		})
		.toBe(false);
	await expect(card.locator('[data-testid="capacity-plan-status"]')).toHaveText('expired');
	const switched = await callTool(page, 'inspect_case', {});
	expect(switched.ok).toBe(true);
	if (!switched.ok) return;
	expect(String(switched.data.case_id)).not.toBe(caseId);
	expect(switched.data.staged_proposal).toBeNull();

	// Returning to the original case cannot revive the proposal or approval.
	await page.locator('.case-chip.local .case-activate').first().click();
	const returned = await callTool(page, 'inspect_case', {});
	expect(returned).toMatchObject({
		ok: true,
		data: { case_id: caseId, staged_proposal: null }
	});
	await expect
		.poll(async () => {
			const tools = await listTools(page);
			return tools.includes('apply_capacity_plan');
		})
		.toBe(false);

	const replay = await page.evaluate(
		async ({ args }) => {
			const harness = (
				window as unknown as {
					__tellegenWebMcpTest: {
						execute(name: string, input: Record<string, unknown>): Promise<unknown>;
					};
				}
			).__tellegenWebMcpTest;
			try {
				return { outcome: 'responded', value: await harness.execute('apply_capacity_plan', args) };
			} catch (error) {
				return { outcome: 'threw', message: String(error) };
			}
		},
		{ args: { case_id: caseId, expected_revision: revision, proposal_id: proposalId } }
	);
	expect(replay.outcome).toBe('threw');
	expect(String(replay.message)).toContain('not registered');
});

test('a revised Study goal expires the capacity compatibility approval', async ({ page }) => {
	await installWebMcpHarness(page);
	const { caseId, revision, weightBusId, candidateIds } = await congestCase(page);
	const plan = await callTool(page, 'propose_capacity_plan', {
		case_id: caseId,
		expected_revision: revision,
		objective: { kind: 'weighted_lmp', weights: [{ bus_id: weightBusId, weight: 1 }] },
		candidates: candidateIds,
		max_increase_per_branch_mw: 15,
		budget_mw: 20,
		increment_mw: 5,
		max_changed_lines: 1,
		exact_solve_budget: 6
	});
	if (!plan.ok) throw new Error(plan.error.message);
	const card = page.locator('[data-testid="capacity-plan-card"]').first();
	await card.locator('[data-testid="capacity-plan-approve"]').click();
	const summary = await callTool(page, 'inspect_study', {});
	if (!summary.ok) throw new Error(summary.error.message);
	let offset: number | null = 0,
		serialized = '';
	while (offset !== null) {
		const part = await callTool(page, 'inspect_study', {
			section: 'goal',
			offset,
			expected_revision: summary.data.revision
		});
		if (!part.ok) throw new Error(part.error.message);
		serialized += part.data.fragment;
		offset = part.data.next_offset as number | null;
	}
	const goal = JSON.parse(serialized);
	goal.parent = summary.data.active_goal;
	goal.request = 'Evaluate a revised weighted price goal';
	goal.objective.weights[0].weight = 2;
	const revised = await callTool(page, 'revise_study_goal', {
		study_id: summary.data.id,
		expected_revision: summary.data.revision,
		operation: { kind: 'revise_goal', goal }
	});
	expect(revised.ok).toBe(true);
	await expect(card.locator('[data-testid="capacity-plan-status"]')).toHaveText('expired');
	await expect
		.poll(async () => (await listTools(page)).includes('apply_capacity_plan'))
		.toBe(false);
});
