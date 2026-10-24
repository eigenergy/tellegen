import type { DistAttachmentKind, DistConnection, DistGraph } from '@tellegen/engine';
import type { SchematicGeometry } from './schematic-layout.js';
import { terminalConductors, transformerGlyph, transformerInfo } from './connections.js';

/** A stroked glyph: polylines in symbol units, x along the connection, y down. */
export type Glyph = [number, number][][];

const circle = (cx: number, cy: number, r: number, from = 0, to = 360, steps = 20) => {
	const out: [number, number][] = [];
	for (let i = 0; i <= steps; i++) {
		const a = ((from + ((to - from) * i) / steps) * Math.PI) / 180;
		out.push([cx + r * Math.cos(a), cy + r * Math.sin(a)]);
	}
	return out;
};
const sine = (cx: number, cy: number, half: number, amp: number, steps = 12) =>
	Array.from({ length: steps + 1 }, (_, i): [number, number] => {
		const t = i / steps;
		return [cx - half + 2 * half * t, cy - amp * Math.sin(2 * Math.PI * t)];
	});

/** Branch glyphs, centred on the branch and spanning `±INLINE_HALF` along it, where the drawn line
 * is interrupted. Shapes follow the #177 specimens: a two-winding transformer as two overlapping
 * circles (no conductor continuity), a generic switch as a blade between two contacts, open when
 * the pole is open. A line has no glyph. */
export const INLINE_HALF = 1.4;
export function branchGlyph(kind: string, closed: boolean): Glyph | null {
	if (kind === 'transformer')
		return [
			[
				[-1.4, 0],
				[-1.15, 0]
			],
			circle(-0.45, 0, 0.7),
			circle(0.45, 0, 0.7),
			[
				[1.15, 0],
				[1.4, 0]
			]
		];
	if (kind === 'switch')
		return [
			[
				[-1.4, 0],
				[-1.05, 0]
			],
			circle(-0.9, 0, 0.15, 0, 360, 8),
			circle(0.9, 0, 0.15, 0, 360, 8),
			[
				[1.05, 0],
				[1.4, 0]
			],
			closed
				? [
						[-0.9, 0],
						[1.05, 0]
					]
				: [
						[-0.9, 0],
						[0.7, -0.85]
					]
		];
	return null;
}

/** Attachment glyphs, connected at the origin and extending along +x. A load is a demand arrow, a
 * generator a machine circle with a G, an external source an AC supply circle with a sinusoid, an
 * inverter-based resource a converter box (DC bar, diagonal, AC wave), and a shunt a general
 * admittance block: BMOPF shunts can be conductive, so no capacitor is asserted. A shunt on a bus's
 * neutral and nothing else is a neutral earth through an impedance (an MEN electrode, say): drawn
 * as an impedance box in series with the IEC 60617 earth (three shrinking bars), not as a solid
 * earth, since a shunt always has finite admittance. A perfectly grounded terminal is a solid earth:
 * the earth symbol alone. */
export function attachmentGlyph(kind: SymbolKind): Glyph {
	const lead: [number, number][] = [
		[0, 0],
		[0.4, 0]
	];
	switch (kind) {
		case 'load':
			return [
				[
					[0, 0],
					[0.6, 0]
				],
				[
					[0.6, -0.7],
					[1.8, 0],
					[0.6, 0.7],
					[0.6, -0.7]
				]
			];
		case 'generator':
			return [lead, circle(1.3, 0, 0.9), [...circle(1.3, 0, 0.45, -45, -360, 14), [1.4, 0]]];
		case 'source':
			return [lead, circle(1.3, 0, 0.9), sine(1.3, 0, 0.5, 0.3)];
		case 'ibr':
			return [
				lead,
				[
					[0.4, -0.9],
					[2.2, -0.9],
					[2.2, 0.9],
					[0.4, 0.9],
					[0.4, -0.9]
				],
				[
					[0.4, 0.9],
					[2.2, -0.9]
				],
				[
					[0.6, -0.5],
					[1.1, -0.5]
				],
				sine(1.75, 0.5, 0.3, 0.18, 8)
			];
		case 'ground':
			return [
				[
					[0, 0],
					[0.8, 0],
					[0.8, 0.3]
				],
				[
					[0.4, 0.3],
					[1.2, 0.3]
				],
				[
					[0.55, 0.55],
					[1.05, 0.55]
				],
				[
					[0.7, 0.8],
					[0.9, 0.8]
				]
			];
		case 'earth':
			return [
				[
					[0, 0],
					[0.3, 0]
				],
				[
					[0.3, -0.22],
					[1.1, -0.22],
					[1.1, 0.22],
					[0.3, 0.22],
					[0.3, -0.22]
				],
				[
					[1.1, 0],
					[1.4, 0],
					[1.4, 0.3]
				],
				[
					[1.0, 0.3],
					[1.8, 0.3]
				],
				[
					[1.15, 0.55],
					[1.65, 0.55]
				],
				[
					[1.3, 0.8],
					[1.5, 0.8]
				]
			];
		case 'shunt':
			return [
				lead,
				[
					[0.4, -0.9],
					[2.0, -0.9],
					[2.0, 0.9],
					[0.4, 0.9],
					[0.4, -0.9]
				],
				[
					[0.8, -0.4],
					[1.2, 0],
					[0.8, 0.4]
				],
				[
					[1.2, 0],
					[1.7, 0]
				]
			];
	}
}

/** Conductor marks across a line, centred on it: one slash per phase conductor and a slash ending
 * in a small circle for the neutral (the IEC 60617 convention). */
export function conductorTicks(phases: number, neutral: boolean): Glyph {
	const n = phases + (neutral ? 1 : 0);
	const out: Glyph = [];
	for (let i = 0; i < n; i++) {
		const x = (i - (n - 1) / 2) * 0.4;
		out.push([
			[x - 0.25, 0.45],
			[x + 0.25, -0.45]
		]);
		if (neutral && i === n - 1) out.push(circle(x + 0.33, -0.6, 0.13, 0, 360, 8));
	}
	return out;
}

/** Each graph's neutral terminal per bus id (lower case), built once: `conductorSet` runs per edge,
 * and a scan of the buses each time made the equipment quadratic in the network size. */
const neutralsByGraph = new WeakMap<DistGraph, Map<string, string | null>>();
function neutralOf(graph: DistGraph, bus: string): string | null {
	let index = neutralsByGraph.get(graph);
	if (!index) {
		index = new Map();
		// The first bus of an id wins, as a search in graph order would find it.
		for (const b of graph.buses) {
			const id = b.id.toLowerCase();
			if (!index.has(id)) index.set(id, b.neutral_terminal ?? null);
		}
		neutralsByGraph.set(graph, index);
	}
	return index.get(bus.toLowerCase()) ?? null;
}

/** Phase conductors and whether a neutral runs, from a branch's conductor pairs: a conductor whose
 * from terminal is the from bus's neutral is the neutral. */
export function conductorSet(graph: DistGraph, k: number): { phases: number; neutral: boolean } {
	const edge = graph.edges[k];
	const n = neutralOf(graph, edge.from);
	const neutral = n !== null && edge.conductors.some(([a]) => a === n);
	return { phases: edge.conductors.length - (neutral ? 1 : 0), neutral };
}

/** Attachment symbols: the PowerIO kinds, plus `earth` for a shunt on a bus's neutral alone and
 * `ground` for a bus's perfectly grounded terminals. */
export type SymbolKind = DistAttachmentKind | 'earth' | 'ground';

/** Drawing order of attachment kinds below a bus. A neutral earth shares the shunt slot (the
 * engine layout reserves one slot per PowerIO kind). */
export const ATTACHMENT_ORDER: SymbolKind[] = [
	'source',
	'generator',
	'ibr',
	'load',
	'shunt',
	'earth',
	'ground'
];
export const ATTACHMENT_NAMES: Record<SymbolKind, string> = {
	source: 'source',
	generator: 'generator',
	ibr: 'inverter-based resource',
	load: 'load',
	shunt: 'shunt',
	earth: 'neutral earth',
	ground: 'solid earth'
};

/** Shunts of `bus` connected to its neutral terminal and to nothing else: neutral earths. */
export function neutralEarths(bus: DistGraph['buses'][number]): Set<string> {
	const neutral = bus.neutral_terminal;
	const out = new Set<string>();
	if (!neutral) return out;
	const elsewhere = new Set<string>();
	for (const [terminal, list] of Object.entries(bus.terminal_attachments ?? {}))
		for (const a of list)
			if (a.kind === 'shunt') (terminal === neutral ? out : elsewhere).add(a.id);
	for (const id of elsewhere) out.delete(id);
	return out;
}

/** One symbol: what it stands for and where to pick it. */
export interface EquipmentItem {
	kind: SymbolKind | 'transformer' | 'switch';
	/** Branch index for inline symbols, bus index for attachments. */
	edge: number;
	bus: number;
	ids: string[];
	x: number;
	y: number;
	/** An attachment's terminals in its own order and its connection, where the graph has them. */
	terminals?: string[];
	connection?: DistConnection;
	/** A transformer's vector group and rating, such as "Dyn · 500 kVA". */
	label?: string;
}

/** Attachments of one kind at a bus that connect alike: same terminals, same connection. */
interface AttachmentVariant {
	ids: string[];
	terminals: string[] | null;
	connection?: DistConnection;
}

/** A bus's attachments by kind, split by how they connect. Without per-attachment detail (a graph
 * from before connections were exported) each kind is one variant without terminals. */
function attachmentVariants(
	bus: DistGraph['buses'][number],
	earths: Set<string>
): Map<SymbolKind, AttachmentVariant[]> {
	const out = new Map<SymbolKind, AttachmentVariant[]>();
	const symbol = (kind: DistAttachmentKind, id: string): SymbolKind =>
		kind === 'shunt' && earths.has(id) ? 'earth' : kind;
	if (bus.attachments?.length) {
		const keyed = new Map<string, AttachmentVariant>();
		for (const a of bus.attachments) {
			const kind = symbol(a.kind, a.id);
			const key = `${kind}|${a.connection ?? ''}|${a.terminals.join(',')}`;
			let v = keyed.get(key);
			if (!v) {
				v = { ids: [], terminals: a.terminals, connection: a.connection };
				keyed.set(key, v);
				out.set(kind, [...(out.get(kind) ?? []), v]);
			}
			v.ids.push(a.id);
		}
		return out;
	}
	for (const list of Object.values(bus.terminal_attachments ?? {}))
		for (const a of list) {
			const kind = symbol(a.kind, a.id);
			const v = out.get(kind)?.[0] ?? { ids: [], terminals: null };
			if (!v.ids.includes(a.id)) v.ids.push(a.id);
			out.set(kind, [v]);
		}
	return out;
}

/** Room along a slot for one attachment variant: the lead with its conductor marks, the glyph, and
 * its label. */
const LEAD = 1.6;
const VARIANT_STEP = 7.2;

export interface EquipmentSymbols {
	/** Glyph strokes: `x, y, 0` per segment end. */
	sources: Float32Array;
	targets: Float32Array;
	items: EquipmentItem[];
	/** The geometry with each branch line interrupted where its symbol sits. */
	cut: SchematicGeometry;
	/** Symbol unit in drawing units: glyphs are about two units across. */
	unit: number;
	/** Kinds drawn, for the legend. */
	kinds: Set<EquipmentItem['kind']>;
	/** Owner of each glyph stroke: an edge index, or `-(bus + 1)` for attachment strokes. */
	owner: Int32Array;
	/** Conductor marks on lines (drawn only when zoomed further in), with their edge per stroke. */
	ticks: { sources: Float32Array; targets: Float32Array; edge: Int32Array };
	/** Per edge: `x, y` where its symbol or conductor marks sit (NaN when not drawn). */
	anchors: Float32Array;
	/** Per edge: unit direction of the segment carrying the anchor, pointing from the edge's
	 * `from` bus towards its `to` bus, and the free length upstream of the anchor on it. */
	flowDir: Float32Array;
	room: Float32Array;
	/** Per edge: how many branches are drawn along exactly the same segment (1 when alone), as
	 * when an MV and an LV line hang between buses at the same positions. */
	sharedRoute: Uint8Array;
	/** Routes carrying more than one branch: their member edges, in drawing order. */
	routeGroups: number[][];
	/** Per edge: its index in `routeGroups`, or −1 when its route is its own. */
	routeOf: Int32Array;
	/** Per edge: the segment its symbol or marks sit on (−1 when not drawn). */
	host: Int32Array;
}

function medianEdgeLength(geometry: SchematicGeometry): number {
	const lengths: number[] = [];
	for (let s = 0; s < geometry.segmentEdge.length; s++)
		lengths.push(
			Math.hypot(
				geometry.targets[s * 3] - geometry.sources[s * 3],
				geometry.targets[s * 3 + 1] - geometry.sources[s * 3 + 1]
			)
		);
	const positive = lengths.filter((l) => l > 0).sort((a, b) => a - b);
	return positive.length ? positive[positive.length >> 1] : 1;
}

/** Symbols for every branch and attachment of `graph` drawn on `geometry`.
 *
 * Inline symbols sit on the child-end segment of a tree route, a fixed distance from the child bus
 * so they line up in columns; other branches carry theirs at the middle of their middle segment.
 * Attachments hang from a riser below the bus, one slot per kind (the engine layout reserves that
 * room), each on a short tick with its count when there are several. */
export function schematicEquipment(
	graph: DistGraph,
	geometry: SchematicGeometry
): EquipmentSymbols {
	const engine = geometry.attachmentGap !== undefined && geometry.attachmentGap > 0;
	const gap = engine ? geometry.attachmentGap! : medianEdgeLength(geometry) * 0.25;
	const unit = gap * 0.4;
	const src: number[] = [];
	const dst: number[] = [];
	const own: number[] = [];
	const tick = { src: [] as number[], dst: [] as number[], edge: [] as number[] };
	const items: EquipmentItem[] = [];
	const kinds = new Set<EquipmentItem['kind']>();
	const anchors = new Float32Array(graph.edges.length * 2).fill(NaN);
	const flowDir = new Float32Array(graph.edges.length * 2);
	const room = new Float32Array(graph.edges.length);
	const busIndex = new Map(graph.buses.map((b, i) => [b.id.toLowerCase(), i]));
	const stroke = (
		glyph: Glyph,
		x: number,
		y: number,
		cos: number,
		sin: number,
		owner: number,
		out = { src, dst, edge: own }
	) => {
		for (const line of glyph)
			for (let i = 1; i < line.length; i++) {
				const [ax, ay] = line[i - 1];
				const [bx, by] = line[i];
				out.src.push(x + unit * (ax * cos - ay * sin), y + unit * (ax * sin + ay * cos), 0);
				out.dst.push(x + unit * (bx * cos - by * sin), y + unit * (bx * sin + by * cos), 0);
				out.edge.push(owner);
			}
	};

	// Segments per edge, in order.
	const segs = new Map<number, number[]>();
	for (let s = 0; s < geometry.segmentEdge.length; s++) {
		const k = geometry.segmentEdge[s];
		const list = segs.get(k);
		if (list) list.push(s);
		else segs.set(k, [s]);
	}
	/** Per host segment: the interrupted interval as fractions along it. */
	const cuts = new Map<number, [number, number]>();
	const half = INLINE_HALF * unit;
	// The segment each branch's symbol or marks sit on, and which branches share one exactly:
	// their marks are spread along it instead of drawn on top of each other.
	const host = new Int32Array(graph.edges.length).fill(-1);
	const routes = new Map<string, number[]>();
	// A multi-winding transformer projects to one edge per winding pair; it is drawn once, on its
	// first edge, and its other edges stay plain lines.
	const seenTransformers = new Set<string>();
	const secondary = graph.edges.map((e) => {
		if (e.kind !== 'transformer') return false;
		if (seenTransformers.has(e.id)) return true;
		seenTransformers.add(e.id);
		return false;
	});
	graph.edges.forEach((_, k) => {
		const list = segs.get(k);
		if (!list || secondary[k]) return;
		const s = engine && geometry.role[k] === 0 ? list[list.length - 1] : list[list.length >> 1];
		host[k] = s;
		const a = `${geometry.sources[s * 3]},${geometry.sources[s * 3 + 1]}`;
		const b = `${geometry.targets[s * 3]},${geometry.targets[s * 3 + 1]}`;
		const key = a < b ? `${a}|${b}` : `${b}|${a}`;
		const group = routes.get(key);
		if (group) group.push(k);
		else routes.set(key, [k]);
	});
	const sharedRoute = new Uint8Array(graph.edges.length).fill(1);
	const slot = new Int32Array(graph.edges.length);
	const routeGroups: number[][] = [];
	const routeOf = new Int32Array(graph.edges.length).fill(-1);
	for (const group of routes.values()) {
		group.forEach((k, i) => {
			sharedRoute[k] = Math.min(255, group.length);
			slot[k] = i;
		});
		if (group.length > 1) {
			for (const k of group) routeOf[k] = routeGroups.length;
			routeGroups.push(group);
		}
	}
	graph.edges.forEach((edge, k) => {
		const list = segs.get(k);
		if (!list || secondary[k]) return;
		const info = edge.kind === 'transformer' ? transformerInfo(graph, k) : null;
		const glyph =
			edge.kind === 'transformer'
				? transformerGlyph(info, edge.n_phases)
				: branchGlyph(edge.kind, edge.closed);
		const childEnd = engine && geometry.role[k] === 0;
		const s = host[k];
		const [x0, y0, x1, y1] = [
			geometry.sources[s * 3],
			geometry.sources[s * 3 + 1],
			geometry.targets[s * 3],
			geometry.targets[s * 3 + 1]
		];
		const len = Math.hypot(x1 - x0, y1 - y0);
		// Co-located ends (a transformer between buses at one site): draw along x.
		const cos = len > 0 ? (x1 - x0) / len : 1;
		const sin = len > 0 ? (y1 - y0) / len : 0;
		// Routes run either way between the ends; orient by which end the route starts nearer.
		const from = busIndex.get(edge.from.toLowerCase()) ?? -1;
		const to = busIndex.get(edge.to.toLowerCase()) ?? -1;
		const first = list[0];
		const near = (i: number) =>
			i < 0
				? Infinity
				: Math.hypot(
						geometry.sources[first * 3] - geometry.positions[i * 3],
						geometry.sources[first * 3 + 1] - geometry.positions[i * 3 + 1]
					);
		const forward = near(from) <= near(to) ? 1 : -1;
		flowDir[k * 2] = cos * forward;
		flowDir[k * 2 + 1] = sin * forward;
		if (len < 2.4 * half) {
			// Too short to sit on: anchor at the middle. Transformers and switches still get their
			// symbol there, over the branch rather than in a gap, so they stay visible.
			const [mx, my] = [(x0 + x1) / 2, (y0 + y1) / 2];
			anchors[k * 2] = mx;
			anchors[k * 2 + 1] = my;
			room[k] = len / 2;
			if (glyph) {
				stroke(glyph, mx, my, cos * forward, sin * forward, k);
				const kind = edge.kind as 'transformer' | 'switch';
				items.push({ kind, edge: k, bus: -1, ids: [edge.id], x: mx, y: my, label: info?.label });
				kinds.add(kind);
			}
			return;
		}
		// From the child end: centre 2.5 half-widths back, clamped to the segment's middle. Branches
		// sharing the segment take evenly spaced places along it.
		const along = childEnd
			? Math.max(len / 2, len - 2.5 * half)
			: Math.min(len - half, Math.max(half, (len * (slot[k] + 1)) / (sharedRoute[k] + 1)));
		const x = x0 + cos * along;
		const y = y0 + sin * along;
		anchors[k * 2] = x;
		anchors[k * 2 + 1] = y;
		room[k] = along - half;
		if (!glyph) {
			const { phases, neutral } = conductorSet(graph, k);
			if (phases + (neutral ? 1 : 0) > 0)
				stroke(conductorTicks(phases, neutral), x, y, cos, sin, k, tick);
			return;
		}
		// The from winding at −x, so a transformer's winding marks sit at their own buses.
		stroke(glyph, x, y, cos * forward, sin * forward, k);
		cuts.set(s, [(along - half) / len, (along + half) / len]);
		items.push({
			kind: edge.kind as 'transformer' | 'switch',
			edge: k,
			bus: -1,
			ids: [edge.id],
			x,
			y,
			label: info?.label
		});
		kinds.add(edge.kind as 'transformer' | 'switch');
	});

	graph.buses.forEach((bus, i) => {
		const variants = attachmentVariants(bus, neutralEarths(bus));
		// Perfectly grounded terminals: one solid earth, listing the terminals.
		if (bus.grounded?.length) variants.set('ground', [{ ids: [...bus.grounded], terminals: null }]);
		if (!variants.size) return;
		const [bx, by] = [geometry.positions[i * 3], geometry.positions[i * 3 + 1]];
		const present = ATTACHMENT_ORDER.filter((kind) => variants.has(kind));
		// A neutral earth takes the shunt slot; with a real shunt too, it sits beside it.
		const slots = present.filter((k) => !(k === 'earth' && variants.has('shunt')));
		const owner = -(i + 1);
		const line = (ax: number, ay: number, cx: number, cy: number) => {
			src.push(ax, ay, 0);
			dst.push(cx, cy, 0);
			own.push(owner);
		};
		// Riser from the bus down to the last slot.
		line(bx, by, bx, by + slots.length * gap);
		present.forEach((kind) => {
			const beside = kind === 'earth' && variants.has('shunt');
			const slot = slots.indexOf(beside ? 'shunt' : kind);
			const y = by + (slot + 1) * gap;
			const list = variants.get(kind)!;
			// Several variants in one slot hang from a rail above it, each on its own drop, so they do
			// not read as in series.
			const rail = y - 1.15 * unit;
			if (list.length > 1 && !beside)
				line(bx, rail, bx + unit * (list.length - 1) * VARIANT_STEP, rail);
			// Beside a shunt, a neutral earth continues from the end of the last shunt's glyph, past
			// room for that shunt's label.
			const after = ((variants.get('shunt')?.length ?? 1) - 1) * VARIANT_STEP + LEAD + 2.0;
			const labelRoom = 4.2;
			list.forEach((v, j) => {
				const drop = bx + unit * (beside ? after : j * VARIANT_STEP);
				if (j > 0 && !beside) line(drop, rail, drop, y);
				const x0 = drop + unit * (beside ? labelRoom : LEAD);
				line(drop, y, x0, y);
				if (v.terminals && !beside) {
					const { phases, neutral } = terminalConductors(v.terminals, bus.neutral_terminal);
					if (phases + (neutral ? 1 : 0) > 0)
						stroke(conductorTicks(phases, neutral), drop + (unit * LEAD) / 2, y, 1, 0, owner, tick);
				}
				stroke(attachmentGlyph(kind), x0, y, 1, 0, owner);
				items.push({
					kind,
					edge: -1,
					bus: i,
					ids: v.ids,
					x: x0 + unit * 1.2,
					y,
					...(v.terminals ? { terminals: v.terminals, connection: v.connection } : {})
				});
			});
			kinds.add(kind);
		});
	});

	return {
		sources: Float32Array.from(src),
		targets: Float32Array.from(dst),
		items,
		cut: cuts.size ? interrupt(geometry, cuts) : geometry,
		unit,
		kinds,
		owner: Int32Array.from(own),
		ticks: {
			sources: Float32Array.from(tick.src),
			targets: Float32Array.from(tick.dst),
			edge: Int32Array.from(tick.edge)
		},
		anchors,
		flowDir,
		room,
		sharedRoute,
		routeGroups,
		routeOf,
		host
	};
}

/** A copy of `geometry` with each cut segment split around its interval; edge ownership is kept,
 * so picking either half selects the branch. */
function interrupt(
	geometry: SchematicGeometry,
	cuts: Map<number, [number, number]>
): SchematicGeometry {
	const m = geometry.segmentEdge.length + cuts.size;
	const sources = new Float32Array(m * 3);
	const targets = new Float32Array(m * 3);
	const segmentEdge = new Int32Array(m);
	let o = 0;
	const push = (k: number, a: number[], b: number[]) => {
		sources.set(a, o * 3);
		targets.set(b, o * 3);
		segmentEdge[o++] = k;
	};
	for (let s = 0; s < geometry.segmentEdge.length; s++) {
		const a = [geometry.sources[s * 3], geometry.sources[s * 3 + 1], 0];
		const b = [geometry.targets[s * 3], geometry.targets[s * 3 + 1], 0];
		const cut = cuts.get(s);
		const k = geometry.segmentEdge[s];
		if (!cut) {
			push(k, a, b);
			continue;
		}
		const at = (t: number) => [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, 0];
		push(k, a, at(cut[0]));
		push(k, at(cut[1]), b);
	}
	return { ...geometry, sources, targets, segmentEdge };
}

/** An SVG path for a glyph in symbol units, for legends. */
export function glyphPath(glyph: Glyph, scale: number, dx = 0, dy = 0): string {
	const f = (v: number) => Math.round(v * 100) / 100;
	return glyph
		.map((line) =>
			line.map(([x, y], i) => `${i ? 'L' : 'M'}${f(dx + x * scale)} ${f(dy + y * scale)}`).join('')
		)
		.join(' ');
}

/** Power-flow chevrons: one per closed branch with a result, just upstream of its symbol or marks
 * on the same segment, pointing the way active power flows (from the `from` end when the power
 * into the branch there is positive). Branches carrying under `minWatts` get none. */
export function flowArrows(
	symbols: EquipmentSymbols,
	ends: ({ p: number } | null)[][],
	closed: boolean[],
	minWatts = 1
): { sources: Float32Array; targets: Float32Array; edge: Int32Array } {
	const src: number[] = [];
	const dst: number[] = [];
	const owner: number[] = [];
	const u = symbols.unit;
	const chevron: [number, number][] = [
		[-0.3, -0.4],
		[0.3, 0],
		[-0.3, 0.4]
	];
	ends.forEach(([a, b], k) => {
		const ax = symbols.anchors[k * 2];
		if (!closed[k] || !Number.isFinite(ax)) return;
		const p = a ? a.p : b ? -b.p : null;
		if (p === null || !(Math.abs(p) >= minWatts)) return;
		const sign = p > 0 ? 1 : -1;
		const cos = symbols.flowDir[k * 2] * sign;
		const sin = symbols.flowDir[k * 2 + 1] * sign;
		// Upstream along the segment's own direction, whichever way the power goes.
		const back = Math.min(2 * INLINE_HALF * u, symbols.room[k] * 0.8);
		const x = ax - symbols.flowDir[k * 2] * back;
		const y = symbols.anchors[k * 2 + 1] - symbols.flowDir[k * 2 + 1] * back;
		for (let i = 1; i < chevron.length; i++) {
			const [px, py] = chevron[i - 1];
			const [qx, qy] = chevron[i];
			src.push(x + u * (px * cos - py * sin), y + u * (px * sin + py * cos), 0);
			dst.push(x + u * (qx * cos - qy * sin), y + u * (qx * sin + qy * cos), 0);
			owner.push(k);
		}
	});
	return {
		sources: Float32Array.from(src),
		targets: Float32Array.from(dst),
		edge: Int32Array.from(owner)
	};
}

/** Complex power into each element (loads, generators, IBRs, shunts) by element id, and for
 * sources the power they draw (the negative of what they inject), so every attachment reads with
 * the same convention: positive is consumption. */
export function elementPowers(result: {
	element_ports: { element: string; power_into_element: { re: number; im: number } }[];
	source_reactions: { source: string; power_into_network: { re: number; im: number } }[];
}): Map<string, { p: number; q: number }> {
	const out = new Map<string, { p: number; q: number }>();
	const add = (id: string, p: number, q: number) => {
		const row = out.get(id) ?? { p: 0, q: 0 };
		row.p += p;
		row.q += q;
		out.set(id, row);
	};
	for (const port of result.element_ports)
		add(port.element, port.power_into_element.re, port.power_into_element.im);
	for (const r of result.source_reactions)
		add(r.source, -r.power_into_network.re, -r.power_into_network.im);
	return out;
}

/** `12.3 kW 4.1 kvar` for a power in W and var. */
export function formatPower(p: number, q: number): string {
	const scale = Math.max(Math.abs(p), Math.abs(q));
	const [k, unit] = scale >= 1e6 ? [1e6, 'M'] : scale >= 1e3 ? [1e3, 'k'] : [1, ''];
	const f = (v: number) => (v / k).toFixed(Math.abs(v / k) >= 100 ? 0 : 1);
	return `${f(p)} ${unit}W ${f(q)} ${unit}var`;
}
