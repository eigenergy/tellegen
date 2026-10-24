import type { DistAttachmentKind, DistGraph } from '@tellegen/engine';
import type { GalvanicZones, ZoneState } from './galvanic-zones.js';
import {
	attachmentGlyph,
	branchGlyph,
	type EquipmentSymbols,
	type Glyph
} from './schematic-equipment.js';
import type { SchematicGeometry } from './schematic-layout.js';
import { transformerGlyph, transformerInfo } from './connections.js';

/** Equipment that should stay findable when the drawing is zoomed out: drawn at a fixed screen
 * size, placed by priority so the overview never fills with markers. */
export interface Landmark {
	kind: 'source' | 'generator' | 'ibr' | 'transformer';
	x: number;
	y: number;
	/** Direction along the branch, for transformers. */
	cos: number;
	sin: number;
	/** Lower is placed first: 0 sources (always), 1 transformers whose network has a problem,
	 * 2 machines and large IBRs, 3 other transformers, 4 small IBRs. */
	tier: number;
	/** Within a tier, larger first: generation in W, or the fed network's bus count. */
	weight: number;
	state: ZoneState | null;
	ids: string[];
	bus: number;
	edge: number;
	/** The marker, when it differs by unit: a transformer's winding marks or centre tap. */
	glyph?: Glyph;
}

/** An IBR counts as large from this share of all generation and IBR output upwards. */
export const LARGE_IBR_SHARE = 0.01;

/** Landmark candidates in priority order. Generators and IBRs are grouped per bus and kind, sized by
 * solved output when `powers` is given, else by the bus's declared generation. */
export function landmarkCandidates(
	graph: DistGraph,
	geometry: SchematicGeometry,
	symbols: EquipmentSymbols | null,
	zones: GalvanicZones,
	zoneStates: ZoneState[],
	powers: Map<string, { p: number; q: number }> | null
): Landmark[] {
	const out: Landmark[] = [];
	const at = (i: number) => [geometry.positions[i * 3], geometry.positions[i * 3 + 1]];
	const gens: Landmark[] = [];
	graph.buses.forEach((bus, i) => {
		const byKind = new Map<DistAttachmentKind, Set<string>>();
		for (const list of Object.values(bus.terminal_attachments ?? {}))
			for (const a of list) byKind.set(a.kind, (byKind.get(a.kind) ?? new Set()).add(a.id));
		const [x, y] = at(i);
		const base = { x, y, cos: 1, sin: 0, state: null, bus: i, edge: -1 };
		if (bus.has_source || byKind.has('source'))
			out.push({
				...base,
				kind: 'source',
				tier: 0,
				weight: 0,
				ids: [...(byKind.get('source') ?? [bus.id])]
			});
		for (const kind of ['generator', 'ibr'] as const) {
			const ids = [...(byKind.get(kind) ?? [])];
			if (!ids.length) continue;
			const solved = powers
				? ids.reduce((sum, id) => sum + Math.max(0, -(powers.get(id)?.p ?? 0)), 0)
				: 0;
			const weight = solved || Math.max(0, bus.gen_kw * 1000);
			gens.push({ ...base, kind, tier: 2, weight, ids });
		}
	});
	const total = gens.reduce((sum, g) => sum + g.weight, 0);
	for (const g of gens)
		if (g.kind === 'ibr' && !(total > 0 && g.weight >= LARGE_IBR_SHARE * total)) g.tier = 4;
	out.push(...gens);

	// The network each transformer feeds, for its state and size.
	const fed = new Map<number, number>();
	zones.zones.forEach((z, zi) => {
		if (z.transformers.length && !z.hasSource) fed.set(z.transformers[0], zi);
	});
	const index = new Map(graph.buses.map((b, i) => [b.id.toLowerCase(), i]));
	// One marker per transformer, on its first winding-pair edge.
	const seen = new Set<string>();
	graph.edges.forEach((edge, k) => {
		if (edge.kind !== 'transformer' || seen.has(edge.id)) return;
		seen.add(edge.id);
		let x = symbols?.anchors[k * 2] ?? NaN;
		let y = symbols?.anchors[k * 2 + 1] ?? NaN;
		// A vertical branch has cos exactly 0, so test for a missing direction, not a falsy one.
		const fx = symbols?.flowDir[k * 2] ?? 0;
		const fy = symbols?.flowDir[k * 2 + 1] ?? 0;
		const known = fx !== 0 || fy !== 0;
		let cos = known ? fx : 1;
		let sin = known ? fy : 0;
		if (!Number.isFinite(x)) {
			const a = index.get(edge.from.toLowerCase());
			const b = index.get(edge.to.toLowerCase());
			if (a === undefined || b === undefined) return;
			const [ax, ay] = at(a);
			const [bx, by] = at(b);
			[x, y] = [(ax + bx) / 2, (ay + by) / 2];
			const len = Math.hypot(bx - ax, by - ay) || 1;
			[cos, sin] = [(bx - ax) / len, (by - ay) / len];
		}
		const zi = fed.get(k);
		const state = zi === undefined ? null : zoneStates[zi];
		const problem = state === 'violation' || state === 'warning';
		out.push({
			kind: 'transformer',
			x,
			y,
			cos,
			sin,
			tier: problem ? 1 : 3,
			weight: zi === undefined ? 0 : zones.zones[zi].buses.length,
			state,
			ids: [edge.id],
			bus: -1,
			edge: k,
			glyph: transformerGlyph(transformerInfo(graph, k), edge.n_phases, false)
		});
	});
	return out.sort((a, b) => a.tier - b.tier || b.weight - a.weight);
}

/** Greedy placement on a coarse screen grid: sources always; others only inside the view, on a
 * free cell (and its neighbours), up to `budget` markers. */
export function placeLandmarks(
	candidates: Landmark[],
	toScreen: (x: number, y: number) => [number, number],
	view: { width: number; height: number; cell?: number; budget?: number }
): Landmark[] {
	const cell = view.cell ?? 28;
	const budget = view.budget ?? 60;
	const taken = new Set<string>();
	const placed: Landmark[] = [];
	for (const c of candidates) {
		const [sx, sy] = toScreen(c.x, c.y);
		const inside = sx >= 0 && sx <= view.width && sy >= 0 && sy <= view.height;
		if (!inside) continue;
		const gx = Math.floor(sx / cell);
		const gy = Math.floor(sy / cell);
		if (c.tier > 0) {
			if (placed.length >= budget) break;
			let free = true;
			for (let dx = -1; dx <= 1 && free; dx++)
				for (let dy = -1; dy <= 1 && free; dy++)
					if (taken.has(`${gx + dx},${gy + dy}`)) free = false;
			if (!free) continue;
		}
		taken.add(`${gx},${gy}`);
		placed.push(c);
	}
	return placed;
}

/** The marker glyph for a landmark, centred on the origin: the symbol itself without its lead. */
export function landmarkGlyph(kind: Landmark['kind']): Glyph {
	if (kind === 'transformer') return branchGlyph('transformer', true)!.slice(1, 3);
	return attachmentGlyph(kind)
		.slice(1)
		.map((line) => line.map(([x, y]): [number, number] => [x - 1.3, y]));
}

/** Stroke segments for placed landmarks, `unit` drawing units per glyph unit (a fixed number of
 * pixels at the current zoom), with the landmark index of each segment. */
export function landmarkStrokes(
	placed: Landmark[],
	unit: number
): { sources: Float32Array; targets: Float32Array; owner: Int32Array } {
	const src: number[] = [];
	const dst: number[] = [];
	const owner: number[] = [];
	placed.forEach((l, j) => {
		for (const line of l.glyph ?? landmarkGlyph(l.kind))
			for (let i = 1; i < line.length; i++) {
				const [ax, ay] = line[i - 1];
				const [bx, by] = line[i];
				src.push(l.x + unit * (ax * l.cos - ay * l.sin), l.y + unit * (ax * l.sin + ay * l.cos), 0);
				dst.push(l.x + unit * (bx * l.cos - by * l.sin), l.y + unit * (bx * l.sin + by * l.cos), 0);
				owner.push(j);
			}
	});
	return {
		sources: Float32Array.from(src),
		targets: Float32Array.from(dst),
		owner: Int32Array.from(owner)
	};
}
