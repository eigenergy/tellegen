import type { CapacityPlanOutcomeJson, CapacityPlanSpecJson } from '@tellegen/svelte';
import type { StudyBundle, StudyView } from '@tellegen/engine';
import type { GoalDraft } from './workspace.svelte.js';

export interface CapacityStudyBinding {
	studyId: string;
	revision: number;
	proposal: string;
	goal: string;
	base: string;
	state: string;
}

export function capacityGoal(spec: CapacityPlanSpecJson): GoalDraft {
	return {
		title: 'Capacity upgrade study',
		formulation: 'dcopf',
		request: 'Lower the specified weighted marginal prices within the capacity upgrade limits.',
		interpretation:
			'Minimize the weighted active-power prices. Branch rating changes use MW relative to the captured starting point.',
		objective: {
			kind: 'weighted_observable',
			operand: { Price: 'Active' },
			weights: spec.objective.weights.map((w) => ({ element: w.bus, weight: w.weight }))
		},
		decisions: {
			variables: spec.candidates.map((element) => ({
				id: element,
				element,
				intervention: 'branch_rating',
				lower: 0,
				upper: spec.max_increase_per_branch_mw,
				increment: spec.increment_mw,
				budget_weight: 1
			})),
			total_budget: spec.budget_mw,
			max_changed_elements: spec.max_changed_lines,
			demand: null
		}
	};
}

/** Preserve the capacity tool's response shape while the Study owns exact trials. */
export function capacityOutcome(
	bundle: StudyBundle,
	proposal: string,
	spec: CapacityPlanSpecJson
): CapacityPlanOutcomeJson {
	const d = bundle.document,
		record = d.experiments[proposal];
	if (!record?.start_state || !record.goal) throw new Error('Capacity Study has no starting state');
	const view = (id: string) => JSON.parse(bundle.artifacts[d.states[id].view].text) as StudyView;
	const phi = (id: string) => {
		const prices = new Map(view(id).lmp?.map((x) => [x.bus, x.value]));
		return spec.objective.weights.reduce((sum, w) => {
			const value = prices.get(w.bus);
			if (value === undefined) throw new Error(`Saved price is unavailable at bus ${w.bus}`);
			return sum + value * w.weight;
		}, 0);
	};
	const evidence = record.evidence.map((id) => JSON.parse(bundle.artifacts[id].text));
	const directions = evidence.find((e) => Array.isArray(e.directions))?.directions ?? [];
	const baseline = phi(record.start_state);
	let parentValue = baseline,
		parentChanges = spec.candidates.map(() => 0),
		iteration = -1;
	let acceptedValue = baseline,
		acceptedChanges = parentChanges,
		acceptedSolve = 1;
	const iterations = record.trials.map((trial, index) => {
		const details = JSON.parse(bundle.artifacts[trial.evidence[0]].text);
		if (details.iteration !== iteration) {
			iteration = details.iteration;
			parentValue = acceptedValue;
			parentChanges = acceptedChanges;
		}
		const predicted = (trial.predicted_value ?? parentValue) - parentValue;
		const exact = trial.exact_value == null ? null : trial.exact_value - parentValue;
		const error = exact === null ? null : Math.abs(exact - predicted);
		if (trial.accepted) {
			acceptedValue = trial.exact_value!;
			acceptedChanges = trial.changes;
			acceptedSolve = index + 3;
		}
		return {
			gradient: (directions[iteration]?.gradient ?? []).map(
				(g: { decision: string; value: number }) => ({ branch: g.decision, value: g.value })
			),
			delta_mw: spec.candidates
				.map((branch, i) => ({ branch, delta_mw: trial.changes[i] - parentChanges[i] }))
				.filter((x) => Math.abs(x.delta_mw) > 1e-9),
			predicted_phi_delta: predicted,
			exact_phi_delta: exact,
			first_order_error: error,
			first_order_error_rel: error === null || exact === 0 ? null : error / Math.abs(exact!),
			accepted: trial.accepted,
			reason:
				trial.failure ?? (trial.accepted ? 'Exact improvement retained' : 'Candidate not selected')
		};
	});
	const selected = d.recommended_state ?? record.start_state;
	return {
		baseline: {
			phi: baseline,
			declared_objective: view(record.start_state).objective ?? 0,
			exact_solve: 1
		},
		exact_verified_result: {
			phi: phi(selected),
			declared_objective: view(selected).objective ?? 0,
			exact_solve: acceptedSolve
		},
		baseline_phi: baseline,
		final_phi: phi(selected),
		proposal: spec.candidates
			.map((branch, i) => ({ branch, delta_mw: acceptedChanges[i] }))
			.filter((x) => Math.abs(x.delta_mw) > 1e-9),
		spent_budget_mw: acceptedChanges.reduce((sum, x) => sum + Math.abs(x), 0),
		iterations,
		exact_solves: record.solve_count + 1
	};
}
