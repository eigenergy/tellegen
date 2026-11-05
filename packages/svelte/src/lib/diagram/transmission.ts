import type { Network, NetworkSubstation, TxLayout, TxLayoutInput } from '@tellegen/engine';
import type { Landmark } from './landmarks.js';
import type { SchematicGeometry } from './schematic-layout.js';
import { levelLabel } from './voltage-levels.js';

/** Voltage tiers: the hue family a level is drawn in. Colour alone separates only a few classes
 * on a map (any two lines can touch), so levels within a tier are told apart by line width,
 * shade and their legend label. */
export type TxTier = 'ehv' | 'hv' | 'sub' | 'low' | 'unknown';

export const TX_TIERS: { tier: TxTier; name: string; from: number }[] = [
	{ tier: 'ehv', name: 'extra high voltage', from: 300 },
	{ tier: 'hv', name: 'high voltage', from: 100 },
	{ tier: 'sub', name: 'subtransmission', from: 35 },
	{ tier: 'low', name: 'below 35 kV', from: 1e-9 },
	{ tier: 'unknown', name: 'voltage not given', from: -Infinity }
];

/** The tier of a nominal voltage in kV; 0 or missing is unknown. */
export function txTier(kv: number): TxTier {
	return TX_TIERS.find((t) => kv >= t.from)!.tier;
}

/** One entry of the voltage legend: a level, or adjacent levels merged into a range. */
export interface VoltageEntry {
	/** Nominal levels in kV, highest first; empty for the unknown entry. */
	levels: number[];
	label: string;
	/** The named band of its highest level, for the key's headings. */
	tier: TxTier;
	/** The colour ramp it is drawn in: its tier's, or a lower transmission tier's ramp that the
	 * case leaves unused, so the case's main levels get distinct hues (see `voltageLegend`). */
	hue: TxTier;
	/** Position among the entries drawn in its hue (0 highest) and how many share it. */
	inTier: number;
	tierSize: number;
	/** Position among all entries, 0 for the highest. */
	rank: number;
	/** Branches drawn at these levels. */
	branches: number;
}

export interface VoltageLegend {
	entries: VoltageEntry[];
	/** The entry a nominal voltage belongs to. */
	entryOf: (kv: number) => number;
}

/** Levels closer than this (relative) are one level: 132 and 132.0001 kV, rounding in files. */
const SAME_LEVEL = 1e-3;

/** A voltage legend with one entry per distinct nominal level, highest first, at most `budget`
 * entries. When a case has more levels, adjacent levels merge into ranges, cheapest merge first.
 * A merge costs the voltage ratio it spans (log) times the square root of its higher level, and
 * merges across tiers cost ten times more: so closely spaced low levels (115 and 138 kV) merge
 * first and extra high voltage levels stay distinct longest. `levels` maps each nominal kV to
 * how many branches are drawn at it (0 kV: voltage not given, always its own entry). */
export function voltageLegend(levels: Map<number, number>, budget = 6): VoltageLegend {
	const known = [...levels.entries()].filter(([kv]) => kv > 0).sort((a, b) => b[0] - a[0]);
	const hasUnknown = [...levels.keys()].some((kv) => !(kv > 0));
	const unknown = [...levels.entries()].filter(([kv]) => !(kv > 0)).reduce((n, [, c]) => n + c, 0);
	// Collapse levels that differ only by rounding.
	let groups: { levels: number[]; branches: number }[] = [];
	for (const [kv, count] of known) {
		const last = groups[groups.length - 1];
		if (last && Math.abs(last.levels[last.levels.length - 1] - kv) <= SAME_LEVEL * kv)
			last.branches += count;
		else groups.push({ levels: [kv], branches: count });
	}
	const room = Math.max(1, budget - (hasUnknown ? 1 : 0));
	const cost = (hi: { levels: number[] }, lo: { levels: number[] }) => {
		const top = hi.levels[0];
		const bottom = lo.levels[lo.levels.length - 1];
		const cross = txTier(top) !== txTier(bottom) ? 10 : 1;
		return Math.log(top / bottom) * Math.sqrt(top) * cross;
	};
	while (groups.length > room) {
		let best = 0;
		for (let i = 1; i < groups.length - 1; i++)
			if (cost(groups[i], groups[i + 1]) < cost(groups[best], groups[best + 1])) best = i;
		const [a, b] = [groups[best], groups[best + 1]];
		groups = [
			...groups.slice(0, best),
			{ levels: [...a.levels, ...b.levels], branches: a.branches + b.branches },
			...groups.slice(best + 2)
		];
	}
	const kvLabel = (kv: number) => `${Number(kv.toPrecision(4))}`;
	const entries: VoltageEntry[] = groups.map((g, rank) => {
		const [top, bottom] = [g.levels[0], g.levels[g.levels.length - 1]];
		return {
			levels: g.levels,
			label: top === bottom ? `${kvLabel(top)} kV` : `${kvLabel(bottom)}–${kvLabel(top)} kV`,
			tier: txTier(top),
			hue: txTier(top),
			inTier: 0,
			tierSize: 0,
			rank,
			branches: g.branches
		};
	});
	if (hasUnknown)
		entries.push({
			levels: [],
			label: 'voltage not given',
			tier: 'unknown',
			hue: 'unknown',
			inTier: 0,
			tierSize: 1,
			rank: entries.length,
			branches: unknown
		});
	assignHues(entries);
	for (const e of entries) {
		const same = entries.filter((o) => o.hue === e.hue);
		e.tierSize = same.length;
		e.inTier = same.indexOf(e);
	}
	const entryOf = (kv: number) => {
		if (!(kv > 0)) return entries.length - 1;
		const i = entries.findIndex(
			(e) =>
				e.levels.length &&
				kv <= e.levels[0] * (1 + SAME_LEVEL) &&
				kv >= e.levels[e.levels.length - 1] * (1 - SAME_LEVEL)
		);
		return i >= 0 ? i : entries.length - 1;
	};
	return { entries, entryOf };
}

/** The transmission hues, highest first. Lower tiers stay neutral grey. */
const HUE_ORDER: TxTier[] = ['ehv', 'hv', 'sub'];

/** Give the case's transmission levels distinct hues where its tiers alone would not. Walking the
 * entries highest first, an entry whose tier hue is already taken moves to the next lower hue,
 * provided no lower entry has that hue by its own tier. So 230 and 115 kV (both high voltage)
 * draw blue and cyan, 500/230/115 kV magenta, blue and cyan, while 345/138/115/69 kV keeps 115
 * kV a lighter blue because 69 kV needs cyan. Hues only move down, so the highest levels keep
 * the colour their voltage suggests across cases. */
function assignHues(entries: VoltageEntry[]) {
	let last = -1;
	entries.forEach((e, i) => {
		const own = HUE_ORDER.indexOf(e.tier);
		if (own < 0) return;
		let slot = own;
		if (own <= last) {
			// Taken: the next lower hue when free, else share the lowest so far (shaded lighter),
			// so hues never step back up as voltage falls.
			const next = last + 1;
			const claimed = entries.slice(i + 1).some((o) => HUE_ORDER.indexOf(o.tier) === next);
			slot = next < HUE_ORDER.length && !claimed ? next : last;
		}
		e.hue = HUE_ORDER[slot];
		last = Math.max(last, slot);
	});
}

/** Branches per nominal level (each branch at its higher end's voltage), for `voltageLegend`. */
export function branchLevels(graph: TxGraph): Map<number, number> {
	const out = new Map<number, number>();
	for (let k = 0; k < graph.branchKv.length; k++) {
		const kv = Math.round(graph.branchKv[k] * 1000) / 1000;
		out.set(kv, (out.get(kv) ?? 0) + 1);
	}
	// Buses at a level no branch reaches still need an entry (an isolated generator bus).
	for (let i = 0; i < graph.kv.length; i++) {
		const kv = Math.round(graph.kv[i] * 1000) / 1000;
		if (!out.has(kv)) out.set(kv, 0);
	}
	return out;
}

/** A balanced network as columnar arrays in payload order, for drawing. Buses are addressed by
 * their row (`index.get(id)`); branch ends are rows, -1 when the bus is not in the payload. */
export interface TxGraph {
	busId: Int32Array;
	index: Map<number, number>;
	/** Nominal kV per bus; 0 when unknown. */
	kv: Float32Array;
	/** Substation per bus, into `substations`. */
	substation: Int32Array;
	substations: NetworkSubstation[];
	/** Whether the payload carried its own grouping (else every bus stands alone). */
	grouped: boolean;
	demandMw: Float32Array;
	genMw: Float32Array;
	from: Int32Array;
	to: Int32Array;
	/** Per branch: joins two voltage levels. */
	transformer: Uint8Array;
	inService: Uint8Array;
	rateMw: Float32Array;
	/** Per branch: the higher nominal kV of its two ends. */
	branchKv: Float32Array;
}

export function txGraph(network: Network): TxGraph {
	const n = network.buses.length;
	const m = network.branches.length;
	const index = new Map(network.buses.map((b, i) => [b.id, i]));
	const busId = Int32Array.from(network.buses, (b) => b.id);
	const kv = Float32Array.from(network.buses, (b) => b.base_kv ?? 0);
	const grouped =
		!!network.substations &&
		network.buses.every(
			(b) => b.substation !== undefined && b.substation < network.substations!.length
		);
	// Without a grouping from the engine, every bus is its own substation; the drawing still works,
	// it just has nothing to merge.
	const substations: NetworkSubstation[] = grouped
		? network.substations!
		: network.buses.map((b) => ({
				key: `bus:${b.id}`,
				name: null,
				source: 'single',
				kv: b.base_kv ? [b.base_kv] : []
			}));
	const substation = grouped
		? Int32Array.from(network.buses, (b) => b.substation!)
		: Int32Array.from({ length: n }, (_, i) => i);
	const from = Int32Array.from(network.branches, (br) => index.get(br.from) ?? -1);
	const to = Int32Array.from(network.branches, (br) => index.get(br.to) ?? -1);
	const at = (row: number) => (row >= 0 ? kv[row] : 0);
	const transformer = Uint8Array.from(network.branches, (br, k) => {
		if (br.transformer !== undefined) return br.transformer ? 1 : 0;
		// Older payloads: the same voltage test the engine applies.
		const [a, b] = [at(from[k]), at(to[k])];
		return a > 0 && b > 0 && Math.abs(a - b) > KV_RATIO_TOLERANCE * Math.max(a, b) ? 1 : 0;
	});
	return {
		busId,
		index,
		kv,
		substation,
		substations,
		grouped,
		demandMw: Float32Array.from(network.buses, (b) => b.demand_mw),
		genMw: Float32Array.from(network.buses, (b) => b.gen_mw),
		from,
		to,
		transformer,
		inService: Uint8Array.from(network.branches, (br) => (br.status ? 1 : 0)),
		rateMw: Float32Array.from(network.branches, (br) => br.rate_mw),
		branchKv: Float32Array.from({ length: m }, (_, k) => Math.max(at(from[k]), at(to[k])))
	};
}

/** The engine's `GroupingOptions::default().kv_ratio_tolerance`, for payloads that predate the
 * per-branch `transformer` flag. A Rust test pins the engine value to this one; change both. */
export const KV_RATIO_TOLERANCE = 0.01;

const R = 6_378_137;
/** Longitude/latitude to Web Mercator metres, y down (as the schematic draws). */
function mercator(lon: number, lat: number): [number, number] {
	const clamped = Math.max(-85, Math.min(85, lat));
	return [
		(R * lon * Math.PI) / 180,
		-R * Math.log(Math.tan(Math.PI / 4 + (clamped * Math.PI) / 360))
	];
}

/** The network as placed in its data: geographic positions in Web Mercator metres (y down) or
 * drawing coordinates as given, relative to their centroid, so float32 keeps metre detail. Each
 * branch follows its payload path (a routed line, or the straight segment). Edge roles: 0 in
 * service, 2 out of service. */
export function txSourceLayout(graph: TxGraph, network: Network): SchematicGeometry {
	const geographic = network.coordinate_space !== 'diagram';
	const project = geographic ? mercator : (x: number, y: number): [number, number] => [x, y];
	const n = graph.busId.length;
	const xy = network.buses.map((b) => project(b.lon, b.lat));
	let cx = 0;
	let cy = 0;
	for (const [x, y] of xy) {
		cx += x;
		cy += y;
	}
	cx /= Math.max(1, n);
	cy /= Math.max(1, n);
	const positions = new Float32Array(n * 3);
	let [x0, y0, x1, y1] = [Infinity, Infinity, -Infinity, -Infinity];
	xy.forEach(([x, y], i) => {
		positions[i * 3] = x - cx;
		positions[i * 3 + 1] = y - cy;
		x0 = Math.min(x0, x - cx);
		x1 = Math.max(x1, x - cx);
		y0 = Math.min(y0, y - cy);
		y1 = Math.max(y1, y - cy);
	});
	let segments = 0;
	for (const br of network.branches) segments += Math.max(1, br.path.length - 1);
	const sources = new Float32Array(segments * 3);
	const targets = new Float32Array(segments * 3);
	const segmentEdge = new Int32Array(segments);
	let s = 0;
	network.branches.forEach((br, k) => {
		const a = graph.from[k];
		const b = graph.to[k];
		const points: [number, number][] =
			br.path.length >= 2
				? br.path.map(([lon, lat]) => {
						const [x, y] = project(lon, lat);
						return [x - cx, y - cy];
					})
				: a >= 0 && b >= 0
					? [
							[positions[a * 3], positions[a * 3 + 1]],
							[positions[b * 3], positions[b * 3 + 1]]
						]
					: [];
		// Geographic routes may be stored in either endpoint order. Normalize once, so local
		// tangents, flow arrows and transformer winding colours all follow from → to, even
		// where the route doubles back against the straight endpoint chord.
		if (points.length >= 2 && a >= 0 && b >= 0) {
			const distance = (p: [number, number], row: number) =>
				Math.hypot(p[0] - positions[row * 3], p[1] - positions[row * 3 + 1]);
			const first = points[0];
			const last = points[points.length - 1];
			if (distance(first, b) + distance(last, a) < distance(first, a) + distance(last, b))
				points.reverse();
		}
		for (let i = 1; i < points.length; i++, s++) {
			sources[s * 3] = points[i - 1][0];
			sources[s * 3 + 1] = points[i - 1][1];
			targets[s * 3] = points[i][0];
			targets[s * 3 + 1] = points[i][1];
			segmentEdge[s] = k;
		}
	});
	return {
		positions,
		sources: sources.subarray(0, s * 3),
		targets: targets.subarray(0, s * 3),
		segmentEdge: segmentEdge.subarray(0, s),
		role: Uint8Array.from(graph.inService, (on) => (on ? 0 : 2)),
		bounds: Number.isFinite(x0) ? [x0, y0, x1, y1] : [0, 0, 1, 1],
		...(geographic ? { mercatorOrigin: [cx, cy] as [number, number] } : {})
	};
}

/** Each substation's centre on a layout (mean of its buses), as `x, y` pairs. */
export function substationCentres(graph: TxGraph, geometry: SchematicGeometry): Float32Array {
	const k = graph.substations.length;
	const sum = new Float64Array(k * 2);
	const count = new Uint32Array(k);
	for (let i = 0; i < graph.busId.length; i++) {
		const s = graph.substation[i];
		sum[s * 2] += geometry.positions[i * 3];
		sum[s * 2 + 1] += geometry.positions[i * 3 + 1];
		count[s]++;
	}
	const out = new Float32Array(k * 2);
	for (let s = 0; s < k; s++) {
		if (!count[s]) continue;
		out[s * 2] = sum[s * 2] / count[s];
		out[s * 2 + 1] = sum[s * 2 + 1] / count[s];
	}
	return out;
}

/** The median drawn length of in-service branches, in drawing units: the scale at which symbols
 * on branches become legible. 1 when nothing is drawn. */
export function typicalBranchLength(graph: TxGraph, geometry: SchematicGeometry): number {
	const lengths: number[] = [];
	for (let k = 0; k < graph.from.length; k++) {
		const [a, b] = [graph.from[k], graph.to[k]];
		if (a < 0 || b < 0 || a === b || !graph.inService[k]) continue;
		const d = Math.hypot(
			geometry.positions[a * 3] - geometry.positions[b * 3],
			geometry.positions[a * 3 + 1] - geometry.positions[b * 3 + 1]
		);
		if (d > 0) lengths.push(d);
	}
	if (!lengths.length) return 1;
	lengths.sort((p, q) => p - q);
	return lengths[lengths.length >> 1];
}

/** Where a branch's symbol sits and which way it points: the middle of its first and last bus. */
export function branchAnchor(
	graph: TxGraph,
	geometry: SchematicGeometry,
	k: number
): { x: number; y: number; cos: number; sin: number } | null {
	const [a, b] = [graph.from[k], graph.to[k]];
	if (a < 0 || b < 0) return null;
	const [ax, ay] = [geometry.positions[a * 3], geometry.positions[a * 3 + 1]];
	const [bx, by] = [geometry.positions[b * 3], geometry.positions[b * 3 + 1]];
	const len = Math.hypot(bx - ax, by - ay);
	return {
		x: (ax + bx) / 2,
		y: (ay + by) / 2,
		cos: len > 0 ? (bx - ax) / len : 1,
		sin: len > 0 ? (by - ay) / len : 0
	};
}

/** Landmarks that keep a transmission drawing findable when zoomed out, in priority order:
 * generators by capacity (one per bus), then transformers by their higher voltage. Placed by
 * `placeLandmarks` like the multiconductor ones. Branch states come from `branchState` when a
 * result is shown. */
export function txLandmarks(
	graph: TxGraph,
	geometry: SchematicGeometry,
	branchState: ((k: number) => Landmark['state']) | null = null,
	/** Per-branch anchors on the drawn geometry (`edgeAnchors`); bus midpoints when absent. */
	anchors: Float32Array | null = null
): Landmark[] {
	const out: Landmark[] = [];
	for (let i = 0; i < graph.busId.length; i++) {
		if (!(graph.genMw[i] > 0)) continue;
		out.push({
			kind: 'generator',
			x: geometry.positions[i * 3],
			y: geometry.positions[i * 3 + 1],
			cos: 1,
			sin: 0,
			tier: 2,
			weight: graph.genMw[i],
			state: null,
			ids: [String(graph.busId[i])],
			bus: i,
			edge: -1
		});
	}
	for (let k = 0; k < graph.from.length; k++) {
		if (!graph.transformer[k] || !graph.inService[k]) continue;
		const anchor = anchors
			? {
					x: anchors[k * 4],
					y: anchors[k * 4 + 1],
					cos: anchors[k * 4 + 2],
					sin: anchors[k * 4 + 3]
				}
			: branchAnchor(graph, geometry, k);
		if (!anchor || !Number.isFinite(anchor.x)) continue;
		const state = branchState?.(k) ?? null;
		out.push({
			kind: 'transformer',
			...anchor,
			// A transformer near its rating is worth finding before a quiet one.
			tier: state === 'violation' || state === 'warning' ? 1 : 3,
			weight: graph.branchKv[k],
			state,
			ids: [],
			bus: -1,
			edge: k
		});
	}
	return out.sort((a, b) => a.tier - b.tier || b.weight - a.weight);
}

const mw = (v: number) => `${v.toLocaleString(undefined, { maximumFractionDigits: 1 })} MW`;

/** A bus as a tooltip reads it: name and number, voltage level, substation, and its value. */
export function txBusText(
	graph: TxGraph,
	network: Network,
	i: number,
	value: { label: string; text: string } | null = null
): string {
	const bus = network.buses[i];
	const sub = graph.substations[graph.substation[i]];
	const lines = [
		`${bus.name ? `${bus.name} · ` : ''}bus ${bus.id}`,
		[
			graph.kv[i] > 0 ? levelLabel(graph.kv[i] * 1000) : null,
			graph.grouped && sub ? `substation ${sub.name ?? sub.key.replace(/^(sub|bus):/, '')}` : null
		]
			.filter(Boolean)
			.join(' · '),
		[
			graph.demandMw[i] ? `load ${mw(graph.demandMw[i])}` : null,
			graph.genMw[i] ? `generation up to ${mw(graph.genMw[i])}` : null
		]
			.filter(Boolean)
			.join(' · '),
		value ? `${value.label} ${value.text}` : null
	];
	return lines.filter(Boolean).join('\n');
}

/** A branch as a tooltip reads it: kind, ends, voltage, rating and solved flow. */
export function txBranchText(
	graph: TxGraph,
	network: Network,
	k: number,
	flow: { mw: number; loading: number } | null = null
): string {
	const br = network.branches[k];
	const kind = graph.transformer[k] ? 'Transformer' : 'Line';
	const [a, b] = [graph.from[k], graph.to[k]];
	const kv =
		graph.transformer[k] && a >= 0 && b >= 0
			? [graph.kv[a], graph.kv[b]]
					.filter((v) => v > 0)
					.map((v) => levelLabel(v * 1000))
					.join(' / ')
			: graph.branchKv[k] > 0
				? levelLabel(graph.branchKv[k] * 1000)
				: '';
	return [
		`${kind} ${br.id}, bus ${br.from} to bus ${br.to}`,
		[kv, br.rate_mw > 0 ? `rating ${mw(br.rate_mw)}` : null, br.status ? null : 'out of service']
			.filter(Boolean)
			.join(' · '),
		flow ? `flow ${mw(flow.mw)} · ${(flow.loading * 100).toFixed(0)} % of rating` : null
	]
		.filter(Boolean)
		.join('\n');
}

/** The engine layout's input: the graph's columns with the shown positions as seeds, so the
 * schematic comes back in the same frame (and the same Web Mercator origin) as the geography. */
export function txLayoutInput(graph: TxGraph, seeds: SchematicGeometry): TxLayoutInput {
	const n = graph.busId.length;
	const xy = new Array<number>(n * 2);
	for (let i = 0; i < n; i++) {
		xy[i * 2] = seeds.positions[i * 3];
		xy[i * 2 + 1] = seeds.positions[i * 3 + 1];
	}
	return {
		bus_substation: Array.from(graph.substation),
		kv: Array.from(graph.kv),
		from: Array.from(graph.from),
		to: Array.from(graph.to),
		in_service: Array.from(graph.inService),
		xy
	};
}

/** What a substation overview draws beyond buses and branches. */
export interface TxSchematicParts {
	/** `x0, y0, x1, y1` per bus. */
	busbars: Float32Array;
	/** `x0, y0, x1, y1` per substation. */
	boxes: Float32Array;
	/** Drawing units per interior grid block, before tier scaling. */
	unit: number;
}

/** The engine's substation overview as GPU-ready geometry, keeping the seeds' Web Mercator origin
 * so the basemap stays registered. */
export function fromTxLayout(
	layout: TxLayout,
	origin: [number, number] | undefined
): { geometry: SchematicGeometry; parts: TxSchematicParts } {
	const n = layout.positions.length / 2;
	const positions = new Float32Array(n * 3);
	for (let i = 0; i < n; i++) {
		positions[i * 3] = layout.positions[i * 2];
		positions[i * 3 + 1] = layout.positions[i * 2 + 1];
	}
	const s = layout.segment_edge.length;
	const sources = new Float32Array(s * 3);
	const targets = new Float32Array(s * 3);
	for (let j = 0; j < s; j++) {
		sources[j * 3] = layout.segments[j * 4];
		sources[j * 3 + 1] = layout.segments[j * 4 + 1];
		targets[j * 3] = layout.segments[j * 4 + 2];
		targets[j * 3 + 1] = layout.segments[j * 4 + 3];
	}
	return {
		geometry: {
			positions,
			sources,
			targets,
			segmentEdge: Int32Array.from(layout.segment_edge),
			role: Uint8Array.from(layout.role),
			bounds: layout.bounds,
			version: layout.version,
			...(origin ? { mercatorOrigin: origin } : {})
		},
		parts: {
			busbars: Float32Array.from(layout.busbars),
			boxes: Float32Array.from(layout.boxes),
			unit: layout.unit
		}
	};
}

/** Per branch, where its symbol and label go on the drawn geometry: the point halfway along its
 * drawn path, with the direction there (`x, y, cos, sin`), and the path length. NaN when the
 * branch is not drawn. */
export function edgeAnchors(
	geometry: SchematicGeometry,
	edges: number
): { anchors: Float32Array; lengths: Float32Array } {
	const anchors = new Float32Array(edges * 4).fill(NaN);
	const lengths = new Float32Array(edges);
	const { sources: a, targets: b, segmentEdge } = geometry;
	const len = (j: number) => Math.hypot(b[j * 3] - a[j * 3], b[j * 3 + 1] - a[j * 3 + 1]);
	for (let j = 0; j < segmentEdge.length; j++) lengths[segmentEdge[j]] += len(j);
	const walked = new Float32Array(edges);
	for (let j = 0; j < segmentEdge.length; j++) {
		const k = segmentEdge[j];
		if (!Number.isNaN(anchors[k * 4])) continue;
		const l = len(j);
		const half = lengths[k] / 2;
		if (walked[k] + l >= half) {
			const t = l > 0 ? (half - walked[k]) / l : 0.5;
			const dx = b[j * 3] - a[j * 3];
			const dy = b[j * 3 + 1] - a[j * 3 + 1];
			anchors.set(
				[a[j * 3] + t * dx, a[j * 3 + 1] + t * dy, l > 0 ? dx / l : 1, l > 0 ? dy / l : 0],
				k * 4
			);
		}
		walked[k] += l;
	}
	return { anchors, lengths };
}

/** Which way active power flows along each branch's drawn path at its anchor: +1 along the path's
 * direction there, -1 against. `mw(k)` gives branch `k`'s flow, positive from → to, or null when
 * unknown; zero and unknown flows and undrawn branches are left out. */
export function flowDirections(
	graph: TxGraph,
	_geometry: SchematicGeometry,
	anchors: Float32Array,
	mw: (k: number) => number | null
): { edges: number[]; dir: Int8Array } {
	const edges: number[] = [];
	const dir: number[] = [];
	for (let k = 0; k < graph.from.length; k++) {
		const f = mw(k);
		const [a, b] = [graph.from[k], graph.to[k]];
		if (
			f === null ||
			!Number.isFinite(f) ||
			Math.abs(f) < 1e-6 ||
			!graph.inService[k] ||
			a < 0 ||
			b < 0 ||
			!Number.isFinite(anchors[k * 4])
		)
			continue;
		edges.push(k);
		// Source routes are normalized by txSourceLayout; engine routes also run from → to.
		dir.push(Math.sign(f));
	}
	return { edges, dir: Int8Array.from(dir) };
}
