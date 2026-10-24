import type { DistGraph, DistGraphBus, DistGraphEdge } from '@tellegen/engine';
import type { SearchEntity } from './search.js';
import type { BusLimits, BranchRating, SnapshotBranch, SnapshotBus } from './snapshot.js';
import type { TableNames } from './results-table.js';

/** Everything the workbench derives from one multiconductor graph, in graph order. */
export interface GraphInputs {
	buses: SnapshotBus[];
	branches: SnapshotBranch[];
	names: TableNames;
	entities: SearchEntity[];
}

/** Declared limits only. Non-uniform phase–phase bounds have no declared pair order, so they are
 * left undeclared rather than guessed. */
export function busLimits(bus: Pick<DistGraphBus, 'voltage_limits'>): BusLimits | null {
	const l = bus.voltage_limits;
	if (l?.vpn_min?.length && l.vpn_max?.length === l.vpn_min.length)
		return { reference: 'phase-neutral', min: l.vpn_min, max: l.vpn_max };
	const uniform = (v?: number[]) => (v?.length && v.every((x) => x === v[0]) ? v[0] : null);
	const min = uniform(l?.vpp_min);
	const max = uniform(l?.vpp_max);
	return min !== null && max !== null ? { reference: 'phase-phase', min, max } : null;
}

export function edgeRating(edge: Pick<DistGraphEdge, 'rating'>): BranchRating | null {
	const r = edge.rating;
	if (r?.i_max?.length) return { kind: 'current', amps: r.i_max };
	const byBus = r?.s_rating_by_bus;
	if (byBus && Object.values(byBus).some((va) => va > 0)) return { kind: 'apparent', byBus };
	return null;
}

export function graphInputs(graph: DistGraph): GraphInputs {
	const index = new Map(graph.buses.map((b, i) => [b.id.toLowerCase(), i]));
	const busIndex = (id: string) => index.get(id.toLowerCase()) ?? -1;
	const buses = graph.buses.map((b): SnapshotBus => ({
		id: b.id,
		terminals: b.terminals,
		neutral: b.neutral_terminal ?? null,
		limits: busLimits(b)
	}));
	const branches = graph.edges.map((e): SnapshotBranch => ({
		id: e.id,
		kind: e.kind,
		from: e.from,
		to: e.to,
		fromTerminals: e.conductors.map((c) => c[0]),
		toTerminals: e.conductors.map((c) => c[1]),
		inService: e.closed,
		rating: edgeRating(e)
	}));
	return {
		buses,
		branches,
		names: {
			buses: graph.buses.map((b) => ({ id: b.id })),
			branches: graph.edges.map((e) => ({
				id: e.id,
				family: e.kind,
				from: busIndex(e.from),
				to: busIndex(e.to),
				inService: e.closed
			}))
		},
		entities: [
			...graph.buses.map((b, i): SearchEntity => ({
				kind: 'bus',
				index: i,
				id: b.id,
				family: 'bus',
				qualified: b.id
			})),
			...graph.edges.map((e, i): SearchEntity => ({
				kind: 'branch',
				index: i,
				id: e.id,
				family: e.kind,
				qualified: e.id
			}))
		]
	};
}
