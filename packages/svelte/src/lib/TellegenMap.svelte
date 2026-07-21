<script lang="ts">
	import 'maplibre-gl/dist/maplibre-gl.css';
	import { tick } from 'svelte';
	import type { Layer, PickingInfo } from '@deck.gl/core';
	import type { IconLayer, PathLayer, ScatterplotLayer } from '@deck.gl/layers';
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
	import {
		attachmentColor,
		edgeColor,
		edgeWidth,
		isPhaseTerminal,
		phaseColor,
		transformerMarks,
		type PlacedMultiBus,
		type PlacedMultiEdge,
		type TransformerMark
	} from './multiconductor.js';
	import { transformerIcon } from './transformer-icon.js';
	import { getAppState, getController } from './context.svelte.js';
	import { displayFmt, fmt, rgbaCss } from './format.js';
	import type { RGBA } from './colors.js';

	let {
		onbusclick,
		onlocalbusclick,
		onbranchclick,
		onlocalbranchclick,
		onmultibusclick,
		onmultiedgeclick,
		onplacecase,
		onmapclick
	}: {
		onbusclick: (caseId: string, busId: number) => void;
		onlocalbusclick: (caseId: string, busId: number) => void;
		onbranchclick?: (caseId: string, branchId: number) => void;
		onlocalbranchclick?: (caseId: string, branchId: number) => void;
		onmultibusclick: (caseId: string, busId: string) => void;
		onmultiedgeclick: (caseId: string, edgeId: string) => void;
		onplacecase: (lon: number, lat: number) => void;
		onmapclick: () => void;
	} = $props();

	const app = getAppState();
	const ctrl = getController();

	const STYLE = 'https://basemaps.cartocdn.com/gl/positron-nolabels-gl-style/style.json';
	const CUE_MARGIN_PX = 64;
	const BRANCH_FOCUS_MS = 850;
	const BRANCH_FOCUS_MAX_ZOOM = 9.5;

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
		IconLayer: typeof IconLayer;
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

	// A multiconductor bus is badged by its dominant attachment role (source,
	// then generator, IBR, load, shunt); a bus with none stays neutral. The full
	// per-terminal attachment list expands in the selection overlay.
	function multiBusFill(b: PlacedMultiBus): [number, number, number, number] {
		const primary = b.attachmentKinds[0];
		return primary ? attachmentColor(primary) : busNeutral;
	}

	// Layers whose click selects something (a bus or a branch); the overlay-level
	// click handler must not treat those clicks as "empty map" and clear the selection.
	function isSelectableLayer(layerId: string | undefined): boolean {
		return Boolean(
			layerId?.startsWith('buses-') ||
				layerId?.startsWith('local-buses-') ||
				layerId?.startsWith('branches-') ||
				layerId?.startsWith('local-branches-') ||
				layerId?.startsWith('multi-buses-') ||
				layerId?.startsWith('multi-edges-')
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
	const selectedBranchCue = $derived.by(() => {
		const branchId = app.selectedBranch;
		const c = app.active ?? app.activeLocal;
		if (!c || branchId === null) return null;
		const branches =
			c instanceof CaseState ? c.network?.branches : (c.view?.branches ?? c.network?.branches);
		const branch = branches?.find((b) => b.id === branchId);
		const path = branch?.path.filter(([lon, lat]) => Number.isFinite(lon) && Number.isFinite(lat));
		return branch && path && path.length >= 2
			? { key: `${c.id}-${branch.id}`, caseId: c.id, branchId: branch.id, path }
			: null;
	});
	let selectedCueEl = $state.raw<HTMLDivElement | null>(null);
	let selectedBranchCueEl = $state.raw<SVGSVGElement | null>(null);
	let selectedBranchHaloPath = $state.raw<SVGPathElement | null>(null);
	let selectedBranchLinePath = $state.raw<SVGPathElement | null>(null);
	let handledFrameSeq = 0;

	// The selected multiconductor bus and its incident edges, whose per-conductor
	// strands and terminal stack the detail overlay draws in screen space.
	const selectedMultiDetail = $derived.by(() => {
		const c = app.activeMulti;
		const busId = c?.selectedBusId ?? null;
		if (!c?.view || busId === null) return null;
		const bus = c.view.buses.find((b) => b.id === busId);
		if (!bus) return null;
		const edges = c.view.edges.filter((e) => e.from === busId || e.to === busId);
		return { key: `${c.id}-${busId}`, bus, edges };
	});
	let multiDetailEl = $state.raw<SVGSVGElement | null>(null);

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

	function syncSelectedBranchCue() {
		if (
			!map ||
			!selectedBranchCueEl ||
			!selectedBranchHaloPath ||
			!selectedBranchLinePath ||
			!selectedBranchCue
		) {
			return;
		}
		const activeMap = map;
		const { clientWidth, clientHeight } = activeMap.getContainer();
		const points = selectedBranchCue.path
			.map(([lon, lat]) => activeMap.project([lon, lat]))
			.filter((p) => Number.isFinite(p.x) && Number.isFinite(p.y));
		if (clientWidth === 0 || clientHeight === 0 || points.length < 2) {
			selectedBranchCueEl.style.display = 'none';
			return;
		}

		let minX = Infinity;
		let minY = Infinity;
		let maxX = -Infinity;
		let maxY = -Infinity;
		const d = points
			.map((p, i) => {
				minX = Math.min(minX, p.x);
				minY = Math.min(minY, p.y);
				maxX = Math.max(maxX, p.x);
				maxY = Math.max(maxY, p.y);
				return `${i === 0 ? 'M' : 'L'}${p.x.toFixed(1)} ${p.y.toFixed(1)}`;
			})
			.join(' ');
		const inRange =
			maxX >= -CUE_MARGIN_PX &&
			maxY >= -CUE_MARGIN_PX &&
			minX <= clientWidth + CUE_MARGIN_PX &&
			minY <= clientHeight + CUE_MARGIN_PX;

		selectedBranchCueEl.setAttribute('viewBox', `0 0 ${clientWidth} ${clientHeight}`);
		selectedBranchCueEl.style.display = inRange ? 'block' : 'none';
		selectedBranchHaloPath.setAttribute('d', d);
		selectedBranchLinePath.setAttribute('d', d);
	}

	const col = (c: RGBA) => rgbaCss(c);

	/** A small ground symbol centered at (x, y): a short stem into three
	 * shrinking bars, the standard earth glyph. */
	function groundGlyph(x: number, y: number): string {
		return (
			`<g class="mc-ground">` +
			`<line x1="${x}" y1="${y - 4}" x2="${x}" y2="${y}"/>` +
			`<line x1="${x - 5}" y1="${y}" x2="${x + 5}" y2="${y}"/>` +
			`<line x1="${x - 3}" y1="${y + 2.5}" x2="${x + 3}" y2="${y + 2.5}"/>` +
			`<line x1="${x - 1.5}" y1="${y + 5}" x2="${x + 1.5}" y2="${y + 5}"/></g>`
		);
	}

	// Draw the selected bus's terminal-level detail in screen space: each incident
	// edge fans into one strand per conductor (offset perpendicular, colored by
	// phase), and the bus expands a stack of terminal badges with ground markers
	// and attachment squares. Rebuilt per frame from map.project, like the branch
	// cue. Bus ids and terminal names come from the file, so they are escaped.
	function syncMultiDetail() {
		if (!map || !multiDetailEl) return;
		const detail = selectedMultiDetail;
		if (!detail) {
			multiDetailEl.style.display = 'none';
			multiDetailEl.innerHTML = '';
			return;
		}
		const m = map;
		const { clientWidth, clientHeight } = m.getContainer();
		if (clientWidth === 0 || clientHeight === 0) {
			multiDetailEl.style.display = 'none';
			return;
		}
		const { bus, edges } = detail;
		const busPt = m.project([bus.lon, bus.lat]);

		const STRAND_SPACING = 5;
		let strands = '';
		for (const e of edges) {
			const selectedIsFrom = e.from === bus.id;
			const other = selectedIsFrom ? e.path[e.path.length - 1] : e.path[0];
			const b = m.project(other);
			const dx = b.x - busPt.x;
			const dy = b.y - busPt.y;
			const len = Math.hypot(dx, dy) || 1;
			const px = -dy / len;
			const py = dx / len;
			const n = e.conductors.length || Math.max(e.n_phases, 1);
			for (let i = 0; i < n; i++) {
				const pair = e.conductors[i];
				const term = (selectedIsFrom ? pair?.[0] : pair?.[1]) ?? '';
				const off = (i - (n - 1) / 2) * STRAND_SPACING;
				const ax = (busPt.x + px * off).toFixed(1);
				const ay = (busPt.y + py * off).toFixed(1);
				const bx = (b.x + px * off).toFixed(1);
				const by = (b.y + py * off).toFixed(1);
				strands += `<line x1="${ax}" y1="${ay}" x2="${bx}" y2="${by}" stroke="${col(phaseColor(term))}" stroke-width="2.4" stroke-linecap="round" opacity="0.9"/>`;
			}
		}

		// The terminal stack sits to the right of the bus; a leader connects them.
		const terminals = bus.terminals.slice(0, 12);
		const STACK_DX = 26;
		const STACK_DY = 18;
		const sx = busPt.x + STACK_DX;
		const top = busPt.y - ((terminals.length - 1) / 2) * STACK_DY;
		let stack = `<line class="mc-leader" x1="${busPt.x.toFixed(1)}" y1="${busPt.y.toFixed(1)}" x2="${sx.toFixed(1)}" y2="${busPt.y.toFixed(1)}"/>`;
		terminals.forEach((t, k) => {
			const cy = top + k * STACK_DY;
			const grounded = bus.grounded.includes(t);
			const fill = isPhaseTerminal(t) ? col(phaseColor(t)) : col([120, 114, 102, 255]);
			if (grounded) stack += groundGlyph(sx - 14, cy);
			stack += `<circle cx="${sx.toFixed(1)}" cy="${cy.toFixed(1)}" r="6" fill="${fill}" stroke="#20242b" stroke-width="1.2"/>`;
			stack += `<text class="mc-term" x="${(sx + 11).toFixed(1)}" y="${(cy + 3.5).toFixed(1)}">${esc(t)}</text>`;
			// Attachment squares for elements landing on this terminal.
			const kinds = [...new Set((bus.terminalAttachments[t] ?? []).map((a) => a.kind))];
			kinds.slice(0, 5).forEach((kind, j) => {
				const bxp = sx + 26 + j * 9;
				stack += `<rect x="${bxp.toFixed(1)}" y="${(cy - 4).toFixed(1)}" width="7" height="7" rx="1.5" fill="${col(attachmentColor(kind))}"><title>${esc(kind)}</title></rect>`;
			});
		});

		multiDetailEl.setAttribute('viewBox', `0 0 ${clientWidth} ${clientHeight}`);
		multiDetailEl.style.display = 'block';
		multiDetailEl.innerHTML = `<g class="mc-strands">${strands}</g><g class="mc-stack">${stack}</g>`;
	}

	// Coalesce cue syncs to one per animation frame: 'move' fires for every
	// camera change (including zooms), and each sync reprojects the whole
	// selected branch path.
	let cueSyncScheduled = false;
	function scheduleCueSync() {
		if (cueSyncScheduled) return;
		cueSyncScheduled = true;
		requestAnimationFrame(() => {
			cueSyncScheduled = false;
			syncSelectedCue();
			syncSelectedBranchCue();
			syncMultiDetail();
		});
	}

	/** Camera padding that keeps the framed subject clear of the panel chrome.
	 * The branch variant also clears the rating readout on the panel's lower edge. */
	function framePadding(width: number, height: number, kind: 'case' | 'branch') {
		if (width <= 760) {
			const bottom =
				kind === 'branch'
					? Math.min(Math.round(height * 0.42), 300)
					: Math.min(Math.round(height * 0.5), 330);
			return { top: 130, left: 24, right: 24, bottom };
		}
		return kind === 'branch'
			? { top: 106, left: 388, right: 76, bottom: 80 }
			: { top: 96, left: 380, right: 60, bottom: 64 };
	}

	/** Fold lon/lat pairs into LngLat bounds; null when no point is finite. */
	function foldBounds(
		points: Iterable<[number, number]>
	): [[number, number], [number, number]] | null {
		let minLon = Infinity;
		let minLat = Infinity;
		let maxLon = -Infinity;
		let maxLat = -Infinity;
		for (const [lon, lat] of points) {
			minLon = Math.min(minLon, lon);
			minLat = Math.min(minLat, lat);
			maxLon = Math.max(maxLon, lon);
			maxLat = Math.max(maxLat, lat);
		}
		if (!Number.isFinite(minLon) || !Number.isFinite(minLat)) return null;
		return [
			[minLon, minLat],
			[maxLon, maxLat]
		];
	}

	function branchBounds(path: [number, number][]): LngLatBoundsLike | null {
		const bounds = foldBounds(path);
		if (!bounds) return null;
		let [[minLon, minLat], [maxLon, maxLat]] = bounds;
		if (minLon === maxLon) {
			minLon -= 0.005;
			maxLon += 0.005;
		}
		if (minLat === maxLat) {
			minLat -= 0.005;
			maxLat += 0.005;
		}
		return [
			[minLon, minLat],
			[maxLon, maxLat]
		];
	}

	function prefersReducedMotion(): boolean {
		return (
			typeof window !== 'undefined' &&
			window.matchMedia('(prefers-reduced-motion: reduce)').matches
		);
	}

	/** Fly to the selected branch; true when a camera move started. */
	function focusSelectedBranch(): boolean {
		if (!map || !selectedBranchCue) return false;
		const bounds = branchBounds(selectedBranchCue.path);
		if (!bounds) return false;
		const { clientWidth, clientHeight } = map.getContainer();
		const camera = map.cameraForBounds(bounds, {
			padding: framePadding(clientWidth, clientHeight, 'branch'),
			maxZoom: Math.min(map.getZoom() + 2, BRANCH_FOCUS_MAX_ZOOM)
		});
		if (!camera?.center) return false;
		map.easeTo({
			center: camera.center,
			zoom: camera.zoom ?? map.getZoom(),
			duration: prefersReducedMotion() ? 0 : BRANCH_FOCUS_MS,
			easing: (t) => 1 - Math.pow(1 - t, 3)
		});
		return true;
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
		const layerId = info.layer?.id;
		if (layerId?.startsWith('multi-edges-')) {
			const e = object as PlacedMultiEdge;
			const state = e.kind === 'switch' ? (e.closed ? ' &#8901; closed' : ' &#8901; open') : '';
			return {
				html: `<div class="tt"><b>${esc(e.kind)} ${esc(e.from)}&#8201;&ndash;&#8201;${esc(e.to)}</b> ${e.n_phases}&#8202;&phi;${state}</div>`
			};
		}
		if (layerId?.startsWith('multi-buses-')) {
			const b = object as PlacedMultiBus;
			const roles = b.attachmentKinds.length ? b.attachmentKinds.join(', ') : 'none';
			return {
				html: `<div class="tt"><b>bus ${esc(b.id)}</b>
					${b.terminals.length} terminals${b.grounded.length ? ` &#8901; ${b.grounded.length} grounded` : ''}<br>
					load ${b.load_kw.toFixed(0)} kW &#8901; gen ${b.gen_kw.toFixed(0)} kW<br>
					<span style="opacity:0.6">${esc(roles)}</span></div>`
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
			ScatterplotLayer: layers.ScatterplotLayer,
			IconLayer: layers.IconLayer
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
				if (!app.placingId && !isSelectableLayer(info.layer?.id)) onmapclick();
			},
			getTooltip: tooltip,
			getCursor: ({ isHovering, isDragging }: CursorState) =>
				app.placingId ? 'crosshair' : isDragging ? 'grabbing' : isHovering ? 'pointer' : 'grab'
		});
	}

	function initMap(container: HTMLDivElement) {
		let cleanup = () => {};
		let cancelled = false;
		void loadMapModules()
			.then(({ maplibregl, MapboxOverlay, PathLayer, ScatterplotLayer, IconLayer }) => {
				if (cancelled) return;
				layerCtors = { PathLayer, ScatterplotLayer, IconLayer };
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
					if (app.placingId) onplacecase(e.lngLat.lng, e.lngLat.lat);
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
				const syncCues = () => scheduleCueSync();
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
				// 'move' fires for every camera change, including zooms.
				m.on('load', syncCues);
				m.on('move', syncCues);
				m.on('resize', syncCues);

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
					m.off('load', syncCues);
					m.off('move', syncCues);
					m.off('resize', syncCues);
					app.settleFrame();
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
		const { PathLayer, ScatterplotLayer, IconLayer } = layerCtors;
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
		// Multiconductor cases (viewing only): the bus graph with edges styled by
		// kind and buses badged by attachment role. Selecting a bus fans its
		// incident conductors and expands its terminal stack in the SVG overlay.
		for (const c of app.multiCases) {
			if (!c.view) continue;
			const selectedId = c.id === app.activeMultiId ? c.selectedBusId : null;
			const selectedEdgeId = c.id === app.activeMultiId ? c.selectedEdgeId : null;
			layers.push(
				new PathLayer<PlacedMultiEdge>({
					id: `multi-edges-${c.id}`,
					data: c.view.edges,
					getPath: (e) => e.path,
					// The selected edge draws in the selection blue of the branch cue,
					// wider, so its panel detail has an obvious source on the map.
					getColor: (e) =>
						e.id === selectedEdgeId ? [47, 111, 187, 255] : edgeColor(e.kind, e.closed),
					getWidth: (e) => edgeWidth(e.kind, e.n_phases) + (e.id === selectedEdgeId ? 2 : 0),
					widthUnits: 'pixels',
					widthMinPixels: 1.2,
					capRounded: true,
					jointRounded: true,
					miterLimit: 2,
					pickable: true,
					autoHighlight: true,
					highlightColor: [32, 36, 43, 90],
					onClick: (info: PickingInfo) => {
						const e = info.object as PlacedMultiEdge | undefined;
						if (!e) return false;
						onmultiedgeclick(c.id, e.id);
						return true;
					},
					updateTriggers: {
						getColor: [selectedEdgeId],
						getWidth: [selectedEdgeId]
					}
				}),
				// The IEC two-circle symbol at each transformer midpoint, rotated into
				// the edge bearing. Not pickable: hover and click fall through to the
				// transformer edge beneath it.
				new IconLayer<TransformerMark>({
					id: `multi-xfmr-${c.id}`,
					data: transformerMarks(c.view.edges),
					getIcon: () => transformerIcon(),
					getPosition: (m) => m.position,
					getAngle: (m) => m.angle,
					billboard: false,
					getSize: 16,
					sizeUnits: 'pixels',
					getColor: (m) =>
						m.id === selectedEdgeId ? [47, 111, 187, 255] : edgeColor('transformer', true),
					pickable: false,
					updateTriggers: {
						getColor: [selectedEdgeId]
					}
				}),
				new ScatterplotLayer<PlacedMultiBus>({
					id: `multi-buses-${c.id}`,
					data: c.view.buses,
					getPosition: (b) => [b.lon, b.lat],
					getRadius: (b) => busRadius(Math.max(b.load_kw, b.gen_kw)),
					radiusUnits: 'pixels',
					radiusMinPixels: 3,
					getFillColor: (b) => multiBusFill(b),
					stroked: true,
					billboard: true,
					getLineColor: (b) =>
						selectedId === b.id
							? [32, 36, 43, 255]
							: b.has_source
								? [63, 111, 187, 220]
								: [46, 42, 34, 120],
					getLineWidth: (b) => (selectedId === b.id ? 2.5 : b.has_source ? 2 : 1),
					lineWidthUnits: 'pixels',
					pickable: true,
					autoHighlight: true,
					highlightColor: [32, 36, 43, 70],
					onClick: (info: PickingInfo) => {
						const bus = info.object as PlacedMultiBus | undefined;
						if (!bus) return false;
						onmultibusclick(c.id, bus.id);
						return true;
					},
					updateTriggers: {
						getLineColor: [selectedId],
						getLineWidth: [selectedId]
					}
				})
			);
		}
		overlay.setProps({ layers });
	});

	$effect(() => {
		if (!map) return;
		map.getCanvas().style.cursor = app.placingId ? 'crosshair' : '';
	});

	$effect(() => {
		if (!map) return;
		void mapGen;
		void selectedBusCue?.key;
		void selectedBranchCue?.key;
		void selectedMultiDetail?.key;
		void tick().then(scheduleCueSync);
	});

	function boundsFor(target: string): LngLatBoundsLike | null {
		const points: [number, number][] = [];
		const fold = (pts: { lon: number; lat: number }[]) => {
			for (const b of pts) points.push([b.lon, b.lat]);
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
		for (const c of app.multiCases) {
			if (target !== 'all' && c.id !== target) continue;
			if (c.view) fold(c.view.buses);
		}
		return foldBounds(points);
	}

	// Fly to whatever the header, the initial load, or a branch selection asked
	// for, and settle the request's promise once the camera lands so callers can
	// defer heavy work past the animation. A branch request waits (unhandled)
	// until its cue exists; anything unframeable settles immediately.
	$effect(() => {
		const seq = app.frameSeq;
		if (!map || seq === handledFrameSeq) return;
		const m = map;
		const target = app.frameTarget;
		const settle = () => app.settleFrame();
		if (typeof target === 'object') {
			const cue = selectedBranchCue;
			if (!cue || cue.caseId !== target.caseId || cue.branchId !== target.branchId) return;
			handledFrameSeq = seq;
			if (focusSelectedBranch()) m.once('moveend', settle);
			else settle();
			return;
		}
		handledFrameSeq = seq;
		const bounds = boundsFor(target);
		if (!bounds) {
			settle();
			return;
		}
		const { clientWidth: w, clientHeight: h } = m.getContainer();
		m.fitBounds(bounds, {
			padding: framePadding(w, h, 'case'),
			duration: prefersReducedMotion() ? 0 : 1400
		});
		m.once('moveend', settle);
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
		{#if selectedBranchCue}
			{#key selectedBranchCue.key}
				<svg class="selected-branch-cue" bind:this={selectedBranchCueEl} aria-hidden="true">
					<path class="selected-branch-halo" bind:this={selectedBranchHaloPath}></path>
					<path class="selected-branch-line" bind:this={selectedBranchLinePath}></path>
				</svg>
			{/key}
		{/if}
		{#if selectedMultiDetail}
			<!-- Content is built imperatively in syncMultiDetail (screen-space
			     projection); file-derived text is escaped there. -->
			<svg class="multi-detail" bind:this={multiDetailEl} aria-hidden="true"></svg>
		{/if}
	</div>
{/key}

{#if app.placingId && map}
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

	.selected-branch-cue {
		position: absolute;
		inset: 0;
		z-index: calc(var(--z-chrome) - 1);
		display: none;
		width: 100%;
		height: 100%;
		overflow: visible;
		pointer-events: none;
	}

	.multi-detail {
		position: absolute;
		inset: 0;
		z-index: calc(var(--z-chrome) - 1);
		display: none;
		width: 100%;
		height: 100%;
		overflow: visible;
		pointer-events: none;
	}

	.multi-detail :global(.mc-leader) {
		stroke: rgba(32, 36, 43, 0.35);
		stroke-width: 1;
		stroke-dasharray: 2 2;
	}

	.multi-detail :global(.mc-ground line) {
		stroke: #3a3a3a;
		stroke-width: 1.3;
	}

	.multi-detail :global(.mc-term) {
		fill: var(--ink, #20242b);
		font-family: var(--font-mono);
		font-size: 10px;
	}

	.selected-branch-cue path {
		fill: none;
		stroke-linecap: round;
		stroke-linejoin: round;
		vector-effect: non-scaling-stroke;
	}

	.selected-branch-halo {
		stroke: rgba(47, 111, 187, 0.2);
		stroke-width: 14;
		filter: drop-shadow(0 0 14px rgba(47, 111, 187, 0.2));
		animation:
			selected-branch-pop 420ms cubic-bezier(0.34, 1.56, 0.64, 1),
			selected-branch-pulse 1.8s ease-in-out 420ms infinite;
	}

	.selected-branch-line {
		stroke: rgba(47, 111, 187, 0.92);
		stroke-width: 3;
		animation: selected-branch-line-pop 420ms cubic-bezier(0.34, 1.56, 0.64, 1);
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

	@keyframes selected-branch-pop {
		0% {
			opacity: 0;
			stroke-width: 4;
		}
		65% {
			opacity: 0.95;
			stroke-width: 18;
		}
		100% {
			opacity: 0.82;
			stroke-width: 14;
		}
	}

	@keyframes selected-branch-line-pop {
		0% {
			opacity: 0;
			stroke-width: 1;
		}
		65% {
			opacity: 1;
			stroke-width: 4;
		}
		100% {
			opacity: 0.95;
			stroke-width: 3;
		}
	}

	@keyframes selected-branch-pulse {
		0%,
		100% {
			opacity: 0.82;
			stroke-width: 14;
			filter: drop-shadow(0 0 14px rgba(47, 111, 187, 0.2));
		}
		50% {
			opacity: 0.42;
			stroke-width: 22;
			filter: drop-shadow(0 0 24px rgba(47, 111, 187, 0.16));
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
		.selected-bus-cue,
		.selected-branch-halo,
		.selected-branch-line {
			animation: none;
		}
	}
</style>
