import type { DistGraph } from '@tellegen/engine';
import type { DisplayTree } from './display-tree.js';
import type { ResultSnapshot } from './snapshot.js';
import { busState, loadingState } from './summary.js';

/** A galvanic zone: buses joined by lines and closed switches, separated from the rest only by
 * transformers. On a distribution feeder the zones without a source are the LV networks. */
export interface GalvanicZone {
	buses: number[];
	/** Branches inside the zone. */
	edges: number[];
	/** Transformers with a winding in the zone; the feeding one first when the display tree has it. */
	transformers: number[];
	hasSource: boolean;
}

export interface GalvanicZones {
	zones: GalvanicZone[];
	/** Zone index per bus. */
	busZone: Int32Array;
}

/** Connected components over closed non-transformer branches. */
export function galvanicZones(graph: DistGraph, tree?: DisplayTree): GalvanicZones {
	const n = graph.buses.length;
	const index = new Map(graph.buses.map((b, i) => [b.id.toLowerCase(), i]));
	const ends = graph.edges.map(
		(e) =>
			[index.get(e.from.toLowerCase()) ?? -1, index.get(e.to.toLowerCase()) ?? -1] as [
				number,
				number
			]
	);
	const root = Int32Array.from({ length: n }, (_, i) => i);
	const find = (i: number): number => {
		while (root[i] !== i) i = root[i] = root[root[i]];
		return i;
	};
	graph.edges.forEach((e, k) => {
		const [a, b] = ends[k];
		if (a < 0 || b < 0 || !e.closed || e.kind === 'transformer') return;
		const ra = find(a);
		const rb = find(b);
		if (ra !== rb) root[ra] = rb;
	});
	const busZone = new Int32Array(n).fill(-1);
	const zones: GalvanicZone[] = [];
	for (let i = 0; i < n; i++) {
		const r = find(i);
		if (busZone[r] < 0) {
			busZone[r] = zones.length;
			zones.push({ buses: [], edges: [], transformers: [], hasSource: false });
		}
		busZone[i] = busZone[r];
		const z = zones[busZone[i]];
		z.buses.push(i);
		if (graph.buses[i].has_source) z.hasSource = true;
	}
	graph.edges.forEach((e, k) => {
		const [a, b] = ends[k];
		if (a < 0 || b < 0) return;
		if (e.kind === 'transformer') {
			for (const z of new Set([busZone[a], busZone[b]])) zones[z].transformers.push(k);
		} else if (busZone[a] === busZone[b]) zones[busZone[a]].edges.push(k);
	});
	// The feeding transformer is the one the display tree enters the zone through.
	if (tree)
		for (const z of zones) {
			const feed = z.buses
				.map((i) => tree.parentEdge[i])
				.find((k) => k >= 0 && graph.edges[k].kind === 'transformer');
			if (feed !== undefined) z.transformers = [feed, ...z.transformers.filter((k) => k !== feed)];
		}
	return { zones, busZone };
}

export type ZoneState = 'normal' | 'warning' | 'violation' | 'missing';

/** A zone's worst state: a voltage outside its band or a branch over its rating is a violation,
 * loading at or above the warning threshold a warning. Its transformers count. */
export function zoneState(zone: GalvanicZone, snapshot: ResultSnapshot | null): ZoneState {
	if (!snapshot) return 'missing';
	const rank = { missing: 0, normal: 1, warning: 2, violation: 3 };
	let state: ZoneState = 'missing';
	const raise = (s: ZoneState) => {
		if (rank[s] > rank[state]) state = s;
	};
	for (const i of zone.buses) {
		const s = busState(snapshot.buses[i]);
		raise(s === 'violation' ? 'violation' : s === 'normal' ? 'normal' : 'missing');
	}
	for (const k of [...zone.transformers, ...zone.edges]) {
		const s = loadingState(snapshot.branches[k]?.loading ?? null);
		if (s !== 'missing') raise(s);
	}
	return state;
}

/** The outline of a zone in a schematic layout: one band per drawing column (depth), from the
 * zone's highest bus to the lowest attachment symbol below it, joined into a staircase. Tidy and
 * leaf-order layouts keep each subtree's rows contiguous per column, so neighbouring zones'
 * outlines do not overlap; a zone nested inside another sits inside its outline. */
export function zoneOutline(
	zone: GalvanicZone,
	positions: ArrayLike<number>,
	below: ArrayLike<number>,
	options: { dx: number; pad: number }
): [number, number][] {
	const columns = new Map<number, [number, number]>();
	for (const i of zone.buses) {
		const x = positions[i * 3];
		const y = positions[i * 3 + 1];
		const c = columns.get(x);
		if (c) {
			c[0] = Math.min(c[0], y);
			c[1] = Math.max(c[1], y + below[i]);
		} else columns.set(x, [y, y + below[i]]);
	}
	const xs = [...columns.keys()].sort((a, b) => a - b);
	const { dx, pad } = options;
	// The first column starts just after the feeding transformer's symbol; the last leaves room
	// for attachment symbols and their values to the right of the bus.
	const left = (j: number) => (j === 0 ? xs[0] - 0.15 * dx : (xs[j - 1] + xs[j]) / 2);
	const right = (j: number) => (j === xs.length - 1 ? xs[j] + 0.4 * dx : (xs[j] + xs[j + 1]) / 2);
	const top: [number, number][] = [];
	const bottom: [number, number][] = [];
	xs.forEach((x, j) => {
		const [lo, hi] = columns.get(x)!;
		top.push([left(j), lo - pad], [right(j), lo - pad]);
		bottom.push([left(j), hi + pad], [right(j), hi + pad]);
	});
	return [...top, ...bottom.reverse()];
}

/** A zone's corridor, for source positions: the segments of its own branches and its buses, to be
 * drawn as a band of fixed screen width. Unlike a hull it covers only where the network runs, so
 * networks that interleave along streets stay apart; every bus belongs to exactly one zone. */
export interface ZoneCorridor {
	z: number;
	/** Segment indices into the drawing's sources, targets and segmentEdge. */
	segments: number[];
	buses: number[];
	/** Bounding box of its buses: [x0, y0, x1, y1]. */
	bounds: [number, number, number, number];
}

export function zoneCorridors(
	zones: GalvanicZones,
	geometry: { positions: ArrayLike<number>; segmentEdge: ArrayLike<number> },
	include: (zone: GalvanicZone) => boolean
): ZoneCorridor[] {
	const edgeZone = new Map<number, number>();
	zones.zones.forEach((zone, z) => {
		if (include(zone)) for (const k of zone.edges) edgeZone.set(k, z);
	});
	const out = new Map<number, ZoneCorridor>();
	zones.zones.forEach((zone, z) => {
		if (!include(zone)) return;
		let [x0, y0, x1, y1] = [Infinity, Infinity, -Infinity, -Infinity];
		for (const i of zone.buses) {
			const x = geometry.positions[i * 3];
			const y = geometry.positions[i * 3 + 1];
			[x0, y0, x1, y1] = [Math.min(x0, x), Math.min(y0, y), Math.max(x1, x), Math.max(y1, y)];
		}
		out.set(z, { z, segments: [], buses: zone.buses, bounds: [x0, y0, x1, y1] });
	});
	for (let s = 0; s < geometry.segmentEdge.length; s++) {
		const z = edgeZone.get(geometry.segmentEdge[s]);
		if (z !== undefined) out.get(z)!.segments.push(s);
	}
	return [...out.values()];
}

/** A zone's voltage level in volts: the declared rating of a winding or source in it (the
 * largest), else its declared limits (midpoint of the band; a phase–neutral or phase–ground band
 * times √3), else its solved voltages, as the median over its buses. Null when none is known. */
export function zoneVoltage(
	zone: GalvanicZone,
	graph: DistGraph,
	snapshot: ResultSnapshot | null
): number | null {
	let nominal = 0;
	for (const i of zone.buses) nominal = Math.max(nominal, graph.buses[i].nominal_voltage ?? 0);
	if (nominal > 0) return nominal;
	const levels: number[] = [];
	const mid = (lo?: number[], hi?: number[]) =>
		lo?.length && hi?.length ? (Math.min(...lo) + Math.max(...hi)) / 2 : null;
	const values = (byTerminal?: Record<string, number>) => byTerminal && Object.values(byTerminal);
	for (const i of zone.buses) {
		const l = graph.buses[i].voltage_limits;
		const pp = mid(l?.vpp_min, l?.vpp_max);
		// Phase–neutral or phase–ground: the phase voltage, √3 below line to line.
		const phaseBand =
			mid(l?.vpn_min, l?.vpn_max) ??
			mid(values(l?.v_min_by_terminal), values(l?.v_max_by_terminal));
		let v = pp ?? (phaseBand === null ? null : phaseBand * Math.sqrt(3));
		if (v === null && snapshot) {
			const pairs = snapshot.buses[i]?.pairs ?? [];
			const phase = pairs.filter((p) => p.zone === 'phase–phase');
			const single = pairs.filter((p) => p.zone !== 'phase–phase');
			if (phase.length) v = Math.max(...phase.map((p) => p.volts));
			else if (single.length) v = Math.max(...single.map((p) => p.volts)) * Math.sqrt(3);
		}
		if (v !== null && Number.isFinite(v) && v > 0) levels.push(v);
	}
	if (!levels.length) return null;
	levels.sort((a, b) => a - b);
	return levels[levels.length >> 1];
}

/** `mv` from 1 kV phase–phase up (drawn heavier), else `lv`. Without a known voltage the source
 * zone counts as `mv`, others as `lv`. */
export function zoneLevel(zone: GalvanicZone, volts: number | null): 'mv' | 'lv' {
	if (volts === null) return zone.hasSource ? 'mv' : 'lv';
	return volts >= 1000 ? 'mv' : 'lv';
}

/** One-line zone summary for its label: lowest phase–neutral (else phase–ground, else
 * phase–phase) voltage and highest loading among its branches and transformers, when solved. */
export function zoneSummary(
	zone: GalvanicZone,
	snapshot: ResultSnapshot | null
): { minVolts: number | null; maxLoading: number | null } {
	if (!snapshot) return { minVolts: null, maxLoading: null };
	let pn = Infinity;
	let pg = Infinity;
	let pp = Infinity;
	for (const i of zone.buses)
		for (const p of snapshot.buses[i]?.pairs ?? []) {
			if (!Number.isFinite(p.volts)) continue;
			if (p.zone === 'phase–neutral') pn = Math.min(pn, p.volts);
			else if (p.zone === 'phase–ground') pg = Math.min(pg, p.volts);
			else pp = Math.min(pp, p.volts);
		}
	let loading = -Infinity;
	for (const k of [...zone.transformers, ...zone.edges]) {
		const l = snapshot.branches[k]?.loading;
		if (l !== null && l !== undefined && Number.isFinite(l)) loading = Math.max(loading, l);
	}
	return {
		minVolts: [pn, pg, pp].find(Number.isFinite) ?? null,
		maxLoading: Number.isFinite(loading) ? loading : null
	};
}
