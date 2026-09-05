import type { CapacityPlanOutcomeJson } from '@tellegen/svelte';

export function capacityPlanExactPhiDelta(outcome: CapacityPlanOutcomeJson): number {
	return outcome.final_phi - outcome.baseline_phi;
}

export function capacityPlanPredictedPhiDelta(outcome: CapacityPlanOutcomeJson): number {
	return outcome.iterations
		.filter((iteration) => iteration.accepted)
		.reduce((sum, iteration) => sum + iteration.predicted_phi_delta, 0);
}

export function capacityPlanFirstOrderError(outcome: CapacityPlanOutcomeJson): number | null {
	if (!outcome.iterations.some((iteration) => iteration.accepted)) return null;
	return Math.abs(capacityPlanExactPhiDelta(outcome) - capacityPlanPredictedPhiDelta(outcome));
}
