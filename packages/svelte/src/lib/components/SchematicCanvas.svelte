<script lang="ts">
	import type {
		Deck,
		Layer,
		OrthographicView,
		OrthographicViewState,
		PickingInfo
	} from '@deck.gl/core';
	import { untrack } from 'svelte';
	import { mcSchematicLayout, type SchematicStrategy } from '@tellegen/engine';
	import type { MulticonductorCase } from '../state.svelte.js';
	import type { RGBA } from '../colors.js';
	import { priceGradient, sensGradient } from '../colors.js';
	import { getPanelLayout } from '../panels.svelte.js';
	import { getAppState } from '../context.svelte.js';
	import {
		bandColour,
		branchLabel,
		busLabel,
		busStacks,
		displaceBuses,
		spreadOffsets,
		chordsInto,
		displayTree,
		ease,
		graphInputs,
		diagramLayer,
		EMPTY_GEOMETRY,
		fromEngineLayout,
		lerpInto,
		loadingColour,
		MISSING_COLOUR,
		MORPH_MAX_BUSES,
		MORPH_MS,
		placeLabels,
		screenAnchored,
		snapshotFromMcPf,
		sourceLayout,
		titleBlock,
		worstBand,
		attachmentGlyph,
		ATTACHMENT_NAMES,
		describeBus,
		treeWalk,
		type WalkKey,
		BASEMAP_STYLE,
		basemapCamera,
		cameraFromMap,
		fanRoute,
		routeMiddle,
		type FannedBranch,
		galvanicZones,
		landmarkCandidates,
		landmarkGlyph,
		landmarkStrokes,
		placeLandmarks,
		type Landmark,
		voltageClass,
		levelLabel,
		VOLTAGE_CLASSES,
		feeders,
		pathToSource,
		type VoltageClass,
		zoneSummary,
		zoneVoltage,
		zoneCorridors,
		zoneOutline,
		zoneState,
		type GalvanicZone,
		conductorTicks,
		elementPowers,
		flowArrows,
		energisation,
		shortNames,
		branchGlyph,
		transformerGlyph,
		transformerInfo,
		glyphPath,
		schematicEquipment,
		type EquipmentItem,
		type Glyph,
		type LabelCandidate,
		type SchematicGeometry
	} from '../diagram/index.js';
	import { canvasPng, download, fileStem, sceneSvg } from './schematic/export.js';
	import {
		createLayerCache,
		morphLayers,
		sceneLayers,
		stackRadius,
		type Colour,
		type SchematicScene
	} from './schematic/layers.js';
	import type { FanState } from './schematic/paint.js';
	import { DEAD, HALO, INK, LEVEL_COLOURS } from './schematic/palette.js';
	import PickCard from './schematic/PickCard.svelte';
	import { SchematicText, tooltip } from './schematic/text.js';
	import { HoverPin, StoredFlag } from './schematic/view-prefs.svelte.js';
	import {
		applyBasemapTuning,
		attachBasemapFallback,
		basemapCredit,
		type BasemapProvider,
		type BasemapSpec
	} from '../basemap.js';

	/** Without a basemap from the map: the keyless CARTO style it used to default to. */
	const DEFAULT_BASEMAP: BasemapSpec = {
		primary: BASEMAP_STYLE,
		fallback: null,
		hideLabels: false,
		primaryProvider: 'carto',
		fallbackProvider: null
	};

	let {
		mc,
		onselectbus,
		onselectedge,
		onclose,
		basemap: basemapSpec = DEFAULT_BASEMAP
	}: {
		mc: MulticonductorCase;
		onselectbus: (id: string) => void;
		onselectedge: (id: string) => void;
		onclose: () => void;
		/** The background map under geographic positions: the map's own basemap, light-themed. */
		basemap?: BasemapSpec;
	} = $props();

	type Layout = 'schematic' | 'source';
	const COLOURS: Record<Colour, string> = {
		none: 'Equipment and topology',
		'voltage-band': 'Voltage within declared band',
		'loading-heat': 'Loading, % of rating'
	};
	const app = getAppState();
	const panels = getPanelLayout();

	let layout = $state<Layout>('schematic');
	let strategy = $state<SchematicStrategy>('tidy');
	/** The geometry on screen; replaced by `show`, which animates between layouts. */
	let shown = $state.raw<SchematicGeometry | null>(null);
	let colour = $state<Colour>('none');
	let width = $state(0);
	let height = $state(0);
	let canvas = $state<HTMLCanvasElement>();
	let deck = $state.raw<Deck<OrthographicView> | null>(null);
	let modules: {
		core: typeof import('@deck.gl/core');
		layers: typeof import('@deck.gl/layers');
	} | null = null;
	let camera = $state.raw<{ target: [number, number, number]; zoom: number }>({
		target: [0, 0, 0],
		zoom: 0
	});
	let labels = $state.raw<(LabelCandidate & { position: [number, number] })[]>([]);
	/** Map tiles under geographic positions, as for the main map; they fade in and out with the
	 * layout switch. While fading out they stay pinned to the screen as the camera moves on. */
	const basemapPref = new StoredFlag('tellegen.schematic.basemap', ['on', 'off'], true);
	let basemapOpacity = $state(0);
	const BASEMAP_FADE_MS = 700;
	let mapBox = $state<HTMLDivElement>();
	let basemap: import('maplibre-gl').Map | null = null;
	let basemapLoading = false;
	/** Who serves the basemap on screen, for its credit line. */
	let basemapProvider = $state<BasemapProvider>('custom');
	/** Landmarks placed for the current view, with their fixed-pixel glyph strokes. */
	let marks = $state.raw<{
		placed: Landmark[];
		strokes: { sources: Float32Array; targets: Float32Array; owner: Int32Array };
	} | null>(null);
	let status = $state<string | null>('Loading the schematic renderer…');

	const graph = $derived(mc.graph!);
	const tree = $derived(displayTree(graph));
	const inputs = $derived(graphInputs(graph));
	const source = $derived(
		mc.view
			? sourceLayout(graph, mc.view.buses, mc.coordsKind === 'geographic', tree, mc.view.edges)
			: null
	);
	/** "Separate overlapping buses and lines": every stack and shared route stays spread. */
	const separatePref = new StoredFlag('tellegen.schematic.separate', ['on', 'off'], false);
	// Stacked buses: several buses at exactly the same point (MV and LV at one pole). At rest a ring
	// and a ×n badge mark the stack; hovering it, or the keyboard cursor on one of its buses,
	// spreads the buses on a small circle of fixed screen size, with a card listing them; clicking
	// pins it. "Separate overlapping buses and lines" keeps every stack spread.
	const stacks = $derived(shown && layout === 'source' ? busStacks(shown) : null);
	const stackPin = new HoverPin();
	let cursorStack = $state<number | null>(null);
	/** The stack whose card shows: pinned, else hovered, else the keyboard cursor's. */
	const activeStack = $derived(stackPin.active ?? cursorStack);
	const spreadStacks = $derived(
		!stacks
			? []
			: separatePref.value
				? stacks.groups.map((_, g) => g)
				: activeStack === null
					? []
					: [activeStack]
	);
	/** Pixels per drawing unit in quarter zoom steps, so spread stacks keep a fixed screen size. */
	const stackScale = $derived(2 ** (Math.floor(camera.zoom * 4) / 4));
	const geometry = $derived.by<SchematicGeometry>(() => {
		if (!shown) return EMPTY_GEOMETRY;
		if (!stacks || !spreadStacks.length) return shown;
		const offsets = new Map<number, [number, number]>();
		for (const g of spreadStacks) {
			const members = stacks.groups[g];
			spreadOffsets(members.length, stackScale, stackRadius(members.length)).forEach((o, m) =>
				offsets.set(members[m], o)
			);
		}
		return displaceBuses(graph, shown, offsets);
	});
	const basemapWanted = $derived(
		basemapPref.value &&
			layout === 'source' &&
			mc.coordsKind === 'geographic' &&
			!!source?.mercatorOrigin
	);
	let fadeFrame = 0;
	$effect(() => {
		const target = basemapWanted ? 1 : 0;
		untrack(() => {
			cancelAnimationFrame(fadeFrame);
			const start = basemapOpacity;
			if (start === target) return;
			if (matchMedia('(prefers-reduced-motion: reduce)').matches) {
				basemapOpacity = target;
				return;
			}
			const t0 = performance.now();
			const step = (now: number) => {
				const k = Math.min(1, (now - t0) / BASEMAP_FADE_MS);
				basemapOpacity = start + (target - start) * ease(k);
				if (k < 1) fadeFrame = requestAnimationFrame(step);
			};
			fadeFrame = requestAnimationFrame(step);
		});
	});

	// The basemap map is created the first time it is wanted, and follows the schematic camera only
	// while wanted: fading out, it stays where it was on screen as the drawing moves to its new layout.
	$effect(() => {
		if (!basemapWanted || !mapBox || basemap || basemapLoading) return;
		basemapLoading = true;
		const container = mapBox;
		const spec = basemapSpec;
		basemapProvider = spec.primaryProvider ?? 'custom';
		Promise.all([
			import('maplibre-gl'),
			// As the map loads it: a package export keeps the worker out of the dev pre-bundle.
			import('@tellegen/svelte/maplibre-worker').then((m) => m.maplibreWorkerUrl())
		])
			.then(([maplibre, workerUrl]) => {
				maplibre.setWorkerUrl(workerUrl);
				const origin = source?.mercatorOrigin ?? [0, 0];
				const m = new maplibre.Map({
					container,
					style: spec.primary,
					interactive: false,
					attributionControl: false,
					...basemapCamera(camera, origin)
				});
				// The same style handling as the map: labels hidden when asked, OpenFreeMap tuned, and
				// the fallback style when the primary cannot be shown.
				m.on('style.load', () => applyBasemapTuning(m, spec, 'light'));
				attachBasemapFallback(m, spec, {
					onFallback: (reason) => (basemapProvider = reason.to ?? 'custom')
				});
				basemap = m;
			})
			.catch(() => {
				basemapLoading = false;
			});
	});
	$effect(() => {
		const origin = source?.mercatorOrigin;
		void [width, height];
		if (!basemapWanted || !origin) return;
		const view = basemapCamera(camera, origin);
		untrack(() => {
			basemap?.resize();
			basemap?.jumpTo(view);
		});
	});
	$effect(() => () => basemap?.remove());
	const powers = $derived(mc.overviewResult ? elementPowers(mc.overviewResult) : null);
	const snapshot = $derived(
		mc.overviewResult ? snapshotFromMcPf(mc.overviewResult, inputs.buses, inputs.branches) : null
	);
	/** Equipment symbols on the shown layout: branch symbols interrupt their line. */
	const equipment = $derived(shown ? schematicEquipment(graph, geometry) : null);
	/** Symbols draw once they are legible (a glyph about 6 px across); below that the plain lines
	 * and their kind colours carry the topology. */
	const symbolsOn = $derived(!!equipment && equipment.unit * 2 ** camera.zoom >= 3);
	/** The branch geometry as drawn, interrupted under symbols when they show. */
	const drawn = $derived<SchematicGeometry>(symbolsOn && equipment ? equipment.cut : geometry);
	/** Galvanic zones: buses joined without passing a transformer. Those without a source (the LV
	 * networks of a feeder) are tinted by their worst state: in the schematic as an outline that
	 * follows the drawing column by column, on geographic positions as a corridor along their own
	 * branches (a hull would overlap the hulls of networks interleaved with it). */
	const zones = $derived(galvanicZones(graph, tree));
	const zoneStates = $derived(zones.zones.map((z) => zoneState(z, snapshot)));
	const outlined = (zone: GalvanicZone) => !zone.hasSource && zone.buses.length >= 2;
	const zoneShapes = $derived.by(() => {
		if (!shown || !shown.attachmentGap) return [];
		const g = shown;
		// Column spacing of the engine layout: any tree edge spans exactly one column.
		let dx = 0;
		for (let i = 0; i < graph.buses.length && !dx; i++)
			if (tree.parent[i] !== i) dx = Math.abs(g.positions[i * 3] - g.positions[tree.parent[i] * 3]);
		const gap = g.attachmentGap ?? 0;
		const below = graph.buses.map((b) => {
			const kinds = new Set(
				Object.values(b.terminal_attachments ?? {}).flatMap((list) => list.map((a) => a.kind))
			);
			return kinds.size * gap;
		});
		if (!(dx > 0)) return [];
		return zones.zones.flatMap((zone, z) => {
			if (!outlined(zone)) return [];
			const polygon = zoneOutline(zone, g.positions, below, { dx, pad: gap * 0.5 });
			return polygon.length >= 3 ? [{ z, polygon }] : [];
		});
	});
	const corridors = $derived(
		shown && !shown.attachmentGap ? zoneCorridors(zones, shown, outlined) : []
	);
	/** The corridor under the pointer, as a network to emphasise. */
	let hoveredZone = $state(-1);
	/** Each network's voltage level (declared rating, else limits, else solved) and class. MV and
	 * above (from 1 kV phase–phase) is drawn heavier than LV, as on a paper single-line diagram,
	 * and each class has its own shade; a transformer takes its higher side. */
	const zoneVolts = $derived(zones.zones.map((z) => zoneVoltage(z, graph, snapshot)));
	const zoneClass = $derived(zones.zones.map((z, i) => voltageClass(z, zoneVolts[i])));
	const zoneMv = $derived(zoneClass.map((c) => c !== 'lv'));
	const CLASS_RANK: Record<VoltageClass, number> = { lv: 0, mv: 1, sub: 2, hv: 3 };
	const edgeClass = $derived(
		graph.edges.map((e, k): VoltageClass => {
			const { from, to } = inputs.names.branches[k];
			const at = (i: number): VoltageClass => (i >= 0 ? zoneClass[zones.busZone[i]] : 'lv');
			if (e.kind !== 'transformer') return at(from >= 0 ? from : to);
			return CLASS_RANK[at(from)] >= CLASS_RANK[at(to)] ? at(from) : at(to);
		})
	);
	const edgeMv = $derived(Uint8Array.from(edgeClass, (c) => (c === 'lv' ? 0 : 1)));
	/** The levels in the drawing, per class, for the key: "MV · 11 kV". */
	const levelKey = $derived(
		VOLTAGE_CLASSES.flatMap(({ cls, name }) => {
			if (!zoneClass.includes(cls)) return [];
			const levels = [
				...new Set(
					zoneVolts.flatMap((v, z) => (zoneClass[z] === cls && v !== null ? [levelLabel(v)] : []))
				)
			];
			return [{ cls, label: levels.length ? `${name} · ${levels.slice(0, 3).join(', ')}` : name }];
		})
	);
	/** Zoomed out, LV detail fades so the MV backbone, zone outlines and states read first. */
	const zoomedOut = $derived(!symbolsOn);
	const mvBuses = $derived(graph.buses.flatMap((_, i) => (zoneMv[zones.busZone[i]] ? [i] : [])));
	const allBuses = $derived(graph.buses.map((_, i) => i));
	const transformerNames = $derived.by(() => {
		const ks = graph.edges.flatMap((e, k) => (e.kind === 'transformer' ? [k] : []));
		const short = shortNames(
			ks.map((k) => graph.edges[k].id),
			16
		);
		return new Map(ks.map((k, j) => [k, short[j]]));
	});
	/** Zone tags at each outline's top-left: feeding transformer, size, and when solved the
	 * lowest voltage and highest loading. Drawn once the outline is wide enough on screen. */
	const zoneTags = $derived(
		[
			...zoneShapes.map(({ z, polygon }) => {
				const xs = polygon.map((p) => p[0]);
				const ys = polygon.map((p) => p[1]);
				return { z, bounds: [Math.min(...xs), Math.min(...ys), Math.max(...xs), Math.max(...ys)] };
			}),
			...corridors
		].map(({ z, bounds: [x0, y0, x1] }) => {
			const zone = zones.zones[z];
			const t = zone.transformers[0];
			const { minVolts, maxLoading } = zoneSummary(zone, snapshot);
			const v = (x: number) => (x >= 1000 ? `${(x / 1000).toFixed(2)} kV` : `${x.toFixed(1)} V`);
			const text = [
				t === undefined ? 'network' : (transformerNames.get(t) ?? graph.edges[t].id),
				`${zone.buses.length} buses`,
				...(minVolts === null ? [] : [`min ${v(minVolts)}`]),
				...(maxLoading === null ? [] : [`${maxLoading.toFixed(0)} %`])
			].join(' · ');
			return {
				z,
				text,
				position: [x0, y0] as [number, number],
				width: x1 - x0
			};
		})
	);
	/** Sources, generators, IBRs and transformers stay findable when zoomed out: drawn at a fixed
	 * screen size and placed by priority (see `landmarks.ts`), handing over to the full symbols as
	 * the drawing grows. */
	const landmarksPref = new StoredFlag('tellegen.schematic.landmarks', ['on', 'off'], true);
	const landmarkPool = $derived(
		shown ? landmarkCandidates(graph, geometry, equipment, zones, zoneStates, powers) : []
	);
	const markName = $derived.by(() => {
		const ids = [...new Set(landmarkPool.flatMap((l) => (l.kind === 'transformer' ? [] : l.ids)))];
		const short = shortNames(ids, 16);
		return new Map(ids.map((id, j) => [id, short[j]]));
	});
	/** Zoom in quarter steps: zone tags appear and go only when this changes, not on every pan. */
	const tagZoom = $derived(Math.floor(camera.zoom * 4) / 4);
	/** Conductor marks need a closer view than symbols; bus names closer still. */
	const ticksOn = $derived(!!equipment && equipment.unit * 2 ** camera.zoom >= 6);
	const namesOn = $derived(!!equipment && equipment.unit * 2 ** camera.zoom >= 9);
	/** Solved values beside symbols and flow arrows on branches, once there is room for them. */
	const valuesOn = $derived(!!snapshot && ticksOn);
	const arrows = $derived(
		equipment && snapshot
			? flowArrows(
					equipment,
					snapshot.branches.map((b) => b.ends),
					graph.edges.map((e) => e.closed)
				)
			: null
	);
	const live = $derived(energisation(graph));
	const names = $derived(shortNames(graph.buses.map((b) => b.id)));
	/** Whether the symbol key is expanded; remembered per viewer. */
	const keyPref = new StoredFlag('tellegen.schematic.key', ['open', 'closed'], true);
	/** Keyboard cursor: the bus the arrow keys walk from, distinct from the selection. */
	let cursor = $state(-1);
	let announcement = $state('');
	const walk = $derived(treeWalk(tree, geometry.positions));
	/** Shared-route member chosen with Tab at the cursor bus (−1 for none). */
	let tabEdge = -1;
	function moveCursor(to: number) {
		if (to < 0) return;
		tabEdge = -1;
		cursor = to;
		const g = stacks?.stackOf[to] ?? -1;
		cursorStack = g >= 0 ? g : null;
		announcement =
			describeBus(graph, tree, walk, to, text.voltageNote(to)) +
			(g >= 0 ? `. One of ${stacks!.groups[g].length} buses at this point` : '');
		bringIntoView([geometry.positions[to * 3], geometry.positions[to * 3 + 1]]);
	}
	// Shared routes: branches drawn along exactly the same segment (MV and LV on shared poles).
	// At rest the route carries a count badge; hovering it fans its members apart by a few pixels,
	// tapering into the shared ends, with a card listing them; clicking pins it open. The key can
	// keep every shared route separated.
	const routePin = new HoverPin();
	/** The one route whose card shows: pinned, else hovered. */
	const activeGroup = $derived(equipment && symbolsOn ? routePin.active : null);
	/** Routes drawn fanned out now. */
	const fannedGroups = $derived(
		!equipment || !symbolsOn
			? []
			: separatePref.value
				? equipment.routeGroups.map((_, g) => g)
				: activeGroup === null
					? []
					: [activeGroup]
	);
	const fannedEdges = $derived(
		new Set(fannedGroups.flatMap((g) => equipment?.routeGroups[g] ?? []))
	);
	function fanned(): FannedBranch[] {
		if (!equipment || !fannedGroups.length) return [];
		const scale = 2 ** tagZoom;
		return fannedGroups.flatMap((g) =>
			fanRoute(equipment.routeGroups[g], equipment.host, geometry, scale, {
				spreadPx: 11,
				taperPx: 18
			})
		);
	}
	const fan = $derived<FanState>({
		edges: fannedEdges,
		host: equipment?.host ?? null,
		geometry
	});
	/** The kinds of transformer in the drawing, for the key; `unknown` without winding detail. */
	const transformerKinds = $derived(
		new Set(
			graph.edges.flatMap((e, k) =>
				e.kind === 'transformer' ? [transformerInfo(graph, k)?.kind ?? 'unknown'] : []
			)
		)
	);
	/** Key entries per transformer kind; the three-phase one shows a Δ–Yn unit as its example. */
	const winding = (connection: 'wye' | 'delta', terminals: number) => ({
		bus: '',
		connection,
		terminals: Array.from({ length: terminals }, (_, i) => String(i + 1)),
		v_ref: 0,
		s_rating: 0
	});
	const TRANSFORMER_KEY: [string, Glyph, string][] = [
		[
			'poly',
			transformerGlyph(
				{ kind: 'poly', from: winding('delta', 3), to: winding('wye', 4), label: '' },
				3
			),
			'three-phase, windings Δ and Y (n: neutral out)'
		],
		[
			'split',
			transformerGlyph({ kind: 'split', from: null, to: null, label: '' }, 1),
			'split-phase (centre tap)'
		],
		[
			'single',
			transformerGlyph({ kind: 'single', from: null, to: null, label: '' }, 1),
			'single-phase'
		],
		['unknown', branchGlyph('transformer', true)!, 'transformer']
	];
	const legendGlyphs = $derived(
		equipment
			? (
					[
						...TRANSFORMER_KEY.filter(([k]) => transformerKinds.has(k as never)).map(
							([, glyph, label]) => ['transformer', glyph, label] as const
						),
						['switch', branchGlyph('switch', true), 'switch, closed'],
						['switch', branchGlyph('switch', false), 'switch, open'],
						...(['source', 'generator', 'ibr', 'load', 'shunt', 'earth', 'ground'] as const).map(
							(k) => [k, attachmentGlyph(k), ATTACHMENT_NAMES[k]] as const
						)
					] as [EquipmentItem['kind'], Glyph | null, string][]
				).filter(
					([kind, glyph, label]) =>
						glyph &&
						equipment.kinds.has(kind) &&
						(kind !== 'switch' ||
							graph.edges.some(
								(e) => e.kind === 'switch' && e.closed === (label === 'switch, closed')
							))
				)
			: []
	);
	/** Buses the case gives no position, in source positions. */
	const unplaced = $derived(
		layout === 'source' && geometry.inferred
			? geometry.inferred.reduce((n, v) => n + (v ? 1 : 0), 0)
			: 0
	);
	/** Toolbar and legend start right of the left dock, so no panel covers them. */
	const toolLeft = $derived(panels.compact ? 12 : panels.dockWidth('left') + 24);
	const busIndex = $derived(new Map(graph.buses.map((b, i) => [b.id, i])));
	const selectedBus = $derived(
		mc.selectedBusId === null ? -1 : (busIndex.get(mc.selectedBusId) ?? -1)
	);
	const selectedEdges = $derived(
		new Set(
			mc.selectedEdgeId === null
				? []
				: graph.edges.flatMap((e, k) => (e.id === mc.selectedEdgeId ? [k] : []))
		)
	);

	const hoveredBus = $derived(
		mc.hovered?.kind === 'bus' ? (busIndex.get(mc.hovered.id) ?? -1) : -1
	);
	const hoveredEdges = $derived(
		new Set(
			mc.hovered?.kind === 'branch'
				? graph.edges.flatMap((e, k) => (e.id === mc.hovered!.id ? [k] : []))
				: []
		)
	);

	/** Feeders of every network: the branches leaving its supply bus, on MV and LV alike. */
	const feederSet = $derived(feeders(graph, tree, zones));
	/** "Path to source": the hover also traces the drawn path back to the source. */
	const pathPref = new StoredFlag('tellegen.schematic.path', ['on', 'off'], false);
	/** Where the hovered thing sits: its network, its feeder (−1 for a supply bus, a transformer or a
	 * corridor), and the bus the path to the source starts from. */
	const hoverPlace = $derived.by((): { zone: number; feeder: number; from: number } | null => {
		if (hoveredZone >= 0) return { zone: hoveredZone, feeder: -1, from: -1 };
		if (hoveredBus >= 0)
			return {
				zone: zones.busZone[hoveredBus],
				feeder: feederSet.busFeeder[hoveredBus],
				from: hoveredBus
			};
		for (const k of hoveredEdges) {
			const { from, to } = inputs.names.branches[k];
			if (from < 0 || to < 0) return null;
			const down = tree.depth[from] >= tree.depth[to] ? from : to;
			// A transformer shows the network it feeds; a branch its feeder and network.
			const transformer = graph.edges[k].kind === 'transformer';
			return {
				zone: zones.busZone[down],
				feeder: transformer ? -1 : feederSet.edgeFeeder[k],
				from: down
			};
		}
		return null;
	});
	const activeZone = $derived(hoverPlace?.zone ?? -1);
	/** Where a network is supplied: the source bus of the source network, else its feeding
	 * transformer's symbol (or midway between its buses). */
	function supplyPoint(z: number): [number, number] | null {
		const zone = zones.zones[z];
		const g = geometry.positions;
		if (zone.hasSource) {
			const i = zone.buses.find((b) => graph.buses[b].has_source);
			return i === undefined ? null : [g[i * 3], g[i * 3 + 1]];
		}
		const k = zone.transformers[0];
		if (k === undefined) return null;
		const a = equipment?.anchors;
		if (a && Number.isFinite(a[k * 2])) return [a[k * 2], a[k * 2 + 1]];
		const { from, to } = inputs.names.branches[k];
		if (from < 0 || to < 0) return null;
		return [(g[from * 3] + g[to * 3]) / 2, (g[from * 3 + 1] + g[to * 3 + 1]) / 2];
	}
	const hover = $derived.by((): SchematicScene['hover'] => {
		if (!hoverPlace || hoverPlace.zone < 0) return null;
		const { zone: z, feeder: f, from } = hoverPlace;
		return {
			network: new Set(zones.zones[z].edges),
			feeder: new Set(f >= 0 ? feederSet.feeders[f].edges : []),
			path: new Set(pathPref.value && from >= 0 ? pathToSource(tree, from) : []),
			supply: supplyPoint(z),
			// Neutral, whatever the network's state: the corridors and outlines show the state, the
			// hover shows where.
			tint: HALO
		};
	});
	/** Per-edge and per-bus colours for the active mode; results never recolour inactive branches. */
	const edgeColours = $derived(
		graph.edges.map((e, k): RGBA => {
			if (!live.edges[k]) return DEAD;
			if (!e.closed || colour === 'none' || !snapshot) return LEVEL_COLOURS[edgeClass[k]];
			if (colour === 'loading-heat') {
				const loading = snapshot.branches[k].loading;
				return loading === null ? MISSING_COLOUR : loadingColour(loading);
			}
			const { from, to } = inputs.names.branches[k];
			if (from < 0 || to < 0) return MISSING_COLOUR;
			const t = worstBand(snapshot.buses[tree.depth[from] > tree.depth[to] ? from : to].pairs);
			return t === null ? MISSING_COLOUR : bandColour(t);
		})
	);
	const busColours = $derived(
		graph.buses.map((_, i): RGBA => {
			if (!live.buses[i]) return DEAD;
			if (colour !== 'voltage-band' || !snapshot) return INK;
			const t = worstBand(snapshot.buses[i].pairs);
			return t === null ? MISSING_COLOUR : bandColour(t);
		})
	);

	/** Whether `view` shows at least one bus of `g`. */
	function showsAny(
		g: SchematicGeometry,
		view: { target: [number, number, number]; zoom: number }
	) {
		const scale = 2 ** view.zoom;
		const [hw, hh] = [width / 2 / scale, height / 2 / scale];
		for (let i = 0; i < g.positions.length; i += 3) {
			const dx = g.positions[i] - view.target[0];
			const dy = g.positions[i + 1] - view.target[1];
			if (Math.abs(dx) <= hw && Math.abs(dy) <= hh) return true;
		}
		return false;
	}

	// Record the geographic view on the case, so the map can continue from it when this diagram is
	// closed (TellegenMap applies it). A plain field, written here rather than on teardown: writing
	// app state while a component is torn down left the map's deriveds stale.
	$effect(() => {
		const origin = source?.mercatorOrigin;
		const view = layout === 'source' && origin && shown ? basemapCamera(camera, origin) : null;
		untrack(() => {
			mc.lastGeoView = view ? { center: view.center, zoom: view.zoom, bearing: 0, pitch: 0 } : null;
		});
	});

	/** Screen insets of the area no panel or toolbar covers. */
	function freeArea() {
		return {
			left: panels.compact ? 28 : panels.dockWidth('left') + 44,
			right: panels.compact ? 28 : panels.dockWidth('right') + 44,
			top: panels.top + 64,
			bottom: panels.compact ? app.sheetInset + 24 : 88
		};
	}

	function fitCamera(g: SchematicGeometry) {
		const [x0, y0, x1, y1] = g.bounds;
		const { left, right, top, bottom } = freeArea();
		const w = Math.max(80, width - left - right);
		const h = Math.max(80, height - top - bottom);
		const zoom = Math.log2(Math.min(w / Math.max(1, x1 - x0), h / Math.max(1, y1 - y0)));
		const scale = 2 ** zoom;
		// Centre the drawing in the free area between the docked panels.
		const cx = (x0 + x1) / 2 - (left - right) / 2 / scale;
		const cy = (y0 + y1) / 2 - (top - bottom) / 2 / scale;
		// Schematic units are sized for about 1:1 to 2:1; closer, a small network's symbols balloon.
		const max = g.attachmentGap ? 1 : 4;
		return { target: [cx, cy, 0] as [number, number, number], zoom: Math.min(zoom, max) };
	}

	let morph: {
		from: Float32Array;
		cur: Float32Array;
		s: Float32Array;
		t: Float32Array;
		endpoints: Int32Array;
		start: number | null;
		warm: boolean;
		raf: number;
	} | null = null;

	const text = $derived(
		new SchematicText({
			graph,
			zones,
			zoneStates,
			outlined,
			transformerNames,
			live,
			equipment,
			snapshot,
			powers,
			valuesOn,
			edgeMv,
			zoneMv,
			inferred: layout === 'source' ? (geometry.inferred ?? null) : null,
			feeders: feederSet,
			tree,
			busNames: names,
			labelsOn: namesOn
		})
	);

	/** Layer data kept between frames, so a hover redraws only what it changes. */
	const layerCache = createLayerCache();

	/** Everything a frame draws; the layers and the SVG export both read it. */
	function scene(): SchematicScene {
		return {
			graph,
			layout,
			colour,
			shown: shown ?? EMPTY_GEOMETRY,
			geometry,
			drawn,
			symbols: symbolsOn ? equipment : null,
			ticksOn,
			valuesOn,
			labelsOn: namesOn,
			zoomedOut,
			live,
			edgeColours,
			busColours,
			edgeMv,
			snapshot,
			powers,
			itemText: (d) => text.itemValue(d),
			arrows,
			selectedBus,
			selectedEdges,
			hoveredBus,
			hoveredEdges,
			cursor,
			labels,
			marks,
			markName,
			mvBuses,
			allBuses,
			zones: {
				states: zoneStates,
				shapes: zoneShapes,
				tags: zoneTags,
				tagZoom,
				corridors
			},
			hover,
			stacks,
			spreadStacks,
			textSource: text,
			cache: layerCache,
			fans: fanned(),
			fannedGroups,
			fan
		};
	}

	function layers(): Layer[] {
		if (!modules) return [];
		return morph ? morphLayers(modules, morph, graph.buses.length) : sceneLayers(modules, scene());
	}

	/** The stack a pick belongs to: a stacked bus, a stack's ring, or the symbol of a branch
	 * between two buses of one stack (a pole transformer, drawn on the stack's point). */
	function pickedStack(info: PickingInfo): number | null {
		if (!stacks || info.index < 0 || !info.layer) return null;
		if (info.layer.id === 'stack-rings') return (info.object as { g: number }).g;
		if (info.layer.id.startsWith('buses-')) {
			const g = stacks.stackOf[info.object as number];
			return g >= 0 ? g : null;
		}
		if (info.layer.id === 'equipment-hits') {
			const item = info.object as EquipmentItem;
			if (item.edge < 0) return null;
			const e = graph.edges[item.edge];
			const g = stacks.stackOf[busIndex.get(e.from) ?? -1] ?? -1;
			const h = stacks.stackOf[busIndex.get(e.to) ?? -1] ?? -1;
			return g >= 0 && g === h ? g : null;
		}
		return null;
	}
	/** The route a pick belongs to, if it is shared. */
	function pickedGroup(info: PickingInfo): number | null {
		if (!equipment || info.index < 0 || !info.layer) return null;
		const id = info.layer.id;
		if (id === 'route-badges') return (info.object as { g: number }).g;
		const k =
			id === 'fan-branches'
				? fanEdgeAt(info.index)
				: id.startsWith('branches-')
					? drawn.segmentEdge[info.index]
					: -1;
		const g = k >= 0 ? equipment.routeOf[k] : -1;
		return g >= 0 ? g : null;
	}
	/** Edge of the n-th fanned segment (three per member, in `fanned()` order). */
	function fanEdgeAt(index: number): number {
		const fans = fanned();
		return fans[Math.floor(index / 3)]?.edge ?? -1;
	}

	/** Value labels for the view, placed by priority: selection, then severity, then magnitude. */
	function updateLabels() {
		const g = geometry;
		const scale = 2 ** camera.zoom;
		const x0 = camera.target[0] - width / 2 / scale;
		const y0 = camera.target[1] - height / 2 / scale;
		const screen = (x: number, y: number): [number, number] => [(x - x0) * scale, (y - y0) * scale];
		const inView = (x: number, y: number) => {
			const [sx, sy] = screen(x, y);
			return sx >= 0 && sx <= width && sy >= 0 && sy <= height;
		};
		const candidates: (LabelCandidate & { position: [number, number] })[] = [];
		if (selectedBus >= 0) {
			const [x, y] = [g.positions[selectedBus * 3], g.positions[selectedBus * 3 + 1]];
			candidates.push({
				text: names[selectedBus],
				state: 'normal',
				priority: Infinity,
				selected: true,
				screen: screen(x, y),
				position: [x, y]
			});
		}
		if (snapshot && camera.zoom > -3) {
			if (colour === 'voltage-band') {
				for (let i = 0; i < graph.buses.length && candidates.length < 4000; i++) {
					const [x, y] = [g.positions[i * 3], g.positions[i * 3 + 1]];
					if (i === selectedBus || !inView(x, y)) continue;
					const c = busLabel(snapshot, i);
					if (c) candidates.push({ ...c, screen: screen(x, y), position: [x, y] });
				}
			} else if (colour === 'loading-heat') {
				const seen = new Set<number>();
				for (let s = 0; s < g.segmentEdge.length && candidates.length < 4000; s++) {
					const k = g.segmentEdge[s];
					if (seen.has(k)) continue;
					seen.add(k);
					const [x, y] = [
						(g.sources[s * 3] + g.targets[s * 3]) / 2,
						(g.sources[s * 3 + 1] + g.targets[s * 3 + 1]) / 2
					];
					if (!inView(x, y)) continue;
					const c = branchLabel(snapshot, k, 'loading');
					if (c)
						candidates.push({
							...c,
							selected: selectedEdges.has(k),
							screen: screen(x, y),
							position: [x, y]
						});
				}
			}
		}
		if (cursor >= 0 && cursor !== selectedBus) {
			const [x, y] = [g.positions[cursor * 3], g.positions[cursor * 3 + 1]];
			candidates.push({
				text: names[cursor],
				state: 'normal',
				priority: Infinity,
				selected: true,
				screen: screen(x, y),
				position: [x, y]
			});
		}
		// Close in, with no result values to show, name the buses (short names; hover for the full id).
		if (namesOn && (colour === 'none' || !snapshot))
			for (let i = 0; i < graph.buses.length && candidates.length < 4000; i++) {
				const [x, y] = [g.positions[i * 3], g.positions[i * 3 + 1]];
				if (i === selectedBus || !inView(x, y)) continue;
				candidates.push({
					text: names[i],
					state: 'normal',
					priority: 0,
					screen: screen(x, y),
					position: [x, y]
				});
			}
		labels = placeLabels(candidates, { width, height, budget: 60 });
		if (zoomedOut && landmarksPref.value && landmarkPool.length) {
			const placed = placeLandmarks(landmarkPool, screen, { width, height });
			marks = { placed, strokes: landmarkStrokes(placed, 8 / scale) };
		} else marks = null;
	}

	function render() {
		deck?.setProps({ layers: layers() });
	}

	// Build the renderer once the canvas exists.
	$effect(() => {
		if (!canvas || deck) return;
		const element = canvas;
		let cancelled = false;
		Promise.all([import('@deck.gl/core'), import('@deck.gl/layers')])
			.then(([core, layers]) => {
				if (cancelled) return;
				modules = { core, layers };
				camera = fitCamera(geometry);
				deck = new core.Deck({
					canvas: element,
					width: '100%',
					height: '100%',
					views: new core.OrthographicView({ id: 'schematic', flipY: true }),
					initialViewState: { ...camera, minZoom: -24, maxZoom: 8 },
					controller: { doubleClickZoom: true },
					pickingRadius: 6,
					getCursor: ({ isHovering, isDragging }) =>
						isDragging ? 'grabbing' : isHovering ? 'pointer' : 'grab',
					onViewStateChange: ({ viewState }) => {
						const v = viewState as OrthographicViewState;
						camera = { target: v.target as [number, number, number], zoom: v.zoom as number };
					},
					onHover: (info: PickingInfo) => {
						const id = info.layer?.id ?? '';
						const hz =
							info.index >= 0 && id.startsWith('zones') ? (info.object as { z: number }).z : -1;
						if (hz !== hoveredZone) hoveredZone = hz;
						let next: { kind: 'bus' | 'branch'; id: string } | null = null;
						if (info.index >= 0 && id.startsWith('buses-'))
							next = { kind: 'bus', id: graph.buses[info.object as number].id };
						else if (info.index >= 0 && id.startsWith('branches-'))
							next = { kind: 'branch', id: graph.edges[drawn.segmentEdge[info.index]].id };
						else if (info.index >= 0 && id === 'fan-branches')
							next = { kind: 'branch', id: graph.edges[fanEdgeAt(info.index)].id };
						else if (info.index >= 0 && id === 'equipment-hits') {
							const item = info.object as EquipmentItem;
							next =
								item.edge >= 0
									? { kind: 'branch', id: graph.edges[item.edge].id }
									: { kind: 'bus', id: graph.buses[item.bus].id };
						}
						if (next?.kind !== mc.hovered?.kind || next?.id !== mc.hovered?.id) mc.hovered = next;
						const g = symbolsOn ? pickedGroup(info) : null;
						if (g !== null) routePin.open(g);
						else routePin.leave();
						const sg = pickedStack(info);
						if (sg !== null) stackPin.open(sg);
						else stackPin.leave();
					},
					onClick: (info: PickingInfo) => {
						if (info.index < 0 || !info.layer) {
							// A click on empty canvas closes a pinned shared route or stack.
							routePin.pinned = null;
							stackPin.pinned = null;
							return;
						}
						if (info.layer.id === 'stack-rings') {
							stackPin.pinned = (info.object as { g: number }).g;
							return;
						}
						// A stacked bus is ambiguous while its stack is closed: open it instead of guessing.
						const stack = pickedStack(info);
						if (stack !== null && !spreadStacks.includes(stack)) {
							stackPin.pinned = stack;
							return;
						}
						if (stack !== null) stackPin.pinned = stack;
						if (info.layer.id === 'route-badges') {
							routePin.pinned = (info.object as { g: number }).g;
							return;
						}
						if (info.layer.id === 'fan-branches') {
							pickedHere = true;
							setTimeout(() => (pickedHere = false));
							routePin.pinned = pickedGroup(info);
							onselectedge(graph.edges[fanEdgeAt(info.index)].id);
							return;
						}
						// A shared route is ambiguous when collapsed: open its members instead of guessing.
						const shared = info.layer.id.startsWith('branches-') ? pickedGroup(info) : null;
						if (shared !== null && !separatePref.value) {
							routePin.pinned = shared;
							return;
						}
						// Skip the fly-to for this selection; cleared after effects run, as a click on the
						// already-selected item changes nothing.
						pickedHere = true;
						setTimeout(() => (pickedHere = false));
						if (info.layer.id === 'landmark-halos') {
							const l = info.object as Landmark;
							if (l.edge >= 0) onselectedge(graph.edges[l.edge].id);
							else onselectbus(graph.buses[l.bus].id);
							bringIntoView([l.x, l.y]);
							return;
						}
						if (info.layer.id.startsWith('zones')) {
							fitZone(zones.zones[(info.object as { z: number }).z]);
							return;
						}
						if (info.layer.id.startsWith('buses-'))
							onselectbus(graph.buses[info.object as number].id);
						else if (info.layer.id.startsWith('branches-'))
							onselectedge(graph.edges[drawn.segmentEdge[info.index]].id);
						else if (info.layer.id === 'equipment-hits') {
							const item = info.object as EquipmentItem;
							if (item.edge >= 0) onselectedge(graph.edges[item.edge].id);
							else onselectbus(graph.buses[item.bus].id);
						}
					},
					getTooltip: (info: PickingInfo) =>
						tooltip(info, text, {
							inOpenStack: (pick) => activeStack !== null && pickedStack(pick) === activeStack,
							segmentEdge: (index) => drawn.segmentEdge[index],
							symbolsOn
						})
				});
				// Open where the user just was: the case's own positions (the map they were looking at),
				// when it has them. The schematic is laid out meanwhile, so switching to it is quick.
				if (source) {
					void show('source', strategy);
					void target('schematic', strategy).catch(() => {});
				} else void show('schematic', strategy);
			})
			.catch(
				(e: unknown) =>
					(status = `The schematic could not load: ${e instanceof Error ? e.message : e}`)
			);
		return () => {
			cancelled = true;
		};
	});
	$effect(() => () => {
		if (morph) cancelAnimationFrame(morph.raf);
		stackPin.dispose();
		routePin.dispose();
		deck?.finalize();
	});

	// Redraw when the data, colours or selection change; relabel when the camera moves.
	$effect(() => {
		void [
			geometry,
			drawn,
			edgeColours,
			busColours,
			selectedBus,
			selectedEdges,
			labels,
			cursor,
			valuesOn,
			namesOn,
			arrows,
			hoveredBus,
			hoveredEdges,
			zoomedOut,
			zoneTags,
			tagZoom,
			corridors,
			hover,
			marks,
			fannedGroups,
			fannedEdges,
			spreadStacks
		];
		if (!morph) render();
	});
	let labelFrame = 0;
	$effect(() => {
		void [
			camera,
			landmarksPref.value,
			landmarkPool,
			zoomedOut,
			colour,
			snapshot,
			selectedBus,
			selectedEdges,
			geometry,
			width,
			height,
			namesOn,
			cursor
		];
		if (!deck) return;
		cancelAnimationFrame(labelFrame);
		labelFrame = requestAnimationFrame(updateLabels);
	});

	/** Set while a click in the canvas changes the selection: that selection is already in view. */
	let pickedHere = false;
	const ready = $derived(!!deck && !!shown);
	/** Where the selection is drawn: its bus, or its branch's symbol or conductor marks. */
	function selectionPoint(): [number, number] | null {
		const g = geometry;
		if (selectedBus >= 0) return [g.positions[selectedBus * 3], g.positions[selectedBus * 3 + 1]];
		for (const k of selectedEdges) {
			const a = equipment?.anchors;
			if (a && Number.isFinite(a[k * 2])) return [a[k * 2], a[k * 2 + 1]];
			const { from, to } = inputs.names.branches[k];
			if (from >= 0 && to >= 0)
				return [
					(g.positions[from * 3] + g.positions[to * 3]) / 2,
					(g.positions[from * 3 + 1] + g.positions[to * 3 + 1]) / 2
				];
		}
		return null;
	}

	// Fly to a selection made outside the canvas (issues, search, tables, profile) when it is off
	// screen or drawn too small to read, zooming in until symbols show. Panning afterwards is free.
	// On opening, the view continues from the map: an existing selection is centred if it is off
	// screen, but not zoomed into.
	let opened = false;
	$effect(() => {
		void [selectedBus, selectedEdges, mc.revealSeq];
		if (!ready) return;
		untrack(() => {
			if (pickedHere || morph) {
				pickedHere = false;
				return;
			}
			const p = selectionPoint();
			if (p) bringIntoView(p, opened);
			opened = true;
		});
	});

	/** Fit the view to a zone. */
	function fitZone(zone: GalvanicZone) {
		let [x0, y0, x1, y1] = [Infinity, Infinity, -Infinity, -Infinity];
		for (const i of zone.buses) {
			const [x, y] = [geometry.positions[i * 3], geometry.positions[i * 3 + 1]];
			[x0, y0, x1, y1] = [Math.min(x0, x), Math.min(y0, y), Math.max(x1, x), Math.max(y1, y)];
		}
		const pad = (equipment?.unit ?? 1) * 3;
		setCamera(fitCamera({ ...geometry, bounds: [x0 - pad, y0 - pad, x1 + pad, y1 + pad] }), true);
	}

	/** Centre `p` in the uncovered view when it is outside it or drawn too small to read. */
	function bringIntoView(p: [number, number], zoomIn = true) {
		if (!Number.isFinite(p[0]) || !Number.isFinite(p[1])) return;
		const legible = equipment && zoomIn ? Math.log2(8 / equipment.unit) : camera.zoom;
		const zoom = Math.min(8, Math.max(camera.zoom, legible));
		const { left, right, top, bottom } = freeArea();
		const scale = 2 ** camera.zoom;
		const sx = (p[0] - camera.target[0]) * scale + width / 2;
		const sy = (p[1] - camera.target[1]) * scale + height / 2;
		const visible = sx > left && sx < width - right && sy > top && sy < height - bottom;
		if (visible && zoom === camera.zoom) return;
		// Centre it in the free area, not behind a docked panel.
		const k = 2 ** zoom;
		const target: [number, number, number] = [
			p[0] - (left - right) / 2 / k,
			p[1] - (top - bottom) / 2 / k,
			0
		];
		setCamera({ target, zoom }, true);
	}

	function setCamera(next: { target: [number, number, number]; zoom: number }, fly = false) {
		camera = next;
		const animate = fly && modules && !matchMedia('(prefers-reduced-motion: reduce)').matches;
		deck?.setProps({
			initialViewState: {
				...next,
				minZoom: -24,
				maxZoom: 8,
				...(animate
					? {
							transitionDuration: 450,
							transitionInterpolator: new modules!.core.LinearInterpolator(['target', 'zoom'])
						}
					: {})
			}
		});
	}

	// The schematic is laid out in the engine (Rust); each strategy is computed once per graph.
	const engineLayouts = new Map<SchematicStrategy, Promise<SchematicGeometry>>();
	let showSeq = 0;
	function target(nextLayout: Layout, nextStrategy: SchematicStrategy): Promise<SchematicGeometry> {
		if (nextLayout === 'source' && source) return Promise.resolve(source);
		let pending = engineLayouts.get(nextStrategy);
		if (!pending) {
			pending = mcSchematicLayout(graph, { strategy: nextStrategy }).then(fromEngineLayout);
			engineLayouts.set(nextStrategy, pending);
			pending.catch(() => engineLayouts.delete(nextStrategy));
		}
		return pending;
	}

	/** Show a layout. Buses glide from where they are on screen to the new layout along straight
	 * chords; a warm-up frame uploads the target geometry before the clock starts. */
	async function show(nextLayout: Layout, nextStrategy: SchematicStrategy) {
		if (nextLayout === 'source' && !source) return;
		const seq = ++showSeq;
		const slow = setTimeout(() => {
			if (seq === showSeq) status = 'Laying out the schematic…';
		}, 150);
		let next: SchematicGeometry;
		try {
			next = await target(nextLayout, nextStrategy);
		} catch (e) {
			status = `The schematic could not be laid out: ${e instanceof Error ? e.message : e}`;
			return;
		} finally {
			clearTimeout(slow);
		}
		if (seq !== showSeq) return;
		status = null;
		const previous = shown;
		const from = { ...camera };
		layout = nextLayout;
		strategy = nextStrategy;
		shown = next;
		// Opening on geographic positions, continue from the main map's view when it shows part of
		// this feeder; otherwise fit the drawing.
		const fromMap =
			!previous && nextLayout === 'source' && next.mercatorOrigin && app.camera
				? cameraFromMap(app.camera, next.mercatorOrigin)
				: null;
		const to = fromMap && showsAny(next, fromMap) ? fromMap : fitCamera(next);
		setCamera(to);
		const reduced = matchMedia('(prefers-reduced-motion: reduce)').matches;
		if (!previous || !deck || reduced || graph.buses.length > MORPH_MAX_BUSES) return;
		if (morph) cancelAnimationFrame(morph.raf);
		const before = previous.positions;
		const endpoints = Int32Array.from(
			graph.edges.flatMap((_, k) => {
				const { from: a, to: b } = inputs.names.branches[k];
				return a >= 0 && b >= 0 ? [a, b] : [];
			})
		);
		const n = endpoints.length / 2;
		morph = {
			from: screenAnchored(before, from, to),
			cur: new Float32Array(before.length),
			s: new Float32Array(n * 3),
			t: new Float32Array(n * 3),
			endpoints,
			start: null,
			warm: false,
			raf: 0
		};
		const destination = next.positions;
		const tick = (now: number) => {
			const m = morph;
			if (!m) return;
			if (!m.warm) {
				m.warm = true;
				m.raf = requestAnimationFrame(tick);
				return;
			}
			m.start ??= now;
			const k = ease(Math.min(1, (now - m.start) / MORPH_MS));
			lerpInto(m.cur, m.from, destination, k);
			chordsInto(m.s, m.t, m.endpoints, m.cur);
			if (k < 1) {
				render();
				m.raf = requestAnimationFrame(tick);
			} else {
				morph = null;
				render();
			}
		};
		morph.raf = requestAnimationFrame(tick);
	}

	function zoomBy(delta: number) {
		setCamera({ target: camera.target, zoom: Math.max(-24, Math.min(8, camera.zoom + delta)) });
	}
	const WALK: Record<string, WalkKey> = {
		ArrowLeft: 'parent',
		ArrowRight: 'child',
		ArrowUp: 'previous',
		ArrowDown: 'next',
		Home: 'root'
	};
	function onkeydown(e: KeyboardEvent) {
		const step = 80 / 2 ** camera.zoom;
		const pan: Record<string, [number, number]> = {
			ArrowLeft: [-step, 0],
			ArrowRight: [step, 0],
			ArrowUp: [0, -step],
			ArrowDown: [0, step]
		};
		if (WALK[e.key] && !e.shiftKey && shown && !morph) {
			// Walk the tree; the first key starts at the selection, or at the first source.
			e.preventDefault();
			moveCursor(
				cursor < 0
					? selectedBus >= 0
						? selectedBus
						: walk.step(-1, 'root')
					: walk.step(cursor, WALK[e.key])
			);
		} else if (e.key === 'Tab' && !e.shiftKey && cursor >= 0 && equipment) {
			// Cycle the branches on shared routes at the cursor bus; past the last, Tab moves on.
			const members = graph.edges.flatMap((_, k) => {
				const { from, to } = inputs.names.branches[k];
				return (from === cursor || to === cursor) && equipment!.routeOf[k] >= 0 ? [k] : [];
			});
			const next = members.indexOf(tabEdge) + 1;
			if (next >= members.length) {
				tabEdge = -1;
				routePin.pinned = null;
				return;
			}
			e.preventDefault();
			tabEdge = members[next];
			routePin.pinned = equipment.routeOf[tabEdge];
			mc.hovered = { kind: 'branch', id: graph.edges[tabEdge].id };
			announcement = `${text.branch(tabEdge)}. Enter selects; Tab for the next.`;
		} else if (e.key === 'Enter' && tabEdge >= 0) {
			e.preventDefault();
			onselectedge(graph.edges[tabEdge].id);
		} else if (e.key === 'Enter' && cursor >= 0) {
			e.preventDefault();
			onselectbus(graph.buses[cursor].id);
		} else if (e.key === 'Escape' && stackPin.pinned !== null) {
			stackPin.pinned = null;
		} else if (e.key === 'Escape' && routePin.pinned !== null) {
			routePin.pinned = null;
			tabEdge = -1;
		} else if (e.key === 'Escape' && cursor >= 0) {
			cursor = -1;
			cursorStack = null;
			announcement = '';
		} else if (pan[e.key]) {
			e.preventDefault();
			setCamera({
				target: [camera.target[0] + pan[e.key][0], camera.target[1] + pan[e.key][1], 0],
				zoom: camera.zoom
			});
		} else if (e.key === '+' || e.key === '=') zoomBy(0.5);
		else if (e.key === '-' || e.key === '_') zoomBy(-0.5);
		else if (e.key === 'f' || e.key === 'F') setCamera(fitCamera(geometry));
	}

	const layoutName = $derived(
		layout === 'schematic'
			? `Schematic (${strategy === 'tidy' ? 'tidy' : 'leaf order'} from the sources)`
			: mc.coordsKind === 'geographic'
				? 'Source positions (Web Mercator)'
				: 'Source positions (drawing)'
	);
	function exportTitle() {
		return titleBlock({
			dataset: mc.label,
			layout: layoutName,
			view: COLOURS[colour],
			camera,
			result: snapshot
				? {
						kind: 'solved',
						label: snapshot.label,
						iterations: snapshot.iterations,
						revision: `solve ${mc.result?.solve_count ?? '?'}`
					}
				: null,
			exported: new Date().toISOString()
		});
	}
	async function exportPng() {
		if (!deck || !canvas) return;
		const blob = await canvasPng(deck, canvas, exportTitle());
		if (blob) download(blob, `${fileStem(mc.label)}.png`);
	}
	/** The schematic as a PowerIO diagram layer: load it with the case to reuse the layout. */
	function exportLayer() {
		const layer = diagramLayer(graph, geometry, {
			strategy,
			version: geometry.version ?? 0
		});
		download(new Blob([layer], { type: 'application/geo+json' }), `${fileStem(mc.label)}.geo.json`);
	}
	function exportSvg() {
		try {
			const svg = sceneSvg(scene(), { camera, width, height }, exportTitle());
			download(new Blob([svg], { type: 'image/svg+xml' }), `${fileStem(mc.label)}.svg`);
		} catch (e) {
			status = e instanceof Error ? e.message : String(e);
			setTimeout(() => (status = null), 4000);
		}
	}
</script>

<div class="schematic-stage" bind:clientWidth={width} bind:clientHeight={height}>
	<p id="schematic-keys" class="sr-only">
		Arrow keys walk the network from bus to bus: left to the feeding bus, right onward, up and down
		between neighbours; Home returns to the source; Enter selects; Escape stops. Where lines share a
		route, Tab at a bus steps through them. Shift with the arrow keys pans; plus and minus zoom; F
		fits the drawing.
	</p>
	<!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions (The canvas supports keyboard panning and zooming; selection is also available from the overview.) -->
	<div
		class="canvas"
		role="application"
		aria-label="Schematic diagram"
		aria-describedby="schematic-keys"
		tabindex="0"
		{onkeydown}
	>
		<div class="basemap" bind:this={mapBox} style:opacity={basemapOpacity} aria-hidden="true"></div>
		<canvas bind:this={canvas}></canvas>
		<p class="sr-only" aria-live="polite">{announcement}</p>
	</div>
	<div
		class="toolbar"
		style:top="{panels.top + 8}px"
		style:left="{toolLeft}px"
		role="toolbar"
		aria-label="Schematic"
	>
		<div class="segmented" role="group" aria-label="Layout">
			<button
				aria-pressed={layout === 'source'}
				disabled={!source}
				title={source ? 'Positions from the case data' : 'This case has no source positions'}
				onclick={() => show('source', strategy)}
				>{mc.coordsKind === 'geographic' ? 'Geographic' : 'Source positions'}</button
			>
			<button aria-pressed={layout === 'schematic'} onclick={() => show('schematic', strategy)}
				>Schematic</button
			>
		</div>
		{#if layout === 'source' && mc.coordsKind === 'geographic'}
			<label class="check" title="Map tiles under the geographic positions, as on the main map"
				><input
					type="checkbox"
					checked={basemapPref.value}
					onchange={(e) => basemapPref.set(e.currentTarget.checked)}
				/> Basemap</label
			>
		{/if}
		<label class="check" title="Hovering also traces the drawn path back to the source"
			><input
				type="checkbox"
				checked={pathPref.value}
				onchange={(e) => pathPref.set(e.currentTarget.checked)}
			/> Path to source</label
		>
		{#if layout === 'schematic'}
			<label>
				<span class="sr-only">Schematic arrangement</span>
				<select
					value={strategy}
					title="Tidy packs subtrees closely; leaf order gives every leaf its own row"
					onchange={(e) => show('schematic', e.currentTarget.value as SchematicStrategy)}
				>
					<option value="tidy">Tidy</option>
					<option value="leaf-order">Leaf order</option>
				</select>
			</label>
		{/if}
		<label class="colour">
			<span class="sr-only">Colour by</span>
			<select
				bind:value={colour}
				disabled={!snapshot}
				title={snapshot ? 'Colour by' : 'Run the power flow to colour by results'}
			>
				{#each Object.entries(COLOURS) as [value, label] (value)}<option {value}>{label}</option
					>{/each}
			</select>
		</label>
		<button onclick={() => setCamera(fitCamera(geometry))} title="Fit the drawing (F)">Fit</button>
		{#if live.deadBuses}<span
				class="note"
				role="status"
				title="Buses with no path to a source through closed branches are drawn faint"
				>{live.deadBuses.toLocaleString()} bus{live.deadBuses === 1 ? '' : 'es'} not energised</span
			>{/if}
		{#if unplaced}<span
				class="note"
				role="status"
				title="The case gives these buses no position; each is drawn next to a bus it connects to"
				>{unplaced.toLocaleString()} bus{unplaced === 1 ? '' : 'es'} without a position</span
			>{/if}
		<button onclick={exportPng} title="PNG of this view with a title block">PNG</button>
		<button onclick={exportSvg} title="Vector SVG of the visible region with a title block"
			>SVG</button
		>
		<button onclick={onclose} title="Return to the map">Close</button>
		{#if layout === 'schematic'}<button
				onclick={exportLayer}
				title="Download this schematic as a PowerIO diagram layer (.geo.json) to reuse with the case"
				>Layout</button
			>{/if}
	</div>
	{#if colour !== 'none' && snapshot}
		<div class="legend" style:top="{panels.top + 50}px" style:left="{toolLeft}px">
			{#if colour === 'loading-heat'}
				Loading <span>0 %</span><i style:background={priceGradient}></i><span>≥ 100 %</span>
			{:else}
				Voltage <span>min</span><i style:background={sensGradient}></i><span>max</span> of each declared
				band, worst pair
			{/if}
			· grey: no limit or rating{live.deadBuses ? ' · faint: not energised' : ''}
		</div>
	{/if}
	{#if colour === 'none' && zoomedOut && landmarkPool.length}
		<details
			class="key"
			style:bottom="{panels.compact ? app.sheetInset + 12 : 24}px"
			style:left="{toolLeft}px"
			open={keyPref.value}
			ontoggle={(e) => keyPref.set(e.currentTarget.open)}
		>
			<summary>Key</summary>
			{#if levelKey.length}
				<ul class="symbols" aria-label="Voltage levels">
					{#each levelKey as level (level.cls)}
						<li>
							<svg viewBox="-2 -12 28 24" width="28" height="24" aria-hidden="true"
								><path
									class="level"
									style:stroke="rgb({LEVEL_COLOURS[level.cls].slice(0, 3).join(' ')})"
									d="M-2 0H26"
								/></svg
							>{level.label}
						</li>
					{/each}
				</ul>
			{/if}
			<label class="toggle"
				><input
					type="checkbox"
					checked={landmarksPref.value}
					onchange={(e) => landmarksPref.set(e.currentTarget.checked)}
				/> Landmarks</label
			>
			{#if landmarksPref.value}
				<ul class="symbols" aria-label="Landmarks">
					{#each [['source', 'source, always shown'], ['generator', 'generator'], ['ibr', 'large inverter-based resource'], ['transformer', 'transformer']] as [kind, label] (kind)}
						{#if landmarkPool.some((l) => l.kind === kind)}
							<li>
								<svg viewBox="-14 -12 28 24" width="28" height="24" aria-hidden="true"
									><path d={glyphPath(landmarkGlyph(kind as Landmark['kind']), 7)} /></svg
								>{label}
							</li>
						{/if}
					{/each}
				</ul>
				<p class="hint">More appear as you zoom in.</p>
			{/if}
		</details>
	{/if}
	{#if colour === 'none' && symbolsOn && legendGlyphs.length}
		<details
			class="key"
			style:bottom="{panels.compact ? app.sheetInset + 12 : 24}px"
			style:left="{toolLeft}px"
			open={keyPref.value}
			ontoggle={(e) => keyPref.set(e.currentTarget.open)}
		>
			<summary>Key</summary>
			{#if levelKey.length}
				<ul class="symbols" aria-label="Voltage levels">
					{#each levelKey as level (level.cls)}
						<li>
							<svg viewBox="-2 -12 28 24" width="28" height="24" aria-hidden="true"
								><path
									class="level"
									style:stroke="rgb({LEVEL_COLOURS[level.cls].slice(0, 3).join(' ')})"
									d="M-2 0H26"
								/></svg
							>{level.label}
						</li>
					{/each}
				</ul>
			{/if}
			<ul class="symbols" aria-label="Symbols">
				{#if equipment?.routeGroups.length || stacks?.groups.length}
					<li>
						<label class="toggle"
							><input
								type="checkbox"
								checked={separatePref.value}
								onchange={(e) => separatePref.set(e.currentTarget.checked)}
							/> Separate overlapping buses and lines</label
						>
					</li>
				{/if}
				{#if ticksOn && equipment?.ticks.edge.length}
					<li>
						<svg viewBox="-2 -12 28 24" width="28" height="24" aria-hidden="true"
							><path class="wire" d="M-2 0H26" /><path
								d={glyphPath(conductorTicks(3, true), 8, 12, 0)}
							/></svg
						>phases, ○ neutral
					</li>
				{/if}
				{#if valuesOn && arrows?.edge.length}
					<li>
						<svg viewBox="-2 -12 28 24" width="28" height="24" aria-hidden="true"
							><path class="wire" d="M-2 0H26" /><path
								class="flow"
								d="M9.6 -3.2L14.4 0L9.6 3.2"
							/></svg
						>active power flow
					</li>
				{/if}
				{#each legendGlyphs as [kind, glyph, label], i (i)}
					<li>
						<svg viewBox="-2 -12 28 24" width="28" height="24" aria-hidden="true"
							><path
								d={glyphPath(glyph!, 8, kind === 'transformer' || kind === 'switch' ? 12 : 2, 0)}
							/>{#if kind === 'transformer' || kind === 'switch'}<path
									class="wire"
									d="M-2 0H0.8M23.2 0H26"
								/>{:else}<path class="wire" d="M-2 -10V0H2" />{/if}</svg
						>{label}
					</li>
				{/each}
			</ul>
		</details>
	{/if}
	{#if activeStack !== null && stacks && shown}
		{@const members = stacks.groups[activeStack]}
		{@const k = 2 ** camera.zoom}
		<PickCard
			x={(shown.positions[members[0] * 3] - camera.target[0]) * k + width / 2}
			y={(shown.positions[members[0] * 3 + 1] - camera.target[1]) * k + height / 2}
			stage={{ width, height }}
			gap={stackRadius(members.length) + 10}
			reach={460}
			heading="{members.length} buses at this point"
			label="Buses at this point"
			rows={text.stackRows(members)}
			pin={stackPin}
			selected={(row) => selectedBus === row.i}
			onhover={(row) => (mc.hovered = row ? { kind: 'bus', id: row.id } : null)}
			onpick={(row) => {
				stackPin.pinned = activeStack;
				onselectbus(row.id);
			}}
		>
			{#snippet cells(row)}
				<span class="id" title={row.id}>{row.id}</span>
				<span>{row.level}</span>
				<span>{row.attached || '—'}</span>
				{#if row.voltage}<span>{row.voltage}</span>{/if}
			{/snippet}
		</PickCard>
	{/if}
	{#if activeGroup !== null && equipment}
		{@const members = equipment.routeGroups[activeGroup]}
		{@const middle = routeMiddle(members, equipment.host, geometry)}
		{#if middle}
			{@const k = 2 ** camera.zoom}
			<PickCard
				x={(middle[0] - camera.target[0]) * k + width / 2}
				y={(middle[1] - camera.target[1]) * k + height / 2}
				stage={{ width, height }}
				gap={16}
				reach={440}
				heading="{members.length} branches on this route"
				label="Branches on this route"
				rows={text.routeRows(members)}
				pin={routePin}
				selected={(row) => selectedEdges.has(row.k)}
				onhover={(row) => (mc.hovered = row ? { kind: 'branch', id: row.id } : null)}
				onpick={(row) => {
					routePin.pinned = activeGroup;
					onselectedge(row.id);
				}}
			>
				{#snippet cells(row)}
					<span class="id" title={row.id}>{row.id}</span>
					<span>{row.kind}</span>
					<span>{row.level}</span>
					<span>{row.wires}</span>
					{#if row.loading}<span class={row.state}>{row.loading}</span>{/if}
				{/snippet}
			</PickCard>
		{/if}
	{/if}
	{#if basemapOpacity > 0}
		<p
			class="attribution"
			style:opacity={basemapOpacity}
			style:right="{panels.compact ? 12 : panels.dockWidth('right') + 24}px"
			style:bottom="{panels.compact ? app.sheetInset + 8 : 8}px"
		>
			{basemapCredit(basemapProvider)}
		</p>
	{/if}
	{#if status}<p class="status" role="status">{status}</p>{/if}
</div>

<style>
	.schematic-stage {
		position: absolute;
		inset: 0;
		background: var(--bg);
		color: var(--ink);
	}
	.canvas,
	canvas {
		position: absolute;
		inset: 0;
		width: 100%;
		height: 100%;
	}
	.canvas:focus-visible {
		outline: 2px solid var(--accent);
		outline-offset: -3px;
	}
	.toolbar {
		position: absolute;
		flex-wrap: wrap;
		max-width: calc(100% - 24px);
		display: flex;
		gap: 6px;
		align-items: center;
		padding: 5px;
		font-size: 11px;
		background: var(--surface-panel, var(--paper));
		border: 1px solid var(--line);
		border-radius: 4px;
		box-shadow: var(--elev-1);
	}
	.toolbar button,
	.toolbar select {
		font: inherit;
		color: var(--ink);
		background: var(--paper);
		border: 1px solid var(--line);
		border-radius: 3px;
		padding: 4px 8px;
		cursor: pointer;
	}
	.toolbar button:disabled,
	.toolbar select:disabled {
		opacity: 0.45;
		cursor: default;
	}
	.note {
		color: var(--text-secondary);
		padding: 0 4px;
	}
	.segmented {
		display: flex;
	}
	.segmented button {
		border-radius: 0;
	}
	.segmented button:first-child {
		border-radius: 3px 0 0 3px;
	}
	.segmented button:last-child {
		border-radius: 0 3px 3px 0;
		border-left: 0;
	}
	.segmented button[aria-pressed='true'] {
		background: var(--accent-soft);
		color: var(--accent);
	}
	.legend {
		position: absolute;
		display: flex;
		gap: 6px;
		align-items: center;
		padding: 4px 8px;
		font: 10px var(--font-mono);
		color: var(--text-secondary);
		background: var(--paper);
		border: 1px solid var(--line);
		border-radius: 3px;
	}
	.key {
		position: absolute;
		padding: 4px 8px;
		font: 10px var(--font-mono);
		color: var(--text-secondary);
		background: var(--paper);
		border: 1px solid var(--line);
		border-radius: 3px;
		box-shadow: var(--elev-1);
	}
	.basemap {
		position: absolute;
		inset: 0;
		pointer-events: none;
	}
	.attribution {
		position: absolute;
		margin: 0;
		padding: 1px 6px;
		font: 10px var(--font-mono);
		color: var(--text-secondary);
		background: color-mix(in srgb, var(--paper) 80%, transparent);
		border-radius: 2px;
		pointer-events: none;
	}
	.toolbar .check {
		display: flex;
		gap: 4px;
		align-items: center;
	}
	.key .toggle {
		display: flex;
		gap: 4px;
		align-items: center;
		margin-top: 4px;
		color: var(--ink);
	}
	.key .hint {
		margin: 2px 0 0;
	}
	.key summary {
		cursor: pointer;
		color: var(--ink);
	}
	.symbols {
		margin: 4px 0 0;
		padding: 0;
		list-style: none;
	}
	.symbols li {
		display: flex;
		gap: 4px;
		align-items: center;
	}
	.symbols path {
		fill: none;
		stroke: var(--ink);
		stroke-width: 1.1;
	}
	.symbols path.flow {
		stroke: var(--accent);
		stroke-width: 1.5;
	}
	.symbols path.level {
		stroke-width: 3;
	}
	.symbols path.wire {
		stroke: var(--text-secondary);
	}
	.legend i {
		display: inline-block;
		width: 90px;
		height: 7px;
		border-radius: 2px;
	}
	.status {
		position: absolute;
		inset: 0;
		display: grid;
		place-items: center;
		margin: 0;
		color: var(--text-secondary);
		pointer-events: none;
	}
	.sr-only {
		position: absolute;
		width: 1px;
		height: 1px;
		overflow: hidden;
		clip-path: inset(50%);
		white-space: nowrap;
	}
</style>
