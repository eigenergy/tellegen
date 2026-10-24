import type { DistGraph } from '@tellegen/engine';
import type { SchematicGeometry } from './schematic-layout.js';

/** Buses drawn at exactly the same point (an MV and an LV bus at one pole, say). */
export interface BusStacks {
	/** Groups of two or more bus indices, in graph order. */
	groups: number[][];
	/** Per bus: its group index, or −1 when it stands alone. */
	stackOf: Int32Array;
}

export function busStacks(geometry: SchematicGeometry): BusStacks {
	const n = geometry.positions.length / 3;
	const byPoint = new Map<string, number[]>();
	for (let i = 0; i < n; i++) {
		const key = `${geometry.positions[i * 3]},${geometry.positions[i * 3 + 1]}`;
		const list = byPoint.get(key);
		if (list) list.push(i);
		else byPoint.set(key, [i]);
	}
	const groups: number[][] = [];
	const stackOf = new Int32Array(n).fill(-1);
	for (const list of byPoint.values())
		if (list.length > 1) {
			for (const i of list) stackOf[i] = groups.length;
			groups.push(list);
		}
	return { groups, stackOf };
}

/** Offsets in drawing units that spread a stack's members on a circle of `radiusPx` screen pixels
 * (`scale` pixels per unit): two side by side, more around the circle from the top. */
export function spreadOffsets(count: number, scale: number, radiusPx = 12): [number, number][] {
	const r = radiusPx / scale;
	if (count === 2)
		return [
			[-r, 0],
			[r, 0]
		];
	return Array.from({ length: count }, (_, i): [number, number] => {
		const a = -Math.PI / 2 + (2 * Math.PI * i) / count;
		return [r * Math.cos(a), r * Math.sin(a)];
	});
}

/** `geometry` with the given buses moved by their offsets, and every branch end at a moved bus
 * moved with it (interior route points stay put, so routes bend from the spread bus back onto
 * their path). A branch whose two ends share a point (a pole transformer between stacked buses)
 * takes its first end as `from` and its last as `to`. Returns `geometry` itself when nothing
 * moves. */
export function displaceBuses(
	graph: DistGraph,
	geometry: SchematicGeometry,
	offsets: Map<number, [number, number]>
): SchematicGeometry {
	if (!offsets.size) return geometry;
	const positions = Float32Array.from(geometry.positions);
	for (const [i, [dx, dy]] of offsets) {
		positions[i * 3] += dx;
		positions[i * 3 + 1] += dy;
	}
	const sources = Float32Array.from(geometry.sources);
	const targets = Float32Array.from(geometry.targets);
	const index = new Map(graph.buses.map((b, i) => [b.id.toLowerCase(), i]));
	// First and last segment of each edge.
	const first = new Map<number, number>();
	const last = new Map<number, number>();
	for (let s = 0; s < geometry.segmentEdge.length; s++) {
		const k = geometry.segmentEdge[s];
		if (!first.has(k)) first.set(k, s);
		last.set(k, s);
	}
	const at = (i: number): [number, number] => [
		geometry.positions[i * 3],
		geometry.positions[i * 3 + 1]
	];
	const near = (x: number, y: number, i: number) => {
		const [px, py] = at(i);
		return x === px && y === py;
	};
	graph.edges.forEach((e, k) => {
		const s0 = first.get(k);
		const s1 = last.get(k);
		if (s0 === undefined || s1 === undefined) return;
		const a = index.get(e.from.toLowerCase()) ?? -1;
		const b = index.get(e.to.toLowerCase()) ?? -1;
		const sx = geometry.sources[s0 * 3];
		const sy = geometry.sources[s0 * 3 + 1];
		const tx = geometry.targets[s1 * 3];
		const ty = geometry.targets[s1 * 3 + 1];
		// Which bus each end belongs to: the one drawn there, `from` first when both are.
		const startBus = a >= 0 && near(sx, sy, a) ? a : b >= 0 && near(sx, sy, b) ? b : -1;
		const endBus =
			b >= 0 && near(tx, ty, b) && b !== startBus
				? b
				: a >= 0 && near(tx, ty, a) && a !== startBus
					? a
					: -1;
		const ds = startBus >= 0 ? offsets.get(startBus) : undefined;
		const de = endBus >= 0 ? offsets.get(endBus) : undefined;
		if (ds) {
			sources[s0 * 3] += ds[0];
			sources[s0 * 3 + 1] += ds[1];
		}
		if (de) {
			targets[s1 * 3] += de[0];
			targets[s1 * 3 + 1] += de[1];
		}
	});
	return { ...geometry, positions, sources, targets };
}
