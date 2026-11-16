<script lang="ts">
	import type { Deck, OrthographicView, OrthographicViewState, PickingInfo } from '@deck.gl/core';
	import { tick, untrack } from 'svelte';
	import { txSchematicLayout } from '@tellegen/engine';
	import type { Network, Solution } from '../api.js';
	import {
		demandColor,
		priceColor,
		priceGradient,
		scalarColor,
		scalarDomain,
		type RGBA
	} from '../colors.js';
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
		DEFAULT_LOADING_WARNING,
		hasStatedRating,
		isRated,
		operatingLoadingColor,
		operatingVoltageColor,
		voltageGradient,
		VOLTAGE_DOMAIN
	} from '../diagram/transmission-results.js';
	import { getAppState, getController, getUiConfig } from '../context.svelte.js';
	import { docsLink } from '../diagram/transmission-explain.js';
	import { displayFmt, formulationLabel } from '../format.js';
	import type { CameraSnapshot } from '../state.svelte.js';
	import { getPanelLayout } from '../panels.svelte.js';
	import { basemapCredit, type BasemapSpec } from '../basemap.js';
	import {
		BASEMAP_STYLE,
		basemapCamera,
		busBounds,
		cameraFromMap,
		fitBounds,
		openingCamera,
		freeInsets,
		glyphPath,
		attachmentGlyph,
		landmarkGlyph,
		landmarkStrokes,
		levelLabel,
		MIN_ZOOM,
		maxZoomFor,
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
		voltageBandPosition,
		fieldImage,
		median,
		priceFieldColour,
		priceSpread,
		voltageFieldColour,
		MORPH_MAX_BUSES,
		startMorph,
		type Morph,
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
	import StatBoxes from './StatBoxes.svelte';
	import TxResultInspection from './schematic/TxResultInspection.svelte';
	import TxEquipmentPanel from './schematic/TxEquipmentPanel.svelte';
	import { equipmentDrawing, equipmentItems } from '../diagram/transmission-equipment.js';
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
		/** The background map under geographic positions: the map's own basemap, in its theme. */
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
	/** The results panel collapsed to its summary: by default on a phone, where it would cover
	 * much of the diagram. A new selection opens it, since the details are what was asked for. */
	let brief = $state(getPanelLayout().compact);
	let displayOptions = $state(false);
	let resultInspector = $state<ReturnType<typeof TxResultInspection>>();
	const warningPref = new StoredNumber(
		'tellegen.transmission.loading-warning',
		0,
		99,
		DEFAULT_LOADING_WARNING * 100
	);
	let gauges = $state.raw<OperatingGauge[]>([]);
	const formulation = $derived(ctrl.activeFormulation);
	/** The active case's last solve ended without a result. An unrelated error elsewhere in
	 * the app does not make this result out of date. */
	const solveFailed = $derived.by(() => {
		const active = ctrl.activeSolvable;
		return !!active && !active.solving && active.solveBackend !== null && active.solveMs == null;
	});
	const resultContext = $derived(
		operatingContext(formulation, solution, !!ctrl.activeSolvable?.solving, solveFailed)
	);
	const operatingOn = $derived(colour === 'operating');
	const economicsOn = $derived(colour === 'economics');
	/** Operating limits with solved voltage magnitudes (AC). */
	const voltageView = $derived(operatingOn && resultContext.voltage);
	const economics = $derived(economicsFor(solution, formulation));
	/** -1 fits the price scale to the prices, as the Network panel's legend does. */
	let priceScale = $state(-1);
	const transferOn = $derived(colour === 'transfer');
	/** A view with its own results panel (key, settings and lists). */
	const panelView = $derived(operatingOn || economicsOn || transferOn);
	const uiConfig = getUiConfig();
	/** The calculation's name as the export title block reads it. */
	const calculationName = $derived(formulationLabel(formulation));
	let transferScale = $state(500);
	const dcTransfer = $derived(!resultContext.voltage);
	const transferSolution = $derived(resultContext.current);
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
		if (!voltageView) return null;
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
	/** Branches that can carry a utilization gauge: rated, with a flow, and in Power transfers
	 * overloaded. `busy` ones (above normal, or normal from 60%) are shown whenever on screen;
	 * `quiet` ones only while selected or hovered. Kept off the per-frame label pass. */
	const gaugePool = $derived.by(() => {
		const busy: number[] = [];
		const quiet = new Set<number>();
		if ((operatingOn && solution) || (transferOn && transferSolution))
			network.branches.forEach((br, edge) => {
				const f = flows.get(br.id);
				const state = loadingStates[edge];
				if (transferOn && state !== 'violation') return;
				if (!f || !hasStatedRating(f.loading_info) || !isRated(state)) return;
				if (state === 'normal' && f.loading < 0.6) quiet.add(edge);
				else busy.push(edge);
			});
		return { busy, quiet };
	});
	/** The station with the worst voltage violation, shown when nothing is selected or hovered. */
	const worstStation = $derived.by(() => {
		let index = -1;
		let worst = 0;
		stationSummaries.forEach((summary, i) => {
			if ((summary.worst?.assessment.exceedance ?? 0) > worst) {
				worst = summary.worst!.assessment.exceedance;
				index = i;
			}
		});
		return index;
	});
	/** A substation's name, or the bus it stands for when it has none. */
	function stationName(station: { name?: string | null; key: string }): string {
		if (station.name) return station.name;
		const bus = /^bus:(\d+)$/.exec(station.key)?.[1];
		return bus ? `Bus ${bus}` : station.key;
	}
	const activeStation = $derived.by(() => {
		if (!voltageView) return null;
		const index =
			selectedBus >= 0
				? graph.substation[selectedBus]
				: hoveredBus >= 0
					? graph.substation[hoveredBus]
					: worstStation;
		return index < 0
			? null
			: {
					name: stationName(graph.substations[index]),
					...stationSummaries[index]
				};
	});
	const voltageMarkers = $derived.by(() => {
		if (!voltageView) return [];
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
	let status = $state<string | null>('Loading…');
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
		preview?.kind === 'branch' ? (graph.branchIndex.get(preview.id) ?? -1) : -1
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
						label: 'Voltage',
						unit: 'pu',
						copy: 'Voltage',
						gradient: voltageGradient,
						values: [...solvedVoltages].map(([bus, value]) => ({ bus, value }))
					}
				: null
			: economicsOn
				? {
						mode: 'price' as const,
						label: 'Price',
						unit: PRICE_UNIT,
						copy: 'Price',
						gradient: priceGradient,
						values: economics.prices
					}
				: transferOn
					? null
					: ctrl.activeDisplay
	);
	const resultsOn = $derived(colour !== 'level' && !!solution);
	const busValues = $derived(new Map((display?.values ?? []).map((v) => [v.bus, v.value])));
	const domain = $derived(
		economicsOn
			? priceScale < 0
				? scalarDomain('price', economics.prices.map((v) => v.value).filter(Number.isFinite))
				: PRICE_SCALES[priceScale]
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
							? [215, 38, 30, 250]
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
			} else if (f && (state === 'violation' || state === 'at-limit')) {
				widths[k] = Math.max(6, level.widths[k] + 3);
				raised[k] = 1;
			} else if (f && state === 'warning') {
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
							? [172, 179, 188, 200]
							: uniform !== null
								? UNIFORM_INK
								: scalarColor(
										display.mode,
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
		voltageView && solvedVoltages.size
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
			const spread = priceSpread(values, priceCentre);
			for (let i = 0; i < n; i++) values[i] -= priceCentre;
			// The hue comes from the price ramp the buses and the legend use; the deviation from
			// the median only sets how strong the shading is.
			const centre = priceCentre;
			const lo = domain?.lo ?? centre - spread;
			const span = Math.max((domain?.hi ?? centre + spread) - lo, 1e-12);
			colourOf = (v: number) => {
				const strength = priceFieldColour(v, spread, 130 * fade);
				if (!strength) return null;
				const [r, g, b] = scalarColor('price', Math.min(1, Math.max(0, (v + centre - lo) / span)));
				return [r, g, b, strength[3]];
			};
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
	 * symbols are legible; side by side when a bus has both. Nearly every bus has load, so loads
	 * wait for the full equipment detail and stay quieter than generators: they are context, not
	 * the result. Both fade with buses outside a highlighted voltage level. */
	const attachments = $derived.by(() => {
		if (span < 35 || (parts && parts.unit * 2 ** camera.zoom < 3)) return null;
		const unit = 7 / 2 ** stepZoom;
		const points: NonNullable<TxScene['attachments']>['points'] = [];
		for (let i = 0; i < graph.busId.length; i++) {
			const gen = graph.genMw[i] > 0;
			const load =
				symbolsOn &&
				(transferOn ? modelDemand(network.buses[i], caseDemandDeltas) : graph.demandMw[i]) > 0;
			if (!gen && !load) continue;
			const x = parts ? parts.busbars[i * 4 + 2] : geometry.positions[i * 3];
			const y = parts ? parts.busbars[i * 4 + 3] : geometry.positions[i * 3 + 1];
			const dx = gen && load ? 1.5 * unit : 0;
			const dim = focusEntry >= 0 && legend.entryOf(graph.kv[i]) !== focusEntry;
			if (gen) points.push({ x: x - dx, y, rootX: x, bus: i, kind: 'generator', dim });
			if (load) points.push({ x: x + dx, y, rootX: x, bus: i, kind: 'load', dim });
		}
		return points.length ? { points, unit } : null;
	});

	let equipmentOpen = $state(false);
	let selectedEquipment = $state('');
	const equipmentInventory = $derived(equipmentItems(network));
	const auxiliaryEquipment = $derived(
		equipmentDrawing(network, graph, geometry, parts, anchors, 7 / 2 ** stepZoom, symbolsOn)
	);
	const phaseShifters = $derived(
		new Set(
			(network.equipment?.transformers ?? [])
				.filter((t) => t.phase_shifter)
				.flatMap((t) => {
					const row = graph.branchIndex.get(t.branch);
					return row === undefined ? [] : [row];
				})
		)
	);
	async function selectEquipment(key: string) {
		selectedEquipment = key;
		equipmentOpen = true;
		const item = equipmentInventory.find((i) => i.key === key);
		if (item?.branch !== undefined) await inspectResult('branch', item.branch);
		else if (item?.bus !== undefined) await inspectResult('bus', item.bus);
		if (panels.compact && panels.drawer === 'network') panels.drawer = null;
	}

	const selectedBus = $derived(
		app.selectedBus === null ? -1 : (graph.index.get(app.selectedBus) ?? -1)
	);
	const selectedEdge = $derived(
		app.selectedBranch === null ? -1 : (graph.branchIndex.get(app.selectedBranch) ?? -1)
	);

	const internalStarRows = $derived(
		new Set(
			(network.equipment?.three_winding ?? []).flatMap((t) => {
				const row = t.star_bus === null ? undefined : graph.index.get(t.star_bus);
				return row === undefined ? [] : [row];
			})
		)
	);
	const threeWindingBranches = $derived(
		new Set((network.equipment?.three_winding ?? []).flatMap((t) => t.branches))
	);
	const transformerEdges = $derived(
		network.branches.flatMap((_, k) =>
			!threeWindingBranches.has(network.branches[k].id) &&
			(graph.transformer[k] || phaseShifters.has(k)) &&
			graph.from[k] >= 0 &&
			graph.to[k] >= 0
				? [k]
				: []
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
	/** A line's label state: at its limit is red, as the line is drawn. */
	const branchState = (k: number): 'normal' | 'warning' | 'violation' | null => {
		const state = loadingStates[k];
		return state === 'at-limit'
			? 'violation'
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
	const TIER_NAMES = Object.fromEntries(
		TX_TIERS.map((t) => [t.tier, t.name.charAt(0).toUpperCase() + t.name.slice(1)])
	);

	/** Width of the summary docked at the right edge on a wide screen, beside the drawing. */
	const SUMMARY_WIDTH = 340;
	const summaryShown = $derived((colour !== 'level' || levelsPresent.length > 0) && !equipmentOpen);
	/** The toolbar's width on one wide screen: two rows of views and drawing controls. */
	const TOOLBAR_WIDTH = 600;
	/** The width between the left and right docks. */
	const freeWidth = $derived(
		width -
			(panels.dockWidth('left') + 24) -
			(panels.dockWidth('right') ? panels.dockWidth('right') + 24 : 12)
	);
	/** The summary docks at the right edge when the toolbar still fits beside it; between two
	 * open panels it sits under the toolbar instead. */
	const summaryDocked = $derived(
		!panels.compact && summaryShown && freeWidth >= TOOLBAR_WIDTH + SUMMARY_WIDTH + 16
	);
	/** On a wide screen, an undocked summary sits under the toolbar at the left. */
	const summaryBelow = $derived(!panels.compact && summaryShown && !summaryDocked);
	/** Screen insets no panel, toolbar or summary covers. */
	const freeArea = () =>
		freeInsets({
			compact: panels.compact,
			top:
				panels.top +
				Math.max(0, toolbarHeight - 48) +
				((panels.compact && (resultsOn || economicsOn || transferOn)) || summaryBelow
					? legendHeight + 8
					: 0),
			dockWidth: (side) =>
				panels.dockWidth(side) +
				(side === 'right' && summaryDocked
					? SUMMARY_WIDTH + (panels.dockWidth('right') ? 16 : 0)
					: 0),
			sheetInset: app.sheetInset
		});
	/** The deepest zoom for this drawing, whatever its units (see `maxZoomFor`). */
	const maxZoom = $derived(maxZoomFor(geometry.bounds));
	const fitAll = () => fitBounds(geometry.bounds, { width, height }, freeArea(), maxZoom);

	function setCamera(next: DiagramCamera, fly = false) {
		camera = next;
		const animate = fly && kit && !matchMedia('(prefers-reduced-motion: reduce)').matches;
		deck?.setProps({
			initialViewState: {
				...next,
				minZoom: MIN_ZOOM,
				maxZoom,
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
					Math.min(maxZoom, Math.max(camera.zoom, legible))
				)
			: revealPoint(p, camera, view, area, legible, maxZoom);
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
		const physicalBusColours = internalStarRows.size ? busColours.slice() : busColours;
		for (const row of internalStarRows) physicalBusColours[row * 4 + 3] = 0;
		return {
			internalStarRows,
			equipment: auxiliaryEquipment,
			phaseShifters,
			graph,
			geometry,
			edgeColours: edgePaint.colours,
			edgeWidths: edgePaint.widths,
			busColours: physicalBusColours,
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
		if (kit)
			deck?.setProps({
				layers: txLayers(kit, scene(), morph ? [] : branchLayers)
			});
	}

	function valueText(i: number): { label: string; text: string } | null {
		if (transferOn) {
			const bus = network.buses[i];
			const generation = transferSolution ? dispatch.get(bus.id) : undefined;
			const demand = modelDemand(bus, caseDemandDeltas);
			const parts = [];
			if (generation !== undefined) parts.push(`Generation ${generation.toFixed(1)} MW`);
			if (demand !== 0) parts.push(`Load ${demand.toFixed(1)} MW`);
			return parts.length ? { label: 'Power', text: parts.join(', ') } : null;
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
		const name = (i: number) =>
			internalStarRows.has(i)
				? `Analysis star ${network.buses[i].id}`
				: (network.buses[i].name ?? `Bus ${network.buses[i].id}`);
		const candidates: (LabelCandidate & { position: [number, number] })[] = [];
		const at = (i: number): [number, number] => [
			geometry.positions[i * 3],
			geometry.positions[i * 3 + 1]
		];
		if (selectedBus >= 0) {
			const p = at(selectedBus);
			const v = resultsOn ? valueText(selectedBus) : null;
			candidates.push({
				text: v ? `${name(selectedBus)}  ${v.text}` : name(selectedBus),
				state: 'normal',
				priority: 1e9,
				screen: screen(...p),
				position: p,
				selected: true
			});
		}
		if (namesOn && !parts)
			for (let i = 0; i < graph.busId.length; i++) {
				if (i === selectedBus || internalStarRows.has(i)) continue;
				const p = at(i);
				const sp = screen(...p);
				if (!inView(sp)) continue;
				const transfer = transferOn ? valueText(i) : null;
				candidates.push({
					text: transfer ? `${name(i)}  ${transfer.text}` : name(i),
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
					if (internalStarRows.has(i)) continue;
					// Off-screen busbars first, before building any text.
					const p: [number, number] = [parts.busbars[i * 4], parts.busbars[i * 4 + 1]];
					const sp = screen(...p);
					if (!inView(sp)) continue;
					const transfer = transferOn ? valueText(i) : null;
					const level = graph.kv[i] > 0 ? levelLabel(graph.kv[i] * 1000) : '';
					if (!level && !transfer) continue;
					candidates.push({
						text: transfer ? [level, transfer.text].filter(Boolean).join(', ') : level,
						state: 'normal',
						priority: -1,
						screen: sp,
						position: p
					});
				}
		}
		if (colour === 'results' && resultsOn && symbolsOn)
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
					text: `${(f.loading * 100).toFixed(0)}%`,
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
					text: `Bus ${graph.busId[i]}  ${marker.kind === 'deenergized' ? 'De-energized' : v !== undefined ? `${v.toFixed(3)} pu` : marker.kind}`,
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
					text: magnitude === undefined ? '—' : `${magnitude.toFixed(1)} MW`,
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
		const addGauge = (edge: number, selected: boolean) => {
			const [x, y] = screen(anchors[edge * 4], anchors[edge * 4 + 1]);
			if (!Number.isFinite(x) || !Number.isFinite(y)) return;
			const loading = flows.get(network.branches[edge].id)!.loading;
			gaugeCandidates.push({
				edge,
				ax: x,
				ay: y,
				loading,
				state: loadingStates[edge],
				selected
			});
		};
		const isPointed = (edge: number) => edge === selectedEdge || edge === hoveredEdge;
		// Gauges mark the flagged lines, the same set the panel counts; any other line shows its
		// gauge only while selected or hovered.
		for (const edge of gaugePool.busy) {
			const selected = isPointed(edge);
			if (!selected && loadingStates[edge] === 'normal') continue;
			addGauge(edge, selected);
		}
		// A quiet branch shows its gauge only while selected or hovered.
		for (const edge of new Set([selectedEdge, hoveredEdge]))
			if (edge >= 0 && gaugePool.quiet.has(edge)) addGauge(edge, true);
		if (!symbolsOn && landmarksPref.value && landmarkPool.length) {
			const placed = placeLandmarks(landmarkPool, screen, { width, height });
			marks = { placed, strokes: landmarkStrokes(placed, 8 / scale) };
		} else marks = null;
		// Gauges stay clear of the generator and transformer symbols where they can.
		const obstacles = (marks?.placed ?? []).map((m) => screen(m.x, m.y));
		gauges = placeOperatingGauges(
			gaugeCandidates,
			{
				left: inset.left,
				top: inset.top,
				right: width - inset.right,
				bottom: height - inset.bottom
			},
			panels.compact ? 10 : 32,
			obstacles
		);
	}

	/** The bus row or branch a deck.gl pick stands for, from whichever layer was hit; -1 for
	 * neither. A landmark stands for its branch, or its bus when it marks a generator. */
	function picked(
		info: PickingInfo
	): { bus: number; edge: number; landmark: Landmark | null } | null {
		if (info.index < 0) return null;
		const at = (bus: number, edge: number, landmark: Landmark | null = null) => ({
			bus,
			edge,
			landmark
		});
		switch (info.layer?.id) {
			case 'buses':
			case 'busbars':
				return at(info.index, -1);
			case 'attachments-pick':
				return at(info.object.bus, -1);
			case 'branches':
				return at(-1, geometry.segmentEdge[info.index]);
			case 'transformers':
				return at(-1, transformers?.edges[info.index] ?? -1);
			case 'landmark-halos': {
				const l = info.object as Landmark;
				return at(l.edge >= 0 ? -1 : l.bus, l.edge, l);
			}
			default:
				return null;
		}
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
				// Open where the map was when it shows part of this case at a useful size; otherwise
				// fit the drawing to the free area.
				const fromMap =
					geometry.mercatorOrigin && app.camera
						? cameraFromMap(app.camera, geometry.mercatorOrigin)
						: null;
				camera = openingCamera(fromMap, fitAll(), (c) =>
					showsAnyPosition(geometry.positions, c, { width, height }, freeArea())
				);
				deck = new core.Deck({
					canvas: element,
					width: '100%',
					height: '100%',
					views: new core.OrthographicView({ id: 'transmission', flipY: true }),
					initialViewState: { ...camera, minZoom: MIN_ZOOM, maxZoom },
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
						const t = picked(info);
						// A landmark highlights its branch, never its bus.
						hoveredBus = t && !t.landmark ? t.bus : -1;
						hoveredEdge = t?.edge ?? -1;
					},
					onClick: (info: PickingInfo) => {
						if (info.layer?.id.startsWith('equipment-') && info.object?.key) {
							selectEquipment(info.object.key);
							return;
						}
						if (info.index < 0 || !info.layer?.id) {
							onclear();
							return;
						}
						pickedHere = true;
						setTimeout(() => (pickedHere = false));
						const t = picked(info);
						if (!t) return;
						if (t.edge >= 0) onselectbranch(network.branches[t.edge].id);
						else if (t.bus >= 0) onselectbus(graph.busId[t.bus]);
						if (t.landmark) {
							pickedHere = false;
							bringIntoView([t.landmark.x, t.landmark.y], true);
						}
					},
					getTooltip: (info: PickingInfo) => {
						const t = picked(info);
						if (t && t.edge >= 0) return { text: branchTooltip(t.edge) };
						if (t && t.bus >= 0) return { text: busTooltip(t.bus) };
						return null;
					}
				});
				status = null;
			})
			.catch((e: unknown) => {
				console.error(e);
				status = 'Diagram failed';
			});
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

	let morph: Morph | null = null;
	let showSeq = 0;
	/** Show a layout. Buses glide from where they are on screen to the new layout along straight
	 * chords, as in the multiconductor schematic; a warm-up frame uploads the target first. */
	async function show(next: Layout) {
		const seq = ++showSeq;
		const slow = setTimeout(() => {
			if (seq === showSeq) status = 'Laying out…';
		}, 150);
		let drawing: {
			geometry: SchematicGeometry;
			parts: TxSchematicParts | null;
		};
		try {
			drawing = await target(next);
		} catch (e) {
			console.error(e);
			status = 'Layout failed';
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
			: fitBounds(
					drawing.geometry.bounds,
					{ width, height },
					freeArea(),
					maxZoomFor(drawing.geometry.bounds)
				);
		setCamera(to);
		const reduced = matchMedia('(prefers-reduced-motion: reduce)').matches;
		if (!deck || reduced || graph.busId.length > MORPH_MAX_BUSES) return;
		if (morph) cancelAnimationFrame(morph.raf);
		const endpoints = Int32Array.from(
			network.branches.flatMap((_, k) =>
				graph.from[k] >= 0 && graph.to[k] >= 0 ? [graph.from[k], graph.to[k]] : []
			)
		);
		morph = startMorph(before, drawing.geometry.positions, endpoints, from, to, (m) => {
			morph = m;
			render();
		});
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
			auxiliaryEquipment,
			phaseShifters,
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

	// The summary moves between the right edge and the space under the toolbar as side panels
	// open and close; it then covers a different part of the drawing, so the drawing refits to
	// the area left free (after a frame, once the summary's new height is measured).
	let summaryWasBelow: boolean | null = null;
	$effect(() => {
		const below = summaryBelow;
		if (!deck) return;
		const changed = summaryWasBelow !== null && summaryWasBelow !== below;
		summaryWasBelow = below;
		if (!changed) return;
		const frame = requestAnimationFrame(() => untrack(() => setCamera(fitAll())));
		return () => cancelAnimationFrame(frame);
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
	$effect(() => {
		if (selectedBus >= 0 || selectedEdge >= 0) untrack(() => (brief = false));
	});
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
								const k = graph.branchIndex.get(target.branchId) ?? -1;
								return k >= 0 ? [graph.from[k], graph.to[k]] : [];
							})();
				const b = busBounds(
					geometry.positions,
					rows.filter((r) => r >= 0),
					typical
				);
				if (b)
					setCamera(
						fitBounds(b, { width, height }, freeArea(), Math.min(maxZoom, camera.zoom + 2)),
						true
					);
			}
			app.settleFrame();
		});
	});

	function zoomBy(delta: number) {
		setCamera({
			target: camera.target,
			zoom: Math.max(MIN_ZOOM, Math.min(maxZoom, camera.zoom + delta))
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

	/** The diagram's views, always visible so users can see what the diagram offers. */
	const VIEWS: { value: Colour; label: string }[] = [
		{ value: 'level', label: 'Voltage' },
		{ value: 'operating', label: 'Limits' },
		{ value: 'economics', label: 'Prices' },
		{ value: 'transfer', label: 'Flows' },
		{ value: 'results', label: 'Bus values' }
	];
	/** Arrow keys move between views, as in any radio group. */
	function viewKey(e: KeyboardEvent) {
		const step =
			e.key === 'ArrowRight' || e.key === 'ArrowDown'
				? 1
				: e.key === 'ArrowLeft' || e.key === 'ArrowUp'
					? -1
					: 0;
		if (!step) return;
		e.preventDefault();
		const i = VIEWS.findIndex((v) => v.value === colour);
		colour = VIEWS[(i + step + VIEWS.length) % VIEWS.length].value;
		tick().then(() =>
			(e.currentTarget as HTMLElement | null)
				?.querySelector<HTMLElement>('[aria-checked="true"]')
				?.focus()
		);
	}
	const toolLeft = $derived(panels.compact ? 12 : panels.dockWidth('left') + 24);
	const layoutName = $derived(
		layout === 'schematic' ? 'Schematic' : geographic ? 'Geographic' : 'Drawing'
	);
	async function exportPng() {
		if (!deck || !canvas) return;
		const title = titleBlock({
			dataset: label,
			layout: layoutName,
			view: operatingOn
				? `Limits, ${calculationName}`
				: economicsOn
					? `Prices, ${calculationName}`
					: transferOn
						? `Flows, ${calculationName}`
						: resultsOn
							? `${display?.copy ?? 'Bus values'}, ${calculationName}`
							: 'Voltage',
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
			const over = gauge.state === 'violation' || gauge.state === 'at-limit';
			const colour = over ? '#d7261e' : gauge.state === 'warning' ? '#e3a008' : '#4a5563';
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
			ctx.fillStyle = over ? '#d7261e' : '#ffffff';
			ctx.fillRect(x, y, GAUGE_WIDTH, GAUGE_HEIGHT);
			ctx.lineWidth = 1;
			ctx.strokeRect(x, y, GAUGE_WIDTH, GAUGE_HEIGHT);
			ctx.font = `${over ? '600 ' : ''}12px "IBM Plex Sans", sans-serif`;
			ctx.textAlign = 'center';
			ctx.textBaseline = 'middle';
			ctx.fillStyle = over ? '#ffffff' : gauge.state === 'warning' ? '#8a5a00' : '#18202b';
			ctx.fillText(gaugeText(gauge.loading), x + GAUGE_WIDTH / 2, y + GAUGE_HEIGHT / 2);
		}
		ctx.restore();
		const lines = transferOn
			? [`Flows, ${calculationName}`, `Width 0 to ${transferScale} MW`]
			: [
					`Limits, ${calculationName}`,
					...(resultContext.voltage ? ['Voltage 0.90 to 1.10 pu'] : []),
					`Loading: amber from ${warningPref.value}%, red at 100%`
				];
		ctx.font = '12px "IBM Plex Sans", sans-serif';
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
		if (!voltageView) return '';
		const s = stationSummaries[graph.substation[i]];
		return [
			s.min && s.max ? `Range: ${s.min.value.toFixed(4)}–${s.max.value.toFixed(4)} pu` : '',
			s.worst
				? `Worst: bus ${s.worst.bus}, ${s.worst.assessment.state === 'low' ? '−' : '+'}${s.worst.assessment.exceedance.toFixed(4)} pu`
				: ''
		]
			.filter(Boolean)
			.join('\n');
	}
	function busTooltip(i: number): string {
		return [
			txBusText(graph, network, i, valueText(i)),
			voltageView && network.buses[i].vmin_pu != null && network.buses[i].vmax_pu != null
				? `Limits: ${network.buses[i].vmin_pu}–${network.buses[i].vmax_pu} pu`
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
		const kv = (bus: number) => (bus >= 0 ? graph.kv[bus] : '—');
		return [
			`${graph.transformer[k] ? 'Transformer' : 'Line'} ${network.branches[k].id}: ${network.branches[k].from}–${network.branches[k].to}`,
			`kV: ${kv(graph.from[k])} / ${kv(graph.to[k])}`,
			f && (!transferOn || transferReadings[k].from !== undefined)
				? `P: ${f.mw.toFixed(2)} MW${transferOn ? `, ${transferDirectionText(transferReadings[k], network.branches[k])}` : ''}`
				: '',
			...(economicsOn
				? [`Status: ${thermalStatus(economics.branches.get(network.branches[k].id))}`]
				: []),
			hasStatedRating(info) && f && isRated(loadingStates[k])
				? `Loading: ${(f.loading * 100).toFixed(1)}% of ${info!.rating} ${info!.basis.toUpperCase()}`
				: `Loading: ${loadingStateText[loadingStates[k]]}`
		]
			.filter(Boolean)
			.join('\n');
	}
	const rgb = (c: RGBA) => `rgb(${c.slice(0, 3).join(' ')})`;
	const casingColour = (c: RGBA): RGBA => [
		Math.round(c[0] * CASING_SHADE),
		Math.round(c[1] * CASING_SHADE),
		Math.round(c[2] * CASING_SHADE),
		c[3]
	];
</script>

{#snippet voltageKey()}
	<section class="voltage-key" aria-label="Voltage">
		<div class="key-heading">
			<strong>Voltage</strong>
			<a
				class="docs"
				href={docsLink(uiConfig.docsHref, 'voltage')}
				target="_blank"
				rel="noreferrer"
				aria-label="Voltage docs">Docs</a
			>
		</div>
		{#if graph.grouped && !brief}
			<StatBoxes
				label="Network size"
				stats={[
					{ label: 'Substations', value: graph.substations.length.toLocaleString('en-US') },
					{ label: 'Buses', value: graph.busId.length.toLocaleString('en-US') }
				]}
			/>
		{/if}
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
		</ul>
		{#if !brief}
			<ul class="symbols" aria-label="Equipment">
				{#if transformerEdges.length}
					<li>
						<svg viewBox="-14 -12 28 24" width="28" height="24" aria-hidden="true"
							><path d={glyphPath(landmarkGlyph('transformer'), 7)} /></svg
						>Transformer
					</li>
				{/if}
				{#if outOfService}
					<li>
						<svg viewBox="-2 -12 28 24" width="28" height="24" aria-hidden="true"
							><path d="M-2 0H26" stroke-dasharray="4 3" /></svg
						>Out of service
					</li>
				{/if}
				<li>
					<svg viewBox="-2 -12 28 24" width="28" height="24" aria-hidden="true"
						><path d={glyphPath(attachmentGlyph('load'), 10)} class="load-key" /></svg
					>Load
				</li>
				<li>
					<svg viewBox="-2 -12 28 24" width="28" height="24" aria-hidden="true"
						><path d={glyphPath(attachmentGlyph('generator'), 9)} /></svg
					>Generator
				</li>
				{#if landmarkPool.length}
					<li>
						<svg viewBox="-14 -12 28 24" width="28" height="24" aria-hidden="true"
							><path d={glyphPath(landmarkGlyph('generator'), 7)} /></svg
						>Power plant
					</li>
				{/if}
			</ul>
			{#if landmarkPool.length}
				<label class="setting-toggle"
					>Show power plants<input
						type="checkbox"
						checked={landmarksPref.value}
						onchange={(e) => landmarksPref.set(e.currentTarget.checked)}
					/></label
				>
			{/if}
		{/if}
	</section>
{/snippet}

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
		class:compact={panels.compact}
		bind:clientHeight={toolbarHeight}
		style:max-width="calc(100% - {toolLeft +
			(panels.compact
				? 12
				: panels.dockWidth('right') + 24 + (summaryDocked ? SUMMARY_WIDTH + 16 : 0))}px)"
		style:top="{panels.top + 8}px"
		style:left="{toolLeft}px"
		role="toolbar"
		aria-label="Single-line diagram"
	>
		{#if panels.compact}
			<select class="view-select" aria-label="Diagram view" bind:value={colour}>
				{#each VIEWS as v (v.value)}<option value={v.value}>{v.label}</option>{/each}
			</select>
			<button
				aria-expanded={equipmentOpen}
				aria-controls="tx-equipment-panel"
				onclick={() => (equipmentOpen = !equipmentOpen)}>Equipment</button
			>
		{:else}
			<div
				class="segmented views"
				role="radiogroup"
				aria-label="Diagram view"
				tabindex="-1"
				onkeydown={viewKey}
			>
				{#each VIEWS as v (v.value)}
					<button
						role="radio"
						aria-checked={colour === v.value}
						tabindex={colour === v.value ? 0 : -1}
						onclick={() => (colour = v.value)}>{v.label}</button
					>
				{/each}
			</div>
		{/if}
		{#if panels.compact}
			<!-- On a phone the drawing controls wait behind Display, so the toolbar is one row. -->
			<div class="mobile-view">
				<button
					aria-expanded={displayOptions}
					aria-controls="tx-display-options"
					onclick={() => (displayOptions = !displayOptions)}>Display</button
				>
				<button onclick={onclose}>Show map</button>
			</div>
		{/if}
		{#if !panels.compact || displayOptions}
			<div class="controls" id={panels.compact ? 'tx-display-options' : undefined}>
				{#if panels.compact}
					<select
						aria-label="Diagram layout"
						value={layout}
						onchange={(e) => show(e.currentTarget.value as Layout)}
					>
						<option value="source">{geographic ? 'Geographic' : 'Drawing'}</option>
						<option value="schematic">Schematic</option>
					</select>
				{:else}
					<div class="segmented" role="group" aria-label="Layout">
						<button aria-pressed={layout === 'source'} onclick={() => show('source')}
							>{geographic ? 'Geographic' : 'Drawing'}</button
						>
						<button aria-pressed={layout === 'schematic'} onclick={() => show('schematic')}
							>Schematic</button
						>
					</div>
					<button
						aria-expanded={equipmentOpen}
						aria-controls="tx-equipment-panel"
						onclick={() => (equipmentOpen = !equipmentOpen)}>Equipment</button
					>
				{/if}
				<button onclick={() => setCamera(fitAll())} title="Fit the drawing (F)">Fit</button>
				<button onclick={() => zoomBy(-0.5)} aria-label="Zoom out" title="Zoom out">−</button>
				<button onclick={() => zoomBy(0.5)} aria-label="Zoom in" title="Zoom in">+</button>
				<div class="display-options" id={panels.compact ? undefined : 'tx-display-options'}>
					{#if geographic}
						<label class="check"
							><input
								type="checkbox"
								checked={basemapPref.value}
								onchange={(e) => basemapPref.set(e.currentTarget.checked)}
							/> Basemap</label
						>
					{/if}
					<!-- Always present, so the toolbar keeps one width from view to view. -->
					<label class="check" class:off={!fieldKind}
						><input
							type="checkbox"
							checked={fieldPref.value}
							disabled={!fieldKind}
							onchange={(e) => fieldPref.set(e.currentTarget.checked)}
						/> Shading</label
					>
					<button onclick={exportPng}>Save image</button>
				</div>
			</div>
		{/if}
	</div>
	{#if equipmentOpen}
		<div
			id="tx-equipment-panel"
			class="equipment-overlay"
			style:top="{panels.top + toolbarHeight + 16}px"
			style:left="{toolLeft}px"
			style:max-width="calc(100% - {toolLeft + 12}px)"
			style:max-height="calc(100% - {panels.top +
				toolbarHeight +
				32 +
				Math.max(app.sheetInset, panels.bottom)}px)"
		>
			<TxEquipmentPanel
				moduleJson={app.activeLocal?.studyInputJson ?? undefined}
				{formulation}
				{network}
				selected={selectedEquipment}
				onselect={selectEquipment}
				onclose={() => (equipmentOpen = false)}
			/>
		</div>
	{/if}
	{#if summaryShown}
		<div
			class="legend panel-select"
			class:operating={panelView}
			bind:clientHeight={legendHeight}
			class:docked={summaryDocked}
			style:top="{summaryDocked ? panels.top + 8 : panels.top + toolbarHeight + 16}px"
			style:left={summaryDocked ? undefined : `${toolLeft}px`}
			style:right={summaryBelow
				? undefined
				: `${panels.compact ? 12 : panels.dockWidth('right') + (panels.dockWidth('right') ? 32 : 16)}px`}
			style:width={panels.compact ? undefined : `${Math.min(SUMMARY_WIDTH, freeWidth)}px`}
			style:max-height={summaryDocked
				? `calc(100% - ${panels.top + 8 + panels.bottom}px)`
				: `min(38vh, calc(100% - ${panels.top + toolbarHeight + 32 + app.sheetInset}px))`}
		>
			{#if colour === 'level' || panelView}
				<button
					class="collapse"
					aria-expanded={!brief}
					aria-label={brief ? 'Show details' : 'Hide details'}
					title={brief ? 'Show details' : 'Hide details'}
					onclick={() => (brief = !brief)}
					><svg width="12" height="12" viewBox="0 0 12 12" aria-hidden="true"
						><path
							d={brief ? 'm3 4.5 3 3 3-3' : 'm3 7.5 3-3 3 3'}
							fill="none"
							stroke="currentColor"
							stroke-width="1.5"
						/></svg
					></button
				>
			{/if}
			{#if colour === 'level'}
				{@render voltageKey()}
			{:else}
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
						{brief}
						{onselectbus}
					/>
				{:else if economicsOn}
					<TxEconomicsPanel
						{formulation}
						context={resultContext}
						{economics}
						scale={priceScale}
						onscale={(scale) => (priceScale = scale)}
						domain={domain ?? PRICE_SCALES[0]}
						bus={network.buses[selectedBus]}
						branch={network.branches[selectedEdge]}
						busPrice={busValues.get(graph.busId[selectedBus])}
						flow={flows.get(network.branches[selectedEdge]?.id)}
						onbranch={inspectListedBranch}
						onpreview={setPreview}
						{brief}
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
						{brief}
					/>
				{:else if !solution}
					No result
				{:else if display?.mode === 'voltage'}
					Voltage <span>0.90</span><i style:background={voltageGradient}></i><span>1.10 pu</span>
				{:else if display && uniform !== null}
					{display.label}: {uniform.toPrecision(4)}
					{display.unit}
				{:else if display && domain}
					{display.label} <span>{displayFmt(display.mode, domain.lo)}</span><i
						style:background={display.gradient}
					></i><span>{displayFmt(display.mode, domain.hi)} {display.unit}</span>
				{/if}
				{#if panelView}
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
						{brief}
						{onclear}
						onframe={frameSelection}
					/>
				{:else if solution}
					<ul class="loading-key" aria-label="Line loading">
						<li><i class="swatch normal"></i>Below {warningPref.value}%</li>
						<li><i class="swatch near"></i>Near limit</li>
						<li><i class="swatch over"></i>At limit</li>
						<li><i class="swatch unrated"></i>Unrated</li>
					</ul>
				{/if}
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
						class:violation={gauge.state === 'violation' || gauge.state === 'at-limit'}
						class:warning={gauge.state === 'warning'}
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
					class:over={gauge.state === 'violation' || gauge.state === 'at-limit'}
					class:warning={gauge.state === 'warning'}
					style:left="{gauge.x}px"
					style:top="{gauge.y}px"
					title={branchTooltip(gauge.edge)}
					aria-pressed={gauge.edge === selectedEdge}
					aria-label="Branch {branch.id}, {(gauge.loading * 100).toFixed(
						1
					)} percent, {loadingStateText[gauge.state]}"
					onclick={() => onselectbranch(branch.id)}
				>
					<span class="pill">{gaugeText(gauge.loading)}</span>
				</button>
			{/each}
		</div>
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
	.equipment-overlay {
		overflow: auto;
		position: absolute;
		z-index: 12;
		width: 370px;
	}
	.mobile-view {
		display: contents;
	}
	.loading-key {
		display: flex;
		flex-wrap: wrap;
		gap: 4px 14px;
		margin: 0;
		padding: 0;
		list-style: none;
	}
	.loading-key li {
		display: flex;
		align-items: center;
		gap: 6px;
	}
	.loading-key .swatch {
		width: 18px;
		height: 3px;
		border-radius: 2px;
	}
	.swatch.normal {
		background: rgb(158 153 144);
	}
	.swatch.near {
		background: var(--amber);
	}
	.swatch.over {
		background: var(--red);
	}
	.swatch.unrated {
		background: rgb(155 150 140 / 0.45);
	}
	.toolbar select {
		font: inherit;
		background: var(--paper);
		color: var(--ink);
		border: 1px solid var(--line);
		border-radius: var(--radius-control);
		padding: 4px;
	}
	/* Phone: one row. The view select is the panels' shared select at touch size;
	   the buttons beside it are quiet text buttons of the same height and size. */
	.toolbar.compact {
		gap: 2px;
		font-size: 15px;
	}
	.toolbar.compact select {
		appearance: none;
		-webkit-appearance: none;
		min-width: 0;
		height: 44px;
		padding: 0 26px 0 8px;
		font-size: 16px;
		background-color: var(--surface-control);
		background-image:
			linear-gradient(45deg, transparent 50%, var(--ink-dim) 50%),
			linear-gradient(135deg, var(--ink-dim) 50%, transparent 50%);
		background-position:
			right 12px center,
			right 8px center;
		background-size:
			4px 4px,
			4px 4px;
		background-repeat: no-repeat;
		text-overflow: ellipsis;
	}
	.toolbar.compact .view-select {
		flex: 1 1 0;
	}
	.toolbar.compact button {
		height: 44px;
		padding: 0 8px;
		white-space: nowrap;
	}
	.toolbar.compact button[aria-expanded='true'] {
		color: var(--text-accent);
		background: var(--accent-soft);
	}
	.compact .controls {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: 2px 4px;
		width: 100%;
		padding: 4px 0 0;
		margin-top: 4px;
		border-top: 1px solid var(--line);
	}
	.display-options {
		display: flex;
		align-items: center;
		gap: 8px;
		flex-wrap: wrap;
	}

	/* The multiconductor schematic's look, so both diagrams read as one tool. */
	.tx-stage {
		position: absolute;
		inset: 0;
		background: var(--bg);
		color: var(--ink);
		/* Limit labels on the drawing: dark amber on light, light amber on dark. */
		--gauge-bg: color-mix(in srgb, var(--raised) 94%, transparent);
		--gauge-line: var(--line);
		--gauge-ink: var(--ink);
		--gauge-warn: #8a5a00;
	}
	:global(:root[data-theme='dark']) .tx-stage {
		--gauge-warn: #f2c14e;
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
	/* Two rows with two weights: the views read as tabs, the drawing controls
	   below them as quiet buttons. */
	.toolbar {
		position: absolute;
		width: max-content;
		right: 12px;
		display: flex;
		flex-direction: column;
		align-items: stretch;
		gap: 0;
		padding: 0;
		font-size: var(--fs-label);
		background: var(--surface-panel, var(--paper));
		border: 1px solid var(--line);
		border-radius: var(--radius-panel);
		box-shadow: var(--elev-1);
	}
	.toolbar.compact {
		flex-direction: row;
		flex-wrap: wrap;
		align-items: center;
		padding: 5px;
	}
	.controls {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: 4px;
		padding: 4px 6px;
	}
	.toolbar button {
		font: inherit;
		color: var(--ink);
		background: none;
		border: 1px solid transparent;
		border-radius: var(--radius-control);
		padding: 4px 8px;
		cursor: pointer;
	}
	.toolbar button:hover:not(:disabled) {
		background: var(--surface-hover);
	}
	.toolbar button:disabled {
		opacity: 0.45;
		cursor: default;
	}
	.toolbar .check {
		display: flex;
		gap: 4px;
		align-items: center;
	}
	.toolbar .check.off {
		color: var(--text-tertiary);
	}
	.segmented {
		display: flex;
		border: 1px solid var(--line);
		border-radius: var(--radius-control);
	}
	.segmented button {
		border-radius: 0;
		border: 0;
	}
	.segmented button:first-child {
		border-radius: 3px 0 0 3px;
	}
	.segmented button:last-child {
		border-radius: 0 3px 3px 0;
	}
	.segmented button + button {
		border-left: 1px solid var(--line);
	}
	/* The views, as tabs. When the toolbar is squeezed between panels they wrap
	   to a second row, so every view stays readable. */
	.views {
		max-width: 100%;
		flex-wrap: wrap;
		border: 0;
		border-bottom: 1px solid var(--line);
		border-radius: 0;
		padding: 0 4px;
	}
	.views button {
		position: relative;
		white-space: nowrap;
		flex: none;
		padding: 9px 10px;
		font-size: var(--fs-body);
		color: var(--text-secondary);
		border: 0;
		border-radius: 0;
	}
	.views button + button {
		border-left: 0;
	}
	.views button:hover:not(:disabled) {
		background: none;
		color: var(--ink);
	}
	.views button[aria-checked='true'] {
		color: var(--ink);
		background: none;
	}
	.views button[aria-checked='true']::after {
		content: '';
		position: absolute;
		left: 10px;
		right: 10px;
		bottom: -1px;
		height: 2px;
		background: var(--accent);
	}
	.views button:focus-visible {
		outline-offset: -3px;
	}
	.segmented button[aria-pressed='true'] {
		background: var(--accent-soft);
		color: var(--text-accent);
	}
	.gauges {
		position: absolute;
		inset: 0;
		pointer-events: none;
	}
	.gauges.stale {
		opacity: 0.4;
	}
	/* One style per severity: red fill at or over the rating, amber outline near it. The
	   button is the hit area; on a touch screen it grows to 44px around the same label. */
	.utilization-gauge {
		position: absolute;
		width: 54px;
		height: 22px;
		display: flex;
		align-items: center;
		padding: 0;
		border: 0;
		background: none;
		cursor: pointer;
		pointer-events: auto;
	}
	.pill {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 100%;
		height: 22px;
		box-sizing: border-box;
		background: var(--gauge-bg);
		border: 1px solid var(--gauge-line);
		border-radius: var(--radius-control);
		color: var(--gauge-ink);
		font: 12px var(--font-sans);
		font-variant-numeric: tabular-nums;
	}
	.utilization-gauge.warning .pill {
		color: var(--gauge-warn);
		border-color: var(--amber);
	}
	.utilization-gauge.over .pill {
		color: #fff;
		background: var(--red);
		border-color: var(--red);
		font-weight: 600;
	}
	.utilization-gauge[aria-pressed='true'] .pill {
		outline: 2px solid var(--accent);
		outline-offset: 1px;
	}
	.utilization-gauge:focus-visible {
		outline: none;
	}
	.utilization-gauge:focus-visible .pill {
		outline: 2px solid var(--focus-ring);
		outline-offset: 1px;
	}
	@media (pointer: coarse) {
		.utilization-gauge {
			height: 44px;
			margin-top: -11px;
		}
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
		stroke: var(--gauge-warn);
	}
	.leaders .warning circle {
		fill: var(--gauge-warn);
	}
	.leaders .violation line {
		stroke: var(--red);
	}
	.leaders .violation circle {
		fill: var(--red);
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
		padding: 6px 10px;
		font: var(--fs-label) / 1.5 var(--font-sans);
		color: var(--text-secondary);
		/* Chromium's headless shell can occlude the canvases past the scroll clip with a solid colour. */
		background: linear-gradient(var(--paper), var(--paper));
		border: 1px solid var(--line);
		border-radius: var(--radius-panel);
	}
	/* Docked at the right edge beside the drawing, never over it. */
	.legend.docked {
		align-content: flex-start;
		box-shadow: var(--elev-1);
	}
	.legend.operating {
		display: flow-root;
		max-width: 720px;
	}
	/* The details toggle sits in the heading row's right corner, after the Docs link. */
	.legend .collapse {
		position: absolute;
		top: 6px;
		right: 8px;
		display: grid;
		place-items: center;
		width: 24px;
		height: 24px;
		padding: 0;
		color: var(--ink);
		background: var(--surface-control);
		border: 1px solid var(--line);
		border-radius: var(--radius-control);
		cursor: pointer;
	}
	.legend .collapse:hover {
		border-color: var(--border-strong);
	}
	.legend.operating :global(.heading) {
		min-height: 24px;
		padding-right: 32px;
	}
	@media (pointer: coarse) {
		.legend .collapse {
			width: 32px;
			height: 32px;
		}
		.legend.operating :global(.heading) {
			min-height: 32px;
			padding-right: 40px;
		}
	}
	.legend i {
		display: inline-block;
		width: 90px;
		height: 7px;
		border-radius: var(--radius-control);
	}
	.voltage-key {
		width: 100%;
		min-width: 0;
		color: var(--text-primary);
	}
	/* The same heading row as the result cards: the name, then Docs, clear of the collapse toggle. */
	.key-heading {
		display: flex;
		align-items: center;
		gap: var(--space-sm);
		min-height: 24px;
		margin-bottom: var(--space-xs);
		padding-right: 32px;
		font-size: var(--fs-label);
	}
	.key-heading strong {
		color: var(--ink);
		font-size: var(--fs-body);
	}
	.key-heading .docs {
		margin-left: auto;
		color: var(--text-accent);
		text-decoration: none;
	}
	.key-heading .docs:hover {
		text-decoration: underline;
	}
	.setting-toggle {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 12px;
		min-height: 34px;
		margin-top: 6px;
		border-top: 1px solid var(--line);
		cursor: pointer;
	}
	.setting-toggle input {
		width: 16px;
		height: 16px;
		margin: 0;
		accent-color: var(--accent);
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
		fill: rgb(134 128 118);
		stroke: rgb(134 128 118);
	}
	@media (pointer: coarse) {
		.toolbar button,
		.toolbar .check,
		.symbols .level,
		.setting-toggle {
			min-height: 44px;
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
		border-radius: var(--radius-control);
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
		font-size: 12px;
	}
	.attribution {
		position: absolute;
		margin: 0;
		padding: 1px 6px;
		font: 12px var(--font-sans);
		font-variant-numeric: tabular-nums;
		color: var(--text-secondary);
		background: color-mix(in srgb, var(--paper) 80%, transparent);
		border-radius: var(--radius-control);
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
