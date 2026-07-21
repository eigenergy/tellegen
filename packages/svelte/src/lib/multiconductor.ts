/** Pure helpers for the multiconductor viewing path: place a distribution
 * bus/terminal graph onto the map, and the phase/edge/attachment palette the
 * layers and the terminal detail share. No solve, no reactive state here — the
 * `MulticonductorCase` state and the map own those. */

import type {
	DistAttachmentKind,
	DistEdgeKind,
	DistGraph,
	DistGraphAttachment,
	DistGraphBus,
	Topology
} from '@tellegen/engine';
import { placeSyntheticTopology, type PlacementCenter } from './synthetic-layout.js';
import type { RGBA } from './colors.js';
import { extent } from './format.js';

/** A bus positioned on the map, carrying its terminal-level detail. */
export interface PlacedMultiBus {
	id: string;
	lon: number;
	lat: number;
	terminals: string[];
	grounded: string[];
	load_kw: number;
	gen_kw: number;
	has_source: boolean;
	/** Distinct attachment kinds on the bus, in a stable order, for the badges. */
	attachmentKinds: DistAttachmentKind[];
	terminalAttachments: Record<string, DistGraphAttachment[]>;
}

/** An edge positioned on the map: a two-point path plus its typed detail. */
export interface PlacedMultiEdge {
	id: string;
	kind: DistEdgeKind;
	from: string;
	to: string;
	conductors: [string, string][];
	closed: boolean;
	n_phases: number;
	path: [[number, number], [number, number]];
}

/** The placed graph the map renders. */
export interface MultiView {
	buses: PlacedMultiBus[];
	edges: PlacedMultiEdge[];
}

const ATTACHMENT_ORDER: DistAttachmentKind[] = [
	'source',
	'generator',
	'ibr',
	'load',
	'shunt'
];

/** Flatten a bus's per-terminal attachments to the distinct kinds present, in a
 * stable render order. */
function attachmentKindsOf(bus: DistGraphBus): DistAttachmentKind[] {
	const present = new Set<DistAttachmentKind>();
	for (const list of Object.values(bus.terminal_attachments ?? {})) {
		for (const a of list) present.add(a.kind);
	}
	return ATTACHMENT_ORDER.filter((kind) => present.has(kind));
}

function placedBus(bus: DistGraphBus, lon: number, lat: number): PlacedMultiBus {
	return {
		id: bus.id,
		lon,
		lat,
		terminals: bus.terminals,
		grounded: bus.grounded,
		load_kw: bus.load_kw,
		gen_kw: bus.gen_kw,
		has_source: bus.has_source,
		attachmentKinds: attachmentKindsOf(bus),
		terminalAttachments: bus.terminal_attachments ?? {}
	};
}

/** Case-insensitive lookup from a bus id to its position; the graph stores ids
 * in their source case while edges reference the canonical id, which can differ
 * only in case. */
function positionIndex(coords: Map<string, [number, number]>): (id: string) => [number, number] | undefined {
	const lower = new Map<string, [number, number]>();
	for (const [id, xy] of coords) lower.set(id.toLowerCase(), xy);
	return (id: string) => coords.get(id) ?? lower.get(id.toLowerCase());
}

/** Build the placed view from a coordinate map keyed by bus id. Buses without a
 * position are dropped, and so is any edge missing an endpoint position. */
function viewFrom(graph: DistGraph, coords: Map<string, [number, number]>): MultiView {
	const at = positionIndex(coords);
	const buses: PlacedMultiBus[] = [];
	for (const bus of graph.buses) {
		const xy = at(bus.id);
		if (xy) buses.push(placedBus(bus, xy[0], xy[1]));
	}
	const edges: PlacedMultiEdge[] = [];
	for (const edge of graph.edges) {
		const from = at(edge.from);
		const to = at(edge.to);
		if (!from || !to) continue;
		edges.push({
			id: edge.id,
			kind: edge.kind,
			from: edge.from,
			to: edge.to,
			conductors: edge.conductors,
			closed: edge.closed,
			n_phases: edge.n_phases,
			path: [from, to]
		});
	}
	return { buses, edges };
}

/** Place a geographic graph: each bus's `xy` is `[longitude, latitude]` and
 * drops straight onto the map. */
export function buildGeographicView(graph: DistGraph): MultiView {
	const coords = new Map<string, [number, number]>();
	for (const bus of graph.buses) {
		if (bus.xy) coords.set(bus.id, bus.xy);
	}
	return viewFrom(graph, coords);
}

/** Whether at least two buses carry a planar `xy`, enough to fit a layout to. */
function hasPlanarCoords(graph: DistGraph): boolean {
	let n = 0;
	for (const bus of graph.buses) if (bus.xy) n++;
	return n >= 2;
}

/** Fit provided planar coordinates into a longitude/latitude box at `center`,
 * preserving the drawing's shape. Mirrors the box the synthetic layout uses so
 * the two placement kinds read at the same scale. Buses without a position are
 * dropped; the map falls back to the layout when too few carry one. */
function projectPlanar(graph: DistGraph, center: PlacementCenter): Map<string, [number, number]> {
	const placed = graph.buses.filter((b) => b.xy);
	const xs = placed.map((b) => b.xy![0]);
	const ys = placed.map((b) => b.xy![1]);
	const { min: minX, max: maxX } = extent(xs);
	const { min: minY, max: maxY } = extent(ys);
	// One uniform scale for both axes, so the drawing keeps its aspect ratio
	// instead of being stretched to fill a square.
	const s = 0.92 / Math.max(maxX - minX || 1, maxY - minY || 1);
	const cx = (minX + maxX) / 2;
	const cy = (minY + maxY) / 2;
	const span = Math.min(4.5, Math.max(0.32, Math.sqrt(Math.max(placed.length, 1)) * 0.08));
	const lonScale = Math.max(Math.cos((center.lat * Math.PI) / 180), 0.25);
	const latSpan = span;
	const lonSpan = span / lonScale;
	const coords = new Map<string, [number, number]>();
	for (const bus of placed) {
		// Center on [0.5, 0.5], then into the box at center. Larger planar y
		// reads as further north.
		const nx = 0.5 + (bus.xy![0] - cx) * s;
		const ny = 0.5 + (bus.xy![1] - cy) * s;
		coords.set(bus.id, [
			center.lon + (nx - 0.5) * lonSpan,
			center.lat + (ny - 0.5) * latSpan
		]);
	}
	return coords;
}

/** Build a numeric-id `Topology` from the graph so the shared synthetic force
 * layout can lay it out. Bus ids become array indices; the string id rides in
 * `uid`, and edges map their endpoints to those indices. */
function toTopology(graph: DistGraph): { topology: Topology; idByIndex: string[] } {
	const idByIndex = graph.buses.map((b) => b.id);
	const indexById = new Map<string, number>();
	graph.buses.forEach((b, i) => indexById.set(b.id.toLowerCase(), i));
	const index = (id: string) => indexById.get(id.toLowerCase());
	const topology: Topology = {
		buses: graph.buses.map((b, i) => ({
			id: i,
			uid: b.id,
			demand_mw: b.load_kw,
			gen_mw: b.gen_kw
		})),
		branches: graph.edges.flatMap((e, i) => {
			const from = index(e.from);
			const to = index(e.to);
			if (from === undefined || to === undefined) return [];
			return [
				{
					id: i,
					uid: e.id,
					from,
					to,
					rate_mw: 0,
					status: e.closed ? 1 : 0
				}
			];
		})
	};
	return { topology, idByIndex };
}

/** Lay the graph out with the shared synthetic layout at `center`, then map
 * the numeric-id result back to string bus ids. Source buses are passed as
 * root hints so a radial feeder's tree layout starts at its substation. */
function layoutSynthetic(graph: DistGraph, center: PlacementCenter): Map<string, [number, number]> {
	const { topology, idByIndex } = toTopology(graph);
	const roots = graph.buses.flatMap((b, i) => (b.has_source ? [i] : []));
	const placed = placeSyntheticTopology(topology, center, { roots });
	const coords = new Map<string, [number, number]>();
	for (const bus of placed.buses) {
		const id = idByIndex[bus.id];
		if (id !== undefined) coords.set(id, [bus.lon, bus.lat]);
	}
	return coords;
}

/** Place a non-geographic graph at a map `center`: fit the provided planar
 * coordinates when the case carries them, otherwise fall back to the shared
 * synthetic force layout. */
export function placeMultiView(graph: DistGraph, center: PlacementCenter): MultiView {
	const coords = hasPlanarCoords(graph)
		? projectPlanar(graph, center)
		: layoutSynthetic(graph, center);
	return viewFrom(graph, coords);
}

// ---------------------------------------------------------------------------
// Palette: phase-colored conductors, edge kinds, attachment badges. The hues
// reuse the app's warm/earth family so the multiconductor view sits beside the
// single-phase view without a palette clash.
// ---------------------------------------------------------------------------

/** Rust, green, and steel-blue for the three phases; a warm gray for the
 * neutral/ground return. Distinct in hue and lightness so they survive color
 * vision deficiency and never read as the LMP ramp. */
const PHASE_A: RGBA = [194, 86, 75, 255];
const PHASE_B: RGBA = [74, 143, 95, 255];
const PHASE_C: RGBA = [63, 111, 187, 255];
const NEUTRAL: RGBA = [120, 114, 102, 255];

/** Color a conductor by its terminal name. OpenDSS node numbers map 1/2/3 to
 * the three phases; a named phase (`a`/`b`/`c`) maps the same way; anything else
 * (neutral `n`, ground, the fourth wire) is the neutral gray. */
export function phaseColor(terminal: string): RGBA {
	switch (terminal.trim().toLowerCase()) {
		case '1':
		case 'a':
			return PHASE_A;
		case '2':
		case 'b':
			return PHASE_B;
		case '3':
		case 'c':
			return PHASE_C;
		default:
			return NEUTRAL;
	}
}

/** True when a terminal is a phase conductor (not a neutral/ground return). */
export function isPhaseTerminal(terminal: string): boolean {
	return ['1', '2', '3', 'a', 'b', 'c'].includes(terminal.trim().toLowerCase());
}

/** Base color for an edge by kind and state. Lines are the warm gray of the
 * single-phase branches; transformers are steel; a switch is amber when closed
 * and a faded red when open. */
export function edgeColor(kind: DistEdgeKind, closed: boolean): RGBA {
	if (kind === 'transformer') return [96, 118, 140, 235];
	if (kind === 'switch') return closed ? [212, 116, 34, 235] : [179, 38, 30, 150];
	return [138, 131, 117, 210];
}

/** Edge stroke width in pixels, growing with conductor count so a three-phase
 * feeder reads heavier than a single-phase lateral. Transformers draw a touch
 * wider to stand out as the equipment they are. */
export function edgeWidth(kind: DistEdgeKind, nPhases: number): number {
	const base = 1.4 + 0.7 * Math.min(Math.max(nPhases, 1), 4);
	return kind === 'transformer' ? base + 1 : base;
}

/** A transformer symbol mark at an edge midpoint, angled along the edge.
 * `angle` is degrees counterclockwise from east in the map plane, with the
 * longitude span compressed by cos(latitude) so the symbol tracks the drawn
 * bearing rather than the coordinate-space one. */
export interface TransformerMark {
	id: string;
	position: [number, number];
	angle: number;
}

/** Midpoint marks for the transformer edges of a placed view. */
export function transformerMarks(edges: PlacedMultiEdge[]): TransformerMark[] {
	return edges
		.filter((e) => e.kind === 'transformer')
		.map((e) => {
			const [[x0, y0], [x1, y1]] = e.path;
			const latMid = (y0 + y1) / 2;
			return {
				id: e.id,
				position: [(x0 + x1) / 2, latMid] as [number, number],
				angle:
					(Math.atan2(y1 - y0, (x1 - x0) * Math.cos((latMid * Math.PI) / 180)) * 180) / Math.PI
			};
		});
}

const ATTACHMENT_COLOR: Record<DistAttachmentKind, RGBA> = {
	source: [63, 111, 187, 255],
	generator: [74, 143, 95, 255],
	ibr: [120, 158, 70, 255],
	load: [178, 94, 0, 255],
	shunt: [138, 108, 168, 255]
};

/** Badge color for an attachment kind. */
export function attachmentColor(kind: DistAttachmentKind): RGBA {
	return ATTACHMENT_COLOR[kind];
}

/** A short glyph for an attachment kind, for the panel legend and badges. */
export function attachmentGlyph(kind: DistAttachmentKind): string {
	switch (kind) {
		case 'source':
			return 'src';
		case 'generator':
			return 'gen';
		case 'ibr':
			return 'ibr';
		case 'load':
			return 'load';
		case 'shunt':
			return 'sh';
	}
}
