import type { RGBA } from '../../colors.js';
import type { Landmark, SchematicGeometry } from '../../diagram/index.js';
import { DANGER, DEAD, HIDDEN, INK, SYMBOL_INK, WARNING, lineWidths } from './palette.js';

/** Per-segment widths: MV heavier than LV. */
export function segmentWidths(
	g: SchematicGeometry,
	edgeMv: Uint8Array,
	resultColours: boolean
): Float32Array {
	const [mv, lv] = lineWidths(resultColours);
	return Float32Array.from(g.segmentEdge, (k) => (edgeMv[k] ? mv : lv));
}

/** Whether drawn segment `s` of edge `k` lies on the edge's shared (host) segment of `geometry`,
 * so it is replaced by the fanned path while its route is fanned. */
function onHost(
	d: SchematicGeometry,
	s: number,
	k: number,
	host: Int32Array,
	geometry: SchematicGeometry
): boolean {
	const h = host[k] ?? -1;
	if (h < 0) return false;
	const [ax, ay, bx, by] = [
		geometry.sources[h * 3],
		geometry.sources[h * 3 + 1],
		geometry.targets[h * 3],
		geometry.targets[h * 3 + 1]
	];
	const len = Math.hypot(bx - ax, by - ay) || 1;
	const off = (x: number, y: number) => Math.abs((bx - ax) * (ay - y) - (ax - x) * (by - ay)) / len;
	const tol = len * 1e-6 + 1e-9;
	return (
		off(d.sources[s * 3], d.sources[s * 3 + 1]) <= tol &&
		off(d.targets[s * 3], d.targets[s * 3 + 1]) <= tol
	);
}

/** Drawn segments replaced by their fanned path right now. */
export interface FanState {
	edges: Set<number>;
	host: Int32Array | null;
	geometry: SchematicGeometry;
}

export function hiddenUnderFan(d: SchematicGeometry, s: number, fan: FanState): boolean {
	const k = d.segmentEdge[s];
	return !!fan.host && fan.edges.has(k) && onHost(d, s, k, fan.host, fan.geometry);
}

/** Per-segment colours: the edge's colour, quieter for open or unreachable branches, hidden under
 * a fanned route, and faded for LV when zoomed out. */
export function segmentColours(
	g: SchematicGeometry,
	edgeColours: RGBA[],
	edgeMv: Uint8Array,
	zoomedOut: boolean,
	fan: FanState
): Uint8Array {
	const out = new Uint8Array(g.segmentEdge.length * 4);
	for (let s = 0; s < g.segmentEdge.length; s++) {
		const k = g.segmentEdge[s];
		const c = edgeColours[k];
		let a = g.role[k] === 2 ? Math.min(110, c[3]) : c[3];
		if (hiddenUnderFan(g, s, fan)) a = 0;
		if (zoomedOut && !edgeMv[k]) a *= 0.45;
		out.set([c[0], c[1], c[2], a], s * 4);
	}
	return out;
}

/** Per symbol stroke: ink, or faint when its bus or branch is not energised; hidden for a branch
 * whose route is fanned (its marks are redrawn on the fanned path). An owner is an edge index, or
 * `-bus - 1` for a bus. */
export function strokeColours(
	owner: Int32Array,
	fannedEdges: Set<number>,
	live: { edges: ArrayLike<boolean | number>; buses: ArrayLike<boolean | number> }
): Uint8Array {
	const out = new Uint8Array(owner.length * 4);
	for (let s = 0; s < owner.length; s++) {
		const o = owner[s];
		if (o >= 0 && fannedEdges.has(o)) {
			out.set(HIDDEN, s * 4);
			continue;
		}
		const on = o >= 0 ? live.edges[o] : live.buses[-o - 1];
		out.set(on ? SYMBOL_INK : DEAD, s * 4);
	}
	return out;
}

/** Landmark strokes in ink; a transformer whose network has a problem takes that state's colour. */
export function landmarkColours(m: {
	placed: Landmark[];
	strokes: { owner: Int32Array };
}): Uint8Array {
	const out = new Uint8Array(m.strokes.owner.length * 4);
	for (let s = 0; s < m.strokes.owner.length; s++) {
		const l = m.placed[m.strokes.owner[s]];
		out.set(l.state === 'violation' ? DANGER : l.state === 'warning' ? WARNING : INK, s * 4);
	}
	return out;
}
