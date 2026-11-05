<script lang="ts">
	import type { Deck, OrthographicView, OrthographicViewState, PickingInfo } from '@deck.gl/core';
	import { untrack } from 'svelte';
	import { txSchematicLayout } from '@tellegen/engine';
	import type { Network, Solution } from '../api.js';
	import {
		branchColor,
		demandColor,
		priceColor,
		priceGradient,
		scalarDomain,
		type RGBA
	} from '../colors.js';
	import { getAppState, getController } from '../context.svelte.js';
	import type { CameraSnapshot } from '../state.svelte.js';
	import { getPanelLayout } from '../panels.svelte.js';
	import { basemapCredit, type BasemapSpec } from '../basemap.js';
	import {
		BASEMAP_STYLE,
		basemapCamera,
		busBounds,
		cameraFromMap,
		fitBounds,
		freeInsets,
		glyphPath,
		landmarkGlyph,
		landmarkStrokes,
		levelLabel,
		MAX_ZOOM,
		MIN_ZOOM,
		placeLabels,
		placeLandmarks,
		revealPoint,
		showsAnyPosition,
		titleBlock,
		TX_KV_CLASSES,
		txBranchText,
		txBusText,
		txGraph,
		txKvClass,
		txLandmarks,
		txSourceLayout,
		txLayoutInput,
		fromTxLayout,
		edgeAnchors,
		screenAnchored,
		lerpInto,
		chordsInto,
		ease,
		MORPH_MAX_BUSES,
		MORPH_MS,
		type SchematicGeometry,
		type TxSchematicParts,
		type DiagramCamera,
		type LabelCandidate,
		type Landmark
	} from '../diagram/index.js';
	import { BasemapUnderlay } from './schematic/basemap-underlay.svelte.js';
	import { canvasPng, download, fileStem } from './schematic/export.js';
	import type { DeckKit } from './schematic/layers.js';
	import { StoredFlag } from './schematic/view-prefs.svelte.js';
	import {
		levelPaint,
		OUT_OF_SERVICE,
		TX_LEVEL_COLOURS,
		TX_LEVEL_WIDTHS,
		txLayers,
		type TxScene
	} from './schematic/tx-layers.js';

	const DEFAULT_BASEMAP: BasemapSpec = {
		primary: BASEMAP_STYLE,
		fallback: null,
		hideLabels: false,
		primaryProvider: 'carto',
		fallbackProvider: null
	};

	let {
		network,
		solution = null,
		caseId,
		label,
		onselectbus,
		onselectbranch,
		onclear,
		onclose,
		ongeoview,
		basemap: basemapSpec = DEFAULT_BASEMAP
	}: {
		network: Network;
		solution?: Solution | null;
		caseId: string;
		label: string;
		onselectbus: (id: number) => void;
		onselectbranch: (id: number) => void;
		/** A click on empty canvas. */
		onclear: () => void;
		onclose: () => void;
		/** The geographic view on screen, as a map camera, so the map can continue from it when the
		 * diagram closes. */
		ongeoview?: (view: CameraSnapshot) => void;
		/** The background map under geographic positions: the map's own basemap, light-themed. */
		basemap?: BasemapSpec;
	} = $props();

	type Colour = 'level' | 'results';
	const app = getAppState();
	const ctrl = getController();
	const panels = getPanelLayout();

	let width = $state(0);
	let height = $state(0);
	let canvas = $state<HTMLCanvasElement>();
	let mapBox = $state<HTMLDivElement>();
	let deck = $state.raw<Deck<OrthographicView> | null>(null);
	let kit: DeckKit | null = null;
	let camera = $state.raw<DiagramCamera>({ target: [0, 0, 0], zoom: 0 });
	let colour = $state<Colour>('level');
	let status = $state<string | null>('Loading the diagram renderer…');
	let hoveredBus = $state(-1);
	let hoveredEdge = $state(-1);
	let labels = $state.raw<(LabelCandidate & { position: [number, number] })[]>([]);
	let marks = $state.raw<TxScene['marks']>(null);

	type Layout = 'source' | 'schematic';
	const graph = $derived(txGraph(network));
	const source = $derived(txSourceLayout(graph, network));
	const geographic = $derived(network.coordinate_space !== 'diagram');
	let layout = $state<Layout>('source');
	/** The drawing on screen; replaced by `show`, which animates between layouts. */
	let shown = $state.raw<{ geometry: SchematicGeometry; parts: TxSchematicParts | null } | null>(
		null
	);
	const geometry = $derived(shown?.geometry ?? source);
	const parts = $derived(shown?.parts ?? null);
	const anchored = $derived(edgeAnchors(geometry, network.branches.length));
	/** Per branch: `x, y, cos, sin` halfway along its drawn path. */
	const anchors = $derived(anchored.anchors);
	/** A typical drawn branch, in drawing units: what is legible at a zoom depends on it. */
	const typical = $derived.by(() => {
		const lengths = Array.from(anchored.lengths).filter(
			(l, k) => l > 0 && graph.inService[k] && geometry.role[k] !== 1
		);
		if (!lengths.length) return 1;
		lengths.sort((a, b) => a - b);
		return lengths[lengths.length >> 1];
	});
	const span = $derived(typical * 2 ** camera.zoom);
	/** Zoom in quarter steps, so symbol sizes and landmarks change only when this does. */
	const stepZoom = $derived(Math.floor(camera.zoom * 4) / 4);
	/** In the overview, symbols also wait until substation interiors are legible. */
	const symbolsOn = $derived(span >= 60 && (!parts || parts.unit * 2 ** camera.zoom >= 4));
	const namesOn = $derived(span >= 140 && (!parts || parts.unit * 2 ** camera.zoom >= 6));

	const basemapPref = new StoredFlag('tellegen.schematic.basemap', ['on', 'off'], true);
	const landmarksPref = new StoredFlag('tellegen.schematic.landmarks', ['on', 'off'], true);
	const keyPref = new StoredFlag('tellegen.schematic.key', ['open', 'closed'], true);
	const underlay = new BasemapUnderlay(() => ({
		wanted: basemapPref.value && geographic && !!geometry.mercatorOrigin,
		container: mapBox,
		spec: basemapSpec,
		origin: geometry.mercatorOrigin ?? null,
		camera,
		size: [width, height]
	}));

	// Results: the map's own display series for buses and the solved loading for branches.
	const display = $derived(ctrl.activeDisplay);
	const flows = $derived(
		new Map((solution?.flows ?? []).map((f) => [f.branch, { mw: f.mw, loading: f.loading }]))
	);
	const resultsOn = $derived(colour === 'results' && !!solution);
	$effect(() => {
		if (!solution && colour === 'results') colour = 'level';
	});
	const busValues = $derived(new Map((display?.values ?? []).map((v) => [v.bus, v.value])));
	const domain = $derived(
		display
			? scalarDomain(display.mode, display.values.map((v) => v.value).filter(Number.isFinite))
			: null
	);
	const level = $derived(levelPaint(graph));
	const edgePaint = $derived.by(() => {
		if (!resultsOn) return level;
		const colours = new Uint8Array(level.colours.length);
		const widths = new Float32Array(level.widths);
		network.branches.forEach((br, k) => {
			const f = flows.get(br.id);
			colours.set(
				!graph.inService[k]
					? OUT_OF_SERVICE
					: f
						? branchColor(f.loading, true)
						: [138, 131, 117, 160],
				k * 4
			);
			widths[k] += 0.6;
		});
		return { colours, widths };
	});
	const busColours = $derived.by(() => {
		const n = graph.busId.length;
		const out = new Uint8Array(n * 4);
		for (let i = 0; i < n; i++) {
			let c: RGBA = TX_LEVEL_COLOURS[txKvClass(graph.kv[i])];
			if (resultsOn && display && domain) {
				const v = busValues.get(graph.busId[i]);
				c =
					v === undefined
						? [180, 175, 165, 200]
						: (display.mode === 'demand' ? demandColor : priceColor)(
								(v - domain.lo) / Math.max(domain.hi - domain.lo, 1e-12)
							);
			}
			out.set(c, i * 4);
		}
		return out;
	});
	const busRadius = $derived(Math.max(1.5, Math.min(5, span / 14)));

	const selectedBus = $derived(
		app.selectedBus === null ? -1 : (graph.index.get(app.selectedBus) ?? -1)
	);
	const selectedEdge = $derived(
		app.selectedBranch === null
			? -1
			: network.branches.findIndex((br) => br.id === app.selectedBranch)
	);

	const transformerEdges = $derived(
		network.branches.flatMap((_, k) =>
			graph.transformer[k] && graph.from[k] >= 0 && graph.to[k] >= 0 ? [k] : []
		)
	);
	const transformers = $derived.by(() => {
		if (!symbolsOn) return null;
		const edges = transformerEdges.filter((k) => Number.isFinite(anchors[k * 4]));
		return edges.length ? { edges, unit: 6 / 2 ** stepZoom } : null;
	});
	const branchState = (k: number): Landmark['state'] => {
		const f = flows.get(network.branches[k].id);
		if (!f) return null;
		return f.loading >= 0.999 ? 'violation' : f.loading >= 0.9 ? 'warning' : 'normal';
	};
	const landmarkPool = $derived(
		txLandmarks(graph, geometry, solution ? branchState : null, anchors)
	);
	const levelsPresent = $derived(
		TX_KV_CLASSES.filter(({ cls }) => graph.branchKv.some((kv) => txKvClass(kv) === cls))
	);

	/** Screen insets no panel or toolbar covers. */
	const freeArea = () =>
		freeInsets({
			compact: panels.compact,
			top: panels.top,
			dockWidth: (side) => panels.dockWidth(side),
			sheetInset: app.sheetInset
		});
	const fitAll = () => fitBounds(geometry.bounds, { width, height }, freeArea());

	function setCamera(next: DiagramCamera, fly = false) {
		camera = next;
		const animate = fly && kit && !matchMedia('(prefers-reduced-motion: reduce)').matches;
		deck?.setProps({
			initialViewState: {
				...next,
				minZoom: MIN_ZOOM,
				maxZoom: MAX_ZOOM,
				...(animate
					? {
							transitionDuration: 450,
							transitionInterpolator: new kit!.core.LinearInterpolator(['target', 'zoom'])
						}
					: {})
			}
		});
	}

	/** Where a selection is drawn: its bus, or the middle of its branch. */
	function selectionPoint(): [number, number] | null {
		if (selectedBus >= 0)
			return [geometry.positions[selectedBus * 3], geometry.positions[selectedBus * 3 + 1]];
		if (selectedEdge >= 0 && Number.isFinite(anchors[selectedEdge * 4]))
			return [anchors[selectedEdge * 4], anchors[selectedEdge * 4 + 1]];
		return null;
	}
	/** Centre `p` when it is off screen, zooming in until symbols show when asked. */
	function bringIntoView(p: [number, number], zoomIn: boolean) {
		const legible = zoomIn ? Math.log2(80 / Math.max(typical, 1e-9)) : camera.zoom;
		const next = revealPoint(p, camera, { width, height }, freeArea(), legible);
		if (next) setCamera(next, true);
	}

	function scene(): TxScene {
		return {
			graph,
			geometry,
			edgeColours: edgePaint.colours,
			edgeWidths: edgePaint.widths,
			busColours,
			busRadius,
			selectedBus,
			selectedEdge,
			hoveredBus,
			hoveredEdge,
			anchors,
			transformers,
			parts,
			morph: morph ? { s: morph.s, t: morph.t, cur: morph.cur, endpoints: morph.endpoints } : null,
			marks,
			labels
		};
	}
	function render() {
		if (kit) deck?.setProps({ layers: txLayers(kit, scene()) });
	}

	function valueText(i: number): { label: string; text: string } | null {
		if (!display) return null;
		const v = busValues.get(graph.busId[i]);
		if (v === undefined) return null;
		const digits = display.mode === 'voltage' ? 4 : 2;
		return { label: display.label, text: `${v.toFixed(digits)} ${display.unit}` };
	}

	function updateLabels() {
		const scale = 2 ** camera.zoom;
		const x0 = camera.target[0] - width / 2 / scale;
		const y0 = camera.target[1] - height / 2 / scale;
		const screen = (x: number, y: number): [number, number] => [(x - x0) * scale, (y - y0) * scale];
		const inView = ([sx, sy]: [number, number]) =>
			sx >= 0 && sx <= width && sy >= 0 && sy <= height;
		const name = (i: number) => network.buses[i].name ?? `bus ${network.buses[i].id}`;
		const candidates: (LabelCandidate & { position: [number, number] })[] = [];
		const at = (i: number): [number, number] => [
			geometry.positions[i * 3],
			geometry.positions[i * 3 + 1]
		];
		if (selectedBus >= 0) {
			const p = at(selectedBus);
			const v = resultsOn ? valueText(selectedBus) : null;
			candidates.push({
				text: v ? `${name(selectedBus)} · ${v.text}` : name(selectedBus),
				state: 'normal',
				priority: 1e9,
				screen: screen(...p),
				position: p,
				selected: true
			});
		}
		if (namesOn)
			for (let i = 0; i < graph.busId.length; i++) {
				if (i === selectedBus) continue;
				const p = at(i);
				const sp = screen(...p);
				if (!inView(sp)) continue;
				candidates.push({
					text: name(i),
					state: 'normal',
					priority: graph.kv[i],
					screen: sp,
					position: p
				});
			}
		if (resultsOn && symbolsOn)
			network.branches.forEach((br, k) => {
				const f = flows.get(br.id);
				if (!f || f.loading < 0.6 || !Number.isFinite(anchors[k * 4])) return;
				const a = { x: anchors[k * 4], y: anchors[k * 4 + 1] };
				const sp = screen(a.x, a.y);
				if (!inView(sp)) return;
				candidates.push({
					text: `${(f.loading * 100).toFixed(0)} %`,
					state: f.loading >= 0.999 ? 'violation' : f.loading >= 0.9 ? 'warning' : 'normal',
					priority: 1000 + f.loading,
					screen: sp,
					position: [a.x, a.y]
				});
			});
		labels = placeLabels(candidates, { width, height, budget: 60 });
		if (!symbolsOn && landmarksPref.value && landmarkPool.length) {
			const placed = placeLandmarks(landmarkPool, screen, { width, height });
			marks = { placed, strokes: landmarkStrokes(placed, 8 / scale) };
		} else marks = null;
	}

	// Build the renderer once the canvas exists.
	$effect(() => {
		if (!canvas || deck) return;
		const element = canvas;
		let cancelled = false;
		Promise.all([import('@deck.gl/core'), import('@deck.gl/layers')])
			.then(([core, layers]) => {
				if (cancelled) return;
				kit = { core, layers };
				// Open where the map was when it shows part of this case; otherwise fit the drawing.
				const fromMap =
					geometry.mercatorOrigin && app.camera
						? cameraFromMap(app.camera, geometry.mercatorOrigin)
						: null;
				camera =
					fromMap && showsAnyPosition(geometry.positions, fromMap, { width, height })
						? fromMap
						: fitAll();
				deck = new core.Deck({
					canvas: element,
					width: '100%',
					height: '100%',
					views: new core.OrthographicView({ id: 'transmission', flipY: true }),
					initialViewState: { ...camera, minZoom: MIN_ZOOM, maxZoom: MAX_ZOOM },
					controller: { doubleClickZoom: true },
					pickingRadius: 6,
					getCursor: ({ isHovering, isDragging }) =>
						isDragging ? 'grabbing' : isHovering ? 'pointer' : 'grab',
					onViewStateChange: ({ viewState }) => {
						const v = viewState as OrthographicViewState;
						camera = { target: v.target as [number, number, number], zoom: v.zoom as number };
					},
					onHover: (info: PickingInfo) => {
						const id = info.layer?.id;
						const hit = info.index >= 0;
						hoveredBus = hit && (id === 'buses' || id === 'busbars') ? info.index : -1;
						hoveredEdge =
							hit && id === 'branches'
								? geometry.segmentEdge[info.index]
								: hit && id === 'transformers'
									? (transformers?.edges[info.index] ?? -1)
									: hit && id === 'landmark-halos'
										? (info.object as Landmark).edge
										: -1;
					},
					onClick: (info: PickingInfo) => {
						const id = info.layer?.id;
						if (info.index < 0 || !id) {
							onclear();
							return;
						}
						pickedHere = true;
						setTimeout(() => (pickedHere = false));
						if (id === 'buses' || id === 'busbars') onselectbus(graph.busId[info.index]);
						else if (id === 'branches')
							onselectbranch(network.branches[geometry.segmentEdge[info.index]].id);
						else if (id === 'transformers' && transformers)
							onselectbranch(network.branches[transformers.edges[info.index]].id);
						else if (id === 'landmark-halos') {
							const l = info.object as Landmark;
							if (l.edge >= 0) onselectbranch(network.branches[l.edge].id);
							else onselectbus(graph.busId[l.bus]);
							pickedHere = false;
							bringIntoView([l.x, l.y], true);
						}
					},
					getTooltip: (info: PickingInfo) => {
						const id = info.layer?.id;
						if (info.index < 0) return null;
						if (id === 'buses' || id === 'busbars')
							return { text: txBusText(graph, network, info.index, valueText(info.index)) };
						const k =
							id === 'branches'
								? geometry.segmentEdge[info.index]
								: id === 'transformers'
									? (transformers?.edges[info.index] ?? -1)
									: id === 'landmark-halos'
										? (info.object as Landmark).edge
										: -1;
						if (k >= 0)
							return {
								text: txBranchText(graph, network, k, flows.get(network.branches[k].id) ?? null)
							};
						if (id === 'landmark-halos') {
							const l = info.object as Landmark;
							return { text: txBusText(graph, network, l.bus, valueText(l.bus)) };
						}
						return null;
					}
				});
				status = null;
			})
			.catch(
				(e: unknown) =>
					(status = `The diagram could not load: ${e instanceof Error ? e.message : e}`)
			);
		return () => {
			cancelled = true;
		};
	});
	$effect(() => () => {
		if (morph) cancelAnimationFrame(morph.raf);
		deck?.finalize();
	});

	// The substation overview is laid out in the engine (Rust), once per network.
	let schematic: Promise<{ geometry: SchematicGeometry; parts: TxSchematicParts }> | null = null;
	function target(next: Layout) {
		if (next === 'source') return Promise.resolve({ geometry: source, parts: null });
		// Synthetic positions say nothing about the grid: lay its substations out by their
		// connections instead (sparse stress), in the same frame.
		schematic ??= txSchematicLayout(txLayoutInput(graph, source), {
			seeding: network.synthetic_coords ? 'stress' : 'given'
		}).then((l) => fromTxLayout(l, source.mercatorOrigin));
		schematic.catch(() => (schematic = null));
		return schematic;
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
	let showSeq = 0;
	/** Show a layout. Buses glide from where they are on screen to the new layout along straight
	 * chords, as in the multiconductor schematic; a warm-up frame uploads the target first. */
	async function show(next: Layout) {
		const seq = ++showSeq;
		const slow = setTimeout(() => {
			if (seq === showSeq) status = 'Laying out the substations…';
		}, 150);
		let drawing: { geometry: SchematicGeometry; parts: TxSchematicParts | null };
		try {
			drawing = await target(next);
		} catch (e) {
			status = `The substations could not be laid out: ${e instanceof Error ? e.message : e}`;
			return;
		} finally {
			clearTimeout(slow);
		}
		if (seq !== showSeq) return;
		status = null;
		const before = geometry.positions;
		const from = { ...camera };
		layout = next;
		shown = drawing;
		// Both layouts share one frame, so the camera stays where it is unless nothing would show.
		const to = showsAnyPosition(drawing.geometry.positions, camera, { width, height })
			? camera
			: fitBounds(drawing.geometry.bounds, { width, height }, freeArea());
		setCamera(to);
		const reduced = matchMedia('(prefers-reduced-motion: reduce)').matches;
		if (!deck || reduced || graph.busId.length > MORPH_MAX_BUSES) return;
		if (morph) cancelAnimationFrame(morph.raf);
		const endpoints = Int32Array.from(
			network.branches.flatMap((_, k) =>
				graph.from[k] >= 0 && graph.to[k] >= 0 ? [graph.from[k], graph.to[k]] : []
			)
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
		const destination = drawing.geometry.positions;
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

	// Redraw when data, colours or selection change.
	$effect(() => {
		void [
			geometry,
			parts,
			edgePaint,
			busColours,
			busRadius,
			selectedBus,
			selectedEdge,
			hoveredBus,
			hoveredEdge,
			transformers,
			marks,
			labels
		];
		if (!morph) render();
	});
	// Relabel and re-place landmarks when the camera or what is shown changes.
	let labelFrame = 0;
	$effect(() => {
		void [
			camera,
			width,
			height,
			landmarksPref.value,
			landmarkPool,
			symbolsOn,
			namesOn,
			resultsOn,
			selectedBus,
			busValues
		];
		if (!deck) return;
		cancelAnimationFrame(labelFrame);
		labelFrame = requestAnimationFrame(updateLabels);
	});

	// Record the geographic view on the app, so the map continues from it when this closes.
	$effect(() => {
		const origin = geometry.mercatorOrigin;
		if (!deck || !origin) return;
		const view = basemapCamera(camera, origin);
		untrack(() => ongeoview?.({ center: view.center, zoom: view.zoom, bearing: 0, pitch: 0 }));
	});

	/** Set while a click in the canvas changes the selection: that selection is already in view. */
	let pickedHere = false;
	// Fly to a selection made outside the canvas (search, tables, panels) when it is off screen.
	$effect(() => {
		void [selectedBus, selectedEdge];
		if (!deck) return;
		untrack(() => {
			if (pickedHere) {
				pickedHere = false;
				return;
			}
			const p = selectionPoint();
			if (p) bringIntoView(p, true);
		});
	});

	// Frame requests (Fit case, search results) aimed at this case.
	let handledFrame = -1;
	$effect(() => {
		const seq = app.frameSeq;
		if (!deck || seq === handledFrame) return;
		handledFrame = seq;
		untrack(() => {
			const target = app.frameTarget;
			if (typeof target === 'string') {
				if (target === 'all' || target === caseId) setCamera(fitAll(), true);
			} else if (target.caseId === caseId) {
				const rows =
					'busId' in target
						? [graph.index.get(target.busId) ?? -1]
						: (() => {
								const k = network.branches.findIndex((br) => br.id === target.branchId);
								return k >= 0 ? [graph.from[k], graph.to[k]] : [];
							})();
				const b = busBounds(
					geometry.positions,
					rows.filter((r) => r >= 0),
					typical
				);
				if (b) setCamera(fitBounds(b, { width, height }, freeArea(), camera.zoom + 2), true);
			}
			app.settleFrame();
		});
	});

	function zoomBy(delta: number) {
		setCamera({
			target: camera.target,
			zoom: Math.max(MIN_ZOOM, Math.min(MAX_ZOOM, camera.zoom + delta))
		});
	}
	function onkeydown(e: KeyboardEvent) {
		const step = 80 / 2 ** camera.zoom;
		const pan: Record<string, [number, number]> = {
			ArrowLeft: [-step, 0],
			ArrowRight: [step, 0],
			ArrowUp: [0, -step],
			ArrowDown: [0, step]
		};
		if (pan[e.key]) {
			e.preventDefault();
			setCamera({
				target: [camera.target[0] + pan[e.key][0], camera.target[1] + pan[e.key][1], 0],
				zoom: camera.zoom
			});
		} else if (e.key === '+' || e.key === '=') zoomBy(0.5);
		else if (e.key === '-' || e.key === '_') zoomBy(-0.5);
		else if (e.key === 'f' || e.key === 'F') setCamera(fitAll());
		else if (e.key === 'Escape') onclear();
	}

	const toolLeft = $derived(panels.compact ? 12 : panels.dockWidth('left') + 24);
	const layoutName = $derived(
		layout === 'schematic'
			? `Substation schematic (${network.synthetic_coords ? 'laid out by connection' : `seeded by ${geographic ? 'geography' : 'the drawing'}`})`
			: geographic
				? 'Geographic positions (Web Mercator)'
				: 'Drawing positions'
	);
	async function exportPng() {
		if (!deck || !canvas) return;
		const title = titleBlock({
			dataset: label,
			layout: layoutName,
			view: resultsOn
				? `Buses: ${display?.copy ?? 'results'} · branches: loading`
				: 'Voltage levels',
			camera,
			result: solution
				? { kind: 'solved', label: display?.copy ?? 'Solved', iterations: null, revision: caseId }
				: null,
			exported: new Date().toISOString()
		});
		const blob = await canvasPng(deck, canvas, title);
		if (blob) download(blob, `${fileStem(label)}.png`);
	}
	const rgb = (c: RGBA) => `rgb(${c.slice(0, 3).join(' ')})`;
</script>

<div class="tx-stage" bind:clientWidth={width} bind:clientHeight={height}>
	<p id="tx-keys" class="sr-only">
		Arrow keys pan; plus and minus zoom; F fits the drawing; Escape clears the selection.
	</p>
	<!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions (The canvas supports keyboard panning and zooming; selection is also available from the panels and search.) -->
	<div
		class="canvas"
		role="application"
		aria-label="Single-line diagram"
		aria-describedby="tx-keys"
		tabindex="0"
		{onkeydown}
	>
		<div
			class="basemap"
			bind:this={mapBox}
			style:opacity={underlay.opacity}
			aria-hidden="true"
		></div>
		<canvas bind:this={canvas}></canvas>
	</div>
	<div
		class="toolbar"
		style:top="{panels.top + 8}px"
		style:left="{toolLeft}px"
		role="toolbar"
		aria-label="Single-line diagram"
	>
		<div class="segmented" role="group" aria-label="Layout">
			<button
				aria-pressed={layout === 'source'}
				title="Positions from the case data"
				onclick={() => show('source')}>{geographic ? 'Geographic' : 'Drawing'}</button
			>
			<button
				aria-pressed={layout === 'schematic'}
				title="Substations near their positions, one busbar per bus, lines between them"
				onclick={() => show('schematic')}>Schematic</button
			>
		</div>
		{#if geographic}
			<label class="check" title="Map tiles under the geographic positions, as on the main map"
				><input
					type="checkbox"
					checked={basemapPref.value}
					onchange={(e) => basemapPref.set(e.currentTarget.checked)}
				/> Basemap</label
			>
		{/if}
		<label>
			<span class="sr-only">Colour by</span>
			<select
				bind:value={colour}
				disabled={!solution}
				title={solution ? 'Colour by' : 'Solve the case to colour by results'}
			>
				<option value="level">Voltage levels</option>
				<option value="results">Results</option>
			</select>
		</label>
		<button onclick={() => setCamera(fitAll())} title="Fit the drawing (F)">Fit</button>
		<button onclick={exportPng} title="PNG of this view with a title block">PNG</button>
		<button onclick={onclose} title="Return to the map">Close</button>
	</div>
	{#if resultsOn}
		<div class="legend" style:top="{panels.top + 50}px" style:left="{toolLeft}px">
			{#if display}
				{display.label} <i style:background={display.gradient}></i> ·
			{/if}
			Loading <span>0 %</span><i style:background={priceGradient}></i><span>100 %</span>
		</div>
	{/if}
	{#if !resultsOn && levelsPresent.length}
		<details
			class="key"
			style:bottom="{panels.compact ? app.sheetInset + 12 : 24}px"
			style:left="{toolLeft}px"
			open={keyPref.value}
			ontoggle={(e) => keyPref.set(e.currentTarget.open)}
		>
			<summary>Key</summary>
			<ul class="symbols" aria-label="Voltage levels">
				{#each levelsPresent as l (l.cls)}
					<li>
						<svg viewBox="-2 -12 28 24" width="28" height="24" aria-hidden="true"
							><path
								style:stroke={rgb(TX_LEVEL_COLOURS[l.cls])}
								style:stroke-width={TX_LEVEL_WIDTHS[l.cls]}
								d="M-2 0H26"
							/></svg
						>{l.name}
					</li>
				{/each}
				{#if transformerEdges.length}
					<li>
						<svg viewBox="-14 -12 28 24" width="28" height="24" aria-hidden="true"
							><path d={glyphPath(landmarkGlyph('transformer'), 7)} /></svg
						>transformer
					</li>
				{/if}
			</ul>
			<label class="toggle"
				><input
					type="checkbox"
					checked={landmarksPref.value}
					onchange={(e) => landmarksPref.set(e.currentTarget.checked)}
				/> Landmarks when zoomed out</label
			>
			{#if landmarksPref.value && landmarkPool.length}
				<ul class="symbols" aria-label="Landmarks">
					<li>
						<svg viewBox="-14 -12 28 24" width="28" height="24" aria-hidden="true"
							><path d={glyphPath(landmarkGlyph('generator'), 7)} /></svg
						>generator, largest first
					</li>
				</ul>
			{/if}
			{#if graph.grouped}
				<p class="hint">
					{graph.substations.length.toLocaleString()} substations, {graph.busId.length.toLocaleString()}
					buses
				</p>
			{/if}
		</details>
	{/if}
	{#if underlay.opacity > 0}
		<p
			class="attribution"
			style:opacity={underlay.opacity}
			style:right="{panels.compact ? 12 : panels.dockWidth('right') + 24}px"
			style:bottom="{panels.compact ? app.sheetInset + 8 : 8}px"
		>
			{basemapCredit(underlay.provider)}
		</p>
	{/if}
	{#if status}<p class="status" role="status">{status}</p>{/if}
	<p class="sr-only" aria-live="polite">
		{selectedBus >= 0
			? `Selected ${network.buses[selectedBus].name ?? `bus ${network.buses[selectedBus].id}`}${graph.kv[selectedBus] > 0 ? `, ${levelLabel(graph.kv[selectedBus] * 1000)}` : ''}`
			: ''}
	</p>
</div>

<style>
	/* The multiconductor schematic's look, so both diagrams read as one tool. */
	.tx-stage {
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
	.basemap {
		position: absolute;
		inset: 0;
		pointer-events: none;
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
	.toolbar .check {
		display: flex;
		gap: 4px;
		align-items: center;
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
	.legend i {
		display: inline-block;
		width: 90px;
		height: 7px;
		border-radius: 2px;
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
	.key summary {
		cursor: pointer;
		color: var(--ink);
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
