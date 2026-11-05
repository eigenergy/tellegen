import type { Layer } from '@deck.gl/core';
import type { RGBA } from '../../colors.js';
import {
	attachmentGlyph,
	branchGlyph,
	type LabelCandidate,
	type Landmark,
	type SchematicGeometry,
	type TxGraph,
	type TxSchematicParts,
	type TxTier,
	type VoltageEntry,
	type VoltageLegend
} from '../../diagram/index.js';
import { morphLayers, type DeckKit } from './layers.js';
import {
	ACCENT,
	DANGER,
	INK,
	MONO,
	PAPER,
	SYMBOL_INK,
	WARNING,
	alpha,
	stateInk
} from './palette.js';

/** Voltage tier colours as light-to-dark ramps (dark, mid, light). The three hues pass the
 * all-pairs colour-vision check against each other and against the warning, violation and
 * selection colours at their mid steps; within a tier the shade steps are monotone in lightness.
 * Lower-voltage grey is neutral on purpose. */
export const TX_TIER_RAMPS: Record<TxTier, [RGBA, RGBA, RGBA]> = {
	ehv: [
		[94, 24, 88, 245],
		[155, 47, 143, 245],
		[199, 101, 188, 240]
	],
	hv: [
		[31, 52, 133, 245],
		[51, 82, 196, 240],
		[106, 143, 224, 235]
	],
	sub: [
		[11, 95, 120, 240],
		[21, 146, 181, 235],
		[93, 187, 214, 230]
	],
	low: [
		[111, 106, 96, 225],
		[138, 132, 119, 215],
		[166, 160, 148, 205]
	],
	unknown: [
		[176, 170, 158, 200],
		[176, 170, 158, 200],
		[176, 170, 158, 200]
	]
};

const mix = (a: RGBA, b: RGBA, t: number): RGBA =>
	a.map((v, i) => Math.round(v + (b[i] - v) * t)) as unknown as RGBA;

/** The colour of a legend entry: its hue's mid step when alone in the hue, else from darker
 * (highest level) to lighter, staying near the middle of the ramp. */
export function entryColour(e: VoltageEntry): RGBA {
	const [dark, mid, light] = TX_TIER_RAMPS[e.hue];
	if (e.tierSize <= 1) return mid;
	const t = 0.2 + (0.65 * e.inTier) / (e.tierSize - 1);
	return t < 0.5 ? mix(dark, mid, t / 0.5) : mix(mid, light, (t - 0.5) / 0.5);
}

/** Line width in pixels of a legend entry: heaviest for the highest level, a step per rank. */
export function entryWidth(e: VoltageEntry): number {
	return e.tier === 'unknown' ? 1.1 : Math.max(1.1, 3.6 - 0.5 * e.rank);
}

export interface TxScene {
	graph: TxGraph;
	geometry: SchematicGeometry;
	/** `r, g, b, a` per branch. */
	edgeColours: Uint8Array;
	/** Pixels per branch. */
	edgeWidths: Float32Array;
	/** `r, g, b, a` per bus. */
	busColours: Uint8Array;
	busRadius: number;
	selectedBus: number;
	selectedEdge: number;
	hoveredBus: number;
	hoveredEdge: number;
	/** Equipment hovered or focused in a result list, drawn with a stronger highlight. */
	previewBus: number;
	previewEdge: number;
	/** Bus values shaded over the drawing's area, beneath everything else. */
	field: { image: HTMLCanvasElement; bounds: [number, number, number, number] } | null;
	/** Per branch, its symbol point on the drawn geometry: `x, y, cos, sin` (`edgeAnchors`). */
	anchors: Float32Array;
	/** Transformer symbols on their branch, once legible: branch rows and the glyph scale (drawing
	 * units per glyph unit). */
	transformers: { edges: number[]; unit: number } | null;
	/** Active power direction arrows once results and symbols show: branch rows, +1 when power
	 * flows along the drawn path's direction at the anchor, -1 against, and the glyph scale. */
	arrows: { edges: number[]; dir: Int8Array; unit: number } | null;
	/** Generator and load symbols hanging below their bus once legible: where each hangs from
	 * and the glyph scale. */
	attachments: {
		points: {
			x: number;
			y: number;
			rootX: number;
			bus: number;
			kind: 'generator' | 'load';
			/** Outside a highlighted voltage level. */
			dim?: boolean;
		}[];
		unit: number;
	} | null;
	/** Drawing units per screen pixel, for dashes of a fixed screen size. */
	pixel: number;
	/** Level colour per bus, for transformer windings. */
	busLevels: Uint8Array;
	/** Busbars and substation boxes of the substation overview; null on source positions. */
	parts: TxSchematicParts | null;
	/** Worst station violation, preserving its bus identity in the overview. */
	stationColours: Uint8Array | null;
	voltageMarkers: { bus: number; kind: 'low' | 'high' | 'deenergized' }[];
	/** Buses gliding between layouts: drawn instead of the scene while set. */
	morph: {
		s: Float32Array;
		t: Float32Array;
		cur: Float32Array;
		endpoints: Int32Array;
	} | null;
	marks: {
		placed: Landmark[];
		strokes: {
			sources: Float32Array;
			targets: Float32Array;
			owner: Int32Array;
		};
	} | null;
	labels: (LabelCandidate & { position: [number, number] })[];
}

/** Colour and width per branch for the voltage-level view. `focus` (a legend entry) keeps that
 * level and fades the others; `fadeFrom` fades entries of that rank and below, so the backbone
 * reads first when zoomed out. */
export function levelPaint(
	graph: TxGraph,
	legend: VoltageLegend,
	options: { focus?: number; fadeFrom?: number } = {}
): { colours: Uint8Array; widths: Float32Array } {
	const m = graph.from.length;
	const colours = new Uint8Array(m * 4);
	const widths = new Float32Array(m);
	const paint = legend.entries.map((e) => ({
		colour: entryColour(e),
		width: entryWidth(e)
	}));
	const focus = options.focus ?? -1;
	const fadeFrom = options.fadeFrom ?? Infinity;
	for (let k = 0; k < m; k++) {
		const j = legend.entryOf(graph.branchKv[k]);
		const { colour, width } = paint[j];
		// Out of service keeps its level colour, a little lighter: the dashes carry the state.
		let c: RGBA = graph.inService[k] ? colour : alpha(colour, 190);
		let w = width;
		if (focus >= 0) {
			if (j === focus) w += 1;
			else c = alpha(c, 40);
		} else if (j >= fadeFrom) c = alpha(c, Math.round(c[3] * 0.45));
		colours.set(c, k * 4);
		widths[k] = w;
	}
	return { colours, widths };
}

/** Level colour per bus (its legend entry), independent of any result colouring. */
export function busLevelColours(graph: TxGraph, legend: VoltageLegend): Uint8Array {
	const paint = legend.entries.map(entryColour);
	const out = new Uint8Array(graph.kv.length * 4);
	for (let i = 0; i < graph.kv.length; i++) out.set(paint[legend.entryOf(graph.kv[i])], i * 4);
	return out;
}

/** Per-segment copies of per-branch colours and widths. */
function perSegment(scene: Pick<TxScene, 'geometry' | 'edgeColours' | 'edgeWidths'>) {
	const { segmentEdge } = scene.geometry;
	const s = segmentEdge.length;
	const colours = new Uint8Array(s * 4);
	const widths = new Float32Array(s);
	for (let i = 0; i < s; i++) {
		const k = segmentEdge[i];
		colours.set(scene.edgeColours.subarray(k * 4, k * 4 + 4), i * 4);
		widths[i] = scene.edgeWidths[k];
	}
	return { colours, widths };
}

/** Segment indices of one branch. */
function segmentsOf(geometry: SchematicGeometry, k: number): number[] {
	const out: number[] = [];
	if (k < 0) return out;
	for (let i = 0; i < geometry.segmentEdge.length; i++)
		if (geometry.segmentEdge[i] === k) out.push(i);
	return out;
}

/** Transformer symbols as stroke segments: two interlocking circles across the branch, drawn on
 * paper so the line reads as interrupted. */
function transformerStrokes(scene: TxScene) {
	const t = scene.transformers;
	if (!t) return null;
	const glyph = branchGlyph('transformer', true)!;
	const src: number[] = [];
	const dst: number[] = [];
	const centres: number[] = [];
	const owner: number[] = [];
	const edges: number[] = [];
	const colours: number[] = [];
	for (const k of t.edges) {
		const [x, y, c, s] = scene.anchors.subarray(k * 4, k * 4 + 4);
		if (!Number.isFinite(x)) continue;
		edges.push(k);
		centres.push(x, y, 0);
		// All geometry follows from → to, including routes that double back.
		const [a, b] = [scene.graph.from[k], scene.graph.to[k]];
		const side = (part: number) => (part < 2 ? a : b);
		glyph.forEach((line, part) => {
			const bus = side(part);
			// A shade darker than the line, so pale levels still read as a stroke on paper.
			const colour = scene.busLevels.subarray(bus * 4, bus * 4 + 4);
			for (let i = 1; i < line.length; i++)
				colours.push(colour[0] * 0.72, colour[1] * 0.72, colour[2] * 0.72, 255);
		});
		for (const line of glyph)
			for (let i = 1; i < line.length; i++) {
				const [px, py] = line[i - 1];
				const [qx, qy] = line[i];
				src.push(x + t.unit * (px * c - py * s), y + t.unit * (px * s + py * c), 0);
				dst.push(x + t.unit * (qx * c - qy * s), y + t.unit * (qx * s + qy * c), 0);
				owner.push(k);
			}
	}
	return {
		sources: Float32Array.from(src),
		targets: Float32Array.from(dst),
		centres: Float32Array.from(centres),
		colours: Uint8Array.from(colours),
		owner,
		edges
	};
}

/** Chevrons along each branch pointing the way active power flows; beside the symbol on a
 * transformer. */
function arrowStrokes(scene: TxScene) {
	const a = scene.arrows;
	if (!a) return null;
	const chevron: [number, number][] = [
		[-0.55, -0.7],
		[0.45, 0],
		[-0.55, 0.7]
	];
	const src: number[] = [];
	const dst: number[] = [];
	a.edges.forEach((k, j) => {
		const [x0, y0, c0, s0] = scene.anchors.subarray(k * 4, k * 4 + 4);
		if (!Number.isFinite(x0)) return;
		const d = a.dir[j];
		const [c, s] = [c0 * d, s0 * d];
		const ahead = scene.graph.transformer[k] ? 2.6 : 0;
		const [x, y] = [x0 + c * ahead * a.unit, y0 + s * ahead * a.unit];
		for (let i = 1; i < chevron.length; i++) {
			const [px, py] = chevron[i - 1];
			const [qx, qy] = chevron[i];
			src.push(x + a.unit * (px * c - py * s), y + a.unit * (px * s + py * c), 0);
			dst.push(x + a.unit * (qx * c - qy * s), y + a.unit * (qx * s + qy * c), 0);
		}
	});
	return { sources: Float32Array.from(src), targets: Float32Array.from(dst) };
}

/** Load symbols are drawn in a quiet grey: present on nearly every bus, they are context. */
export const LOAD_INK: RGBA = [134, 128, 118, 255];
/** Load glyphs relative to the generator glyph. */
const LOAD_SCALE = 1.05;

/** The ink of an attachment symbol: generators in symbol ink, loads in grey; faded when outside
 * a highlighted voltage level. */
export function attachmentInk(kind: 'generator' | 'load', dim = false): RGBA {
	const ink = kind === 'load' ? LOAD_INK : SYMBOL_INK;
	return dim ? alpha(ink, 50) : ink;
}

/** Generator and load symbols as strokes, hanging straight down from their points. */
function attachmentStrokes(scene: TxScene) {
	const at = scene.attachments;
	if (!at || !at.points.length) return null;
	const src: number[] = [];
	const dst: number[] = [];
	const colours: number[] = [];
	for (const p of at.points) {
		const ink = attachmentInk(p.kind, p.dim);
		if (p.x !== p.rootX) {
			src.push(p.rootX, p.y, 0);
			dst.push(p.x, p.y, 0);
			colours.push(...ink);
		}
		const unit = at.unit * (p.kind === 'load' ? LOAD_SCALE : 1);
		for (const line of attachmentGlyph(p.kind))
			for (let i = 1; i < line.length; i++) {
				// Rotated a quarter turn: the glyph's +x runs down the screen.
				const [ax, ay] = line[i - 1];
				const [bx, by] = line[i];
				src.push(p.x - unit * ay, p.y + unit * ax, 0);
				dst.push(p.x - unit * by, p.y + unit * bx, 0);
				colours.push(...ink);
			}
	}
	return {
		sources: Float32Array.from(src),
		targets: Float32Array.from(dst),
		colours: Uint8Array.from(colours)
	};
}

/** Out-of-service branches as dashes of a fixed screen length (4 px on, 3 px off). */
function dashStrokes(scene: TxScene) {
	const g = scene.geometry;
	const on = 4 * scene.pixel;
	const period = 7 * scene.pixel;
	const src: number[] = [];
	const dst: number[] = [];
	const colour: number[] = [];
	const width: number[] = [];
	for (let j = 0; j < g.segmentEdge.length; j++) {
		const k = g.segmentEdge[j];
		if (scene.graph.inService[k]) continue;
		const [x0, y0] = [g.sources[j * 3], g.sources[j * 3 + 1]];
		const [x1, y1] = [g.targets[j * 3], g.targets[j * 3 + 1]];
		const len = Math.hypot(x1 - x0, y1 - y0);
		if (!(len > 0)) continue;
		const pieces = Math.min(2000, Math.ceil(len / period));
		for (let i = 0; i < pieces; i++) {
			const t0 = (i * period) / len;
			const t1 = Math.min(1, (i * period + on) / len);
			src.push(x0 + (x1 - x0) * t0, y0 + (y1 - y0) * t0, 0);
			dst.push(x0 + (x1 - x0) * t1, y0 + (y1 - y0) * t1, 0);
			colour.push(...scene.edgeColours.subarray(k * 4, k * 4 + 4));
			width.push(Math.max(1.4, scene.edgeWidths[k] * 0.85));
		}
	}
	return {
		sources: Float32Array.from(src),
		targets: Float32Array.from(dst),
		colours: Uint8Array.from(colour),
		widths: Float32Array.from(width)
	};
}

/** Landmark stroke colours by state: ink, or the warning and violation colours. */
function markColours(marks: NonNullable<TxScene['marks']>): Uint8Array {
	const out = new Uint8Array(marks.strokes.owner.length * 4);
	marks.strokes.owner.forEach((j, i) => {
		const state = marks.placed[j].state;
		out.set(state === 'violation' ? DANGER : state === 'warning' ? WARNING : SYMBOL_INK, i * 4);
	});
	return out;
}

// Station fills and lines occupy the same drawing plane.
const flatSceneProps = (kit: DeckKit) => ({
	coordinateSystem: kit.core.COORDINATE_SYSTEM.CARTESIAN,
	parameters: { depthCompare: 'always' as const, depthWriteEnabled: false }
});

/** How much a line's casing darkens its colour. */
export const CASING_SHADE = 0.55;

/** Lines whose colour fills the stroke, with a thin casing a shade darker so pale colours keep
 * an edge on paper and on the basemap. Colour carries the data, so it takes the stroke's area
 * (thin marks need larger colour differences to be told apart). */
function casedLines(
	kit: DeckKit,
	id: string,
	sources: Float32Array,
	targets: Float32Array,
	colours: Uint8Array,
	widths: Float32Array,
	pickable = false
): Layer[] {
	const { LineLayer } = kit.layers;
	const common = flatSceneProps(kit);
	const casing = colours.slice();
	const glow = colours.slice();
	for (let j = 0; j < widths.length; j++) {
		for (let c = 0; c < 3; c++) casing[j * 4 + c] = Math.round(colours[j * 4 + c] * CASING_SHADE);
		glow[j * 4 + 3] = Math.round(colours[j * 4 + 3] * 0.18);
	}
	const strokes: [string, Uint8Array, Float32Array, boolean][] = [
		[`${id}-glow`, glow, Float32Array.from(widths, (w) => w + 3), false],
		[`${id}-casing`, casing, Float32Array.from(widths, (w) => w + 1.4), false],
		[id, colours, widths, pickable]
	];
	return strokes.map(
		([layerId, paint, weight, pick]) =>
			new LineLayer({
				...common,
				id: layerId,
				data: {
					length: widths.length,
					attributes: {
						getSourcePosition: { value: sources, size: 3 },
						getTargetPosition: { value: targets, size: 3 },
						getColor: { value: paint, size: 4, normalized: true },
						getWidth: { value: weight, size: 1 }
					}
				},
				widthUnits: 'pixels',
				pickable: pick
			})
	);
}

/** Route data stays on the GPU while pan, zoom and selection move the overlays. */
export function txBranchLayers(
	kit: DeckKit,
	scene: Pick<TxScene, 'graph' | 'geometry' | 'edgeColours' | 'edgeWidths'>,
	raised: Uint8Array | null = null
): Layer[] {
	const { geometry: g } = scene;
	const { colours, widths } = perSegment(scene);
	for (let j = 0; j < g.segmentEdge.length; j++)
		if (!scene.graph.inService[g.segmentEdge[j]]) colours[j * 4 + 3] = 0;
	const layers = casedLines(kit, 'branches', g.sources, g.targets, colours, widths, true);
	const top = raised ? raisedSegments(g, raised) : null;
	return top
		? [
				...layers,
				...casedLines(
					kit,
					'branches-raised',
					pick3(g.sources, top),
					pick3(g.targets, top),
					pick4(colours, top),
					Float32Array.from(top, (j) => widths[j])
				)
			]
		: layers;
}

/** Segments of the branches flagged in `raised` (one byte per branch), or null when none. */
export function raisedSegments(g: SchematicGeometry, raised: Uint8Array): Int32Array | null {
	const out: number[] = [];
	for (let j = 0; j < g.segmentEdge.length; j++) if (raised[g.segmentEdge[j]]) out.push(j);
	return out.length ? Int32Array.from(out) : null;
}
const pick3 = (a: Float32Array, rows: Int32Array) => {
	const out = new Float32Array(rows.length * 3);
	rows.forEach((j, i) => out.set(a.subarray(j * 3, j * 3 + 3), i * 3));
	return out;
};
const pick4 = (a: Uint8Array, rows: Int32Array) => {
	const out = new Uint8Array(rows.length * 4);
	rows.forEach((j, i) => out.set(a.subarray(j * 4, j * 4 + 4), i * 4));
	return out;
};

/** The layers of one frame, bottom to top. */
export function txLayers(kit: DeckKit, scene: TxScene, branchLayers: Layer[]): Layer[] {
	if (scene.morph) return morphLayers(kit, scene.morph, scene.graph.busId.length);
	const { BitmapLayer, LineLayer, PolygonLayer, ScatterplotLayer, TextLayer } = kit.layers;
	const common = flatSceneProps(kit);
	const g = scene.geometry;
	const highlight = (k: number, id: string, colour: RGBA, extra: number): Layer[] => {
		const ids = segmentsOf(g, k);
		if (!ids.length) return [];
		return [
			new LineLayer({
				...common,
				id,
				data: ids,
				getSourcePosition: (i: number) => [g.sources[i * 3], g.sources[i * 3 + 1], 0],
				getTargetPosition: (i: number) => [g.targets[i * 3], g.targets[i * 3 + 1], 0],
				getColor: colour,
				getWidth: scene.edgeWidths[k] + extra,
				widthUnits: 'pixels'
			})
		];
	};
	const ring = (i: number, id: string, colour: RGBA, radius: number): Layer[] =>
		i < 0
			? []
			: [
					new ScatterplotLayer({
						...common,
						id,
						data: [i],
						getPosition: (b: number) => [g.positions[b * 3], g.positions[b * 3 + 1], 0],
						filled: false,
						stroked: true,
						getLineColor: colour,
						getLineWidth: 2,
						lineWidthUnits: 'pixels',
						getRadius: radius,
						radiusUnits: 'pixels'
					})
				];
	const tx = transformerStrokes(scene);
	const arrows = arrowStrokes(scene);
	const attached = attachmentStrokes(scene);
	const dashes = scene.graph.inService.includes(0) ? dashStrokes(scene) : null;
	const parts = scene.parts;
	const boxes = parts ? boxPolygons(parts.boxes) : [];
	return [
		...(scene.field
			? [
					new BitmapLayer({
						...common,
						id: 'field',
						image: scene.field.image,
						// left, bottom, right, top; the view runs y downwards, so image row 0 is at y0.
						bounds: [
							scene.field.bounds[0],
							scene.field.bounds[3],
							scene.field.bounds[2],
							scene.field.bounds[1]
						],
						textureParameters: { minFilter: 'linear', magFilter: 'linear' }
					})
				]
			: []),
		...(parts
			? [
					new PolygonLayer({
						...common,
						id: 'substations',
						data: boxes,
						getPolygon: (b: { polygon: [number, number][] }) => b.polygon,
						getFillColor: alpha(PAPER, 200),
						getLineColor: (b: { s: number }) =>
							scene.stationColours
								? (Array.from(scene.stationColours.subarray(b.s * 4, b.s * 4 + 4)) as RGBA)
								: alpha(INK, 90),
						getLineWidth: (b: { s: number }) =>
							scene.stationColours && scene.stationColours[b.s * 4 + 3] > 200 ? 2.5 : 1,
						lineWidthUnits: 'pixels',
						stroked: true,
						filled: true
					})
				]
			: []),
		...highlight(scene.previewEdge, 'preview-branch', alpha(INK, 150), 10),
		...highlight(scene.hoveredEdge, 'hover-branch', alpha(INK, 70), 6),
		...highlight(scene.selectedEdge, 'selected-branch', alpha(ACCENT, 200), 6),
		...branchLayers,
		...(tx
			? [
					new ScatterplotLayer({
						...common,
						id: 'transformers',
						data: {
							length: tx.edges.length,
							attributes: { getPosition: { value: tx.centres, size: 3 } }
						},
						getFillColor: alpha(PAPER, 255),
						getRadius: 1.2 * scene.transformers!.unit,
						radiusUnits: 'common',
						pickable: true
					}),
					new LineLayer({
						...common,
						id: 'transformer-strokes',
						data: {
							length: tx.owner.length,
							attributes: {
								getSourcePosition: { value: tx.sources, size: 3 },
								getTargetPosition: { value: tx.targets, size: 3 },
								getColor: { value: tx.colours, size: 4, normalized: true }
							}
						},
						getWidth: 1.6,
						widthUnits: 'pixels'
					})
				]
			: []),
		...(parts
			? casedLines(
					kit,
					'busbars',
					barEnds(parts.busbars, 0),
					barEnds(parts.busbars, 2),
					scene.busColours,
					new Float32Array(parts.busbars.length / 4).fill(5),
					true
				)
			: []),
		...(dashes
			? casedLines(
					kit,
					'open-branches',
					dashes.sources,
					dashes.targets,
					dashes.colours,
					dashes.widths
				)
			: []),
		...(attached
			? [
					new ScatterplotLayer({
						...common,
						id: 'attachments-pick',
						data: scene.attachments!.points,
						getPosition: (p: NonNullable<TxScene['attachments']>['points'][number]) => [
							p.x,
							p.y + 1.2 * scene.attachments!.unit,
							0
						],
						getFillColor: alpha(PAPER, 245),
						getRadius: 12,
						radiusUnits: 'pixels',
						pickable: true
					}),
					new PolygonLayer({
						...common,
						id: 'load-symbols',
						data: scene.attachments!.points.filter((p) => p.kind === 'load'),
						getPolygon: (p: NonNullable<TxScene['attachments']>['points'][number]) =>
							attachmentGlyph('load')[1].map(([x, y]) => [
								p.x - LOAD_SCALE * scene.attachments!.unit * y,
								p.y + LOAD_SCALE * scene.attachments!.unit * x
							]),
						getFillColor: (p: NonNullable<TxScene['attachments']>['points'][number]) =>
							attachmentInk('load', p.dim),
						filled: true,
						stroked: false
					}),
					new LineLayer({
						...common,
						id: 'attachments',
						data: {
							length: attached.sources.length / 3,
							attributes: {
								getSourcePosition: { value: attached.sources, size: 3 },
								getTargetPosition: { value: attached.targets, size: 3 },
								getColor: { value: attached.colours, size: 4, normalized: true }
							}
						},
						getWidth: 1.6,
						widthUnits: 'pixels'
					})
				]
			: []),
		...(arrows && arrows.sources.length
			? [
					new LineLayer({
						...common,
						id: 'flow-arrows',
						data: {
							length: arrows.sources.length / 3,
							attributes: {
								getSourcePosition: { value: arrows.sources, size: 3 },
								getTargetPosition: { value: arrows.targets, size: 3 }
							}
						},
						getColor: INK,
						getWidth: 1.6,
						widthUnits: 'pixels'
					})
				]
			: []),
		new ScatterplotLayer({
			...common,
			id: 'buses',
			visible: !parts,
			data: {
				length: g.positions.length / 3,
				attributes: {
					getPosition: { value: g.positions, size: 3 },
					getFillColor: { value: scene.busColours, size: 4, normalized: true }
				}
			},
			stroked: true,
			getLineColor: alpha(INK, 200),
			getLineWidth: 0.6,
			lineWidthUnits: 'pixels',
			getRadius: scene.busRadius,
			radiusUnits: 'pixels',
			radiusMinPixels: 1.5,
			pickable: true,
			updateTriggers: { getFillColor: [scene.busColours] }
		}),
		new TextLayer({
			...common,
			id: 'voltage-limit-markers',
			data: scene.voltageMarkers,
			updateTriggers: { getPosition: [g.positions] },
			getPosition: (m: { bus: number }) => [g.positions[m.bus * 3], g.positions[m.bus * 3 + 1], 0],
			getText: (m: { kind: string }) => (m.kind === 'low' ? '↓' : m.kind === 'high' ? '↑' : '×'),
			getColor: (m: { kind: string }) =>
				m.kind === 'low'
					? [33, 102, 172, 255]
					: m.kind === 'high'
						? [190, 32, 36, 255]
						: [120, 116, 108, 255],
			getSize: 18,
			getPixelOffset: [-8, -10],
			fontFamily: MONO,
			background: true,
			getBackgroundColor: alpha(PAPER, 230),
			backgroundPadding: [2, 0],
			characterSet: 'auto'
		}),
		...ring(scene.previewBus, 'preview-bus', alpha(INK, 200), scene.busRadius + 9),
		...ring(scene.hoveredBus, 'hover-bus', alpha(INK, 140), scene.busRadius + 4),
		...ring(scene.selectedBus, 'selected-bus', ACCENT, scene.busRadius + 5),
		...(scene.marks
			? [
					new ScatterplotLayer({
						...common,
						id: 'landmark-halos',
						data: scene.marks.placed,
						getPosition: (l: Landmark) => [l.x, l.y, 0],
						getFillColor: alpha(PAPER, 170),
						getRadius: 11,
						radiusUnits: 'pixels',
						pickable: true
					}),
					new LineLayer({
						...common,
						id: 'landmarks',
						data: {
							length: scene.marks.strokes.owner.length,
							attributes: {
								getSourcePosition: {
									value: scene.marks.strokes.sources,
									size: 3
								},
								getTargetPosition: {
									value: scene.marks.strokes.targets,
									size: 3
								},
								getColor: {
									value: markColours(scene.marks),
									size: 4,
									normalized: true
								}
							}
						},
						getWidth: 1.6,
						widthUnits: 'pixels'
					})
				]
			: []),
		new TextLayer({
			...common,
			id: 'labels',
			data: scene.labels,
			getPosition: (l: { position: [number, number] }) => [...l.position, 0],
			getText: (l: LabelCandidate) => l.text,
			getColor: (l: LabelCandidate) => stateInk(l.state),
			getSize: 11,
			getTextAnchor: 'start',
			getAlignmentBaseline: 'bottom',
			getPixelOffset: [6, -5],
			background: true,
			getBackgroundColor: alpha(PAPER, 230),
			backgroundPadding: [3, 1],
			fontFamily: MONO,
			characterSet: 'auto'
		})
	];
}

/** Substation boxes as closed rectangles. */
function boxPolygons(boxes: Float32Array): { s: number; polygon: [number, number][] }[] {
	const out: { s: number; polygon: [number, number][] }[] = [];
	for (let s = 0; s < boxes.length / 4; s++) {
		const [x0, y0, x1, y1] = boxes.subarray(s * 4, s * 4 + 4);
		if (!(x1 > x0) || !(y1 > y0)) continue;
		out.push({
			s,
			polygon: [
				[x0, y0],
				[x1, y0],
				[x1, y1],
				[x0, y1]
			]
		});
	}
	return out;
}

/** One end of each busbar (`offset` 0 for the top, 2 for the bottom) as `x, y, 0` triples. */
function barEnds(busbars: Float32Array, offset: 0 | 2): Float32Array {
	const n = busbars.length / 4;
	const out = new Float32Array(n * 3);
	for (let i = 0; i < n; i++) {
		out[i * 3] = busbars[i * 4 + offset];
		out[i * 3 + 1] = busbars[i * 4 + offset + 1];
	}
	return out;
}
