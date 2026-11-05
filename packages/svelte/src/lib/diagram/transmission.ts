import type { Network, NetworkSubstation } from '@tellegen/engine';
import type { Landmark } from './landmarks.js';
import type { SchematicGeometry } from './schematic-layout.js';
import { levelLabel } from './voltage-levels.js';

/** Voltage classes for transmission drawings, by nominal kV. Bands follow the common levels:
 * 765/500, 345/400, 220/230, 115–161, 66/69, and everything below (subtransmission, generator
 * terminals). */
export type TxKvClass = 'ehv' | 'ehv2' | 'hv' | 'hv2' | 'sub' | 'low';

export const TX_KV_CLASSES: { cls: TxKvClass; name: string; from: number }[] = [
	{ cls: 'ehv', name: '500 kV and above', from: 450 },
	{ cls: 'ehv2', name: '300–450 kV', from: 300 },
	{ cls: 'hv', name: '200–300 kV', from: 200 },
	{ cls: 'hv2', name: '100–200 kV', from: 100 },
	{ cls: 'sub', name: '50–100 kV', from: 50 },
	{ cls: 'low', name: 'below 50 kV', from: 0 }
];

/** The class of a nominal voltage in kV; unknown (0 or missing) counts as the lowest. */
export function txKvClass(kv: number): TxKvClass {
	return TX_KV_CLASSES.find((c) => kv >= c.from)!.cls;
}

/** Rank of a class, 0 for the highest: for ordering, widths and shades. */
export const txKvRank = (cls: TxKvClass): number => TX_KV_CLASSES.findIndex((c) => c.cls === cls);

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
		return a > 0 && b > 0 && Math.abs(a - b) > 0.01 * Math.max(a, b) ? 1 : 0;
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
	branchState: ((k: number) => Landmark['state']) | null = null
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
		const anchor = branchAnchor(graph, geometry, k);
		if (!anchor) continue;
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
