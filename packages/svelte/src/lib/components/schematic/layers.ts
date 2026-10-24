import type { Layer } from '@deck.gl/core';
import type { RGBA } from '../../colors.js';
import type { DistGraph } from '@tellegen/engine';
import {
	branchGlyph,
	conductorSet,
	conductorTicks,
	routeMiddle,
	type BusStacks,
	type EquipmentItem,
	type EquipmentSymbols,
	type FannedBranch,
	type LabelCandidate,
	type Landmark,
	type ResultSnapshot,
	type SchematicGeometry,
	type ZoneCorridor,
	type ZoneState
} from '../../diagram/index.js';
import {
	hiddenUnderFan,
	landmarkColours,
	segmentColours,
	segmentWidths,
	strokeColours,
	type FanState
} from './paint.js';
import {
	ACCENT,
	CORRIDOR_PX,
	INK,
	MONO,
	PAPER,
	PATH,
	SYMBOL_INK,
	ZONE_COLOURS,
	ZONE_INK,
	alpha,
	lineWidths,
	stateInk
} from './palette.js';

/** The deck.gl modules, imported on demand. */
export interface DeckKit {
	core: typeof import('@deck.gl/core');
	layers: typeof import('@deck.gl/layers');
}

export type Colour = 'none' | 'voltage-band' | 'loading-heat';

/** Screen radius of a spread stack of `n` buses: wider for more, so their symbols stay apart. */
export const stackRadius = (n: number) => 8 + 4 * n;

/** Pixel size of the paper behind text. */
const paperText = (a: number) => alpha(PAPER, a);

/** Screen width of the hover halos: wide enough to read under MV and LV lines alike. */
const HALO_PX = 10;

const cartesian = (kit: DeckKit) => ({ coordinateSystem: kit.core.COORDINATE_SYSTEM.CARTESIAN });

/** Buses gliding between layouts along straight chords. */
export function morphLayers(
	kit: DeckKit,
	morph: { s: Float32Array; t: Float32Array; cur: Float32Array; endpoints: Int32Array },
	busCount: number
): Layer[] {
	const common = cartesian(kit);
	return [
		new kit.layers.LineLayer({
			...common,
			id: 'morph-chords',
			data: {
				length: morph.endpoints.length / 2,
				attributes: {
					getSourcePosition: { value: morph.s, size: 3 },
					getTargetPosition: { value: morph.t, size: 3 }
				}
			},
			getColor: [120, 114, 102, 200],
			getWidth: 1,
			widthUnits: 'pixels'
		}),
		new kit.layers.ScatterplotLayer({
			...common,
			id: 'morph-buses',
			data: { length: busCount, attributes: { getPosition: { value: morph.cur, size: 3 } } },
			getFillColor: INK,
			getRadius: 2,
			radiusUnits: 'pixels'
		})
	];
}

/** The last value for the last keys, compared by identity. */
function keyed<R>() {
	let last: unknown[] | null = null;
	let value: R;
	return (keys: unknown[], compute: () => R): R => {
		if (!last || keys.length !== last.length || keys.some((k, i) => k !== last![i])) {
			value = compute();
			last = keys;
		}
		return value;
	};
}

/** Per-canvas memory of layer data. deck.gl compares a layer's `data` by reference, so a layer
 * whose inputs did not change gets the same data and is left alone: a hover or selection change
 * rebuilds only the small highlight layers, not 50,000 segment colours and every label. */
/** Columnar layer data: a count and typed-array attributes. */
type BinaryData = {
	length: number;
	attributes: Record<
		string,
		{ value: Float32Array | Uint8Array; size: number; normalized?: boolean }
	>;
};

export function createLayerCache() {
	return {
		branches: keyed<BinaryData>(),
		underlay: keyed<BinaryData>(),
		equipment: keyed<BinaryData>(),
		ticks: keyed<BinaryData>(),
		arrows: keyed<BinaryData>(),
		itemTexts: keyed<EquipmentItem[]>(),
		transformerTexts: keyed<EquipmentItem[]>(),
		zoneTags: keyed<SchematicScene['zones']['tags']>(),
		corridors: keyed<{ segments: { z: number; s: number }[]; buses: { z: number; i: number }[] }>(),
		segmentsOf: keyed<Map<number, number[]>>()
	};
}
export type LayerCache = ReturnType<typeof createLayerCache>;

/** Everything one frame of the diagram draws, as the canvas derives it. */
export interface SchematicScene {
	graph: DistGraph;
	layout: 'schematic' | 'source';
	colour: Colour;
	/** The layout on screen, before stacks spread. */
	shown: SchematicGeometry;
	/** Bus positions as drawn (stacks spread). */
	geometry: SchematicGeometry;
	/** Branch segments as drawn (cut under symbols when they show). */
	drawn: SchematicGeometry;
	/** Equipment symbols, when they are legible. */
	symbols: EquipmentSymbols | null;
	ticksOn: boolean;
	valuesOn: boolean;
	/** Terminal and transformer labels beside the symbols, at the deepest zoom. */
	labelsOn: boolean;
	zoomedOut: boolean;
	live: { edges: ArrayLike<boolean | number>; buses: ArrayLike<boolean | number> };
	edgeColours: RGBA[];
	busColours: RGBA[];
	edgeMv: Uint8Array;
	snapshot: ResultSnapshot | null;
	/** Changes when attachment powers change (a text trigger). */
	powers: unknown;
	itemText: (d: EquipmentItem) => string;
	arrows: { edge: Int32Array; sources: Float32Array; targets: Float32Array } | null;
	selectedBus: number;
	selectedEdges: Set<number>;
	hoveredBus: number;
	hoveredEdges: Set<number>;
	cursor: number;
	labels: (LabelCandidate & { position: [number, number] })[];
	marks: {
		placed: Landmark[];
		strokes: { sources: Float32Array; targets: Float32Array; owner: Int32Array };
	} | null;
	markName: Map<string, string>;
	mvBuses: number[];
	allBuses: number[];
	zones: {
		states: ZoneState[];
		shapes: { z: number; polygon: [number, number][] }[];
		tags: { z: number; text: string; position: [number, number]; width: number }[];
		tagZoom: number;
		corridors: ZoneCorridor[];
	};
	/** Where the pointer is in the network: its network's branches faintly, its feeder's strongly,
	 * the path back to the source when shown, and the network's supply point. */
	hover: {
		network: Set<number>;
		feeder: Set<number>;
		path: Set<number>;
		supply: [number, number] | null;
		tint: RGBA;
	} | null;
	stacks: BusStacks | null;
	spreadStacks: number[];
	/** Changes whenever attachment text would (the canvas's text source). */
	textSource: unknown;
	cache: LayerCache;
	fans: FannedBranch[];
	fannedGroups: number[];
	fan: FanState;
}

export function sceneLayers(kit: DeckKit, s: SchematicScene): Layer[] {
	const { LineLayer, ScatterplotLayer, TextLayer, PolygonLayer } = kit.layers;
	const common = cartesian(kit);
	const g = s.geometry;
	const d = s.drawn;
	const cache = s.cache;
	const segmentsOf = cache.segmentsOf([d], () => segmentIndex(d));
	const point = (i: number): [number, number, number] => [
		g.positions[i * 3],
		g.positions[i * 3 + 1],
		0
	];
	const segment = (i: number, end: Float32Array): [number, number, number] => [
		end[i * 3],
		end[i * 3 + 1],
		0
	];
	const symbols = s.symbols;
	return [
		// Zone outlines sit beneath everything; they pick only where nothing else is.
		...corridorLayers(kit, s),
		...hoverHalos(kit, s),
		new PolygonLayer({
			...common,
			id: 'zones',
			data: s.zones.shapes,
			getPolygon: (z: { polygon: [number, number][] }) => z.polygon,
			getFillColor: (z: { z: number }) => ZONE_COLOURS[s.zones.states[z.z]][0],
			getLineColor: (z: { z: number }) => ZONE_COLOURS[s.zones.states[z.z]][1],
			getLineWidth: 1,
			lineWidthUnits: 'pixels',
			lineWidthMinPixels: 1,
			pickable: true,
			updateTriggers: { getFillColor: s.zones.states, getLineColor: s.zones.states }
		}),
		new TextLayer({
			...common,
			id: 'zone-tags',
			data: cache.zoneTags(
				[s.zones.tags, s.zones.tagZoom, s.layout === 'source' && !!symbols],
				() =>
					s.layout === 'source' && symbols
						? []
						: s.zones.tags.filter((t) => t.width * 2 ** s.zones.tagZoom >= 150)
			),
			getPosition: (t: { position: [number, number] }) => [...t.position, 0],
			getText: (t: { text: string }) => t.text,
			getColor: (t: { z: number }) => {
				const state = s.zones.states[t.z];
				return state === 'violation' || state === 'warning' ? stateInk(state) : ZONE_INK;
			},
			getSize: 10,
			getTextAnchor: 'start',
			getAlignmentBaseline: 'bottom',
			getPixelOffset: [2, -2],
			background: true,
			getBackgroundColor: paperText(220),
			backgroundPadding: [3, 1],
			fontFamily: MONO,
			characterSet: 'auto',
			updateTriggers: { getColor: s.zones.states }
		}),
		// Result colours are quiet for normal values; a faint underlay keeps the topology legible.
		...(s.colour === 'none'
			? []
			: [
					new LineLayer({
						...common,
						id: 'topology-underlay',
						data: cache.underlay([d], () => ({
							length: d.segmentEdge.length,
							attributes: {
								getSourcePosition: { value: d.sources, size: 3 },
								getTargetPosition: { value: d.targets, size: 3 }
							}
						})),
						getColor: [120, 114, 102, 70],
						getWidth: 1,
						widthUnits: 'pixels'
					})
				]),
		new LineLayer({
			...common,
			id: `branches-${s.layout}`,
			data: cache.branches(
				[d, s.edgeColours, s.edgeMv, s.zoomedOut, s.fan, s.colour !== 'none'],
				() => ({
					length: d.segmentEdge.length,
					attributes: {
						getSourcePosition: { value: d.sources, size: 3 },
						getTargetPosition: { value: d.targets, size: 3 },
						getColor: {
							value: segmentColours(d, s.edgeColours, s.edgeMv, s.zoomedOut, s.fan),
							size: 4,
							normalized: true
						},
						getWidth: { value: segmentWidths(d, s.edgeMv, s.colour !== 'none'), size: 1 }
					}
				})
			),
			widthUnits: 'pixels',
			pickable: true
		}),
		new LineLayer({
			...common,
			id: 'selected-branch',
			data: highlighted(d, segmentsOf, s.selectedEdges, s.fan),
			getSourcePosition: (i: number) => segment(i, d.sources),
			getTargetPosition: (i: number) => segment(i, d.targets),
			getColor: ACCENT,
			getWidth: 4,
			widthUnits: 'pixels'
		}),
		...sharedRouteLayers(kit, s),
		...stackLayers(kit, s),
		...(symbols
			? [
					new LineLayer({
						...common,
						id: 'equipment',
						data: cache.equipment([symbols, s.fan.edges, s.live], () => ({
							length: symbols.sources.length / 3,
							attributes: {
								getSourcePosition: { value: symbols.sources, size: 3 },
								getTargetPosition: { value: symbols.targets, size: 3 },
								getColor: {
									value: strokeColours(symbols.owner, s.fan.edges, s.live),
									size: 4,
									normalized: true
								}
							}
						})),
						getWidth: 1.25,
						widthUnits: 'pixels'
					}),
					...(s.valuesOn && s.arrows ? [flowArrowLayer(kit, s.arrows, s.fan.edges, cache)] : []),
					...(s.ticksOn
						? [
								new LineLayer({
									...common,
									id: 'conductor-ticks',
									data: cache.ticks([symbols, s.fan.edges, s.live], () => ({
										length: symbols.ticks.edge.length,
										attributes: {
											getSourcePosition: { value: symbols.ticks.sources, size: 3 },
											getTargetPosition: { value: symbols.ticks.targets, size: 3 },
											getColor: {
												value: strokeColours(symbols.ticks.edge, s.fan.edges, s.live),
												size: 4,
												normalized: true
											}
										}
									})),
									getWidth: 1,
									widthUnits: 'pixels'
								})
							]
						: []),
					// Invisible discs over each symbol make it clickable and give it a tooltip.
					new ScatterplotLayer({
						...common,
						id: 'equipment-hits',
						data: symbols.items,
						getPosition: (item: EquipmentItem) => [item.x, item.y, 0],
						getRadius: symbols.unit * 1.3,
						radiusUnits: 'common',
						getFillColor: [0, 0, 0, 1],
						pickable: true
					}),
					new TextLayer({
						...common,
						id: 'equipment-text',
						data: cache.itemTexts([symbols.items, s.textSource], () =>
							symbols.items.filter((item) => item.bus >= 0 && s.itemText(item))
						),
						getPosition: (item: EquipmentItem) => [item.x + symbols.unit * 1.1, item.y, 0],
						getText: s.itemText,
						updateTriggers: { getText: [s.valuesOn, s.labelsOn, s.powers] },
						getColor: SYMBOL_INK,
						getSize: 10,
						getTextAnchor: 'start',
						getAlignmentBaseline: 'center',
						// Legible where a label crosses a lead (a neutral earth beside a shunt).
						background: true,
						getBackgroundColor: paperText(200),
						backgroundPadding: [2, 0],
						fontFamily: MONO,
						characterSet: 'auto'
					}),
					// A transformer's vector group and rating, above its symbol: below sits a centre tap.
					new TextLayer({
						...common,
						id: 'transformer-text',
						data: cache.transformerTexts([symbols.items, s.labelsOn], () =>
							s.labelsOn ? symbols.items.filter((item) => item.label) : []
						),
						getPosition: (item: EquipmentItem) => [item.x, item.y, 0],
						getText: (item: EquipmentItem) => item.label ?? '',
						getColor: SYMBOL_INK,
						getSize: 10,
						getTextAnchor: 'middle',
						getAlignmentBaseline: 'bottom',
						getPixelOffset: [0, -12],
						background: true,
						getBackgroundColor: paperText(220),
						backgroundPadding: [2, 1],
						fontFamily: MONO,
						characterSet: 'auto'
					})
				]
			: []),
		new ScatterplotLayer({
			...common,
			id: `buses-${s.layout}`,
			// Zoomed out, LV bus dots are noise unless they carry the voltage colouring.
			data: s.zoomedOut && s.colour !== 'voltage-band' ? s.mvBuses : s.allBuses,
			getPosition: point,
			getFillColor: (i: number) => s.busColours[i],
			// Zoomed out, small dots keep dense chains from merging into bars.
			getRadius: symbols ? 2.6 : 1.4,
			radiusUnits: 'pixels',
			radiusMinPixels: 1,
			pickable: true,
			updateTriggers: {
				getFillColor: [s.colour, s.snapshot],
				getPosition: g,
				getRadius: !!symbols
			}
		}),
		new ScatterplotLayer({
			...common,
			id: 'selected-bus',
			data: s.selectedBus >= 0 ? [s.selectedBus] : [],
			getPosition: point,
			getFillColor: [0, 0, 0, 0],
			getLineColor: ACCENT,
			stroked: true,
			filled: false,
			getRadius: 7,
			radiusUnits: 'pixels',
			lineWidthUnits: 'pixels',
			getLineWidth: 2.5
		}),
		// Hovered in a linked view (profile, table) or here: a soft halo.
		new ScatterplotLayer({
			...common,
			id: 'hovered-bus',
			data: s.hoveredBus >= 0 ? [s.hoveredBus] : [],
			getPosition: point,
			getFillColor: alpha(ACCENT, 60),
			getRadius: 9,
			radiusUnits: 'pixels'
		}),
		new LineLayer({
			...common,
			id: 'hovered-branch',
			data: highlighted(d, segmentsOf, s.hoveredEdges, s.fan),
			getSourcePosition: (i: number) => segment(i, d.sources),
			getTargetPosition: (i: number) => segment(i, d.targets),
			getColor: alpha(ACCENT, 110),
			getWidth: 7,
			widthUnits: 'pixels'
		}),
		...hoverMarks(kit, s),
		...landmarkLayers(kit, s),
		new ScatterplotLayer({
			...common,
			id: 'keyboard-cursor',
			data: s.cursor >= 0 ? [s.cursor] : [],
			getPosition: point,
			getLineColor: INK,
			stroked: true,
			filled: false,
			getRadius: 11,
			radiusUnits: 'pixels',
			lineWidthUnits: 'pixels',
			getLineWidth: 1.5
		}),
		new TextLayer({
			...common,
			id: 'labels',
			data: s.labels,
			getPosition: (l: { position: [number, number] }) => [...l.position, 0],
			getText: (l: LabelCandidate) => l.text,
			getColor: (l: LabelCandidate) => stateInk(l.state),
			getSize: 11,
			getTextAnchor: 'start',
			// Above-right of the point: attachment symbols hang below buses.
			getAlignmentBaseline: 'bottom',
			getPixelOffset: [6, -5],
			background: true,
			getBackgroundColor: paperText(230),
			backgroundPadding: [3, 1],
			fontFamily: MONO,
			characterSet: 'auto'
		})
	];
}

/** Each edge's drawn segments, built once per drawing. */
function segmentIndex(d: SchematicGeometry): Map<number, number[]> {
	const out = new Map<number, number[]>();
	for (let i = 0; i < d.segmentEdge.length; i++) {
		const k = d.segmentEdge[i];
		const list = out.get(k);
		if (list) list.push(i);
		else out.set(k, [i]);
	}
	return out;
}

/** Drawn segments of `edges`, except those replaced by a fanned route: proportional to the edges
 * asked for, not to the drawing. */
function highlighted(
	d: SchematicGeometry,
	segmentsOf: Map<number, number[]>,
	edges: Set<number>,
	fan: FanState
): number[] {
	const out: number[] = [];
	for (const k of edges)
		for (const i of segmentsOf.get(k) ?? []) if (!hiddenUnderFan(d, i, fan)) out.push(i);
	return out;
}

function flowArrowLayer(
	kit: DeckKit,
	arrows: { edge: Int32Array; sources: Float32Array; targets: Float32Array },
	fannedEdges: Set<number>,
	cache: LayerCache
): Layer {
	return new kit.layers.LineLayer({
		...cartesian(kit),
		id: 'flow-arrows',
		data: cache.arrows([arrows, fannedEdges], () => ({
			length: arrows.edge.length,
			attributes: {
				getSourcePosition: { value: arrows.sources, size: 3 },
				getTargetPosition: { value: arrows.targets, size: 3 },
				getColor: {
					value: Uint8Array.from({ length: arrows.edge.length * 4 }, (_, i) =>
						fannedEdges.has(arrows.edge[i >> 2]) ? 0 : ACCENT[i & 3]
					),
					size: 4,
					normalized: true
				}
			}
		})),
		getWidth: 1.5,
		widthUnits: 'pixels'
	});
}

/** Fixed-size markers for sources, generators, IBRs and transformers while zoomed out. */
function landmarkLayers(kit: DeckKit, s: SchematicScene): Layer[] {
	const marks = s.marks;
	if (!marks?.placed.length) return [];
	const { LineLayer, ScatterplotLayer, TextLayer } = kit.layers;
	const common = cartesian(kit);
	return [
		// A paper halo lifts each marker off the faded drawing; it is also the pick target.
		new ScatterplotLayer({
			...common,
			id: 'landmark-halos',
			data: marks.placed,
			getPosition: (l: Landmark) => [l.x, l.y, 0],
			// Semi-transparent: lifts the marker without blanking the drawing under it.
			getFillColor: paperText(150),
			getRadius: 11,
			radiusUnits: 'pixels',
			pickable: true
		}),
		new LineLayer({
			...common,
			id: 'landmarks',
			data: {
				length: marks.strokes.owner.length,
				attributes: {
					getSourcePosition: { value: marks.strokes.sources, size: 3 },
					getTargetPosition: { value: marks.strokes.targets, size: 3 },
					getColor: { value: landmarkColours(marks), size: 4, normalized: true }
				}
			},
			getWidth: 1.6,
			widthUnits: 'pixels'
		}),
		new TextLayer({
			...common,
			id: 'landmark-names',
			data: marks.placed.filter((l) => l.kind === 'source' || l.kind === 'generator'),
			getPosition: (l: Landmark) => [l.x, l.y, 0],
			getText: (l: Landmark) =>
				(s.markName.get(l.ids[0]) ?? l.ids[0]) + (l.ids.length > 1 ? ` +${l.ids.length - 1}` : ''),
			getColor: INK,
			getSize: 10,
			getTextAnchor: 'middle',
			getAlignmentBaseline: 'top',
			getPixelOffset: [0, 14],
			background: true,
			getBackgroundColor: paperText(220),
			backgroundPadding: [2, 1],
			fontFamily: MONO,
			characterSet: 'auto'
		})
	];
}

/** Fanned members of shared routes (with their marks), and the count badges of the others. */
function sharedRouteLayers(kit: DeckKit, s: SchematicScene): Layer[] {
	const eq = s.symbols;
	if (!eq) return [];
	const { LineLayer, TextLayer } = kit.layers;
	const common = cartesian(kit);
	const src: number[] = [];
	const dst: number[] = [];
	const owner: number[] = [];
	for (const f of s.fans)
		for (let i = 1; i < f.path.length; i++) {
			src.push(f.path[i - 1][0], f.path[i - 1][1], 0);
			dst.push(f.path[i][0], f.path[i][1], 0);
			owner.push(f.edge);
		}
	// Marks on the fanned paths: the branch's symbol, or its conductor marks.
	const mSrc: number[] = [];
	const mDst: number[] = [];
	for (const f of s.fans) {
		const e = s.graph.edges[f.edge];
		const glyph = branchGlyph(e.kind, e.closed);
		const { phases, neutral } = conductorSet(s.graph, f.edge);
		const shape = glyph ?? (s.ticksOn ? conductorTicks(phases, neutral) : null);
		if (!shape) continue;
		const [cos, sin] = f.dir;
		const u = eq.unit * (glyph ? 0.8 : 1);
		for (const line of shape)
			for (let i = 1; i < line.length; i++) {
				const [ax, ay] = line[i - 1];
				const [bx, by] = line[i];
				mSrc.push(f.mid[0] + u * (ax * cos - ay * sin), f.mid[1] + u * (ax * sin + ay * cos), 0);
				mDst.push(f.mid[0] + u * (bx * cos - by * sin), f.mid[1] + u * (bx * sin + by * cos), 0);
			}
	}
	const colours = new Uint8Array(owner.length * 4);
	const widths = new Float32Array(owner.length);
	const [mv, lv] = lineWidths(s.colour !== 'none');
	owner.forEach((k, i) => {
		const selected = s.selectedEdges.has(k);
		const hovered = s.hoveredEdges.has(k);
		colours.set(selected ? ACCENT : hovered ? alpha(ACCENT, 150) : s.edgeColours[k], i * 4);
		widths[i] = selected ? 3.5 : hovered ? 4 : s.edgeMv[k] ? mv : lv;
	});
	const badges = s.ticksOn
		? eq.routeGroups.flatMap((members, g) => {
				if (s.fannedGroups.includes(g)) return [];
				const p = routeMiddle(members, eq.host, s.geometry);
				return p ? [{ g, n: members.length, position: p }] : [];
			})
		: [];
	return [
		...(owner.length
			? [
					new LineLayer({
						...common,
						id: 'fan-branches',
						data: {
							length: owner.length,
							attributes: {
								getSourcePosition: { value: Float32Array.from(src), size: 3 },
								getTargetPosition: { value: Float32Array.from(dst), size: 3 },
								getColor: { value: colours, size: 4, normalized: true },
								getWidth: { value: widths, size: 1 }
							}
						},
						widthUnits: 'pixels',
						pickable: true
					}),
					new LineLayer({
						...common,
						id: 'fan-marks',
						data: {
							length: mSrc.length / 3,
							attributes: {
								getSourcePosition: { value: Float32Array.from(mSrc), size: 3 },
								getTargetPosition: { value: Float32Array.from(mDst), size: 3 }
							}
						},
						getColor: SYMBOL_INK,
						getWidth: 1,
						widthUnits: 'pixels'
					})
				]
			: []),
		...(badges.length
			? [
					new TextLayer({
						...common,
						id: 'route-badges',
						data: badges,
						getPosition: (b: { position: [number, number] }) => [...b.position, 0],
						getText: (b: { n: number }) => `×${b.n}`,
						getColor: SYMBOL_INK,
						getSize: 11,
						getPixelOffset: [0, -10],
						background: true,
						getBackgroundColor: paperText(230),
						backgroundPadding: [3, 1],
						fontFamily: MONO,
						characterSet: 'auto',
						pickable: true
					})
				]
			: [])
	];
}

/** Zone corridors on geographic positions while zoomed out: a band of fixed screen width along
 * each LV network's branches, with discs at its buses to round the joints, quiet for networks
 * within limits and in their state's colour otherwise. A translucent band would darken wherever
 * its pieces overlap, so the bands test depth strictly, all at one depth (no per-layer polygon
 * offset): each pixel is tinted once. Close in, the lines speak for themselves. */
function corridorLayers(kit: DeckKit, s: SchematicScene): Layer[] {
	const { corridors, states } = s.zones;
	if (!corridors.length || s.symbols) return [];
	const { LineLayer, ScatterplotLayer } = kit.layers;
	const common = cartesian(kit);
	const g = s.shown;
	const once = { depthWriteEnabled: true, depthCompare: 'less' } as const;
	const band = (z: number): RGBA => {
		const state = states[z];
		if (state === 'warning' || state === 'violation') {
			const fill = ZONE_COLOURS[state][0];
			return alpha(fill, Math.min(255, fill[3] * 3));
		}
		return alpha(ZONE_INK, 56);
	};
	const { segments, buses } = s.cache.corridors([corridors], () => ({
		segments: corridors.flatMap((c) => c.segments.map((i) => ({ z: c.z, s: i }))),
		buses: corridors.flatMap((c) => c.buses.map((i) => ({ z: c.z, i })))
	}));
	return [
		new LineLayer({
			...common,
			id: 'zones',
			data: segments,
			getSourcePosition: (d: { s: number }) => [g.sources[d.s * 3], g.sources[d.s * 3 + 1], 0],
			getTargetPosition: (d: { s: number }) => [g.targets[d.s * 3], g.targets[d.s * 3 + 1], 0],
			getColor: (d: { z: number }) => band(d.z),
			getWidth: CORRIDOR_PX,
			widthUnits: 'pixels',
			parameters: once,
			getPolygonOffset: null,
			pickable: true,
			updateTriggers: { getColor: states }
		}),
		new ScatterplotLayer({
			...common,
			id: 'zones-buses',
			data: buses,
			getPosition: (d: { i: number }) => [g.positions[d.i * 3], g.positions[d.i * 3 + 1], 0],
			getFillColor: (d: { z: number }) => band(d.z),
			getRadius: CORRIDOR_PX / 2,
			radiusUnits: 'pixels',
			parameters: once,
			getPolygonOffset: null,
			pickable: true,
			updateTriggers: { getFillColor: states }
		})
	];
}

/** The hovered network and feeder, as halos beneath the lines: the network faint, the feeder
 * strong, the same on MV and LV. */
function hoverHalos(kit: DeckKit, s: SchematicScene): Layer[] {
	const h = s.hover;
	if (!h) return [];
	const d = s.drawn;
	const halo = (id: string, edges: Set<number>, a: number) =>
		new kit.layers.LineLayer({
			...cartesian(kit),
			id,
			data: highlighted(
				d,
				s.cache.segmentsOf([d], () => segmentIndex(d)),
				edges,
				s.fan
			),
			getSourcePosition: (i: number) => [d.sources[i * 3], d.sources[i * 3 + 1], 0],
			getTargetPosition: (i: number) => [d.targets[i * 3], d.targets[i * 3 + 1], 0],
			getColor: alpha(h.tint, a),
			getWidth: HALO_PX,
			widthUnits: 'pixels'
		});
	return [halo('hover-network', h.network, 90), halo('hover-feeder', h.feeder, 190)];
}

/** Over the lines: the path back to the source, when shown, and a ring at the supply point. */
function hoverMarks(kit: DeckKit, s: SchematicScene): Layer[] {
	const h = s.hover;
	if (!h) return [];
	const d = s.drawn;
	const common = cartesian(kit);
	return [
		...(h.path.size
			? [
					new kit.layers.LineLayer({
						...common,
						id: 'hover-path',
						data: highlighted(
							d,
							s.cache.segmentsOf([d], () => segmentIndex(d)),
							h.path,
							s.fan
						),
						getSourcePosition: (i: number) => [d.sources[i * 3], d.sources[i * 3 + 1], 0],
						getTargetPosition: (i: number) => [d.targets[i * 3], d.targets[i * 3 + 1], 0],
						getColor: PATH,
						getWidth: 3,
						widthUnits: 'pixels'
					})
				]
			: []),
		...(h.supply
			? [
					new kit.layers.ScatterplotLayer({
						...common,
						id: 'hover-supply',
						data: [h.supply],
						getPosition: (p: [number, number]) => [...p, 0],
						getFillColor: alpha(h.tint, 50),
						getLineColor: alpha(h.tint, 220),
						stroked: true,
						getRadius: 13,
						radiusUnits: 'pixels',
						getLineWidth: 2,
						lineWidthUnits: 'pixels'
					})
				]
			: [])
	];
}

/** Spokes from each spread stack's true point to its buses; a ring and ×n badge on the others. */
function stackLayers(kit: DeckKit, s: SchematicScene): Layer[] {
	const stacks = s.stacks;
	if (!stacks) return [];
	const { LineLayer, ScatterplotLayer, TextLayer } = kit.layers;
	const common = cartesian(kit);
	const base = s.shown;
	const point = (g: number): [number, number] => {
		const i = stacks.groups[g][0];
		return [base.positions[i * 3], base.positions[i * 3 + 1]];
	};
	const spokes: { from: [number, number]; to: [number, number] }[] = [];
	for (const g of s.spreadStacks)
		for (const i of stacks.groups[g])
			spokes.push({
				from: point(g),
				to: [s.geometry.positions[i * 3], s.geometry.positions[i * 3 + 1]]
			});
	// A spread stack keeps a faint ring around its circle, so the gaps between its buses still
	// belong to it: the hover holds and a click there pins it.
	const rings = stacks.groups.flatMap((members, g) =>
		s.spreadStacks.includes(g)
			? [{ g, n: members.length, position: point(g), spread: true }]
			: s.symbols
				? [{ g, n: members.length, position: point(g), spread: false }]
				: []
	);
	const closed = rings.filter((r) => !r.spread);
	return [
		...(spokes.length
			? [
					new LineLayer({
						...common,
						id: 'stack-spokes',
						data: spokes,
						getSourcePosition: (d: { from: [number, number] }) => [...d.from, 0],
						getTargetPosition: (d: { to: [number, number] }) => [...d.to, 0],
						getColor: [120, 114, 102, 140],
						getWidth: 1,
						widthUnits: 'pixels'
					})
				]
			: []),
		...(rings.length
			? [
					new ScatterplotLayer({
						...common,
						id: 'stack-rings',
						data: rings,
						getPosition: (d: { position: [number, number] }) => [...d.position, 0],
						stroked: true,
						filled: true,
						getFillColor: [0, 0, 0, 1],
						getLineColor: (d: { spread: boolean }) =>
							d.spread ? alpha(SYMBOL_INK, 70) : SYMBOL_INK,
						getRadius: (d: { spread: boolean; n: number }) => (d.spread ? stackRadius(d.n) + 6 : 6),
						radiusUnits: 'pixels',
						lineWidthUnits: 'pixels',
						getLineWidth: 1,
						pickable: true,
						updateTriggers: { getLineColor: s.spreadStacks, getRadius: s.spreadStacks }
					})
				]
			: []),
		...(closed.length
			? [
					new TextLayer({
						...common,
						id: 'stack-badges',
						data: closed,
						getPosition: (d: { position: [number, number] }) => [...d.position, 0],
						getText: (d: { n: number }) => `×${d.n}`,
						getColor: SYMBOL_INK,
						getSize: 10,
						getTextAnchor: 'end',
						getAlignmentBaseline: 'bottom',
						getPixelOffset: [-6, -5],
						background: true,
						getBackgroundColor: paperText(220),
						backgroundPadding: [2, 1],
						fontFamily: MONO,
						characterSet: 'auto'
					})
				]
			: [])
	];
}
