<script lang="ts">
	import 'maplibre-gl/dist/maplibre-gl.css';
	import { tick } from 'svelte';
	import type { Layer, PickingInfo } from '@deck.gl/core';
	import type { PathLayer, ScatterplotLayer } from '@deck.gl/layers';
	import type { MapboxOverlay } from '@deck.gl/mapbox';
	import type { LngLatBoundsLike, Map as MapLibreMap } from 'maplibre-gl';
	import type { NetworkBranch, NetworkBus } from './api.js';
	import {
		branchColor,
		branchWidth,
		busNeutral,
		busRadius,
		lmpColor,
		scalarDomain,
		sensColor,
		sensFlatColor,
		sensitivityDomain,
		type SensitivityDomain
	} from './colors.js';
	import { caseDeltas, caseRatings, displayMetaFor, displaySeriesFor } from './display.js';
	import { CaseState, type DisplayMode, type LocalCase } from './state.svelte.js';
	import { getAppState, getController } from './context.svelte.js';
	import { displayFmt, fmt } from './format.js';

	let {
		onbusclick,
		onlocalbusclick,
		onbranchclick,
		onlocalbranchclick,
		onplacecase,
		onmapclick
	}: {
		onbusclick: (caseId: string, busId: number) => void;
		onlocalbusclick: (caseId: string, busId: number) => void;
		onbranchclick?: (caseId: string, branchId: number) => void;
		onlocalbranchclick?: (caseId: string, branchId: number) => void;
		onplacecase: (lon: number, lat: number) => void;
		onmapclick: () => void;
	} = $props();

	const app = getAppState();
	const ctrl = getController();

	const STYLE = 'https://basemaps.cartocdn.com/gl/positron-nolabels-gl-style/style.json';

	let map = $state.raw<MapLibreMap | null>(null);
	let overlay = $state.raw<MapboxOverlay | null>(null);
	// Bumped to remount the map after a WebGL context is lost and cannot be
	// repainted in place. See initMap's context-loss handling.
	let mapGen = $state(0);
	// Cap automatic remounts so sustained GPU pressure cannot loop forever.
	let glRebuilds = 0;

	type LayerCtors = {
		PathLayer: typeof PathLayer;
		ScatterplotLayer: typeof ScatterplotLayer;
	};
	type CursorState = {
		isDragging: boolean;
		isHovering: boolean;
	};
	let layerCtors = $state.raw<LayerCtors | null>(null);

	interface CaseDisplay {
		lmp: Map<number, number>;
		scalar: Map<number, number>;
		scalarLo: number;
		scalarHi: number;
		scalarMode: DisplayMode;
		scalarLabel: string;
		scalarUnit: string;
		loading: Map<number, number>;
		mode: 'lmp' | 'sens' | 'preview';
		sens: Map<number, number>;
		sensDomain: SensitivityDomain | null;
		preview: Map<number, number>;
		previewDomain: SensitivityDomain | null;
		/** Fixed drag normalization (column scale × full slider deflection); the
		 * per-frame previewDomain is degree-1 homogeneous in the step, which would
		 * cancel the step out of the color and freeze the intensity. */
		previewScale: number | null;
	}
	type SolvableCase = CaseState | LocalCase;

	// Everything the accessors need, rebuilt when any case's data moves. The
	// LMP scale normalizes per network: each case is an islanded market, so
	// within network structure beats cross network color comparability (the
	// panel legend and tooltips carry the actual numbers). Preview values
	// shift individual buses without rescaling.
	const display = $derived.by(() => {
		const active = app.active ?? app.activeLocal;
		const selectedBus = app.selectedBus;
		const selectedBranch = app.selectedBranch;
		// The active case's column for the live selection: the ∂LMP/∂d column at the
		// selected bus, or the ∂LMP/∂rating column at the selected branch. Both are
		// bus-keyed value maps, so the color pipeline below is target-agnostic.
		const activeSensitivity =
			active &&
			((selectedBus !== null && active.sensitivity?.bus === selectedBus) ||
				(selectedBranch !== null && active.sensitivity?.branch === selectedBranch))
				? active.sensitivity
				: null;
		const activeDeltas = active ? caseDeltas(active) : {};
		const activeRatings = active ? caseRatings(active) : {};
		// Engine first-order LMP preview (Study.preview): predicted per-bus ΔLMP for
		// the live drag, scoped to this case + selection target. Preferred over the JS
		// gradient shift when present; the gradient path stays for server and browser
		// fallbacks.
		const enginePreview =
			active &&
			app.previewLmp &&
			app.previewLmp.caseId === active.id &&
			('bus' in app.previewLmp.target
				? app.previewLmp.target.bus === selectedBus
				: app.previewLmp.target.branch === selectedBranch)
				? app.previewLmp.delta
				: null;
		const previewStep =
			active && activeSensitivity
				? selectedBus !== null && app.previewDeltaMw !== null
					? app.previewDeltaMw - (activeDeltas[selectedBus] ?? 0)
					: selectedBranch !== null && app.previewRatingMw !== null
						? app.previewRatingMw - (activeRatings[selectedBranch] ?? 0)
						: 0
				: 0;
		const previewing = Boolean(
			active?.solving || app.previewActive || enginePreview || Math.abs(previewStep) >= 0.25
		);

		const perCase = new Map<string, CaseDisplay>();
		const addCase = (c: CaseState | LocalCase) => {
			if (!c.network || !c.solution) return;
			const lmp = new Map<number, number>();
			for (const e of c.solution.lmp) lmp.set(e.bus, e.usd_per_mwh);
			// Resolve the active display mode against what this case can show, falling
			// back to LMP when the chosen mode doesn't apply (e.g. |V| under DC OPF).
			const meta = displayMetaFor(c);
			const activeMeta = meta.find((o) => o.mode === app.displayMode) ?? meta[0] ?? null;
			const scalarMode = activeMeta?.mode ?? 'lmp';
			const seriesValues = displaySeriesFor(c, scalarMode);
			const scalar = new Map<number, number>();
			for (const e of seriesValues) scalar.set(e.bus, e.value);
			const { lo: scalarLo, hi: scalarHi } = scalarDomain(
				scalarMode,
				seriesValues.map((e) => e.value)
			);
			const loading = new Map<number, number>();
			for (const f of c.solution.flows) loading.set(f.branch, f.loading);

			const sens = new Map<number, number>();
			const preview = new Map<number, number>();
			let domain: SensitivityDomain | null = null;
			const isActive = c === active;
			if (isActive && enginePreview) {
				// Engine preview owns the LMP shift: add the predicted ΔLMP per bus.
				for (const [bus, dlmp] of enginePreview) {
					preview.set(bus, dlmp);
					lmp.set(bus, (lmp.get(bus) ?? 0) + dlmp);
				}
			}
			if (isActive && activeSensitivity) {
				const values = activeSensitivity.values.map((v) => v.value);
				domain = sensitivityDomain(values);
				for (const v of activeSensitivity.values) {
					sens.set(v.bus, v.value);
				}
				if (!enginePreview && previewStep !== 0) {
					// Fallback first order preview: shift LMPs along the gradient.
					for (const v of activeSensitivity.values) {
						const dlmp = v.value * previewStep;
						preview.set(v.bus, dlmp);
						lmp.set(v.bus, (lmp.get(v.bus) ?? 0) + dlmp);
					}
				}
			}
			const previewValues = [...preview.values()];
			const previewDomain = previewValues.length > 0 ? sensitivityDomain(previewValues) : null;
			const showPreview = Boolean(
				isActive &&
				previewDomain &&
				(app.previewActive || c.solving || enginePreview || Math.abs(previewStep) >= 0.25)
			);
			// The sensitivity and demand-preview overlays are price-space fields (∂LMP/∂d
			// and the ΔLMP preview). They only paint the map under the LMP display mode;
			// under angle or |V| the map stays on the chosen scalar and the sensitivity
			// shows in the side panel, so selecting a bus never silently overrides the
			// angle/|V| coloring with the price ramp.
			const sensEligible = scalarMode === 'lmp';
			// While a sensitivity is still loading for the selected bus, stay in sens
			// mode (renders neutral, not the prior LMP palette) so the node colors
			// don't flash orange->purple between the old and new selection.
			const mode: 'lmp' | 'sens' | 'preview' =
				sensEligible && showPreview
					? 'preview'
					: sensEligible &&
						  isActive &&
						  (selectedBus !== null || selectedBranch !== null) &&
						  (domain || app.sensitivityLoading) &&
						  !previewing
						? 'sens'
						: 'lmp';
			perCase.set(c.id, {
				lmp,
				scalar,
				scalarLo,
				scalarHi,
				scalarMode,
				scalarLabel: activeMeta?.label ?? 'LMP',
				scalarUnit: activeMeta?.unit ?? '$/MWh',
				loading,
				mode,
				sens,
				sensDomain: domain,
				preview,
				previewDomain,
				previewScale: isActive ? ctrl.previewScale : null
			});
		};
		for (const c of app.cases) addCase(c);
		for (const c of app.localCases) addCase(c);
		return perCase;
	});

	function busFill(caseId: string) {
		const d = display.get(caseId);
		return (bus: NetworkBus): [number, number, number, number] => {
			if (!d) return busNeutral;
			if (d.mode === 'sens') {
				if (!d.sensDomain) return busNeutral;
				if (d.sensDomain.flat) return sensFlatColor(d.sensDomain);
				return sensColor((d.sens.get(bus.id) ?? 0) / d.sensDomain.scale);
			}
			if (d.mode === 'preview') {
				// Fixed drag scale first, so intensity tracks the step. It also bypasses
				// the per-frame domain's flat guard, whose absolute epsilon misreads a
				// structured column at a tiny step as flat. previewScale is null when the
				// committed column itself is flat, which falls through to the flat tint.
				if (d.previewScale) {
					return sensColor((d.preview.get(bus.id) ?? 0) / d.previewScale);
				}
				if (!d.previewDomain) return busNeutral;
				if (d.previewDomain.flat) return sensFlatColor(d.previewDomain);
				return sensColor((d.preview.get(bus.id) ?? 0) / d.previewDomain.scale);
			}
			const mid = (d.scalarLo + d.scalarHi) / 2;
			return lmpColor(((d.scalar.get(bus.id) ?? mid) - d.scalarLo) / (d.scalarHi - d.scalarLo));
		};
	}

	// Layers whose click selects something (a bus or a branch); the overlay-level
	// click handler must not treat those clicks as "empty map" and clear the selection.
	function isSelectableLayer(layerId: string | undefined): boolean {
		return Boolean(
			layerId?.startsWith('buses-') ||
				layerId?.startsWith('local-buses-') ||
				layerId?.startsWith('branches-') ||
				layerId?.startsWith('local-branches-')
		);
	}

	const selectedBusCue = $derived.by(() => {
		const busId = app.selectedBus;
		const c = app.active ?? app.activeLocal;
		if (!c || busId === null) return null;
		const buses =
			app.active?.network?.buses ?? app.activeLocal?.network?.buses ?? app.activeLocal?.view?.buses ?? [];
		const bus = buses.find((b) => b.id === busId);
		return bus ? { key: `${c.id}-${bus.id}`, id: bus.id, lon: bus.lon, lat: bus.lat } : null;
	});
	let selectedCueEl = $state.raw<HTMLDivElement | null>(null);

	function syncSelectedCue() {
		if (!map || !selectedCueEl || !selectedBusCue) return;
		const point = map.project([selectedBusCue.lon, selectedBusCue.lat]);
		const { clientWidth, clientHeight } = map.getContainer();
		const inRange =
			point.x >= -40 &&
			point.y >= -40 &&
			point.x <= clientWidth + 40 &&
			point.y <= clientHeight + 40;
		selectedCueEl.style.left = `${point.x}px`;
		selectedCueEl.style.top = `${point.y}px`;
		selectedCueEl.style.display = inRange ? 'block' : 'none';
	}

	/** Case owning a picked network layer; null for display-only layers. */
	function caseOf(info: PickingInfo): SolvableCase | null {
		const layerId = info.layer?.id;
		if (!layerId) return null;
		const bundled = layerId.match(/^(?:buses|branches)-(.+)$/);
		if (bundled) return app.byId(bundled[1]);
		const local = layerId.match(/^local-(?:buses|branches)-(.+)$/);
		if (local) return app.localCases.find((c) => c.id === local[1]) ?? null;
		return null;
	}

	/** Escape free text from a dropped file before it goes into tooltip html
	 * (deck.gl renders the html string as innerHTML). */
	const esc = (s: string) =>
		s
			.replace(/&/g, '&amp;')
			.replace(/</g, '&lt;')
			.replace(/>/g, '&gt;')
			.replace(/"/g, '&quot;')
			.replace(/'/g, '&#39;');

	function tooltip(info: PickingInfo): { html: string } | null {
		const { object } = info;
		if (!object) return null;
		if (info.layer?.id.startsWith('local-subs-')) {
			// PowerWorld .pwd substation: display data only, position inferred from the diagram.
			// The name is free text from a dropped file, so escape it; the number
			// is coerced before interpolation.
			const s = object as { number: number; name: string };
			const named = s.name ? ` ${esc(s.name)}` : '';
			return {
				html: `<div class="tt"><b>substation ${Number(s.number)}</b>${named}<br><span style="opacity:0.6">.pwd diagram &#8901; approx. position</span></div>`
			};
		}
		const c = caseOf(info);
		const d = c ? display.get(c.id) : undefined;
		if ('path' in object) {
			const b = object as NetworkBranch;
			const flow = d
				? `${((d.loading.get(b.id) ?? 0) * 100).toFixed(0)}% of ${b.rate_mw.toFixed(0)} MW`
				: `rate ${b.rate_mw.toFixed(0)} MW`;
			return {
				html: `<div class="tt"><b>line ${b.from}&#8201;&ndash;&#8201;${b.to}</b> ${flow}</div>`
			};
		}
		const bus = object as NetworkBus;
		if (!c) {
			return {
				html: `<div class="tt"><b>bus ${bus.id}</b>
					load ${bus.demand_mw.toFixed(0)} MW &#8901; gen ${bus.gen_mw.toFixed(0)} MW<br>
					<span style="opacity:0.6">local file</span></div>`
			};
		}
		const lmp = d?.lmp.get(bus.id);
		const scalar = d?.scalar.get(bus.id);
		const sens = d?.mode === 'sens' ? d.sens.get(bus.id) : undefined;
		const preview = d?.mode === 'preview' ? d.preview.get(bus.id) : undefined;
		const delta = caseDeltas(c)[bus.id] ?? 0;
		const loadRow =
			delta === 0
				? `load ${bus.demand_mw.toFixed(0)} MW`
				: `load ${(bus.demand_mw + delta).toFixed(0)} MW (${delta > 0 ? '+' : ''}${delta.toFixed(0)})`;
		const sensRow =
			sens === undefined
				? ''
				: `<br>&part;LMP/&part;d ${sens >= 0 ? '+' : ''}${sens.toExponential(2)}`;
		const previewRow =
			preview === undefined
				? ''
				: `<br>&Delta;LMP ${preview >= 0 ? '+' : ''}${preview.toExponential(2)} $/MWh`;
		const localRow =
			c instanceof CaseState ? '' : '<br><span style="opacity:0.6">local file</span>';
		const scalarRow =
			!d || d.scalarMode === 'lmp'
				? `LMP ${lmp === undefined ? '&mdash;' : fmt.format(lmp)} $/MWh`
				: `${d.scalarLabel} ${
						scalar === undefined ? '&mdash;' : displayFmt(d.scalarMode, scalar)
					} ${d.scalarUnit}<br>LMP ${lmp === undefined ? '&mdash;' : fmt.format(lmp)} $/MWh`;
		return {
			html: `<div class="tt"><b>bus ${bus.id}</b>
				${scalarRow}<br>
				${loadRow} &#8901; gen ${bus.gen_mw.toFixed(0)} MW${sensRow}${previewRow}${localRow}</div>`
		};
	}

	async function loadMapModules() {
		const [maplibre, mapbox, layers] = await Promise.all([
			import('maplibre-gl'),
			import('@deck.gl/mapbox'),
			import('@deck.gl/layers')
		]);
		return {
			maplibregl: maplibre.default,
			MapboxOverlay: mapbox.MapboxOverlay,
			PathLayer: layers.PathLayer,
			ScatterplotLayer: layers.ScatterplotLayer
		};
	}

	type MapModules = Awaited<ReturnType<typeof loadMapModules>>;

	// The deck overlay options. interleaved:true renders the network into
	// maplibre's own WebGL2 context instead of a second canvas, so the page
	// holds one GL context rather than two. Safari evicts WebGL contexts under
	// memory and tab pressure, and one context survives that far better; the
	// deck layers also inherit maplibre's 4096 canvas clamp. The flat positron
	// basemap writes no depth, so the overlay always draws on top.
	function buildOverlay(MapboxOverlay: MapModules['MapboxOverlay']): MapboxOverlay {
		return new MapboxOverlay({
			interleaved: true,
			layers: [],
			parameters: {
				depthWriteEnabled: false,
				depthCompare: 'always'
			},
			onClick: (info: PickingInfo) => {
				if (!app.placingLocalId && !isSelectableLayer(info.layer?.id)) onmapclick();
			},
			getTooltip: tooltip,
			getCursor: ({ isHovering, isDragging }: CursorState) =>
				app.placingLocalId ? 'crosshair' : isDragging ? 'grabbing' : isHovering ? 'pointer' : 'grab'
		});
	}

	function initMap(container: HTMLDivElement) {
		let cleanup = () => {};
		let cancelled = false;
		void loadMapModules()
			.then(({ maplibregl, MapboxOverlay, PathLayer, ScatterplotLayer }) => {
				if (cancelled) return;
				layerCtors = { PathLayer, ScatterplotLayer };
				const m = new maplibregl.Map({
					container,
					style: STYLE,
					center: [-85, 36],
					zoom: 4.5,
					canvasContextAttributes: { antialias: true },
					attributionControl: { compact: true }
				});
				const o = buildOverlay(MapboxOverlay);
				m.addControl(o);
				m.on('click', (e) => {
					if (app.placingLocalId) onplacecase(e.lngLat.lng, e.lngLat.lat);
				});
				m.addControl(new maplibregl.NavigationControl({ showCompass: false }), 'bottom-right');

				// WebGL resilience on Safari. Two failure modes seen in the wild:
				//   1. The canvas backing store is discarded while the tab is idle or
				//      backgrounded, so the map shows blank until something forces a
				//      redraw. A manual window resize fixes it; do that automatically
				//      when the tab returns to the foreground.
				//   2. The whole GL context is lost under pressure. maplibre rebuilds
				//      its painter on restore but not the deck custom layer (its Deck
				//      still holds the dead context), so a clean remount is simplest:
				//      bump mapGen to tear the map down and build it again.
				const repaint = () => {
					if (cancelled) return;
					m.resize();
					m.triggerRepaint();
				};
				const onVisible = () => {
					if (document.visibilityState === 'visible') repaint();
				};
				const syncCue = () => syncSelectedCue();
				let rebuildTimer: ReturnType<typeof setTimeout> | null = null;
				const clearRebuild = () => {
					if (rebuildTimer !== null) {
						clearTimeout(rebuildTimer);
						rebuildTimer = null;
					}
				};
				// A rebuilt map that holds a live context for a while proves the GPU
				// recovered, so the rebuild budget is forgiven (see below). Clear that
				// pending forgiveness the moment another loss arrives.
				let stabilizeTimer: ReturnType<typeof setTimeout> | null = null;
				const clearStabilize = () => {
					if (stabilizeTimer !== null) {
						clearTimeout(stabilizeTimer);
						stabilizeTimer = null;
					}
				};
				// At most one rebuild per map instance: the 1500ms timer and
				// webglcontextrestored both call rebuild for a single loss, so guard
				// against counting that loss twice against the budget.
				let rebuilt = false;
				const rebuild = () => {
					clearRebuild();
					if (cancelled || rebuilt) return;
					if (glRebuilds >= 6) {
						app.error =
							'the map lost its graphics context repeatedly; reload the page to restore it';
						return;
					}
					rebuilt = true;
					glRebuilds++;
					mapGen++;
				};
				const onContextLost = () => {
					overlay = null; // stop the layer effect touching a dead overlay
					clearStabilize(); // a fresh loss is not a stable recovery
					rebuildTimer ??= setTimeout(rebuild, 1500);
				};
				document.addEventListener('visibilitychange', onVisible);
				window.addEventListener('focus', repaint);
				window.addEventListener('pageshow', repaint);
				m.on('webglcontextlost', onContextLost);
				m.on('webglcontextrestored', rebuild);
				m.on('load', syncCue);
				m.on('move', syncCue);
				m.on('resize', syncCue);
				m.on('zoom', syncCue);

				map = m;
				overlay = o;
				// Frame the active grid once the map is ready. The initial frame request
				// (from the case load) can fire before the async map import finishes and
				// be lost; re-issuing it here ensures the first view is the active case,
				// not the default world view. A remount opens at the default view too.
				app.requestFrame(app.activeCaseId ?? app.activeLocalId ?? 'all');
				if (mapGen > 0) {
					// This is a rebuilt map. Once it has held a live context without
					// another loss, zero the budget so the cap only ever catches a tight
					// loss loop, not recoveries spread across a long session.
					stabilizeTimer = setTimeout(() => {
						glRebuilds = 0;
						stabilizeTimer = null;
					}, 10000);
				}

				cleanup = () => {
					clearRebuild();
					clearStabilize();
					document.removeEventListener('visibilitychange', onVisible);
					window.removeEventListener('focus', repaint);
					window.removeEventListener('pageshow', repaint);
					m.off('load', syncCue);
					m.off('move', syncCue);
					m.off('resize', syncCue);
					m.off('zoom', syncCue);
					m.remove();
					map = null;
					overlay = null;
				};
			})
			.catch((e) => {
				if (!cancelled) app.error = `map failed to load: ${e instanceof Error ? e.message : e}`;
			});
		return () => {
			cancelled = true;
			cleanup();
		};
	}

	// Sync deck.gl layers with app state. New layer instances diff cheaply;
	// updateTriggers tell deck.gl when accessor outputs changed.
	$effect(() => {
		if (!overlay || !layerCtors) return;
		const { PathLayer, ScatterplotLayer } = layerCtors;
		const layers: Layer[] = [];
		for (const c of app.cases) {
			if (!c.network) continue;
			const d = display.get(c.id);
			layers.push(
				new PathLayer<NetworkBranch>({
					id: `branches-${c.id}`,
					data: c.network.branches,
					getPath: (b) => b.path,
					// The selected branch draws in the selection blue of the bus halo cue,
					// wider, so the ∂LMP/∂rating source line reads at a glance.
					getColor: (b) =>
						c.id === app.activeCaseId && b.id === app.selectedBranch
							? [47, 111, 187, 255]
							: branchColor(d?.loading.get(b.id) ?? 0, b.status === 1),
					getWidth: (b) =>
						c.id === app.activeCaseId && b.id === app.selectedBranch
							? Math.max(branchWidth(d?.loading.get(b.id) ?? 0) + 2, 4.5)
							: branchWidth(d?.loading.get(b.id) ?? 0),
					widthUnits: 'pixels',
					widthMinPixels: 1.5,
					capRounded: true,
					jointRounded: true,
					miterLimit: 2,
					pickable: true,
					autoHighlight: true,
					highlightColor: [32, 36, 43, 90],
					onClick: (info: PickingInfo) => {
						const branch = info.object as NetworkBranch | undefined;
						if (!branch || !onbranchclick) return false;
						onbranchclick(c.id, branch.id);
						return true;
					},
					updateTriggers: {
						getColor: [display, app.selectedBranch, app.activeCaseId],
						getWidth: [display, app.selectedBranch, app.activeCaseId]
					}
				}),
				new ScatterplotLayer<NetworkBus>({
					id: `buses-${c.id}`,
					data: c.network.buses,
					getPosition: (b) => [b.lon, b.lat],
					getRadius: (b) => busRadius(Math.max(b.demand_mw, b.gen_mw)),
					radiusUnits: 'pixels',
					getFillColor: busFill(c.id),
					stroked: true,
					billboard: true,
					getLineColor: (b) =>
						c.id === app.activeCaseId && b.id === app.selectedBus
							? [32, 36, 43, 255]
							: [46, 42, 34, 110],
					getLineWidth: (b) => (c.id === app.activeCaseId && b.id === app.selectedBus ? 2.5 : 1),
					lineWidthUnits: 'pixels',
					pickable: true,
					autoHighlight: true,
					highlightColor: [32, 36, 43, 70],
					onClick: (info: PickingInfo) => {
						const bus = info.object as NetworkBus | undefined;
						if (!bus) return false;
						onbusclick(c.id, bus.id);
						return true;
					},
					updateTriggers: {
						getFillColor: [display],
						getLineColor: [app.selectedBus, app.activeCaseId],
						getLineWidth: [app.selectedBus, app.activeCaseId]
					}
				})
			);
		}
		// Local cases are grey until the browser solve returns. A .pwd display
		// file contributes substation points only, in a cooler slate so they
		// read as diagram derived positions.
		for (const c of app.localCases) {
			if (c.view) {
				const d = display.get(c.id);
				layers.push(
					new PathLayer<NetworkBranch>({
						id: `local-branches-${c.id}`,
						data: c.view.branches,
						getPath: (b) => b.path,
						getColor: (b) =>
							c.id === app.activeLocalId && b.id === app.selectedBranch
								? [47, 111, 187, 255]
								: d
									? branchColor(d.loading.get(b.id) ?? 0, b.status === 1)
									: [138, 131, 117, 150],
						getWidth: (b) =>
							c.id === app.activeLocalId && b.id === app.selectedBranch
								? Math.max((d ? branchWidth(d.loading.get(b.id) ?? 0) : 1.5) + 2, 4.5)
								: d
									? branchWidth(d.loading.get(b.id) ?? 0)
									: 1.5,
						widthUnits: 'pixels',
						widthMinPixels: 1.2,
						capRounded: true,
						jointRounded: true,
						miterLimit: 2,
						pickable: true,
						autoHighlight: true,
						highlightColor: [32, 36, 43, 90],
						onClick: (info: PickingInfo) => {
							const branch = info.object as NetworkBranch | undefined;
							if (!branch || !onlocalbranchclick) return false;
							onlocalbranchclick(c.id, branch.id);
							return true;
						},
						updateTriggers: {
							getColor: [display, app.selectedBranch, app.activeLocalId],
							getWidth: [display, app.selectedBranch, app.activeLocalId]
						}
					}),
					new ScatterplotLayer<NetworkBus>({
						id: `local-buses-${c.id}`,
						data: c.view.buses,
						getPosition: (b) => [b.lon, b.lat],
						getRadius: (b) => busRadius(Math.max(b.demand_mw, b.gen_mw)),
						radiusUnits: 'pixels',
						getFillColor: d ? busFill(c.id) : [110, 115, 120, 200],
						stroked: true,
						billboard: true,
						getLineColor: (b) =>
							c.id === app.activeLocalId && b.id === app.selectedBus
								? [32, 36, 43, 255]
								: [46, 42, 34, 110],
						getLineWidth: (b) => (c.id === app.activeLocalId && b.id === app.selectedBus ? 2.5 : 1),
						lineWidthUnits: 'pixels',
						pickable: true,
						autoHighlight: true,
						highlightColor: [32, 36, 43, 70],
						onClick: (info: PickingInfo) => {
							const bus = info.object as NetworkBus | undefined;
							if (!bus) return false;
							onlocalbusclick(c.id, bus.id);
							return true;
						},
						updateTriggers: {
							getFillColor: [display],
							getLineColor: [app.selectedBus, app.activeLocalId],
							getLineWidth: [app.selectedBus, app.activeLocalId]
						}
					})
				);
			}
			if (c.substations) {
				layers.push(
					new ScatterplotLayer<{ number: number; name: string; lon: number; lat: number }>({
						id: `local-subs-${c.id}`,
						data: c.substations.points,
						getPosition: (s) => [s.lon, s.lat],
						getRadius: 5,
						radiusUnits: 'pixels',
						radiusMinPixels: 3,
						getFillColor: [70, 92, 124, 190],
						stroked: true,
						billboard: true,
						getLineColor: [38, 52, 78, 220],
						getLineWidth: 1,
						lineWidthUnits: 'pixels',
						pickable: true,
						autoHighlight: true,
						highlightColor: [32, 36, 43, 70]
					})
				);
			}
		}
		overlay.setProps({ layers });
	});

	$effect(() => {
		if (!map) return;
		map.getCanvas().style.cursor = app.placingLocalId ? 'crosshair' : '';
	});

	$effect(() => {
		if (!map) return;
		void mapGen;
		void selectedBusCue?.key;
		void tick().then(syncSelectedCue);
	});

	function boundsFor(target: string | 'all'): LngLatBoundsLike | null {
		let minLon = Infinity;
		let minLat = Infinity;
		let maxLon = -Infinity;
		let maxLat = -Infinity;
		let seen = false;
		const fold = (pts: { lon: number; lat: number }[]) => {
			for (const b of pts) {
				minLon = Math.min(minLon, b.lon);
				minLat = Math.min(minLat, b.lat);
				maxLon = Math.max(maxLon, b.lon);
				maxLat = Math.max(maxLat, b.lat);
				seen = true;
			}
		};
		for (const c of app.cases) {
			if (!c.network || (target !== 'all' && c.id !== target)) continue;
			fold(c.network.buses);
		}
		for (const c of app.localCases) {
			if (target !== 'all' && c.id !== target) continue;
			if (c.view) fold(c.view.buses);
			if (c.substations) fold(c.substations.points);
		}
		return seen
			? [
					[minLon, minLat],
					[maxLon, maxLat]
				]
			: null;
	}

	// Fly to whatever the header (or initial load) asked for.
	$effect(() => {
		void app.frameSeq;
		if (!map) return;
		const bounds = boundsFor(app.frameTarget);
		if (!bounds) return;
		const { clientWidth: w, clientHeight: h } = map.getContainer();
		const padding =
			w <= 760
				? { top: 130, left: 24, right: 24, bottom: Math.min(Math.round(h * 0.5), 330) }
				: { top: 96, left: 380, right: 60, bottom: 64 };
		map.fitBounds(bounds, {
			padding,
			duration: 1400
		});
	});
</script>

{#key mapGen}
	<div class="map-stage">
		<div class="map" {@attach initMap}></div>
		{#if selectedBusCue}
			{#key selectedBusCue.key}
				<div class="selected-bus-cue" bind:this={selectedCueEl} aria-hidden="true"></div>
			{/key}
		{/if}
	</div>
{/key}

{#if app.placingLocalId && map}
	<button
		type="button"
		class="place-at-center mono"
		onclick={() => {
			if (!map) return;
			const center = map.getCenter();
			onplacecase(center.lng, center.lat);
		}}
	>
		place at map center
	</button>
{/if}

<style>
	.map-stage,
	.map {
		position: absolute;
		inset: 0;
	}

	.map {
		background: var(--bg);
	}

	.selected-bus-cue {
		position: absolute;
		z-index: calc(var(--z-chrome) - 1);
		display: none;
		width: 28px;
		height: 28px;
		margin: -14px 0 0 -14px;
		border: 2px solid #2f6fbb;
		border-radius: 999px;
		background: rgba(47, 111, 187, 0.1);
		box-shadow:
			0 0 0 4px rgba(47, 111, 187, 0.12),
			0 0 18px rgba(47, 111, 187, 0.24);
		pointer-events: none;
		animation:
			selected-bus-pop 420ms cubic-bezier(0.34, 1.56, 0.64, 1),
			selected-bus-pulse 1.8s ease-in-out 420ms infinite;
	}

	@keyframes selected-bus-pop {
		0% {
			opacity: 0;
			transform: scale(0.35);
		}
		65% {
			opacity: 0.95;
			transform: scale(1.35);
		}
		100% {
			opacity: 0.9;
			transform: scale(1);
		}
	}

	@keyframes selected-bus-pulse {
		0%,
		100% {
			opacity: 0.9;
			transform: scale(1);
			box-shadow:
				0 0 0 4px rgba(47, 111, 187, 0.12),
				0 0 18px rgba(47, 111, 187, 0.24);
		}
		50% {
			opacity: 0.48;
			transform: scale(1.22);
			box-shadow:
				0 0 0 8px rgba(47, 111, 187, 0.08),
				0 0 28px rgba(47, 111, 187, 0.18);
		}
	}

	/* Keyboard/touch equivalent to clicking the map while placing a local case;
	   sits just above PlacementCue's instruction text so the two don't overlap. */
	.place-at-center {
		position: absolute;
		left: 50%;
		bottom: 92px;
		z-index: 15;
		transform: translateX(-50%);
		padding: 7px 14px;
		background: var(--panel);
		border: 1px solid var(--accent);
		border-radius: 3px;
		color: var(--text-accent);
		font-size: 11px;
		cursor: pointer;
		box-shadow: 0 4px 18px rgba(32, 36, 43, 0.1);
	}

	@media (max-width: 420px) {
		.place-at-center {
			bottom: 78px;
		}
	}

	.map :global(.maplibregl-ctrl-bottom-right) {
		right: 20px;
		bottom: 18px;
		display: flex;
		flex-direction: column;
		align-items: flex-end;
		gap: 6px;
	}

	.map :global(.maplibregl-ctrl-bottom-right .maplibregl-ctrl) {
		float: none;
		margin: 0;
	}

	.map :global(.maplibregl-ctrl-attrib) {
		order: -1;
		position: relative;
		right: 2px;
		width: auto;
		min-width: 0;
		max-width: calc(100vw - 96px);
		box-sizing: border-box;
		background: rgba(252, 251, 247, 0.75);
		font-family: var(--font-mono);
		font-size: 10px;
		white-space: nowrap;
		text-align: right;
		box-shadow: var(--elev-1);
	}

	.map :global(.maplibregl-ctrl-attrib a) {
		color: var(--text-secondary);
	}

	.map :global(.deck-tooltip) {
		background: var(--panel) !important;
		border: 1px solid var(--line);
		color: var(--ink) !important;
		font-family: var(--font-mono);
		font-size: 11px;
		line-height: 1.5;
		padding: 8px 10px !important;
		border-radius: 2px;
		box-shadow: 0 2px 10px rgba(32, 36, 43, 0.12);
	}

	@media (max-width: 760px) {
		.map :global(.maplibregl-ctrl-attrib) {
			width: auto;
			min-width: 0;
			right: 0;
			white-space: normal;
		}
	}

	@media (prefers-reduced-motion: reduce) {
		.selected-bus-cue {
			animation: none;
		}
	}
</style>
