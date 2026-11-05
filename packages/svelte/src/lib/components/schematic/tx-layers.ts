import type { Layer } from '@deck.gl/core';
import type { RGBA } from '../../colors.js';
import {
	branchGlyph,
	txKvClass,
	type LabelCandidate,
	type Landmark,
	type SchematicGeometry,
	type TxGraph,
	type TxKvClass
} from '../../diagram/index.js';
import type { DeckKit } from './layers.js';
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

/** Line colour per transmission voltage class: shades of the one blue the multiconductor diagram
 * uses for MV and above, darker for higher levels, and the quiet warm grey below 50 kV. Clear of
 * the warning, violation, selection and path colours (decision D4). */
export const TX_LEVEL_COLOURS: Record<TxKvClass, RGBA> = {
	ehv: [26, 28, 84, 245],
	ehv2: [38, 44, 112, 240],
	hv: [30, 76, 140, 240],
	hv2: [58, 118, 178, 235],
	sub: [104, 150, 196, 230],
	low: [138, 131, 117, 210]
};

/** Line width in pixels per voltage class. */
export const TX_LEVEL_WIDTHS: Record<TxKvClass, number> = {
	ehv: 3.4,
	ehv2: 2.9,
	hv: 2.4,
	hv2: 1.9,
	sub: 1.5,
	low: 1.1
};

/** An out-of-service branch: faint in every colour mode. */
export const OUT_OF_SERVICE: RGBA = [197, 191, 178, 110];

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
	/** Transformer symbols on their branch, once legible: branch rows and the glyph scale (drawing
	 * units per glyph unit). */
	transformers: { edges: number[]; unit: number } | null;
	marks: {
		placed: Landmark[];
		strokes: { sources: Float32Array; targets: Float32Array; owner: Int32Array };
	} | null;
	labels: (LabelCandidate & { position: [number, number] })[];
}

/** Colour and width per branch for the voltage-level view. */
export function levelPaint(graph: TxGraph): { colours: Uint8Array; widths: Float32Array } {
	const m = graph.from.length;
	const colours = new Uint8Array(m * 4);
	const widths = new Float32Array(m);
	for (let k = 0; k < m; k++) {
		const cls = txKvClass(graph.branchKv[k]);
		colours.set(graph.inService[k] ? TX_LEVEL_COLOURS[cls] : OUT_OF_SERVICE, k * 4);
		widths[k] = TX_LEVEL_WIDTHS[cls];
	}
	return { colours, widths };
}

/** Per-segment copies of per-branch colours and widths. */
function perSegment(scene: TxScene) {
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
	const { positions } = scene.geometry;
	for (const k of t.edges) {
		const [a, b] = [scene.graph.from[k], scene.graph.to[k]];
		const [ax, ay, bx, by] = [
			positions[a * 3],
			positions[a * 3 + 1],
			positions[b * 3],
			positions[b * 3 + 1]
		];
		const len = Math.hypot(bx - ax, by - ay) || 1;
		const [c, s] = [(bx - ax) / len, (by - ay) / len];
		const [x, y] = [(ax + bx) / 2, (ay + by) / 2];
		centres.push(x, y, 0);
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
		owner,
		edges: t.edges
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

/** The layers of one frame, bottom to top. Layer ids carry the pick kind: `branches`, `buses`,
 * `transformers`, `landmark-halos`. */
export function txLayers(kit: DeckKit, scene: TxScene): Layer[] {
	const { LineLayer, ScatterplotLayer, TextLayer } = kit.layers;
	const common = { coordinateSystem: kit.core.COORDINATE_SYSTEM.CARTESIAN };
	const g = scene.geometry;
	const seg = perSegment(scene);
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
	return [
		...highlight(scene.hoveredEdge, 'hover-branch', alpha(INK, 70), 6),
		...highlight(scene.selectedEdge, 'selected-branch', alpha(ACCENT, 200), 6),
		new LineLayer({
			...common,
			id: 'branches',
			data: {
				length: g.segmentEdge.length,
				attributes: {
					getSourcePosition: { value: g.sources, size: 3 },
					getTargetPosition: { value: g.targets, size: 3 },
					getColor: { value: seg.colours, size: 4, normalized: true },
					getWidth: { value: seg.widths, size: 1 }
				}
			},
			widthUnits: 'pixels',
			pickable: true,
			updateTriggers: { getColor: [scene.edgeColours], getWidth: [scene.edgeWidths] }
		}),
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
								getTargetPosition: { value: tx.targets, size: 3 }
							}
						},
						getColor: SYMBOL_INK,
						getWidth: 1.4,
						widthUnits: 'pixels'
					})
				]
			: []),
		new ScatterplotLayer({
			...common,
			id: 'buses',
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
								getSourcePosition: { value: scene.marks.strokes.sources, size: 3 },
								getTargetPosition: { value: scene.marks.strokes.targets, size: 3 },
								getColor: { value: markColours(scene.marks), size: 4, normalized: true }
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
