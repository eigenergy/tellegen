import type {
	DecisionSpace,
	Network,
	NetworkBranch,
	NetworkBus,
	StudyObjective
} from '@tellegen/engine';

export type GoalBus = NetworkBus & { name?: string | null; area?: string | number | null };
export type GoalBranch = NetworkBranch & { name?: string | null };
export type EquipmentScope = 'network' | 'area' | 'selected';
export type GoalForm = {
	objective: 'price' | 'voltage';
	intervention: 'capacity' | 'redistribution' | 'placement';
	scope: EquipmentScope;
	area: string;
	buses: string[];
	candidates: string[];
	weighting: 'demand' | 'equal';
	weights: Record<string, number>;
	bounds: Record<string, { lower?: number; upper?: number }>;
	budget: number;
	increment: number;
	cardinality: number;
	increase: number;
	target: number;
};

export const elementKey = (element: { uid?: string | null; id: number }) =>
	element.uid ?? element.id;
export const selectionKey = (element: { uid?: string | null; id: number }) =>
	String(elementKey(element));
export const busLabel = (bus: GoalBus) => (bus.name ? `${bus.id}, ${bus.name}` : `Bus ${bus.id}`);
export const branchLabel = (branch: GoalBranch) =>
	`Line ${branch.id}, ${branch.from} to ${branch.to}${branch.name ? `, ${branch.name}` : ''}`;

export function defaultGoalForm(formulation: string): GoalForm {
	return {
		objective: formulation === 'acpf' ? 'voltage' : 'price',
		intervention: formulation === 'acpf' ? 'redistribution' : 'capacity',
		scope: 'network',
		area: '',
		buses: [],
		candidates: [],
		weighting: 'demand',
		weights: {},
		bounds: {},
		budget: 20,
		increment: 5,
		cardinality: 2,
		increase: 10,
		target: 1
	};
}

/** All eligible elements participate; table pagination only limits rendered rows. */
export function resolveEquipment(network: Network, form: GoalForm) {
	const allBuses = network.buses as GoalBus[];
	const inArea = new Set(
		allBuses.filter((b) => String(b.area ?? '') === form.area).map((b) => b.id)
	);
	const buses = allBuses.filter((bus) => {
		if (form.scope === 'area' && !inArea.has(bus.id)) return false;
		if (form.scope === 'selected' && !form.buses.includes(selectionKey(bus))) return false;
		return form.objective === 'voltage' || bus.demand_mw > 0;
	});
	const eligible: Array<GoalBus | GoalBranch> =
		form.intervention === 'capacity'
			? network.branches.filter((b) => b.editable !== false && b.status === 1 && b.rate_mw > 0)
			: allBuses.filter((b) => b.editable !== false);
	const candidates = eligible.filter((element) => {
		if (form.scope === 'selected') return form.candidates.includes(selectionKey(element));
		if (form.scope !== 'area') return true;
		return 'from' in element
			? inArea.has(element.from) || inArea.has(element.to)
			: inArea.has(element.id);
	});
	const rawWeights = buses.map(
		(bus) =>
			form.weights[selectionKey(bus)] ??
			(form.weighting === 'demand' ? Math.max(0, bus.demand_mw) : 1)
	);
	const total = rawWeights.reduce((sum, weight) => sum + weight, 0);
	return {
		buses: buses.map((bus, i) => ({
			bus,
			weight: total > 0 ? rawWeights[i] / total : 1 / buses.length
		})),
		candidates,
		eligible
	};
}

export function candidateBounds(element: GoalBus | GoalBranch, form: GoalForm) {
	const lower =
		form.intervention === 'redistribution' && 'demand_mw' in element
			? -Math.min(form.budget, Math.floor(element.demand_mw / form.increment) * form.increment)
			: 0;
	return {
		lower: form.bounds[selectionKey(element)]?.lower ?? (lower || 0),
		upper: form.bounds[selectionKey(element)]?.upper ?? form.budget
	};
}

export function buildGoal(network: Network, form: GoalForm, formulation: string) {
	if (form.objective === 'price' && formulation === 'acpf')
		throw new Error('AC power flow does not calculate LMPs. Choose a voltage target.');
	if (form.objective === 'voltage' && formulation === 'dcopf')
		throw new Error(
			'DC OPF does not calculate voltage magnitudes. Use AC power flow or SOCWR OPF.'
		);
	if (form.intervention === 'capacity' && formulation === 'acpf')
		throw new Error('AC power flow does not enforce line ratings. Choose a demand change.');
	if (!Number.isFinite(form.budget) || form.budget <= 0)
		throw new Error('Enter a positive change budget.');
	if (!Number.isFinite(form.increment) || form.increment <= 0 || form.increment > form.budget)
		throw new Error('The increment must be positive and no larger than the change budget.');
	if (!Number.isInteger(form.cardinality) || form.cardinality < 1)
		throw new Error('Enter a positive whole number for maximum changed elements.');
	if (form.scope === 'area' && !form.area) throw new Error('Choose an area.');
	if (Object.values(form.weights).some((weight) => !Number.isFinite(weight) || weight < 0))
		throw new Error('Bus weights must be finite and nonnegative.');
	const { buses, candidates } = resolveEquipment(network, form);
	if (!buses.length)
		throw new Error('Choose at least one bus for the objective. LMP objectives use load buses.');
	if (!candidates.length) throw new Error('Choose at least one line or bus that can be changed.');
	if (form.intervention === 'redistribution' && (candidates.length < 2 || form.cardinality < 2))
		throw new Error(
			'Demand redistribution needs at least two candidate buses and two permitted changes.'
		);
	if (
		form.intervention === 'placement' &&
		(!Number.isFinite(form.increase) || form.increase <= 0 || form.increase > form.budget)
	)
		throw new Error('Added demand must be positive and within the change budget.');
	if (form.objective === 'voltage' && (!Number.isFinite(form.target) || form.target <= 0))
		throw new Error('Enter a positive voltage target in pu.');
	const objective: StudyObjective =
		form.objective === 'price'
			? {
					kind: 'weighted_observable',
					operand: { Price: 'Active' },
					weights: buses.map(({ bus, weight }) => ({ element: elementKey(bus), weight }))
				}
			: {
					kind: 'sum',
					terms: buses.map(({ bus, weight }) => ({
						kind: 'scale',
						factor: weight,
						expression: {
							kind: 'squared_target',
							target: form.target,
							expression: {
								kind: 'weighted_observable',
								operand: { Voltage: 'Magnitude' },
								weights: [{ element: elementKey(bus), weight: 1 }]
							}
						}
					}))
				};
	const decisions: DecisionSpace = {
		variables: candidates.map((element) => {
			const bounds = candidateBounds(element, form);
			if (
				!Number.isFinite(bounds.lower) ||
				!Number.isFinite(bounds.upper) ||
				bounds.lower > 0 ||
				bounds.upper < 0
			)
				throw new Error(`Bounds for ${selectionKey(element)} must be finite and include zero.`);
			if (form.intervention !== 'redistribution' && bounds.lower < 0)
				throw new Error('Capacity upgrades and demand placement cannot have negative changes.');
			if ('demand_mw' in element && bounds.lower < -element.demand_mw)
				throw new Error(`Bus ${element.id} demand cannot fall below zero.`);
			return {
				id: `${form.intervention === 'capacity' ? 'rating' : 'demand'}:${elementKey(element)}`,
				element: elementKey(element),
				intervention:
					form.intervention === 'capacity'
						? ('branch_rating' as const)
						: ('active_demand' as const),
				...bounds,
				increment: form.increment,
				budget_weight: 1
			};
		}),
		total_budget: form.budget,
		max_changed_elements: Math.min(form.cardinality, candidates.length),
		demand:
			form.intervention === 'capacity'
				? null
				: form.intervention === 'placement'
					? { kind: 'placement', increase_mw: form.increase }
					: { kind: 'redistribution' }
	};
	const request =
		form.objective === 'price' ? 'Reduce average LMP' : `Bring voltage closer to ${form.target} pu`;
	const busCount = `${buses.length} ${buses.length === 1 ? 'bus' : 'buses'}`;
	const candidateKind =
		form.intervention === 'capacity'
			? candidates.length === 1
				? 'line'
				: 'lines'
			: candidates.length === 1
				? 'bus'
				: 'buses';
	const interpretation = `${request} at ${busCount}, ${form.weighting === 'demand' ? 'weighted by demand' : 'equally weighted'}. ${form.intervention === 'capacity' ? 'Increase line capacity' : form.intervention === 'placement' ? `Add ${form.increase} MW of demand` : 'Redistribute demand without changing the total'} across ${candidates.length} candidate ${candidateKind}.`;
	return { request, interpretation, objective, decisions, success_value: null };
}

export function objectiveLabel(objective: StudyObjective): string {
	switch (objective.kind) {
		case 'weighted_observable': {
			const operand = objective.operand;
			const label =
				'Price' in operand
					? 'LMP'
					: 'Voltage' in operand
						? 'voltage'
						: 'Dispatch' in operand
							? 'generation'
							: 'line flow';
			return `Weighted ${label}, ${objective.weights.length} ${objective.weights.length === 1 ? 'element' : 'elements'}`;
		}
		case 'squared_target':
			return `Squared deviation from ${objective.target}: ${objectiveLabel(objective.expression)}`;
		case 'scale':
			return `${objective.factor} times ${objectiveLabel(objective.expression)}`;
		case 'sum':
			return `Sum of ${objective.terms.length} objective terms`;
		case 'intervention_penalty':
			return `Change penalty for ${objective.decision}`;
	}
}
