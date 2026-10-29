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

/** Declared bands only, each with both a minimum and a maximum. Phase–phase bounds are ordered by
 * the pairs (1,2), (2,3), (3,1) of the phase terminals (BMOPF 0.2.0), which the engine names in
 * `vpp_pairs`; without those names only a uniform band is taken, as there is nothing to place the
 * others by. Sequence and neutral-to-ground bounds are not ranked. */
export function busLimits(
	bus: Pick<DistGraphBus, 'voltage_limits' | 'terminals'>
): BusLimits | null {
	const l = bus.voltage_limits;
	if (!l) return null;
	const limits: BusLimits = {};
	if (l.vpn_min?.length && l.vpn_max?.length === l.vpn_min.length)
		limits.phaseNeutral = { min: l.vpn_min, max: l.vpn_max };
	const [lo, hi, pairs] = [l.vpp_min, l.vpp_max, l.vpp_pairs];
	if (pairs?.length && lo?.length === pairs.length && hi?.length === pairs.length) {
		limits.phasePhase = pairs.map((pair, k) => ({ pair, min: lo[k], max: hi[k] }));
	} else {
		const uniform = (v?: number[]) => (v?.length && v.every((x) => x === v[0]) ? v[0] : null);
		const min = uniform(lo);
		const max = uniform(hi);
		if (min !== null && max !== null) limits.phasePhase = [{ pair: null, min, max }];
	}
	const below = l.v_min_by_terminal ?? {};
	const above = l.v_max_by_terminal ?? {};
	const ground = bus.terminals.flatMap((terminal) =>
		Object.hasOwn(below, terminal) && Object.hasOwn(above, terminal)
			? [{ terminal, min: below[terminal], max: above[terminal] }]
			: []
	);
	if (ground.length) limits.phaseGround = ground;
	return limits.phaseNeutral || limits.phasePhase || limits.phaseGround ? limits : null;
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
		// Kept as given: `null` declares no neutral, absent leaves the aliases to decide.
		neutral: b.neutral_terminal,
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
			// One entry per piece of equipment: a multi-winding transformer projects to one edge per
			// winding pair, all sharing its id, and is found once, at its first edge.
			...firstEdges(graph).map((i): SearchEntity => {
				const e = graph.edges[i];
				return { kind: 'branch', index: i, id: e.id, family: e.kind, qualified: e.id };
			})
		]
	};
}

/** Index of the first edge of each kind and id, in graph order. */
function firstEdges(graph: DistGraph): number[] {
	const seen = new Set<string>();
	const out: number[] = [];
	graph.edges.forEach((e, i) => {
		const key = `${e.kind}\u0000${e.id}`;
		if (seen.has(key)) return;
		seen.add(key);
		out.push(i);
	});
	return out;
}
