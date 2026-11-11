import type { Network, Solution, SolutionFlow } from '@tellegen/engine';
import { assessVoltage, type LoadingState } from './transmission-results.js';

export function operatingExceptions(
	network: Network,
	voltages: ReadonlyMap<number, number> | null,
	deenergized: ReadonlySet<number>,
	loading: readonly LoadingState[],
	flows: ReadonlyMap<number, SolutionFlow>
) {
	const counts = {
		low: 0,
		high: 0,
		overload: 0,
		warning: 0,
		atLimit: 0,
		voltageUnknown: 0,
		voltageChecked: 0,
		loadingChecked: 0,
		flowMissing: 0,
		ratingUnknown: 0,
		loadingUnknown: 0,
		inactive: 0,
		disconnected: 0
	};
	const buses = [];
	const branches: {
		id: number;
		from: number;
		to: number;
		state: 'violation' | 'at-limit' | 'warning';
		loading: number;
	}[] = [];
	for (const bus of network.buses) {
		if (deenergized.has(bus.id)) {
			counts.disconnected++;
		} else if (voltages) {
			const value = voltages.get(bus.id);
			const { state, exceedance } = assessVoltage(value, bus);
			if (state === 'normal' || state === 'low' || state === 'high') counts.voltageChecked++;
			if (state === 'low' || state === 'high') {
				counts[state]++;
				buses.push({
					id: bus.id,
					name: bus.name,
					station:
						bus.substation === undefined ? null : network.substations?.[bus.substation]?.name,
					state,
					exceedance,
					value: value!
				});
			} else if (state !== 'normal') {
				counts.voltageUnknown++;
			}
		}
	}
	network.branches.forEach((branch, index) => {
		const state = loading[index];
		if (state === 'normal' || state === 'warning' || state === 'at-limit' || state === 'violation')
			counts.loadingChecked++;
		if (state === 'violation' || state === 'at-limit' || state === 'warning') {
			counts[state === 'violation' ? 'overload' : state === 'at-limit' ? 'atLimit' : 'warning']++;
			branches.push({
				id: branch.id,
				from: branch.from,
				to: branch.to,
				state,
				loading: flows.get(branch.id)!.loading
			});
		} else if (state === 'inactive') {
			counts.inactive++;
		} else if (state !== 'normal' && state !== 'deenergized') {
			counts.loadingUnknown++;
			if (state === 'missing') counts.flowMissing++;
			else counts.ratingUnknown++;
		}
	});
	const rank = { violation: 2, 'at-limit': 1, warning: 0 } as const;
	buses.sort((a, b) => b.exceedance - a.exceedance || a.id - b.id);
	branches.sort((a, b) => rank[b.state] - rank[a.state] || b.loading - a.loading || a.id - b.id);
	return { counts, buses, branches };
}

export function dispatchByBus(dispatch: Solution['dispatch']): Map<number, number> {
	const totals = new Map<number, number>();
	for (const { bus, mw } of dispatch) {
		if (bus !== undefined && Number.isFinite(mw)) totals.set(bus, (totals.get(bus) ?? 0) + mw);
	}
	return totals;
}
