<script lang="ts">
	import 'maplibre-gl/dist/maplibre-gl.css';
	import type { Layer, PickingInfo } from '@deck.gl/core';
	import type { PathLayer, ScatterplotLayer } from '@deck.gl/layers';
	import type { MapboxOverlay } from '@deck.gl/mapbox';
	import type { LngLatBoundsLike, Map as MapLibreMap } from 'maplibre-gl';
	import type { NetworkBranch, NetworkBus } from '$lib/api';
	import { branchColor, branchWidth, busRadius, lmpColor, lmpDomain, sensColor } from '$lib/colors';
	import { app, type CaseState } from '$lib/state.svelte';

	let { onbusclick }: { onbusclick: (caseId: string, busId: number) => void } = $props();

	const STYLE = 'https://basemaps.cartocdn.com/gl/positron-nolabels-gl-style/style.json';

	let map = $state.raw<MapLibreMap | null>(null);
	let overlay = $state.raw<MapboxOverlay | null>(null);

	type LayerCtors = {
		PathLayer: typeof PathLayer;
		ScatterplotLayer: typeof ScatterplotLayer;
	};
	let layerCtors = $state.raw<LayerCtors | null>(null);

	interface CaseDisplay {
		lmp: Map<number, number>;
		lo: number;
		hi: number;
		loading: Map<number, number>;
		mode: 'lmp' | 'sens';
		sens: Map<number, number>;
		sensMax: number;
	}

	// Everything the accessors need, rebuilt when any case's data moves. The
	// LMP scale normalizes per network: each case is an islanded market, so
	// within-network structure beats cross-network color comparability (the
	// panel legend and tooltips carry the actual numbers). Preview values
	// shift individual buses without rescaling.
	const display = $derived.by(() => {
		const active = app.active;
		const selected = app.selectedBus;
		const previewStep =
			active && selected !== null && app.previewDeltaMw !== null && active.sensitivity
				? app.previewDeltaMw - (active.deltas[selected] ?? 0)
				: 0;

		const perCase = new Map<string, CaseDisplay>();
		for (const c of app.cases) {
			if (!c.network || !c.solution) continue;
			const lmp = new Map<number, number>();
			for (const e of c.solution.lmp) lmp.set(e.bus, e.usd_per_mwh);
			const { lo, hi } = lmpDomain(c.solution.lmp.map((e) => e.usd_per_mwh));
			const loading = new Map<number, number>();
			for (const f of c.solution.flows) loading.set(f.branch, f.loading);

			const sens = new Map<number, number>();
			let sensMax = 0;
			const isActive = c === active;
			if (isActive && selected !== null && c.sensitivity) {
				for (const v of c.sensitivity.values) {
					sens.set(v.bus, v.value);
					sensMax = Math.max(sensMax, Math.abs(v.value));
				}
				if (previewStep !== 0) {
					// First order preview: shift LMPs along the gradient.
					for (const v of c.sensitivity.values) {
						lmp.set(v.bus, (lmp.get(v.bus) ?? 0) + v.value * previewStep);
					}
				}
			}
			const mode: 'lmp' | 'sens' =
				isActive && selected !== null && sensMax > 0 && previewStep === 0 ? 'sens' : 'lmp';
			perCase.set(c.id, { lmp, lo, hi, loading, mode, sens, sensMax });
		}
		return perCase;
	});

	function busFill(caseId: string) {
		const d = display.get(caseId);
		return (bus: NetworkBus): [number, number, number, number] => {
			if (!d) return [180, 175, 165, 200];
			if (d.mode === 'sens') return sensColor((d.sens.get(bus.id) ?? 0) / d.sensMax);
			const mid = (d.lo + d.hi) / 2;
			return lmpColor(((d.lmp.get(bus.id) ?? mid) - d.lo) / (d.hi - d.lo));
		};
	}

	/** Backend case owning a picked layer; null for local case layers. */
	function caseOf(info: PickingInfo): CaseState | null {
		const id = info.layer?.id.replace(/^(buses|branches)-/, '');
		return id ? app.byId(id) : null;
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
			// Browser-parsed file: topology only, no solution to report.
			return {
				html: `<div class="tt"><b>bus ${bus.id}</b>
					load ${bus.demand_mw.toFixed(0)} MW &#8901; gen ${bus.gen_mw.toFixed(0)} MW<br>
					<span style="opacity:0.6">local file</span></div>`
			};
		}
		const lmp = d?.lmp.get(bus.id);
		const sens = d?.mode === 'sens' ? d.sens.get(bus.id) : undefined;
		const delta = c.deltas[bus.id] ?? 0;
		const loadRow =
			delta === 0
				? `load ${bus.demand_mw.toFixed(0)} MW`
				: `load ${(bus.demand_mw + delta).toFixed(0)} MW (${delta > 0 ? '+' : ''}${delta.toFixed(0)})`;
		const sensRow =
			sens === undefined
				? ''
				: `<br>&part;LMP/&part;d ${sens >= 0 ? '+' : ''}${sens.toExponential(2)}`;
		return {
			html: `<div class="tt"><b>bus ${bus.id}</b>
				LMP ${lmp?.toFixed(2) ?? '&mdash;'} $/MWh<br>
				${loadRow} &#8901; gen ${bus.gen_mw.toFixed(0)} MW${sensRow}</div>`
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
				const o = new MapboxOverlay({
					interleaved: true,
					layers: [],
					parameters: {
						depthWriteEnabled: false,
						depthCompare: 'always'
					},
					getTooltip: tooltip,
					getCursor: ({ isHovering, isDragging }) =>
						isDragging ? 'grabbing' : isHovering ? 'pointer' : 'grab'
				});
				m.addControl(o);
				m.addControl(new maplibregl.NavigationControl({ showCompass: false }), 'bottom-right');
				map = m;
				overlay = o;
				cleanup = () => {
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
					getColor: (b) => branchColor(d?.loading.get(b.id) ?? 0, b.status === 1),
					getWidth: (b) => branchWidth(d?.loading.get(b.id) ?? 0),
					widthUnits: 'pixels',
					widthMinPixels: 1.5,
					capRounded: true,
					jointRounded: true,
					miterLimit: 2,
					pickable: true,
					autoHighlight: true,
					highlightColor: [32, 36, 43, 90],
					updateTriggers: {
						getColor: [display],
						getWidth: [display]
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
					getLineWidth: (b) =>
						c.id === app.activeCaseId && b.id === app.selectedBus ? 2.5 : 1,
					lineWidthUnits: 'pixels',
					pickable: true,
					autoHighlight: true,
					highlightColor: [32, 36, 43, 70],
					onClick: (info: PickingInfo) => {
						const bus = info.object as NetworkBus | undefined;
						if (bus) onbusclick(c.id, bus.id);
					},
					updateTriggers: {
						getFillColor: [display],
						getLineColor: [app.selectedBus, app.activeCaseId],
						getLineWidth: [app.selectedBus, app.activeCaseId]
					}
				})
			);
		}
		// Local cases: topology only, no physics yet, so desaturated graphite
		// against the warm LMP ramp of solved backend cases. A .pwd display
		// file contributes substation points only (no topology), in a cooler
		// slate so they read as diagram-derived positions.
		for (const c of app.localCases) {
			if (c.view) {
				layers.push(
					new PathLayer<NetworkBranch>({
						id: `local-branches-${c.id}`,
						data: c.view.branches,
						getPath: (b) => b.path,
						getColor: [138, 131, 117, 150],
						getWidth: 1.5,
						widthUnits: 'pixels',
						widthMinPixels: 1.2,
						capRounded: true,
						jointRounded: true,
						miterLimit: 2,
						pickable: true
					}),
					new ScatterplotLayer<NetworkBus>({
						id: `local-buses-${c.id}`,
						data: c.view.buses,
						getPosition: (b) => [b.lon, b.lat],
						getRadius: (b) => busRadius(Math.max(b.demand_mw, b.gen_mw)),
						radiusUnits: 'pixels',
						getFillColor: [110, 115, 120, 200],
						stroked: true,
						billboard: true,
						getLineColor: [46, 42, 34, 110],
						getLineWidth: 1,
						lineWidthUnits: 'pixels',
						pickable: true,
						autoHighlight: true,
						highlightColor: [32, 36, 43, 70]
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

<div class="map" {@attach initMap}></div>

<style>
	.map {
		position: absolute;
		inset: 0;
		background: var(--bg);
	}

	/* Lift the bottom-right controls clear of the footer strip. */
	.map :global(.maplibregl-ctrl-bottom-right) {
		bottom: 30px;
	}

	.map :global(.maplibregl-ctrl-attrib) {
		background: rgba(252, 251, 247, 0.75);
		font-family: var(--font-mono);
		font-size: 10px;
	}

	.map :global(.maplibregl-ctrl-attrib a) {
		color: var(--ink-dim);
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
</style>
