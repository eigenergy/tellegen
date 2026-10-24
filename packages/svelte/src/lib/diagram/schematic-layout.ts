import type { DistGraph, SchematicLayout } from '@tellegen/engine';
import type { DisplayTree } from './display-tree.js';

/** Drawing geometry for one layout: bus positions and branch segments (several per routed edge). */
export interface SchematicGeometry {
	/** `x, y, 0` per bus in graph order; y grows downwards. */
	positions: Float32Array;
	sources: Float32Array;
	targets: Float32Array;
	/** Edge index (graph order) for each segment. */
	segmentEdge: Int32Array;
	/** Per edge: 0 tree route, 1 closed non-tree link, 2 open or unreachable. */
	role: Uint8Array;
	bounds: [number, number, number, number];
	/** Engine layout version, for provenance; absent for source positions. */
	version?: number;
	/** Room reserved below each bus per attachment kind (engine layouts only). */
	attachmentGap?: number;
	/** For source positions: buses the data gives no position, drawn next to a connected bus
	 * instead (1), or left at the origin because nothing placed connects to them (2). */
	inferred?: Uint8Array;
	/** For geographic source positions: the Web Mercator point (metres, y down) the positions are
	 * relative to, so map tiles can be placed under them. */
	mercatorOrigin?: [number, number];
}

function segments(
	graph: DistGraph,
	positions: Float32Array,
	index: (id: string) => number,
	route: (edge: number, a: number, b: number) => number[][] | null
): Omit<SchematicGeometry, 'positions' | 'bounds' | 'role'> {
	const src: number[] = [];
	const dst: number[] = [];
	const owner: number[] = [];
	graph.edges.forEach((e, k) => {
		const a = index(e.from);
		const b = index(e.to);
		if (a < 0 || b < 0) return;
		const points = route(k, a, b) ?? [
			[positions[a * 3], positions[a * 3 + 1]],
			[positions[b * 3], positions[b * 3 + 1]]
		];
		for (let i = 1; i < points.length; i++) {
			src.push(points[i - 1][0], points[i - 1][1], 0);
			dst.push(points[i][0], points[i][1], 0);
			owner.push(k);
		}
	});
	return {
		sources: Float32Array.from(src),
		targets: Float32Array.from(dst),
		segmentEdge: Int32Array.from(owner)
	};
}

function bounds(positions: Float32Array): [number, number, number, number] {
	let [x0, y0, x1, y1] = [Infinity, Infinity, -Infinity, -Infinity];
	for (let i = 0; i < positions.length; i += 3) {
		x0 = Math.min(x0, positions[i]);
		x1 = Math.max(x1, positions[i]);
		y0 = Math.min(y0, positions[i + 1]);
		y1 = Math.max(y1, positions[i + 1]);
	}
	return Number.isFinite(x0) ? [x0, y0, x1, y1] : [0, 0, 1, 1];
}

const indexer = (graph: DistGraph) => {
	const map = new Map(graph.buses.map((b, i) => [b.id.toLowerCase(), i]));
	return (id: string) => map.get(id.toLowerCase()) ?? -1;
};

/** Engine schematic layout (`mcSchematicLayout`, computed in Rust) as GPU-ready arrays. */
export function fromEngineLayout(layout: SchematicLayout): SchematicGeometry {
	const n = layout.positions.length / 2;
	const positions = new Float32Array(n * 3);
	for (let i = 0; i < n; i++) {
		positions[i * 3] = layout.positions[i * 2];
		positions[i * 3 + 1] = layout.positions[i * 2 + 1];
	}
	const m = layout.segment_edge.length;
	const sources = new Float32Array(m * 3);
	const targets = new Float32Array(m * 3);
	for (let s = 0; s < m; s++) {
		sources[s * 3] = layout.segments[s * 4];
		sources[s * 3 + 1] = layout.segments[s * 4 + 1];
		targets[s * 3] = layout.segments[s * 4 + 2];
		targets[s * 3 + 1] = layout.segments[s * 4 + 3];
	}
	return {
		positions,
		sources,
		targets,
		segmentEdge: Int32Array.from(layout.segment_edge),
		role: Uint8Array.from(layout.role),
		bounds: layout.bounds,
		version: layout.version,
		attachmentGap: layout.attachment_gap
	};
}

/** An empty drawing, before the first layout arrives. */
export const EMPTY_GEOMETRY: SchematicGeometry = {
	positions: new Float32Array(),
	sources: new Float32Array(),
	targets: new Float32Array(),
	segmentEdge: new Int32Array(),
	role: new Uint8Array(),
	bounds: [0, 0, 1, 1]
};

/** Positions as placed in the source data: geographic longitude/latitude projected to Web Mercator
 * metres (y down), or drawing coordinates as given (already y down, as `DiagramCanvas` draws them),
 * relative to their centroid. Branches are drawn straight. */
export function sourceLayout(
	graph: DistGraph,
	placed: { id: string; lon: number; lat: number }[],
	geographic: boolean,
	tree: DisplayTree,
	/** Routed paths from the case (`MultiView.edges`), drawn in place of straight segments. */
	routes: {
		id: string;
		from: string;
		to: string;
		path: [number, number][];
	}[] = []
): SchematicGeometry {
	const R = 6_378_137;
	const byId = new Map(placed.map((b) => [b.id.toLowerCase(), b]));
	const project = (lon: number, lat: number): [number, number] => {
		if (!geographic) return [lon, lat];
		const clamped = Math.max(-85, Math.min(85, lat));
		return [
			(R * lon * Math.PI) / 180,
			-R * Math.log(Math.tan(Math.PI / 4 + (clamped * Math.PI) / 360))
		];
	};
	// Project in float64 and store relative to the centroid: absolute Web Mercator metres reach 1e7,
	// where float32 resolves only to about a metre and neighbouring LV buses would collapse.
	const xy = graph.buses.map((b) => {
		const p = byId.get(b.id.toLowerCase());
		return p ? project(p.lon, p.lat) : null;
	});
	const known = xy.filter((p): p is [number, number] => p !== null);
	const cx = known.reduce((s, p) => s + p[0], 0) / Math.max(1, known.length);
	const cy = known.reduce((s, p) => s + p[1], 0) / Math.max(1, known.length);
	const positions = new Float32Array(graph.buses.length * 3);
	xy.forEach((p, i) => {
		if (!p) return;
		positions[i * 3] = p[0] - cx;
		positions[i * 3 + 1] = p[1] - cy;
	});
	const index = indexer(graph);
	// A bus without a position (an LV stub behind a pole transformer, say) is drawn next to the
	// nearest connected bus that has one, found breadth-first, rather than at the origin, where its
	// branches would all run to one false point. The offset is a fraction of a typical branch.
	const inferred = new Uint8Array(graph.buses.length);
	if (known.length < graph.buses.length) {
		const neighbours: number[][] = graph.buses.map(() => []);
		graph.edges.forEach((e) => {
			const a = index(e.from);
			const b = index(e.to);
			if (a < 0 || b < 0 || a === b) return;
			neighbours[a].push(b);
			neighbours[b].push(a);
		});
		const lengths: number[] = [];
		graph.edges.forEach((e) => {
			const a = index(e.from);
			const b = index(e.to);
			if (a >= 0 && b >= 0 && xy[a] && xy[b])
				lengths.push(Math.hypot(xy[a]![0] - xy[b]![0], xy[a]![1] - xy[b]![1]));
		});
		const positive = lengths.filter((l) => l > 0).sort((p, q) => p - q);
		const step = (positive.length ? positive[positive.length >> 1] : 1) * 0.5;
		const placedAt = xy.map((p) => p !== null);
		const queue = xy.flatMap((p, i) => (p ? [i] : []));
		const children = new Int32Array(graph.buses.length);
		for (let head = 0; head < queue.length; head++) {
			const u = queue[head];
			for (const v of neighbours[u]) {
				if (placedAt[v]) continue;
				placedAt[v] = true;
				inferred[v] = 1;
				// Fan several unplaced neighbours of one bus around it.
				const angle = Math.PI / 4 + children[u]++ * 2.399963;
				positions[v * 3] = positions[u * 3] + step * Math.cos(angle);
				positions[v * 3 + 1] = positions[u * 3 + 1] + step * Math.sin(angle);
				queue.push(v);
			}
		}
		placedAt.forEach((ok, i) => {
			if (!ok) inferred[i] = 2;
		});
	}
	const role = new Uint8Array(graph.edges.length);
	graph.edges.forEach((e, k) => {
		const a = index(e.from);
		const b = index(e.to);
		role[k] =
			!e.closed || a < 0 || b < 0
				? 2
				: tree.parentEdge[a] === k || tree.parentEdge[b] === k
					? 0
					: 1;
	});
	return {
		positions,
		...segments(graph, positions, index, (k) => {
			// A route belongs to one edge by id and endpoints; parallel edges keep their own paths.
			const e = graph.edges[k];
			const same = (a: string, b: string) => a.toLowerCase() === b.toLowerCase();
			const route = routes.find(
				(r) =>
					same(r.id, e.id) &&
					((same(r.from, e.from) && same(r.to, e.to)) || (same(r.from, e.to) && same(r.to, e.from)))
			);
			if (!route || route.path.length < 2) return null;
			return route.path.map(([lon, lat]) => {
				const [x, y] = project(lon, lat);
				return [x - cx, y - cy];
			});
		}),
		role,
		bounds: bounds(positions),
		...(inferred.some((v) => v) ? { inferred } : {}),
		...(geographic ? { mercatorOrigin: [cx, cy] as [number, number] } : {})
	};
}

/** The drawn schematic as a PowerIO diagram layer (GeoJSON with `powerio_geo.space = "diagram"`):
 * one point per bus and one route per line, matched by line id, y down. `kind: "synthetic"` marks
 * the positions as computed, and `tellegen_layout` records how. Load it with the case to reuse the
 * layout, or hand it to PowerIO tooling. */
export function diagramLayer(
	graph: DistGraph,
	geometry: SchematicGeometry,
	provenance: { strategy: string; version: number }
): string {
	const round = (v: number) => Math.round(v * 100) / 100;
	const features: unknown[] = graph.buses.map((bus, i) => ({
		type: 'Feature',
		properties: { bus: bus.id },
		geometry: {
			type: 'Point',
			coordinates: [round(geometry.positions[i * 3]), round(geometry.positions[i * 3 + 1])]
		}
	}));
	const routes = new Map<number, number[][]>();
	for (let s = 0; s < geometry.segmentEdge.length; s++) {
		const k = geometry.segmentEdge[s];
		const points = routes.get(k) ?? [
			[round(geometry.sources[s * 3]), round(geometry.sources[s * 3 + 1])]
		];
		points.push([round(geometry.targets[s * 3]), round(geometry.targets[s * 3 + 1])]);
		routes.set(k, points);
	}
	graph.edges.forEach((edge, k) => {
		const points = routes.get(k);
		// Only lines carry routes in a PowerIO layer; switches and transformers follow their buses.
		if (edge.kind !== 'line' || !points || points.length < 2) return;
		features.push({
			type: 'Feature',
			properties: { id: edge.id, bus_from: edge.from, bus_to: edge.to },
			geometry: { type: 'LineString', coordinates: points }
		});
	});
	return JSON.stringify({
		type: 'FeatureCollection',
		powerio_geo: { space: 'diagram', kind: 'synthetic' },
		tellegen_layout: { algorithm: 'schematic', ...provenance },
		features
	});
}
