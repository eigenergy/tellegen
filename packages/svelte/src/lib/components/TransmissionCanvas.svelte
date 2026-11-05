<script lang="ts">
	import type { Deck, OrthographicView, OrthographicViewState, PickingInfo } from '@deck.gl/core';
	import { tick, untrack } from 'svelte';
	import { txSchematicLayout } from '@tellegen/engine';
	import type { Network, Solution } from '../api.js';
	import { demandColor, priceColor, priceGradient, scalarDomain, type RGBA } from '../colors.js';
	import {
		assessLoading,
		assessVoltage,
		operatingVoltages,
		operatingContext,
		stationVoltageSummary,
		placeOperatingGauges,
		gaugeLeader,
		gaugeText,
		GAUGE_HEIGHT,
		GAUGE_WIDTH,
		loadingStateText,
		type GaugeCandidate,
		type OperatingGauge,
		hasStatedRating,
		operatingLoadingColor,
		operatingVoltageColor,
		voltageGradient,
		VOLTAGE_DOMAIN
	} from '../diagram/transmission-results.js';
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
		attachmentGlyph,
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
		branchLevels,
		flowDirections,
		TX_TIERS,
		voltageLegend,
		txBusText,
		txGraph,
		txLandmarks,
		txSourceLayout,
		txLayoutInput,
		fromTxLayout,
		edgeAnchors,
		screenAnchored,
		lerpInto,
		chordsInto,
		ease,
		voltageBandPosition,
		fieldImage,
		median,
		priceFieldColour,
		priceSpread,
		voltageFieldColour,
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
	import { StoredFlag, StoredNumber } from './schematic/view-prefs.svelte.js';
	import TxOperatingPanel from './schematic/TxOperatingPanel.svelte';
	import TxResultInspection from './schematic/TxResultInspection.svelte';
	import TxEconomicsPanel from './schematic/TxEconomicsPanel.svelte';
	import TxTransferPanel from './schematic/TxTransferPanel.svelte';
	import {
		modelDemand,
		transferDirectionText,
		transferReading,
		transferWidth
	} from '../diagram/transmission-transfer.js';
	import {
		PRICE_SCALES,
		PRICE_UNIT,
		economicsFor,
		pricedBranch,
		thermalStatus
	} from '../diagram/transmission-economics.js';
	import { operatingExceptions, dispatchByBus } from '../diagram/transmission-inspection.js';
	import {
		levelPaint,
		busLevelColours,
		entryColour,
		entryWidth,
		txLayers,
		txBranchLayers,
		CASING_SHADE,
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

	type Colour = 'level' | 'results' | 'operating' | 'economics' | 'transfer';
	const app = getAppState();
	const ctrl = getController();
	const panels = getPanelLayout();

	let width = $state(0);
	let height = $state(0);
	let toolbarHeight = $state(0);
	let legendHeight = $state(0);
	let canvas = $state<HTMLCanvasElement>();
	let mapBox = $state<HTMLDivElement>();
	let deck = $state.raw<Deck<OrthographicView> | null>(null);
	let kit: DeckKit | null = null;
	let camera = $state.raw<DiagramCamera>({ target: [0, 0, 0], zoom: 0 });
	let colour = $state<Colour>('operating');
	let resultInspector = $state<ReturnType<typeof TxResultInspection>>();
	const warningPref = new StoredNumber('tellegen.transmission.loading-warning', 0, 99, 90);
	let gauges = $state.raw<OperatingGauge[]>([]);
	const formulation = $derived(ctrl.activeFormulation);
	const resultContext = $derived(
		operatingContext(formulation, solution, !!ctrl.activeSolvable?.solving, !!app.error)
	);
	const operatingOn = $derived(colour === 'operating');
	const economicsOn = $derived(colour === 'economics');
	const economics = $derived(economicsFor(solution, formulation));
	let priceScale = $state(0);
	const transferOn = $derived(colour === 'transfer');
	let transferScale = $state(500);
	const dcTransfer = $derived(!resultContext.voltage);
	const transferSolution = $derived(
		solution && (!solution.formulation || solution.formulation === formulation) ? solution : null
	);
	const solvedVoltages = $derived(operatingVoltages(solution, formulation));
	const deenergized = $derived(
		new Set(
			(solution?.diagnostics ?? [])
				.filter((d) => d.code === 'island_deenergized')
				.flatMap((d) => d.buses)
		)
	);
	const stationMembers = $derived.by(() => {
		const groups: Network['buses'][] = Array.from({ length: graph.substations.length }, () => []);
		network.buses.forEach((bus, i) => groups[graph.substation[i]].push(bus));
		return groups;
	});
	const stationSummaries = $derived(
		stationMembers.map((buses) => stationVoltageSummary(buses, solvedVoltages, deenergized))
	);
	const stationColours = $derived.by(() => {
		if (!operatingOn || !resultContext.voltage) return null;
		const colours = new Uint8Array(stationSummaries.length * 4);
		stationSummaries.forEach((summary, i) =>
			colours.set(
				summary.worst
					? operatingVoltageColor(
							summary.worst.value,
							network.buses[graph.index.get(summary.worst.bus)!]
						)
					: [112, 104, 90, 90],
				i * 4
			)
		);
		return colours;
	});
	const flows = $derived(new Map((solution?.flows ?? []).map((f) => [f.branch, f])));
	const loadingStates = $derived(
		network.branches.map((br) =>
			assessLoading(
				flows.get(br.id),
				!!br.status,
				warningPref.value / 100,
				deenergized.has(br.from) || deenergized.has(br.to)
			)
		)
	);
	const exceptions = $derived(
		operatingExceptions(
			network,
			resultContext.voltage ? solvedVoltages : null,
			deenergized,
			loadingStates,
			flows
		)
	);
	const dispatch = $derived(dispatchByBus(solution?.dispatch ?? []));
	const caseDemandDeltas = $derived.by(() => {
		const active = ctrl.activeSolvable;
		return active?.id === caseId ? ctrl.caseDeltas(active) : {};
	});
	const selectedDemand = $derived.by(() => {
		const bus = network.buses[selectedBus];
		return bus ? modelDemand(bus, caseDemandDeltas) : undefined;
	});
	function transferFlow(branch: Network['branches'][number] | undefined) {
		if (
			!transferSolution ||
			!branch?.status ||
			deenergized.has(branch.from) ||
			deenergized.has(branch.to)
		)
			return undefined;
		return flows.get(branch.id);
	}
	const transferReadings = $derived(
		network.branches.map((branch) => transferReading(transferFlow(branch), dcTransfer))
	);
	const transferSummary = $derived.by(() => {
		let zero = 0;
		let missing = 0;
		const transfers: {
			branch: Network['branches'][number];
			reading: ReturnType<typeof transferReading>;
		}[] = [];
		network.branches.forEach((branch, index) => {
			if (!branch.status || deenergized.has(branch.from) || deenergized.has(branch.to)) return;
			const reading = transferReadings[index];
			if (reading.magnitude) transfers.push({ branch, reading });
			else if (reading.from === 0 && reading.to === 0) zero++;
			else missing++;
		});
		transfers.sort(
			(a, b) => b.reading.magnitude! - a.reading.magnitude! || a.branch.id - b.branch.id
		);
		return { transfers: transfers.slice(0, 12), zero, missing };
	});
	const overloadExceptions = $derived({
		...exceptions,
		buses: [],
		branches: transferSolution ? exceptions.branches.filter((row) => row.state === 'violation') : []
	});
	const activeStation = $derived.by(() => {
		if (!operatingOn || !resultContext.voltage) return null;
		let index =
			selectedBus >= 0
				? graph.substation[selectedBus]
				: hoveredBus >= 0
					? graph.substation[hoveredBus]
					: -1;
		if (index < 0) {
			let worst = 0;
			stationSummaries.forEach((summary, i) => {
				if ((summary.worst?.assessment.exceedance ?? 0) > worst) {
					worst = summary.worst!.assessment.exceedance;
					index = i;
				}
			});
		}
		return index < 0
			? null
			: {
					name: graph.substations[index].name ?? graph.substations[index].key,
					...stationSummaries[index]
				};
	});
	const voltageMarkers = $derived.by(() => {
		if (!operatingOn || !resultContext.voltage) return [];
		const ids = symbolsOn
			? network.buses.map((b) => b.id)
			: stationSummaries.flatMap((s) => (s.worst ? [s.worst.bus] : []));
		return ids.flatMap((id) => {
			const bus = graph.index.get(id)!;
			const state = assessVoltage(
				solvedVoltages.get(id),
				network.buses[bus],
				deenergized.has(id)
			).state;
			return state === 'low' || state === 'high' || state === 'deenergized'
				? [{ bus, kind: state }]
				: [];
		});
	});
	let status = $state<string | null>('Loading the diagram renderer…');
	let hoveredBus = $state(-1);
	let hoveredEdge = $state(-1);
	/** Equipment hovered or focused in a result list: highlighted on the diagram. */
	let preview = $state<{ kind: 'bus' | 'branch'; id: number } | null>(null);
	const setPreview = (item: { kind: 'bus' | 'branch'; id: number } | null) => (preview = item);
	let labels = $state.raw<(LabelCandidate & { position: [number, number] })[]>([]);
	let marks = $state.raw<TxScene['marks']>(null);

	type Layout = 'source' | 'schematic';
	const graph = $derived(txGraph(network));
	const previewBus = $derived(preview?.kind === 'bus' ? (graph.index.get(preview.id) ?? -1) : -1);
	const previewEdge = $derived(
		preview?.kind === 'branch' ? network.branches.findIndex((br) => br.id === preview!.id) : -1
	);
	const source = $derived(txSourceLayout(graph, network));
	const geographic = $derived(network.coordinate_space !== 'diagram');
	let layout = $state<Layout>('source');
	/** The drawing on screen; replaced by `show`, which animates between layouts. */
	let shown = $state.raw<{
		geometry: SchematicGeometry;
		parts: TxSchematicParts | null;
	} | null>(null);
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
	const fieldPref = new StoredFlag('tellegen.transmission.field', ['on', 'off'], true);
	const underlay = new BasemapUnderlay(() => ({
		wanted: basemapPref.value && geographic && !!geometry.mercatorOrigin,
		container: mapBox,
		spec: basemapSpec,
		origin: geometry.mercatorOrigin ?? null,
		camera,
		size: [width, height]
	}));

	// Results: the map's own display series for buses and the solved loading for branches.
	const display = $derived(
		operatingOn
			? resultContext.voltage
				? {
						mode: 'voltage' as const,
						label: '|V|',
						unit: 'pu',
						copy: 'Voltage magnitude',
						gradient: voltageGradient,
						values: [...solvedVoltages].map(([bus, value]) => ({ bus, value }))
					}
				: null
			: economicsOn
				? {
						mode: 'price' as const,
						label: 'Total LMP',
						unit: PRICE_UNIT,
						copy: 'Total locational marginal price',
						gradient: priceGradient,
						values: economics.prices
					}
				: transferOn
					? null
					: ctrl.activeDisplay
	);
	const resultsOn = $derived(colour !== 'level' && !!solution);
	$effect(() => {
		if (colour === 'results' && !solution) colour = 'operating';
	});
	const busValues = $derived(new Map((display?.values ?? []).map((v) => [v.bus, v.value])));
	const domain = $derived(
		economicsOn
			? PRICE_SCALES[priceScale]
			: display?.mode === 'voltage'
				? VOLTAGE_DOMAIN
				: display
					? scalarDomain(display.mode, display.values.map((v) => v.value).filter(Number.isFinite))
					: null
	);
	/** One entry per voltage level, merged into ranges past six (`voltageLegend`). */
	const legend = $derived(voltageLegend(branchLevels(graph), 6));
	/** A legend entry hovered or pinned in the key: that level stays, the others fade. */
	let hoveredEntry = $state(-1);
	let pinnedEntry = $state(-1);
	const focusEntry = $derived(hoveredEntry >= 0 ? hoveredEntry : pinnedEntry);
	const busLevels = $derived(busLevelColours(graph, legend));
	// Zoomed out, levels below the top three fade so the backbone reads first.
	const level = $derived(
		levelPaint(graph, legend, {
			focus: focusEntry,
			fadeFrom: symbolsOn ? Infinity : 3
		})
	);
	/** Branch colours and widths per view, and the branches drawn on top of the rest. Result views
	 * keep ordinary branches thin and grey so the exceptions (loading warnings, overloads, binding
	 * limits) are wide, saturated and visible at every zoom. */
	const edgePaint = $derived.by(() => {
		if (colour === 'level') return { ...level, raised: null };
		const colours = new Uint8Array(level.colours.length);
		const widths = new Float32Array(level.widths);
		const raised = new Uint8Array(network.branches.length);
		// Ordinary branches keep a trace of their voltage rank in their width.
		const quiet = (k: number) => Math.max(1, 0.55 * level.widths[k]);
		network.branches.forEach((br, k) => {
			const f = flows.get(br.id);
			const marginal = economicsOn && pricedBranch(economics.branches.get(br.id));
			if (transferOn && graph.inService[k]) {
				const reading = transferReadings[k];
				colours.set(
					reading.magnitude
						? [26, 114, 163, 235]
						: reading.from === 0 && reading.to === 0
							? [105, 101, 95, 205]
							: [158, 153, 143, 100],
					k * 4
				);
				if (resultContext.stale) colours[k * 4 + 3] = Math.round(colours[k * 4 + 3] * 0.4);
				widths[k] = transferWidth(reading.magnitude, transferScale);
				return;
			}
			const state = loadingStates[k];
			colours.set(
				!graph.inService[k]
					? [120, 116, 108, 200]
					: economicsOn
						? marginal
							? [118, 42, 131, 250]
							: [150, 146, 137, 150]
						: f
							? operatingLoadingColor(state)
							: [138, 131, 117, 160],
				k * 4
			);
			if (resultContext.stale) colours[k * 4 + 3] = Math.round(colours[k * 4 + 3] * 0.4);
			if (!graph.inService[k]) return;
			if (economicsOn) {
				widths[k] = marginal ? Math.max(5, level.widths[k] + 2) : quiet(k);
				if (marginal) raised[k] = 1;
			} else if (f && state === 'violation') {
				widths[k] = Math.max(6, level.widths[k] + 3);
				raised[k] = 1;
			} else if (f && (state === 'warning' || state === 'at-limit')) {
				widths[k] = Math.max(4.5, level.widths[k] + 1.5);
				raised[k] = 1;
			} else widths[k] = quiet(k);
		});
		return { colours, widths, raised: raised.includes(1) ? raised : null };
	});
	const busColours = $derived.by(() => {
		const n = graph.busId.length;
		const out = new Uint8Array(n * 4);
		for (let i = 0; i < n; i++) {
			let c: RGBA = [busLevels[i * 4], busLevels[i * 4 + 1], busLevels[i * 4 + 2], 255];
			if (focusEntry >= 0 && legend.entryOf(graph.kv[i]) !== focusEntry) c = [c[0], c[1], c[2], 40];
			if ((resultsOn || operatingOn || economicsOn) && display && domain) {
				const v = busValues.get(graph.busId[i]);
				// Uniform economic/demand series stay neutral. Voltage always retains its
				// fixed scale and case-limit status, including uniform violations.
				c =
					display.mode === 'voltage'
						? operatingVoltageColor(v, network.buses[i], deenergized.has(graph.busId[i]))
						: v === undefined
							? [180, 175, 165, 200]
							: uniform !== null
								? UNIFORM_INK
								: (display.mode === 'demand' ? demandColor : priceColor)(
										(v - domain.lo) / Math.max(domain.hi - domain.lo, 1e-12)
									);
			}
			if (transferOn || (operatingOn && !resultContext.voltage)) c = [150, 145, 135, 200];
			if (colour !== 'level' && resultContext.stale) c = [c[0], c[1], c[2], Math.round(c[3] * 0.4)];
			out.set(c, i * 4);
		}
		return out;
	});
	/** Which bus values shade the area around the network, if any: solved voltage in Operating
	 * limits (AC), or total LMP in OPF economics when prices differ. */
	const fieldKind = $derived(
		operatingOn && resultContext.voltage && solvedVoltages.size
			? ('voltage' as const)
			: economicsOn && economics.prices.length && economics.uniform === null
				? ('price' as const)
				: null
	);
	/** The median total LMP, the centre of the price-separation shading. */
	const priceCentre = $derived(
		fieldKind === 'price' ? (median(economics.prices.map((p) => p.value)) ?? null) : null
	);
	/** The shaded field as an image in drawing coordinates. */
	const field = $derived.by(() => {
		if (!fieldKind || !fieldPref.value || typeof document === 'undefined') return null;
		const n = graph.busId.length;
		const values = new Float64Array(n).fill(NaN);
		for (let i = 0; i < n; i++) {
			const id = graph.busId[i];
			if (deenergized.has(id)) continue;
			const bus = network.buses[i];
			const v =
				fieldKind === 'voltage'
					? voltageBandPosition(solvedVoltages.get(id), bus.vmin_pu, bus.vmax_pu)
					: busValues.get(id);
			if (v !== undefined) values[i] = v;
		}
		const fade = resultContext.stale ? 0.4 : 1;
		let colourOf = (v: number) => voltageFieldColour(v, 150 * fade);
		if (fieldKind === 'price') {
			if (priceCentre === null) return null;
			for (let i = 0; i < n; i++) values[i] -= priceCentre;
			const spread = priceSpread(values, 0);
			colourOf = (v: number) => priceFieldColour(v, spread, 130 * fade);
		}
		const image = fieldImage(geometry.positions, values, colourOf, {
			reach: typical * 1.4,
			maxSize: 320
		});
		if (!image) return null;
		const canvas = document.createElement('canvas');
		canvas.width = image.width;
		canvas.height = image.height;
		canvas
			.getContext('2d')
			?.putImageData(new ImageData(image.data, image.width, image.height), 0, 0);
		return { image: canvas, bounds: image.bounds };
	});
	const windingColours = $derived(
		operatingOn || transferOn ? busLevels.map((_, i) => (i % 4 === 3 ? 255 : 65)) : busLevels
	);
	const busRadius = $derived(Math.max(1.5, Math.min(5, span / 14)));
	/** The display series' single value when it is uniform (as the map's display panel judges it). */
	const uniform = $derived(economicsOn ? economics.uniform : (ctrl.displayStats?.uniform ?? null));
	const UNIFORM_INK: RGBA = [120, 116, 108, 235];

	/** Generator and load symbols, hanging from the bus (or the bottom of its busbar) once
	 * symbols are legible; side by side when a bus has both. */
	const attachments = $derived.by(() => {
		if (span < 35 || (parts && parts.unit * 2 ** camera.zoom < 3)) return null;
		const unit = 7 / 2 ** stepZoom;
		const points: NonNullable<TxScene['attachments']>['points'] = [];
		for (let i = 0; i < graph.busId.length; i++) {
			const gen = graph.genMw[i] > 0;
			const load =
				(transferOn ? modelDemand(network.buses[i], caseDemandDeltas) : graph.demandMw[i]) > 0;
			if (!gen && !load) continue;
			const x = parts ? parts.busbars[i * 4 + 2] : geometry.positions[i * 3];
			const y = parts ? parts.busbars[i * 4 + 3] : geometry.positions[i * 3 + 1];
			const dx = gen && load ? 1.5 * unit : 0;
			if (gen) points.push({ x: x - dx, y, rootX: x, bus: i, kind: 'generator' });
			if (load) points.push({ x: x + dx, y, rootX: x, bus: i, kind: 'load' });
		}
		return points.length ? { points, unit } : null;
	});

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
	/** Per substation, the member bus that names it: its highest voltage, then lowest row. */
	const subLead = $derived.by(() => {
		const lead = new Int32Array(graph.substations.length).fill(-1);
		for (let i = 0; i < graph.busId.length; i++) {
			const s = graph.substation[i];
			if (lead[s] < 0 || graph.kv[i] > graph.kv[lead[s]]) lead[s] = i;
		}
		return lead;
	});
	/** Flow arrows on legible branches once solved, pointing the way active power flows. */
	const arrows = $derived.by(() => {
		if (!resultsOn || !symbolsOn || economicsOn) return null;
		const { edges, dir } = flowDirections(graph, geometry, anchors, (k) =>
			transferOn ? transferReadings[k].direction : (flows.get(network.branches[k].id)?.mw ?? null)
		);
		return edges.length ? { edges, dir, unit: 5 / 2 ** stepZoom } : null;
	});
	const branchState = (k: number): 'normal' | 'warning' | 'violation' | null => {
		const state = loadingStates[k];
		return state === 'at-limit'
			? 'warning'
			: state === 'violation' || state === 'warning' || state === 'normal'
				? state
				: null;
	};
	const landmarkPool = $derived(
		txLandmarks(
			graph,
			geometry,
			solution && !economicsOn && !transferOn ? branchState : null,
			anchors
		)
	);
	const levelsPresent = $derived(legend.entries);
	const outOfService = $derived(graph.inService.some((on) => !on));
	const TIER_NAMES = Object.fromEntries(TX_TIERS.map((t) => [t.tier, t.name]));

	/** Screen insets no panel or toolbar covers. */
	const freeArea = () =>
		freeInsets({
			compact: panels.compact,
			top:
				panels.top +
				Math.max(0, toolbarHeight - 48) +
				(resultsOn || economicsOn || transferOn ? legendHeight + 8 : 0),
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
	function bringIntoView(p: [number, number], zoomIn: boolean, centre = false) {
		const legible = zoomIn ? Math.log2(80 / Math.max(typical, 1e-9)) : camera.zoom;
		const view = { width, height };
		const area = freeArea();
		const next = centre
			? fitBounds(
					[p[0], p[1], p[0], p[1]],
					view,
					area,
					Math.min(MAX_ZOOM, Math.max(camera.zoom, legible))
				)
			: revealPoint(p, camera, view, area, legible);
		if (next) setCamera(next, true);
	}

	function frameSelection() {
		const point = selectionPoint();
		if (point) bringIntoView(point, true, true);
	}

	async function inspectResult(kind: 'bus' | 'branch', id: number) {
		const networkOpen = panels.compact
			? panels.drawer === 'network'
			: panels.panels.some((panel) => panel.id === 'network' && panel.open);
		if (kind === 'bus') onselectbus(id);
		else onselectbranch(id);
		// The diagram has its own inspector. Preserve the user's panel layout.
		await tick();
		if (networkOpen) return;
		if (panels.compact) {
			if (panels.drawer === 'network') panels.drawer = null;
		} else {
			const panel = panels.panels.find((p) => p.id === 'network');
			if (panel?.open) panels.close(panel);
		}
	}

	async function inspectListedBranch(id: number) {
		await inspectResult('branch', id);
		resultInspector?.focusSelection();
		frameSelection();
	}

	function scene(): TxScene {
		return {
			graph,
			geometry,
			edgeColours: edgePaint.colours,
			edgeWidths: edgePaint.widths,
			busColours,
			busLevels: windingColours,
			stationColours,
			voltageMarkers,
			busRadius,
			selectedBus,
			selectedEdge,
			hoveredBus,
			hoveredEdge,
			previewBus,
			previewEdge,
			field,
			anchors,
			arrows,
			attachments,
			pixel: 1 / 2 ** stepZoom,
			transformers,
			parts,
			morph: morph ? { s: morph.s, t: morph.t, cur: morph.cur, endpoints: morph.endpoints } : null,
			marks,
			labels
		};
	}
	// The deck state becomes available after the dynamically imported layer kit.
	const branchLayers = $derived(
		deck && kit
			? txBranchLayers(
					kit,
					{
						graph,
						geometry,
						edgeColours: edgePaint.colours,
						edgeWidths: edgePaint.widths
					},
					edgePaint.raised
				)
			: []
	);
	function render() {
		if (kit) deck?.setProps({ layers: txLayers(kit, scene(), morph ? [] : branchLayers) });
	}

	function valueText(i: number): { label: string; text: string } | null {
		if (transferOn) {
			const bus = network.buses[i];
			const generation = transferSolution ? dispatch.get(bus.id) : undefined;
			const demand = modelDemand(bus, caseDemandDeltas);
			const parts = [];
			if (generation !== undefined) parts.push(`Gen ${generation.toFixed(1)} MW`);
			if (demand !== 0) parts.push(`Load ${demand.toFixed(1)} MW`);
			return parts.length ? { label: 'Power', text: parts.join(' · ') } : null;
		}
		if (!display) return null;
		const v = busValues.get(graph.busId[i]);
		if (v === undefined) return null;
		const digits = display.mode === 'voltage' ? 4 : 2;
		return {
			label: display.label,
			text: `${v.toFixed(digits)} ${display.unit}`
		};
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
		if (namesOn && !parts)
			for (let i = 0; i < graph.busId.length; i++) {
				if (i === selectedBus) continue;
				const p = at(i);
				const sp = screen(...p);
				if (!inView(sp)) continue;
				const transfer = transferOn ? valueText(i) : null;
				candidates.push({
					text: transfer ? `${name(i)} · ${transfer.text}` : name(i),
					state: 'normal',
					priority: graph.kv[i],
					screen: sp,
					position: p
				});
			}
		// In the overview a substation is named once, at its box; close in, its busbars carry
		// their voltage.
		if (namesOn && parts) {
			for (let s = 0; s < graph.substations.length; s++) {
				const [x0, y0, x1] = parts.boxes.subarray(s * 4, s * 4 + 3);
				if (!(x1 > x0)) continue;
				const p: [number, number] = [x0, y0];
				const sp = screen(...p);
				if (!inView(sp)) continue;
				const lead = subLead[s];
				candidates.push({
					text: graph.substations[s].name ?? (lead >= 0 ? name(lead) : graph.substations[s].key),
					state: 'normal',
					priority: lead >= 0 ? graph.kv[lead] : 0,
					screen: sp,
					position: p
				});
			}
			if (parts.unit * scale >= 12)
				for (let i = 0; i < graph.busId.length; i++) {
					const transfer = transferOn ? valueText(i) : null;
					const level = graph.kv[i] > 0 ? levelLabel(graph.kv[i] * 1000) : '';
					if (!level && !transfer) continue;
					const p: [number, number] = [parts.busbars[i * 4], parts.busbars[i * 4 + 1]];
					const sp = screen(...p);
					if (!inView(sp)) continue;
					candidates.push({
						text: transfer ? [level, transfer.text].filter(Boolean).join(' · ') : level,
						state: 'normal',
						priority: -1,
						screen: sp,
						position: p
					});
				}
		}
		if (resultsOn && !operatingOn && !economicsOn && !transferOn && symbolsOn)
			network.branches.forEach((br, k) => {
				const f = flows.get(br.id);
				const state = branchState(k);
				if (
					!f ||
					state === null ||
					!graph.inService[k] ||
					!hasStatedRating(f.loading_info) ||
					f.loading < 0.6 ||
					!Number.isFinite(anchors[k * 4])
				)
					return;
				const a = { x: anchors[k * 4], y: anchors[k * 4 + 1] };
				const sp = screen(a.x, a.y);
				if (!inView(sp)) return;
				candidates.push({
					text: `${(f.loading * 100).toFixed(0)} %`,
					state,
					priority: 1000 + f.loading,
					screen: sp,
					position: [a.x, a.y]
				});
			});
		// Violating bus values retain priority even when ordinary names are hidden.
		if (operatingOn)
			for (const marker of voltageMarkers) {
				const i = marker.bus,
					p = at(i),
					v = solvedVoltages.get(graph.busId[i]);
				if (!inView(screen(...p))) continue;
				candidates.push({
					text: `${marker.kind === 'low' ? '↓' : marker.kind === 'high' ? '↑' : '×'} bus ${graph.busId[i]}${v !== undefined ? ` · ${v.toFixed(4)} pu` : ''}`,
					state: marker.kind === 'deenergized' ? 'normal' : 'violation',
					priority: 1e6,
					screen: screen(...p),
					position: p
				});
			}
		if (transferOn && (selectedEdge >= 0 || hoveredEdge >= 0)) {
			const edge = selectedEdge >= 0 ? selectedEdge : hoveredEdge;
			const p: [number, number] = [anchors[edge * 4], anchors[edge * 4 + 1]];
			const sp = screen(...p);
			if (inView(sp)) {
				const magnitude = transferReadings[edge].magnitude;
				candidates.push({
					text: magnitude === undefined ? 'Active flow unavailable' : `${magnitude.toFixed(1)} MW`,
					state: 'normal',
					priority: 1e9,
					screen: sp,
					position: p,
					selected: true
				});
			}
		}
		labels = placeLabels(candidates, { width, height, budget: 60 });
		const inset = freeArea();
		const gaugeCandidates: GaugeCandidate[] = [];
		if ((operatingOn && solution) || (transferOn && transferSolution))
			network.branches.forEach((br, edge) => {
				const f = flows.get(br.id),
					state = loadingStates[edge];
				const selected = edge === selectedEdge || edge === hoveredEdge;
				if (transferOn && state !== 'violation') return;
				if (
					!f ||
					!hasStatedRating(f.loading_info) ||
					!['normal', 'warning', 'at-limit', 'violation'].includes(state)
				)
					return;
				if (!selected && state === 'normal' && (!symbolsOn || f.loading < 0.6)) return;
				const [x, y] = screen(anchors[edge * 4], anchors[edge * 4 + 1]);
				if (!Number.isFinite(x) || !Number.isFinite(y)) return;
				gaugeCandidates.push({ edge, ax: x, ay: y, loading: f.loading, state, selected });
			});
		gauges = placeOperatingGauges(
			gaugeCandidates,
			{
				left: inset.left,
				top: inset.top,
				right: width - inset.right,
				bottom: height - inset.bottom
			},
			panels.compact ? 10 : 32
		);

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
					fromMap && showsAnyPosition(geometry.positions, fromMap, { width, height }, freeArea())
						? fromMap
						: fitAll();
				deck = new core.Deck({
					canvas: element,
					width: '100%',
					height: '100%',
					views: new core.OrthographicView({ id: 'transmission', flipY: true }),
					initialViewState: { ...camera, minZoom: MIN_ZOOM, maxZoom: MAX_ZOOM },
					controller: { doubleClickZoom: true },
					pickingRadius: matchMedia('(pointer: coarse)').matches ? 12 : 6,
					getCursor: ({ isHovering, isDragging }) =>
						isDragging ? 'grabbing' : isHovering ? 'pointer' : 'grab',
					onViewStateChange: ({ viewState }) => {
						const v = viewState as OrthographicViewState;
						camera = {
							target: v.target as [number, number, number],
							zoom: v.zoom as number
						};
					},
					onHover: (info: PickingInfo) => {
						const id = info.layer?.id;
						const hit = info.index >= 0;
						hoveredBus =
							hit && (id === 'buses' || id === 'busbars')
								? info.index
								: hit && id === 'attachments-pick'
									? info.object.bus
									: -1;
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
						else if (id === 'attachments-pick') onselectbus(graph.busId[info.object.bus]);
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
							return {
								text: busTooltip(info.index)
							};
						if (id === 'attachments-pick')
							return {
								text: busTooltip(info.object.bus)
							};
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
								text: branchTooltip(k)
							};
						if (id === 'landmark-halos') {
							const l = info.object as Landmark;
							return {
								text: busTooltip(l.bus)
							};
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
	let schematic: Promise<{
		geometry: SchematicGeometry;
		parts: TxSchematicParts;
	}> | null = null;
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
		let drawing: {
			geometry: SchematicGeometry;
			parts: TxSchematicParts | null;
		};
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
		const to = showsAnyPosition(drawing.geometry.positions, camera, { width, height }, freeArea())
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
			previewBus,
			previewEdge,
			field,
			transformers,
			arrows,
			attachments,
			stepZoom,
			marks,
			labels,
			voltageMarkers,
			stationColours,
			operatingOn
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
			busValues,
			transferOn,
			transferOn ? [transferReadings, dispatch, caseDemandDeltas] : null,
			geometry,
			parts,
			loadingStates,
			operatingOn,
			voltageMarkers,
			selectedEdge,
			hoveredEdge,
			legendHeight,
			toolbarHeight,
			panels.compact,
			panels.dockWidth('left'),
			panels.dockWidth('right'),
			app.sheetInset
		];
		if (!deck) return;
		cancelAnimationFrame(labelFrame);
		labelFrame = requestAnimationFrame(updateLabels);
	});

	// Preserve the camera unless a resized key or panel covers every bus.
	$effect(() => {
		void [
			legendHeight,
			toolbarHeight,
			width,
			height,
			panels.dockWidth('left'),
			panels.dockWidth('right'),
			app.sheetInset
		];
		if (!deck || width <= 0 || height <= 0 || !graph.busId.length) return;
		untrack(() => {
			if (!showsAnyPosition(geometry.positions, camera, { width, height }, freeArea()))
				setCamera(fitAll());
		});
	});

	// Record the geographic view on the app, so the map continues from it when this closes.
	$effect(() => {
		const origin = geometry.mercatorOrigin;
		if (!deck || !origin) return;
		const view = basemapCamera(camera, origin);
		untrack(() =>
			ongeoview?.({
				center: view.center,
				zoom: view.zoom,
				bearing: 0,
				pitch: 0
			})
		);
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
			view: operatingOn
				? `Operating limits · ${resultContext.freshness} · ${formulation}${resultContext.relaxed ? ' relaxation' : ''}`
				: economicsOn
					? `OPF economics · ${formulation}${resultContext.relaxed ? ' relaxation' : ''} · Total LMP ${domain!.lo} to ${domain!.hi} ${PRICE_UNIT}`
					: transferOn
						? `Power transfers · ${formulation}${resultContext.relaxed ? ' relaxation' : ''}`
						: resultsOn
							? `Buses: ${display?.copy ?? 'results'} · branches: loading`
							: 'Voltage levels',
			camera,
			result: solution
				? {
						kind: 'solved',
						label: display?.copy ?? 'Solved',
						iterations: null,
						revision: caseId
					}
				: null,
			exported: new Date().toISOString()
		});
		const blob = await canvasPng(
			deck,
			canvas,
			title,
			operatingOn || transferOn ? paintResultExport : undefined
		);
		if (blob) download(blob, `${fileStem(label)}.png`);
	}
	function paintResultExport(ctx: CanvasRenderingContext2D) {
		ctx.save();
		ctx.globalAlpha = resultContext.stale ? 0.4 : 1;
		for (const gauge of gauges) {
			const { x, y } = gauge;
			const over = gauge.state === 'violation';
			const colour = over
				? '#be2024'
				: gauge.state === 'warning' || gauge.state === 'at-limit'
					? '#925b05'
					: '#252525';
			const [lx, ly] = gaugeLeader(gauge);
			ctx.strokeStyle = colour;
			ctx.lineWidth = 1.2;
			ctx.beginPath();
			ctx.moveTo(gauge.ax, gauge.ay);
			ctx.lineTo(lx, ly);
			ctx.stroke();
			ctx.fillStyle = colour;
			ctx.beginPath();
			ctx.arc(gauge.ax, gauge.ay, 2.5, 0, 2 * Math.PI);
			ctx.fill();
			ctx.fillStyle = over ? '#be2024' : '#fcfbf7';
			ctx.fillRect(x, y, GAUGE_WIDTH, GAUGE_HEIGHT);
			ctx.lineWidth = gauge.state === 'at-limit' ? 2 : 1;
			ctx.strokeRect(x, y, GAUGE_WIDTH, GAUGE_HEIGHT);
			ctx.font = '10px "IBM Plex Mono", monospace';
			ctx.textAlign = 'center';
			ctx.fillStyle = over ? '#ffffff' : colour;
			ctx.fillText(gaugeText(gauge.loading), x + 32, y + 20);
			ctx.strokeStyle = over ? '#f2b8b5' : '#c9c4ba';
			ctx.lineWidth = 3;
			ctx.beginPath();
			ctx.moveTo(x + 10, y + 31);
			ctx.lineTo(x + 54, y + 31);
			ctx.stroke();
			ctx.strokeStyle = over ? '#ffffff' : colour;
			ctx.beginPath();
			ctx.moveTo(x + 10, y + 31);
			ctx.lineTo(x + 10 + (Math.min(1.25, gauge.loading) / 1.25) * 44, y + 31);
			ctx.stroke();
			ctx.strokeStyle = over ? '#ffffff' : '#252525';
			ctx.lineWidth = 1.5;
			ctx.beginPath();
			ctx.moveTo(x + 45.2, y + 27);
			ctx.lineTo(x + 45.2, y + 35);
			ctx.stroke();
		}
		ctx.restore();
		const context = `${formulation}${resultContext.relaxed ? ' relaxation' : ''} · ${resultContext.freshness} · ${resultContext.status} · base case`;
		const lines = transferOn
			? [
					`Power transfers · ${context}`,
					`Blue line width: 0–${transferScale} MW · Largest reported endpoint |P|`,
					'Chevrons show direction · Red gauges show overload in percent of rating'
				]
			: [
					`Operating limits · ${context}`,
					resultContext.voltage
						? 'Voltage 0.90–1.10 pu · ↓ below / ↑ above case limits'
						: 'DC: voltage magnitude not solved',
					`Loading ${resultContext.basis} · warning ${warningPref.value}% · tick 100% · red: overload`,
					...(activeStation
						? [
								`Station ${activeStation.name}: ${activeStation.min?.value.toFixed(4) ?? 'unknown'}–${activeStation.max?.value.toFixed(4) ?? 'unknown'} pu${activeStation.worst ? ` · worst bus ${activeStation.worst.bus}` : ''}`
							]
						: [])
				];
		ctx.font = '11px "IBM Plex Mono", monospace';
		const wrapped: string[] = [];
		for (const line of lines) {
			let current = '';
			for (const word of line.split(' ')) {
				const next = current ? `${current} ${word}` : word;
				if (current && ctx.measureText(next).width > Math.min(width - 40, 690)) {
					wrapped.push(current);
					current = word;
				} else current = next;
			}
			wrapped.push(current);
		}
		ctx.fillStyle = '#fcfbf7';
		ctx.fillRect(12, 12, Math.min(width - 24, 710), 14 + wrapped.length * 16);
		ctx.fillStyle = '#252525';
		wrapped.forEach((line, i) => ctx.fillText(line, 20, 30 + i * 16));
	}
	function stationText(i: number): string {
		if (!operatingOn || !resultContext.voltage) return '';
		const s = stationSummaries[graph.substation[i]];
		return [
			s.min && s.max
				? `Station range ${s.min.value.toFixed(4)}–${s.max.value.toFixed(4)} pu (min bus ${s.min.bus}, max bus ${s.max.bus})`
				: 'Station voltage unavailable',
			s.worst
				? `Worst violation: bus ${s.worst.bus}, ${s.worst.assessment.state === 'low' ? 'below' : 'above'} limit by ${s.worst.assessment.exceedance.toFixed(4)} pu`
				: '',
			`${s.unknown} missing values or limits; ${s.disconnected} de-energized`
		]
			.filter(Boolean)
			.join('\n');
	}
	function busTooltip(i: number): string {
		return [
			txBusText(graph, network, i, valueText(i)),
			operatingOn && resultContext.voltage
				? `Operating band: ${network.buses[i].vmin_pu ?? 'unknown'}–${network.buses[i].vmax_pu ?? 'unknown'} pu`
				: '',
			stationText(i)
		]
			.filter(Boolean)
			.join('\n');
	}
	function branchTooltip(k: number): string {
		const branch = network.branches[k];
		const f = transferOn ? transferFlow(branch) : flows.get(branch.id);
		const info = f?.loading_info;
		return [
			`${graph.transformer[k] ? 'Transformer' : 'Line'} ${network.branches[k].id}: bus ${network.branches[k].from} to bus ${network.branches[k].to}`,
			`Nominal ${graph.from[k] >= 0 ? graph.kv[graph.from[k]] : 'unknown'} / ${graph.to[k] >= 0 ? graph.kv[graph.to[k]] : 'unknown'} kV`,
			f && (!transferOn || transferReadings[k].from !== undefined)
				? `From-end active power ${f.mw.toFixed(2)} MW`
				: 'Flow result missing',
			...(transferOn ? [transferDirectionText(transferReadings[k], network.branches[k])] : []),
			...(economicsOn ? [thermalStatus(economics.branches.get(network.branches[k].id))] : []),
			`Loading: ${loadingStateText[loadingStates[k]]}`,
			hasStatedRating(info) &&
			f &&
			['normal', 'warning', 'at-limit', 'violation'].includes(loadingStates[k])
				? `${(f.loading * 100).toFixed(2)}% of ${info!.rating} ${info!.basis.toUpperCase()} rating${info!.limiting_end ? ` · limiting end: ${info!.limiting_end}` : ''}`
				: 'Equipment utilization unavailable'
		].join('\n');
	}
	const rgb = (c: RGBA) => `rgb(${c.slice(0, 3).join(' ')})`;
	const casingColour = (c: RGBA): RGBA => [
		Math.round(c[0] * CASING_SHADE),
		Math.round(c[1] * CASING_SHADE),
		Math.round(c[2] * CASING_SHADE),
		c[3]
	];
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
		bind:clientHeight={toolbarHeight}
		style:max-width="calc(100% - {toolLeft +
			(panels.compact ? 12 : panels.dockWidth('right') + 24)}px)"
		style:top="{panels.top + 8}px"
		style:left="{toolLeft}px"
		role="toolbar"
		aria-label="Single-line diagram"
	>
		<label>
			<span class="sr-only">Diagram view</span>
			<select bind:value={colour} title="Diagram view">
				<option value="level">Voltage levels</option>
				<option value="operating">Operating limits</option>
				<option value="economics">OPF economics</option>
				<option value="transfer">Power transfers</option>
				<option value="results" disabled={!solution}>Map results</option>
			</select>
		</label>
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
		{#if fieldKind}
			<label
				class="check"
				title={fieldKind === 'voltage'
					? 'Shade the area around buses near their voltage limits: blue towards the minimum, brown towards the maximum'
					: 'Shade the area around buses by total LMP, on the price scale'}
				><input
					type="checkbox"
					checked={fieldPref.value}
					onchange={(e) => fieldPref.set(e.currentTarget.checked)}
				/> Shading</label
			>
		{/if}
		<button onclick={() => setCamera(fitAll())} title="Fit the drawing (F)">Fit</button>
		<button onclick={() => zoomBy(-0.5)} aria-label="Zoom out" title="Zoom out">−</button>
		<button onclick={() => zoomBy(0.5)} aria-label="Zoom in" title="Zoom in">+</button>
		<button onclick={exportPng} title="PNG of this view with a title block">PNG</button>
		<button onclick={onclose} title="Return to the map">Close</button>
	</div>
	{#if resultsOn || operatingOn || economicsOn || transferOn}
		<div
			class="legend"
			class:operating={operatingOn || economicsOn || transferOn}
			bind:clientHeight={legendHeight}
			style:top="{panels.top + toolbarHeight + 16}px"
			style:left="{toolLeft}px"
			style:right="{panels.compact ? 12 : panels.dockWidth('right') + 24}px"
			style:max-height="min(38vh, calc(100% - {panels.top +
				toolbarHeight +
				32 +
				app.sheetInset}px))"
		>
			{#if operatingOn}
				<TxOperatingPanel
					{formulation}
					context={resultContext}
					voltageAvailable={solvedVoltages.size > 0}
					counts={exceptions.counts}
					warning={warningPref.value}
					onwarning={(value) => warningPref.set(value)}
					station={activeStation}
					shading={!!field}
					{onselectbus}
				/>
			{:else if economicsOn}
				<TxEconomicsPanel
					{formulation}
					context={resultContext}
					{economics}
					scale={priceScale}
					onscale={(scale) => (priceScale = scale)}
					bus={network.buses[selectedBus]}
					branch={network.branches[selectedEdge]}
					busPrice={busValues.get(graph.busId[selectedBus])}
					flow={flows.get(network.branches[selectedEdge]?.id)}
					onbranch={inspectListedBranch}
					onpreview={setPreview}
					shadingCentre={field ? priceCentre : null}
				/>
			{:else if transferOn}
				<TxTransferPanel
					{formulation}
					context={resultContext}
					dc={dcTransfer}
					scale={transferScale}
					onscale={(scale) => (transferScale = scale)}
					{...transferSummary}
					hasDispatch={!!transferSolution && dispatch.size > 0}
					onbranch={inspectListedBranch}
					onpreview={setPreview}
				/>
			{:else if display?.mode === 'voltage'}
				Voltage <span>0.90</span><i style:background={voltageGradient}></i><span>1.10 pu</span> · case
				limits apply ·
			{:else if display && uniform !== null}
				{display.label} uniform at {uniform.toPrecision(4)}
				{display.unit} · grey ·
			{:else if display}
				{display.label} <i style:background={display.gradient}></i> ·
			{/if}
			{#if operatingOn || economicsOn || transferOn}
				<TxResultInspection
					bind:this={resultInspector}
					exceptions={transferOn ? overloadExceptions : exceptions}
					transfer={transferOn
						? { dc: dcTransfer, reading: transferReadings[selectedEdge] }
						: undefined}
					bus={network.buses[selectedBus]}
					branch={network.branches[selectedEdge]}
					voltage={solvedVoltages.get(graph.busId[selectedBus])}
					voltageSolved={resultContext.voltage}
					deenergized={deenergized.has(graph.busId[selectedBus])}
					generation={!transferOn || transferSolution
						? dispatch.get(graph.busId[selectedBus])
						: undefined}
					demand={selectedDemand}
					flow={transferOn
						? transferFlow(network.branches[selectedEdge])
						: flows.get(network.branches[selectedEdge]?.id)}
					loading={!transferOn || transferSolution ? loadingStates[selectedEdge] : undefined}
					stale={resultContext.stale}
					showExceptions={operatingOn || transferOn}
					oninspect={inspectResult}
					onpreview={setPreview}
					{onclear}
					onframe={frameSelection}
				/>
			{:else}
				Branch colour: loading · grey below {warningPref.value} % · amber {warningPref.value}–100 %
				· red above 100 % · unknown ratings faded
			{/if}
		</div>
	{/if}
	{#if operatingOn || transferOn}
		<div
			class="gauges"
			aria-label={transferOn ? 'Branch overload gauges' : 'Branch utilization gauges'}
			class:stale={resultContext.stale}
		>
			<svg class="leaders" aria-hidden="true" {width} {height}>
				{#each gauges as gauge (gauge.edge)}
					{@const [lx, ly] = gaugeLeader(gauge)}
					<g
						class:violation={gauge.state === 'violation'}
						class:warning={gauge.state === 'warning' || gauge.state === 'at-limit'}
					>
						<line x1={gauge.ax} y1={gauge.ay} x2={lx} y2={ly} />
						<circle cx={gauge.ax} cy={gauge.ay} r="2.5" />
					</g>
				{/each}
			</svg>
			{#each gauges as gauge (gauge.edge)}
				{@const branch = network.branches[gauge.edge]}
				<button
					class="utilization-gauge"
					class:violation={gauge.state === 'violation'}
					class:warning={gauge.state === 'warning' || gauge.state === 'at-limit'}
					class:at-limit={gauge.state === 'at-limit'}
					style:left="{gauge.x}px"
					style:top="{gauge.y}px"
					title={branchTooltip(gauge.edge)}
					aria-pressed={gauge.edge === selectedEdge}
					aria-label="Branch {branch.id}, {(gauge.loading * 100).toFixed(
						1
					)} percent, {loadingStateText[gauge.state]}"
					onclick={() => onselectbranch(branch.id)}
				>
					<span>{gaugeText(gauge.loading)}</span>
					<svg width="48" height="10" viewBox="0 0 48 10" aria-hidden="true"
						><path d="M2 5H46" class="gauge-track" /><path
							d="M2 5H{2 + (Math.min(1.25, gauge.loading) / 1.25) * 44}"
							class="gauge-fill"
						/><path d="M37.2 1V9" class="rating-tick" /></svg
					>
				</button>
			{/each}
		</div>
	{/if}
	{#if colour === 'level' && levelsPresent.length}
		<details
			class="key"
			style:bottom="{panels.compact ? app.sheetInset + 12 : 24}px"
			style:left="{toolLeft}px"
			open={keyPref.value}
			ontoggle={(e) => keyPref.set(e.currentTarget.open)}
		>
			<summary>Key</summary>
			<p class="hint">Line colour and width: voltage level</p>
			<ul class="symbols" aria-label="Voltage levels">
				{#each levelsPresent as l, j (l.label)}
					{#if j === 0 || levelsPresent[j - 1].tier !== l.tier}
						<li class="tier">{TIER_NAMES[l.tier]}</li>
					{/if}
					<li>
						<button
							class="level"
							class:dim={focusEntry >= 0 && focusEntry !== j}
							aria-pressed={pinnedEntry === j}
							title="Highlight {l.label} ({l.branches.toLocaleString()} branch{l.branches === 1
								? ''
								: 'es'}); click to keep it highlighted"
							onpointerenter={() => (hoveredEntry = j)}
							onpointerleave={() => (hoveredEntry = -1)}
							onfocus={() => (hoveredEntry = j)}
							onblur={() => (hoveredEntry = -1)}
							onclick={() => (pinnedEntry = pinnedEntry === j ? -1 : j)}
						>
							<svg viewBox="-2 -12 28 24" width="28" height="24" aria-hidden="true"
								><path
									style:stroke={rgb(casingColour(entryColour(l)))}
									style:stroke-width={entryWidth(l) + 1.4}
									d="M-2 0H26"
								/>
								<path
									style:stroke={rgb(entryColour(l))}
									style:stroke-width={entryWidth(l)}
									d="M-2 0H26"
								/></svg
							>{l.label}
						</button>
					</li>
				{/each}
				{#if transformerEdges.length}
					<li>
						<svg viewBox="-14 -12 28 24" width="28" height="24" aria-hidden="true"
							><path d={glyphPath(landmarkGlyph('transformer'), 7)} /></svg
						>transformer
					</li>
				{/if}
				{#if outOfService}
					<li>
						<svg viewBox="-2 -12 28 24" width="28" height="24" aria-hidden="true"
							><path d="M-2 0H26" stroke-dasharray="4 3" /></svg
						>out of service
					</li>
				{/if}
			</ul>
			<ul class="symbols" aria-label="Equipment">
				<li>
					<svg viewBox="-2 -12 28 24" width="28" height="24" aria-hidden="true"
						><path d={glyphPath(attachmentGlyph('load'), 10)} class="load-key" /></svg
					>load (demand arrow)
				</li>
				<li>
					<svg viewBox="-2 -12 28 24" width="28" height="24" aria-hidden="true"
						><path d={glyphPath(attachmentGlyph('generator'), 9)} /></svg
					>generator
				</li>
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
			style:max-height="min(38vh, calc(100% - {panels.top +
				toolbarHeight +
				32 +
				app.sheetInset}px))"
			style:bottom="{panels.compact ? app.sheetInset + 8 : 8}px"
		>
			{basemapCredit(underlay.provider)}
		</p>
	{/if}
	{#if status}<p class="status" role="status">{status}</p>{/if}
	<p class="sr-only" aria-live="polite">
		{selectedBus >= 0
			? `Selected ${network.buses[selectedBus].name ?? `bus ${network.buses[selectedBus].id}`}${graph.kv[selectedBus] > 0 ? `, ${levelLabel(graph.kv[selectedBus] * 1000)}` : ''}`
			: selectedEdge >= 0
				? `Selected branch ${network.branches[selectedEdge].id}`
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
		width: max-content;
		flex-wrap: wrap;
		right: 12px;
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
	.gauges {
		position: absolute;
		inset: 0;
		pointer-events: none;
	}
	.gauges.stale {
		opacity: 0.4;
	}
	.utilization-gauge {
		position: absolute;
		width: 64px;
		height: 44px;
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		gap: 1px;
		background: color-mix(in srgb, var(--paper) 92%, transparent);
		border: 1px solid var(--line);
		border-radius: 4px;
		color: var(--ink);
		font: 10px var(--font-mono);
		cursor: pointer;
		pointer-events: auto;
		padding: 2px;
	}
	.utilization-gauge.warning {
		color: #925b05;
		border-color: #925b05;
	}
	/* At rating: a heavier outline; over rating: filled. Shape and fill, not only hue. */
	.utilization-gauge.at-limit {
		border-width: 2px;
	}
	.utilization-gauge.violation {
		color: #fff;
		background: #be2024;
		border-color: #be2024;
		font-weight: 600;
	}
	.utilization-gauge.violation .gauge-track {
		stroke: #f2b8b5;
	}
	.utilization-gauge.violation .rating-tick {
		stroke: #fff;
	}
	.leaders {
		position: absolute;
		inset: 0;
		overflow: visible;
	}
	.leaders line {
		stroke: #6b665c;
		stroke-width: 1.2;
	}
	.leaders circle {
		fill: #6b665c;
	}
	.leaders .warning line {
		stroke: #925b05;
	}
	.leaders .warning circle {
		fill: #925b05;
	}
	.leaders .violation line {
		stroke: #be2024;
	}
	.leaders .violation circle {
		fill: #be2024;
	}
	.gauge-track {
		stroke: #c9c4ba;
		stroke-width: 3;
	}
	.gauge-fill {
		stroke: currentColor;
		stroke-width: 3;
	}
	.rating-tick {
		stroke: #252525;
		stroke-width: 1.5;
	}
	.legend {
		overflow: auto;
		overscroll-behavior: contain;
		position: absolute;
		height: auto;
		margin-top: 0;
		display: flex;
		flex-wrap: wrap;
		right: 12px;
		gap: 6px;
		align-items: center;
		padding: 4px 8px;
		font: 10px var(--font-mono);
		color: var(--text-secondary);
		background: var(--paper);
		border: 1px solid var(--line);
		border-radius: 3px;
	}
	.legend.operating {
		max-width: 720px;
	}
	.legend i {
		display: inline-block;
		width: 90px;
		height: 7px;
		border-radius: 2px;
	}
	.key {
		max-height: 40vh;
		overflow: auto;
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
	.symbols .load-key {
		fill: var(--ink);
	}
	@media (pointer: coarse) {
		.toolbar button,
		.toolbar select,
		.toolbar .check,
		.symbols .level,
		.key summary,
		.key .toggle {
			min-height: 44px;
			min-width: 44px;
		}
	}
	.symbols .level {
		display: flex;
		gap: 4px;
		align-items: center;
		padding: 0 4px 0 0;
		font: inherit;
		color: var(--ink);
		background: none;
		border: 1px solid transparent;
		border-radius: 3px;
		cursor: pointer;
	}
	.symbols .level[aria-pressed='true'] {
		border-color: var(--line);
		background: var(--accent-soft);
	}
	.symbols .level.dim {
		opacity: 0.45;
	}
	.symbols .tier {
		margin-top: 4px;
		color: var(--text-secondary);
		font-size: 9px;
		letter-spacing: 0.04em;
		text-transform: uppercase;
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
