<script lang="ts" module>
	// Counter for local case ids; module level so ids stay unique across remounts.
	let localSeq = 0;
</script>

<script lang="ts">
	import { onMount } from 'svelte';
	import {
		getCaseNetworkJson,
		getCases,
		getNetwork,
		getSensitivity,
		getSolution,
		openSolveStream
	} from '$lib/api';
	import type { Network, SensitivityColumn, Solution } from '$lib/api';
	import {
		busRadius,
		lmpDomain,
		lmpGradient,
		sensFlatColor,
		sensGradient,
		sensitivityDomain,
		type RGBA
	} from '$lib/colors';
	import {
		applyGeoFile,
		isGeoFile,
		mergeGeoFiles,
		parseGeoFile,
		type GeoFile
	} from '$lib/geo-file';
	import {
		app,
		CaseState,
		LocalCase,
		type DemandRangeMode,
		type DisplayMode
	} from '$lib/state.svelte';
	import { placeSyntheticTopology } from '$lib/synthetic-layout';
	import {
		createStudy,
		FORMULATIONS,
		formatOf,
		ingestCase,
		isDisplayFile,
		isPermanentSensFailure,
		parseDisplay,
		solveDc,
		type BrowserStudy,
		type Formulation
	} from '$lib/wasm';
	import Sparkline from '$lib/Sparkline.svelte';
	import TellegenMap from '$lib/TellegenMap.svelte';

	let abort: AbortController | null = null;
	let fileInput = $state.raw<HTMLInputElement | undefined>(undefined);
	let showFileDropUi = $state(true);
	let casesLoaded = $state(false);
	let loadingBackendCase = $state<string | null>(null);
	let dragDepth = 0;

	const FILE_DROP_QUERY = '(hover: hover) and (pointer: fine) and (min-width: 761px)';
	const HIDDEN_DEFAULT_CASES_KEY = 'tellegen.hiddenDefaultCases.v1';
	// Open on South Carolina by default: it has the most interesting price action.
	const DEFAULT_CASE_ID = 'case500';
	type SolvableCase = CaseState | LocalCase;
	type DemandRangeAnchor = {
		caseId: string;
		bus: number;
		delta: number;
	};
	type DisplayOption = {
		mode: DisplayMode;
		label: string;
		unit: string;
		copy: string;
		gradient: string;
		values: { bus: number; value: number }[];
	};

	let nearbyRangeAnchor = $state<DemandRangeAnchor | null>(null);
	// Default cases the user has closed; drives the restore affordance. Seeded in load().
	let hiddenDefaults = $state<Set<string>>(new Set());

	function isBackendCase(c: SolvableCase): c is CaseState {
		return c instanceof CaseState;
	}

	function isActiveSolveCase(c: SolvableCase): boolean {
		return isBackendCase(c) ? app.activeCaseId === c.id : app.activeLocalId === c.id;
	}

	function caseDeltas(c: SolvableCase) {
		return isBackendCase(c) ? c.deltas : (c.deltas ?? {});
	}

	function isPerturbed(c: SolvableCase | null): boolean {
		return c ? Object.values(caseDeltas(c)).some((mw) => mw !== 0) : false;
	}

	function setNearbyRangeAnchor(c: SolvableCase, bus: number, delta = caseDeltas(c)[bus] ?? 0) {
		nearbyRangeAnchor = { caseId: c.id, bus, delta };
	}

	function errorText(e: unknown): string {
		return e instanceof Error ? e.message : String(e);
	}

	/** The short menu label for a formulation tag (e.g. `acopf` -> `AC OPF`). */
	function formulationLabel(id: Formulation): string {
		return FORMULATIONS.find((f) => f.id === id)?.label ?? id;
	}

	function formulationHint(id: Formulation): string {
		return FORMULATIONS.find((f) => f.id === id)?.hint ?? id;
	}

	function priceCopy(id: Formulation): string {
		return id === 'socwr'
			? 'SOCWR active power balance prices. Select a bus for ∂LMP/∂d and demand perturbation.'
			: 'DC OPF prices. Select a bus for ∂LMP/∂d and demand perturbation.';
	}

	function displayOptionsFor(c: SolvableCase | null): DisplayOption[] {
		if (!c?.solution) return [];
		const options: DisplayOption[] = [
			{
				mode: 'lmp',
				label: 'LMP',
				unit: '$/MWh',
				copy: priceCopy(c.formulation),
				gradient: lmpGradient,
				values: c.solution.lmp.map((e) => ({ bus: e.bus, value: e.usd_per_mwh }))
			}
		];
		if (c.formulation === 'dcopf' && c.solution.va.length > 0) {
			options.push({
				mode: 'angle',
				label: 'angle',
				unit: 'rad',
				copy: 'DC bus voltage phase angle from the current OPF solution.',
				gradient: lmpGradient,
				values: c.solution.va
			});
		}
		if (c.formulation === 'socwr' && c.solution.w.length > 0) {
			options.push({
				mode: 'voltage',
				label: '|V|',
				unit: 'pu',
				copy: 'SOCWR voltage magnitude from the current relaxed solution.',
				gradient: lmpGradient,
				values: c.solution.w.map((s) => ({ bus: s.bus, value: Math.sqrt(Math.max(0, s.value)) }))
			});
		}
		return options;
	}

	function displayDomain(mode: DisplayMode, values: number[]): { lo: number; hi: number } {
		if (mode === 'lmp') return lmpDomain(values);
		if (values.length === 0) return { lo: 0, hi: 1 };
		const rawLo = Math.min(...values);
		const rawHi = Math.max(...values);
		const minSpan = mode === 'voltage' ? 0.02 : 0.04;
		const span = Math.max(rawHi - rawLo, minSpan);
		const mid = (rawLo + rawHi) / 2;
		return { lo: mid - span / 2, hi: mid + span / 2 };
	}

	/** The display name of a case: a backend case's `name`, a local case's `label`. */
	function caseName(c: SolvableCase): string {
		return isBackendCase(c) ? c.name : c.label;
	}

	function readHiddenDefaultCases(): Set<string> {
		if (typeof localStorage === 'undefined') return new Set();
		try {
			const parsed = JSON.parse(localStorage.getItem(HIDDEN_DEFAULT_CASES_KEY) ?? '[]');
			return new Set(Array.isArray(parsed) ? parsed.filter((id) => typeof id === 'string') : []);
		} catch {
			return new Set();
		}
	}

	function writeHiddenDefaultCases(ids: Set<string>) {
		if (typeof localStorage === 'undefined') return;
		try {
			localStorage.setItem(HIDDEN_DEFAULT_CASES_KEY, JSON.stringify([...ids].sort()));
		} catch {
			// Current session removal still works; persistence is best effort.
		}
	}

	function rememberHiddenDefaultCase(id: string) {
		const hidden = readHiddenDefaultCases();
		hidden.add(id);
		writeHiddenDefaultCases(hidden);
		hiddenDefaults = new Set(hidden);
	}

	function restoreDefaultCases() {
		try {
			if (typeof localStorage !== 'undefined') localStorage.removeItem(HIDDEN_DEFAULT_CASES_KEY);
		} catch {
			// Ignore storage failures and reload from the server.
		}
		hiddenDefaults = new Set();
		load();
	}

	function rgbaCss([r, g, b, a]: RGBA): string {
		return `rgba(${r}, ${g}, ${b}, ${(a / 255).toFixed(3)})`;
	}

	onMount(() => {
		const query = window.matchMedia(FILE_DROP_QUERY);
		const syncFileDropUi = () => {
			showFileDropUi = query.matches;
			if (!showFileDropUi) {
				dragDepth = 0;
				app.dragOver = false;
			}
		};
		syncFileDropUi();
		query.addEventListener('change', syncFileDropUi);
		return () => query.removeEventListener('change', syncFileDropUi);
	});

	async function load() {
		try {
			const summaries = await getCases();
			const hidden = readHiddenDefaultCases();
			hiddenDefaults = new Set(hidden);
			app.cases = summaries.filter((s) => !hidden.has(s.id)).map((s) => new CaseState(s));
			app.activeLocalId = null;
			app.placingLocalId = null;
			app.activeCaseId =
				app.cases.find((c) => c.id === DEFAULT_CASE_ID)?.id ?? app.cases[0]?.id ?? null;
			const active = app.active;
			if (active) await loadBackendCase(active, true);
			else app.requestFrame('all');
		} catch (e) {
			app.error = `server unreachable: ${e instanceof Error ? e.message : e}`;
		} finally {
			casesLoaded = true;
		}
	}

	load();

	async function loadBackendCase(c: CaseState, frame = false) {
		if (c.network && c.solution && c.baseSolution) {
			if (frame && app.activeCaseId === c.id) app.requestFrame(c.id);
			return;
		}
		loadingBackendCase = c.id;
		const requestedFormulation = c.formulation;
		try {
			const [network, dcBaseSolution] = await Promise.all([
				c.network ? Promise.resolve(c.network) : getNetwork(c.id),
				// The server caches only the DC OPF base solution. Other formulations
				// must hydrate from the browser Study so their cost, prices, flows, and
				// voltage fields all come from the selected formulation.
				requestedFormulation === 'dcopf' ? getSolution(c.id) : Promise.resolve(null)
			]);
			if (!app.byId(c.id)) return;
			c.network = network;
			if (dcBaseSolution && c.formulation === requestedFormulation) {
				c.baseSolution = dcBaseSolution;
				c.solution = dcBaseSolution;
			}
			if (frame && app.activeCaseId === c.id) app.requestFrame(c.id);
			if (isActiveSolveCase(c) && c.formulation !== 'dcopf' && !c.solution && !c.solving) {
				runSolve(c, app.selectedBus);
			}
		} catch (e) {
			if (app.byId(c.id)) app.error = `${c.name}: ${errorText(e)}`;
		} finally {
			if (loadingBackendCase === c.id) loadingBackendCase = null;
		}
	}

	function localNetwork(c: LocalCase): Network | null {
		if (!c.summary || !c.view) return null;
		return {
			id: c.id,
			name: c.label,
			base_mva: c.summary.base_mva,
			synthetic_coords: c.coordsKind !== 'file' && c.coordsKind !== 'geofile',
			buses: c.view.buses,
			branches: c.view.branches
		};
	}

	function maybeStartLocalSolve(id: string) {
		const c = app.localCases.find((lc) => lc.id === id);
		if (!c?.networkJson || !c.view || !c.summary) return;
		c.network = localNetwork(c) ?? c.network ?? null;
		if (c.networkJson && c.network && !c.solution) runSolve(c, null);
	}

	async function activateCase(id: string) {
		app.activeLocalId = null;
		app.placingLocalId = null;
		if (app.activeCaseId !== id) {
			clearSelection();
			app.activeCaseId = id;
		}
		const c = app.byId(id);
		if (c) await loadBackendCase(c, true);
	}

	async function removeBackendCase(c: CaseState, event?: MouseEvent) {
		event?.stopPropagation();
		rememberHiddenDefaultCase(c.id);
		// Tear down this case's own in-flight server stream whether or not it is the
		// active case (a non-active case can still hold a live stream), and bump the
		// seq so any detached handler no-ops.
		c.closeStream?.();
		c.closeStream = null;
		c.solveSeq++;
		disposeStudy(c);
		if (app.activeCaseId === c.id) clearSelection();
		app.removeCase(c.id);
		const active = app.active;
		if (active) await loadBackendCase(active, true);
	}

	function activateLocal(c: LocalCase) {
		clearSelection();
		// Mirror activateCase's reset: a local and a backend case are mutually
		// exclusive, so drop the backend selection. Otherwise app.active (derived
		// from activeCaseId) stays set and its solve card keeps hovering over the
		// local view.
		app.activeCaseId = null;
		app.activeLocalId = c.id;
		app.placingLocalId = c.coordsKind === 'synthetic_pending' ? c.id : null;
		if (c.view || c.substations) app.requestFrame(c.id);
		maybeStartLocalSolve(c.id);
	}

	function removeLocalCase(c: LocalCase, event?: MouseEvent) {
		event?.stopPropagation();
		if (app.activeLocalId === c.id) {
			// Local cases solve in the browser only (no server stream); the seq bump
			// invalidates any in-flight browser solve.
			c.solveSeq = (c.solveSeq ?? 0) + 1;
			clearSelection();
		}
		disposeStudy(c);
		app.removeLocal(c.id);
	}

	function addAndActivateLocal(c: LocalCase) {
		clearSelection();
		app.activeCaseId = null;
		app.addLocal(c);
		if (c.view || c.substations) app.requestFrame(c.id);
		maybeStartLocalSolve(c.id);
	}

	function placeLocalCase(lon: number, lat: number) {
		const id = app.placingLocalId;
		const c = id ? app.localCases.find((lc) => lc.id === id) : null;
		if (!c?.topology) return;
		c.view = placeSyntheticTopology(c.topology, { lon, lat });
		c.coordsKind = 'synthetic';
		c.syntheticCenter = { lon, lat };
		app.placingLocalId = null;
		app.activeLocalId = c.id;
		app.requestFrame(c.id);
		maybeStartLocalSolve(c.id);
	}

	function moveLocalCase(c: LocalCase) {
		app.activeCaseId = null;
		app.activeLocalId = c.id;
		app.placingLocalId = c.id;
	}

	function withGeoFile(c: LocalCase, geoFiles: GeoFile[]): LocalCase {
		if (!c.topology || geoFiles.length === 0) return c;
		const applied = applyGeoFile(c.topology, mergeGeoFiles(geoFiles));
		c.view = applied.view;
		c.coordsKind = 'geofile';
		c.syntheticCenter = undefined;
		c.geoSource = applied.sourceLabel;
		c.geoWarnings = [
			`${applied.matchedBuses} buses placed from ${applied.sourceLabel}`,
			...(applied.matchedBranches > 0
				? [`${applied.matchedBranches} branch paths matched from geographic file data`]
				: []),
			...applied.warnings
		];
		return c;
	}

	function applyGeoFilesToExisting(geoFiles: GeoFile[]) {
		const target =
			(app.activeLocal?.topology ? app.activeLocal : null) ??
			app.localCases.find((c) => c.coordsKind === 'synthetic_pending') ??
			[...app.localCases].reverse().find((c) => c.topology);
		if (!target?.topology) {
			app.error = 'drop a case file with the geographic file, or select a parsed local case first';
			return;
		}
		try {
			withGeoFile(target, geoFiles);
			app.activeCaseId = null;
			app.activeLocalId = target.id;
			app.placingLocalId = null;
			app.requestFrame(target.id);
			maybeStartLocalSolve(target.id);
			app.error = null;
		} catch (e) {
			app.error = `${geoFiles.map((s) => s.sourceNames.join(' + ')).join(' + ')}: ${
				e instanceof Error ? e.message : e
			}; use place on map for manual placement`;
		}
	}

	// Fetch and cache the raw powerio Network JSON for the browser solver.
	// Returns null when it can't be loaded, so callers fall back to the server.
	async function ensureNetworkJson(c: SolvableCase): Promise<string | null> {
		if (!isBackendCase(c)) return c.networkJson ?? null;
		if (c.networkJson) return c.networkJson;
		try {
			const json = await getCaseNetworkJson(c.id);
			c.networkJson = json;
			return json;
		} catch (e) {
			c.solveFallbackReason = `case fetch failed: ${errorText(e)}`;
			return null;
		}
	}

	// Build-once browser Study per case: the network is parsed and the model built
	// when the Study is created, so a drag re-solves (commit) and previews without
	// re-parsing. Kept in a WeakMap off the reactive/raw case payloads — the wasm
	// handle is neither serialized nor part of any $state.
	const caseStudies = new WeakMap<
		SolvableCase,
		{ study: BrowserStudy; networkJson: string; formulation: Formulation; baseSolution: Solution }
	>();
	// Latch a permanent sensitivity-module failure (the sens build's relaxed SIMD,
	// which Safari rejects) per case so we don't retry createStudy — and the same
	// permanent error — on every drag. Transient failures are not latched.
	const studyUnavailable = new WeakMap<SolvableCase, string>();

	// The case's Study, building it once for `(networkJson, formulation)` and rebuilding
	// (after free) if either changed — so picking a new formulation re-parses and re-solves
	// under that formulation. Returns null when the sens module can't load; the caller then
	// falls back to solveDc/the server, surfacing solveFallbackReason.
	async function getStudy(c: SolvableCase, networkJson: string): Promise<BrowserStudy | null> {
		const latched = studyUnavailable.get(c);
		if (latched) {
			c.solveFallbackReason ??= latched;
			return null;
		}
		const cached = caseStudies.get(c);
		if (cached && cached.networkJson === networkJson && cached.formulation === c.formulation)
			return cached.study;
		if (cached) {
			cached.study.free();
			caseStudies.delete(c);
		}
		try {
			const study = await createStudy(networkJson, c.formulation);
			const baseSolution = study.currentSolution();
			caseStudies.set(c, { study, networkJson, formulation: c.formulation, baseSolution });
			return study;
		} catch (e) {
			const message = errorText(e);
			// Only a genuine browser-capability failure is permanent; latch it so the
			// case stays on the fallback path. Transient errors stay retryable.
			if (isPermanentSensFailure(message)) {
				studyUnavailable.set(c, 'browser study needs SIMD this browser does not support');
			}
			c.solveFallbackReason ??= `browser study unavailable: ${message}`;
			return null;
		}
	}

	// The dLMP/dd column at `busId` for the case's active formulation, solved in the browser.
	// DC OPF takes the light per-call `solveDc` path (byte-identical to the prior behavior, so
	// no regression for the default). AC OPF / SOCWR go through the Study, whose exact re-solve
	// returns the column under the right formulation; the column is null (with the reason on
	// `solveFallbackReason`) when the Study can't be built. Throws only on a hard browser error.
	async function browserSensitivity(
		c: SolvableCase,
		networkJson: string,
		busId: number
	): Promise<SensitivityColumn | null> {
		if (c.formulation === 'dcopf') {
			return (await solveDc(c.id, networkJson, caseDeltas(c), busId)).sensitivity;
		}
		const study = await getStudy(c, networkJson);
		return study ? study.sensitivity(c.id, caseDeltas(c), busId) : null;
	}

	// Release a case's Study (if any) when the case is removed.
	function disposeStudy(c: SolvableCase) {
		const cached = caseStudies.get(c);
		if (cached) {
			cached.study.free();
			caseStudies.delete(c);
		}
	}

	function acceptSensitivity(
		c: SolvableCase,
		col: SensitivityColumn | null,
		busId: number | null,
		sensitivitySeq?: number
	) {
		if (!col || busId === null) return;
		if (col.bus !== busId) return;
		if (!isActiveSolveCase(c) || app.selectedBus !== busId) return;
		if (sensitivitySeq !== undefined && sensitivitySeq !== (c.sensitivitySeq ?? 0)) return;
		c.sensitivity = col;
	}

	function finishSolve(c: SolvableCase, seq: number, sensBus: number | null) {
		if (seq !== (c.solveSeq ?? 0)) return;
		c.solving = false;
		if (isActiveSolveCase(c) && app.selectedBus === sensBus) {
			app.previewActive = false;
			app.previewDeltaMw = null;
			// The committed solution supersedes the live engine preview.
			app.previewLmp = null;
			previewObjective = null;
		}
	}

	async function selectBus(caseId: string, busId: number) {
		app.activeLocalId = null;
		app.placingLocalId = null;
		if (app.activeCaseId !== caseId) {
			clearSelection();
			app.activeCaseId = caseId;
		}
		const c = app.byId(caseId);
		if (!c) return;
		abort?.abort();
		const ac = new AbortController();
		abort = ac;
		const sensitivitySeq = ++c.sensitivitySeq;
		app.error = null;
		app.selectedBus = busId;
		app.previewDeltaMw = null;
		app.previewActive = false;
		app.demandRangeMode = 'local';
		setNearbyRangeAnchor(c, busId);
		app.sensitivityLoading = true;
		c.sensitivity = null;
		try {
			// The dLMP/dd column from the browser solver (under the case's formulation). DC OPF
			// may reconcile a null column via the server; AC OPF / SOCWR are browser-only, so a
			// null column there is reported, never sent to the DC-only server.
			const networkJson = await ensureNetworkJson(c);
			if (networkJson) {
				const sensitivity = await browserSensitivity(c, networkJson, busId);
				if (!ac.signal.aborted && sensitivity)
					acceptSensitivity(c, sensitivity, busId, sensitivitySeq);
				else if (!ac.signal.aborted && c.formulation === 'dcopf') {
					const col = await getSensitivity(caseId, busId, c.deltas, ac.signal);
					if (!ac.signal.aborted) acceptSensitivity(c, col, busId, sensitivitySeq);
				} else if (!ac.signal.aborted && !sensitivity) {
					app.error = `${c.name}: ${formulationLabel(c.formulation)} sensitivity unavailable in the browser${c.solveFallbackReason ? `: ${c.solveFallbackReason}` : ''}`;
				}
			} else if (c.formulation === 'dcopf') {
				const col = await getSensitivity(caseId, busId, c.deltas, ac.signal);
				if (!ac.signal.aborted) acceptSensitivity(c, col, busId, sensitivitySeq);
			} else if (!ac.signal.aborted) {
				app.error = `${c.name}: ${formulationLabel(c.formulation)} needs the browser network JSON, which is unavailable`;
			}
		} catch {
			// The browser path threw. DC OPF reconciles via the server; AC OPF / SOCWR have no
			// server fallback (nothing is solved on the server), so the column stays absent.
			if (c.formulation === 'dcopf') {
				try {
					const col = await getSensitivity(caseId, busId, c.deltas, ac.signal);
					if (!ac.signal.aborted) acceptSensitivity(c, col, busId, sensitivitySeq);
				} catch (e2) {
					if (!ac.signal.aborted && !(e2 instanceof DOMException)) app.error = String(e2);
				}
			}
		} finally {
			if (abort === ac) app.sensitivityLoading = false;
		}
	}

	async function selectLocalBus(localId: string, busId: number) {
		const c = app.localCases.find((lc) => lc.id === localId);
		if (!c?.networkJson || !c.network) return;
		app.activeCaseId = null;
		app.activeLocalId = localId;
		app.placingLocalId = null;
		abort?.abort();
		const ac = new AbortController();
		abort = ac;
		c.sensitivitySeq = (c.sensitivitySeq ?? 0) + 1;
		const sensitivitySeq = c.sensitivitySeq;
		app.error = null;
		app.selectedBus = busId;
		app.previewDeltaMw = null;
		app.previewActive = false;
		app.demandRangeMode = 'local';
		setNearbyRangeAnchor(c, busId);
		app.sensitivityLoading = true;
		c.sensitivity = null;
		try {
			const sensitivity = await browserSensitivity(c, c.networkJson, busId);
			if (!ac.signal.aborted) acceptSensitivity(c, sensitivity, busId, sensitivitySeq);
			if (!ac.signal.aborted && !sensitivity) {
				// A null column means the solve ran but produced no dLMP/dd for this bus (or the
				// Study could not be built); local cases have no server fallback, so say so
				// instead of leaving the panel in LMP view with no explanation.
				app.error = `${c.label}: ${formulationLabel(c.formulation)} sensitivity unavailable in the browser${
					c.solveFallbackReason ? `: ${c.solveFallbackReason}` : ' (no dLMP/dd column for this bus)'
				}`;
			}
		} catch (e) {
			if (!ac.signal.aborted) app.error = `${c.label}: ${e instanceof Error ? e.message : e}`;
		} finally {
			if (abort === ac) app.sensitivityLoading = false;
		}
	}

	function clearSelection() {
		abort?.abort();
		const c = app.active;
		if (c) {
			c.sensitivitySeq++;
			c.sensitivity = null;
		}
		const lc = app.activeLocal;
		if (lc) {
			lc.sensitivitySeq = (lc.sensitivitySeq ?? 0) + 1;
			lc.sensitivity = null;
		}
		app.selectedBus = null;
		app.previewDeltaMw = null;
		app.previewActive = false;
		app.previewLmp = null;
		previewObjective = null;
		app.demandRangeMode = 'local';
		nearbyRangeAnchor = null;
		app.sensitivityLoading = false;
	}

	// Exact DC solve in the browser (wasm). The build-once Study commits the new
	// operating point without re-parsing, returning the dLMP/dd column in the same
	// solve; on a Study failure it falls back to solveDc, and on any browser failure
	// or missing network JSON it reconciles via the server stream (backend cases).
	function runSolve(c: SolvableCase, sensBus: number | null) {
		// Cancel this case's own previous server stream, if any (backend only).
		if (isBackendCase(c)) {
			c.closeStream?.();
			c.closeStream = null;
		}
		c.solveSeq = (c.solveSeq ?? 0) + 1;
		const seq = c.solveSeq;
		app.error = null;
		c.solving = true;
		c.solveBackend = null;
		c.solveFallbackReason = null;
		c.iterations = [];
		c.solveMs = null;
		ensureNetworkJson(c).then(async (networkJson) => {
			if (seq !== (c.solveSeq ?? 0)) return;
			if (!networkJson) {
				c.solveFallbackReason ??= 'browser network JSON unavailable';
				if (isBackendCase(c)) return serverSolve(c, sensBus, seq);
				c.solving = false;
				app.error = `${c.label}: local case has no browser network JSON`;
				return;
			}
			const t0 = performance.now();
			c.solveBackend = 'clarabel-wasm';

			// Build-once Study path: commit the new operating point (no re-parse). The
			// dLMP/dd column for the selected bus is computed in the same solve and comes
			// back with it, so there is no second solve to reconcile it.
			const study = await getStudy(c, networkJson);
			if (seq !== (c.solveSeq ?? 0)) return;
			if (study) {
				try {
					const cached = caseStudies.get(c);
					if (!c.baseSolution && cached?.study === study) c.baseSolution = cached.baseSolution;
					const { solution, iterations, sensitivity } = study.commit(c.id, caseDeltas(c), sensBus);
					if (seq !== (c.solveSeq ?? 0)) return;
					c.solution = solution;
					c.iterations = iterations;
					if (!c.baseSolution && Object.keys(caseDeltas(c)).length === 0) c.baseSolution = solution;
					c.solveMs = Math.round(performance.now() - t0);
					// The commit carried the dLMP/dd column; accept it for the selected bus
					// through the same seq-guarded setter every sensitivity source goes through.
					if (sensBus !== null && sensitivity) acceptSensitivity(c, sensitivity, sensBus);
					finishSolve(c, seq, sensBus);
					return;
				} catch (e) {
					if (seq !== (c.solveSeq ?? 0)) return;
					// A built Study that fails to commit is unexpected; drop it so the next
					// solve rebuilds, then fall through to the solveDc fallback.
					disposeStudy(c);
					c.solveFallbackReason ??= `browser study commit failed: ${errorText(e)}`;
				}
			}

			// The DC fallbacks below (per-call browser solve_dc; the server stream) only solve
			// DC OPF, so they are valid only for the DC formulation. For AC OPF / SOCWR the
			// Study is the sole solver (nothing is solved on the server), so a Study failure
			// there is terminal: surface it rather than silently returning a DC solution.
			if (c.formulation !== 'dcopf') {
				if (seq !== (c.solveSeq ?? 0)) return;
				c.solving = false;
				const why = c.solveFallbackReason ?? 'browser study unavailable';
				app.error = `${caseName(c)}: ${formulationLabel(c.formulation)} runs only in the browser Study, which is unavailable: ${why}`;
				return;
			}

			// Fallback: the original per-call browser solve (re-parses each time). Used
			// when the Study can't be built (e.g. Safari's relaxed-SIMD gap) or a built
			// Study failed to commit; itself falls to the server for backend cases.
			solveDc(c.id, networkJson, caseDeltas(c), sensBus)
				.then(async ({ solution, sensitivity, sensitivityError, iterations }) => {
					if (seq !== (c.solveSeq ?? 0)) return;
					c.solution = solution;
					c.iterations = iterations;
					if (!c.baseSolution && Object.keys(caseDeltas(c)).length === 0) c.baseSolution = solution;
					c.solveMs = Math.round(performance.now() - t0);
					if (sensitivity || sensBus === null) {
						acceptSensitivity(c, sensitivity, sensBus);
					} else if (isBackendCase(c)) {
						// No browser sensitivity column (whether the solve threw or just
						// produced none): reconcile via the server for backend cases.
						try {
							const col = await getSensitivity(c.id, sensBus, c.deltas);
							if (seq !== c.solveSeq) return;
							c.solveBackend = 'clarabel-wasm-server-sensitivity';
							acceptSensitivity(c, col, sensBus);
						} catch (e) {
							if (seq !== c.solveSeq) return;
							c.solveFallbackReason = `server sensitivity failed: ${errorText(e)}`;
							serverSolve(c, sensBus, seq);
							return;
						}
					} else {
						// Local case: no server fallback, so report the gap (including a null
						// column with no error) instead of silently staying in LMP view.
						app.error = `${c.label}: browser sensitivity unavailable${sensitivityError ? `: ${sensitivityError}` : ' (no dLMP/dd column for this bus)'}`;
					}
					finishSolve(c, seq, sensBus);
				})
				.catch((e) => {
					if (seq !== (c.solveSeq ?? 0)) return;
					c.solveFallbackReason = `browser solve failed: ${errorText(e)}`;
					if (isBackendCase(c)) serverSolve(c, sensBus, seq);
					else {
						c.solving = false;
						app.error = `${c.label}: ${c.solveFallbackReason}`;
					}
				});
		});
	}

	function serverSolve(c: CaseState, sensBus: number | null, seq = c.solveSeq) {
		c.solveBackend = 'rust-server';
		c.solveFallbackReason ??= 'browser solve unavailable';
		c.closeStream = openSolveStream(c.id, c.deltas, sensBus, {
			onsolution: (sol) => {
				if (seq !== c.solveSeq) return;
				c.solution = sol;
				c.iterations = sol.iterations ?? [];
				c.solveMs = sol.solve_ms;
			},
			onsensitivity: (col) => {
				if (seq !== c.solveSeq) return;
				acceptSensitivity(c, col, sensBus);
			},
			onfail: (msg) => {
				if (seq !== c.solveSeq) return;
				c.solving = false;
				app.previewActive = false;
				app.previewDeltaMw = null;
				app.error = msg;
			},
			ondone: () => {
				finishSolve(c, seq, sensBus);
			}
		});
	}

	function commitDelta(value: number) {
		const c = activeSolvable;
		const bus = app.selectedBus;
		if (!c || bus === null) return;
		// Refresh the engine preview at the commit value (a typed value may not have
		// driven a drag), then score the commit with the engine's predicted Δobjective.
		runPreview(c, bus, value);
		c.predictedObjective = predictedDeltaObj;
		c.deltas = previewDeltas(c, bus, value);
		app.previewDeltaMw = value;
		app.previewActive = true;
		runSolve(c, bus);
	}

	function finishDemandInput(value: number) {
		if (Math.abs(value - committedDelta) < 0.25) {
			if (!activeSolvable?.solving) {
				app.previewActive = false;
				app.previewDeltaMw = null;
			}
			return;
		}
		commitDelta(value);
	}

	function resetCase(c: SolvableCase) {
		c.deltas = {};
		c.predictedObjective = null;
		app.previewLmp = null;
		previewObjective = null;
		app.previewDeltaMw = app.selectedBus === null ? null : 0;
		app.previewActive = app.selectedBus !== null;
		app.demandRangeMode = 'local';
		if (app.selectedBus !== null) setNearbyRangeAnchor(c, app.selectedBus, 0);
		if (c.baseSolution) c.solution = c.baseSolution;
		runSolve(c, app.selectedBus);
	}

	// Switch the active case to a new OPF formulation. Solving every formulation stays
	// entirely in the browser via the Study (nothing is routed to the server), so this
	// disposes the old Study — `getStudy` rebuilds it for the new formulation, re-parsing
	// and re-solving the base — then re-solves at the committed demand. The base solution
	// is dropped so it is recaptured under the new formulation (a DC and an AC objective
	// are not comparable). A no-op when the choice is unchanged.
	function changeFormulation(c: SolvableCase, next: Formulation) {
		if (c.formulation === next) return;
		// Disabled menu items (e.g. AC OPF, coming soon) are not selectable in the engine yet.
		if (FORMULATIONS.find((f) => f.id === next)?.disabled) return;
		c.formulation = next;
		// The committed point carries over (same demand), but the model and its solution do
		// not; drop the Study and the cached solutions so they rebuild under `next`.
		disposeStudy(c);
		c.baseSolution = null;
		c.solution = null;
		c.iterations = [];
		c.solveMs = null;
		c.predictedObjective = null;
		app.previewLmp = null;
		previewObjective = null;
		app.error = null;
		// The current sensitivity column was computed under the old formulation; clear it so
		// the overlay recomputes (the Study returns the new column with the re-solve below).
		c.sensitivity = null;
		c.sensitivitySeq = (c.sensitivitySeq ?? 0) + 1;
		runSolve(c, app.selectedBus);
	}

	// PowerWorld .pwd files store substation symbols at diagram coordinates,
	// not lat/lon. Auto-generated TAMU layouts are Web Mercator scaled by this
	// constant with both axes in degrees: x = K·lon and y = K·mercdeg(lat),
	// where mercdeg is the Mercator ordinate expressed in degrees. So lon = x/K,
	// and latitude is the inverse gudermannian after converting y/K back to
	// radians. Hand-edited diagrams drift from this, so positions stay
	// approximate. Verified against ACTIVSg200/2000 to within ~0.02 deg.
	const PWD_MERCATOR_K = 535.81608;
	function pwdToLngLat(x: number, y: number): [number, number] {
		const lon = x / PWD_MERCATOR_K;
		const lat = (Math.atan(Math.sinh(((y / PWD_MERCATOR_K) * Math.PI) / 180)) * 180) / Math.PI;
		return [lon, lat];
	}

	/** Parse dropped files in the browser via the powerio wasm module. Case
	 * files (.m, .raw, .aux) become local networks; geographic files can
	 * place those networks; a PowerWorld .pwd becomes a substation point
	 * preview. Files run serially; nothing uploads. */
	async function ingestFiles(files: FileList | File[]) {
		const list = Array.from(files);
		const geoFiles: GeoFile[] = [];
		for (const file of list.filter((f) => isGeoFile(f.name))) {
			app.parsingFile = true;
			try {
				geoFiles.push(parseGeoFile(file.name, await file.text()));
				app.error = null;
			} catch (e) {
				app.error = `${file.name}: ${e instanceof Error ? e.message : e}`;
			} finally {
				app.parsingFile = false;
			}
		}

		let parsedCaseCount = 0;
		for (const file of list.filter((f) => !isGeoFile(f.name))) {
			if (isDisplayFile(file.name)) {
				app.parsingFile = true;
				try {
					const bytes = new Uint8Array(await file.arrayBuffer());
					const display = await parseDisplay(bytes);
					const points = display.substations.map((s) => {
						const [lon, lat] = pwdToLngLat(s.x, s.y);
						return { number: s.number, name: s.name, lon, lat };
					});
					const id = `local-${++localSeq}`;
					addAndActivateLocal(
						new LocalCase({
							id,
							label: file.name.replace(/\.[^.]+$/, ''),
							fileName: file.name,
							summary: null,
							view: null,
							substations: { points, approximate: true }
						})
					);
					app.error = null;
				} catch (e) {
					app.error = `${file.name}: ${e instanceof Error ? e.message : e}`;
				} finally {
					app.parsingFile = false;
				}
				continue;
			}
			const format = formatOf(file.name);
			if (!format) {
				app.error = `${file.name}: not a case or coordinate file (.m, .raw, .aux, .pwd, .csv, .json, .geojson)`;
				continue;
			}
			app.parsingFile = true;
			try {
				const text = await file.text();
				const { network_json, topology, view, ...summary } = await ingestCase(text, format);
				if (format === 'aux' && (summary.n_branch === 0 || summary.n_gen === 0)) {
					app.error = `${file.name}: aux parsed, but no complete network; drop the matching .m or .raw case file`;
					continue;
				}
				const id = `local-${++localSeq}`;
				const label =
					summary.name && summary.name !== 'case'
						? summary.name
						: file.name.replace(/\.[^.]+$/, '');
				const local = new LocalCase({
					id,
					label,
					fileName: file.name,
					summary,
					networkJson: network_json,
					topology,
					coordsKind: summary.coords_kind,
					view
				});
				if (geoFiles.length > 0 && local.coordsKind === 'synthetic_pending') {
					withGeoFile(local, geoFiles);
				}
				addAndActivateLocal(local);
				parsedCaseCount++;
				app.error = null; // a successful parse clears a prior file's error
			} catch (e) {
				app.error = `${file.name}: ${e instanceof Error ? e.message : e}`;
			} finally {
				app.parsingFile = false;
			}
		}

		if (geoFiles.length > 0 && parsedCaseCount === 0) applyGeoFilesToExisting(geoFiles);
	}

	function dragHasFiles(e: DragEvent): boolean {
		return showFileDropUi && (e.dataTransfer?.types.includes('Files') ?? false);
	}

	function onDragEnter(e: DragEvent) {
		if (!dragHasFiles(e)) return;
		e.preventDefault();
		dragDepth++;
		app.dragOver = true;
	}

	function onDragLeave(e: DragEvent) {
		if (!dragHasFiles(e)) return;
		dragDepth = Math.max(0, dragDepth - 1);
		if (dragDepth === 0) app.dragOver = false;
	}

	function onDragOver(e: DragEvent) {
		if (!dragHasFiles(e)) return;
		e.preventDefault();
	}

	function onDrop(e: DragEvent) {
		if (!dragHasFiles(e)) return;
		e.preventDefault();
		dragDepth = 0;
		app.dragOver = false;
		if (e.dataTransfer) ingestFiles(e.dataTransfer.files);
	}

	function splitName(name: string): [string, string] {
		const m = name.match(/^(.*?)\s*\((.*)\)$/);
		return m ? [m[1], m[2]] : [name, ''];
	}

	const activeSolvable = $derived.by(
		(): SolvableCase | null => app.active ?? (app.activeLocal?.network ? app.activeLocal : null)
	);
	const activeFormulation = $derived(activeSolvable?.formulation ?? 'dcopf');

	const networkStats = $derived.by(() => {
		const c = activeSolvable;
		if (!c?.network) return null;
		return {
			buses: c.network.buses.length,
			branches: c.network.branches.length,
			objective: c.solution?.objective ?? null,
			binding: c.solution ? c.solution.flows.filter((f) => f.loading >= 0.999).length : null
		};
	});

	const stats = $derived.by(() => {
		const c = activeSolvable;
		if (!c?.network || !c.solution) return null;
		const lmps = c.solution.lmp.map((e) => e.usd_per_mwh);
		const domain = lmpDomain(lmps);
		const lmpMin = Math.min(...lmps);
		const lmpMax = Math.max(...lmps);
		return {
			buses: c.network.buses.length,
			branches: c.network.branches.length,
			objective: c.solution.objective,
			deltaObjective: c.baseSolution ? c.solution.objective - c.baseSolution.objective : null,
			uniformLmp: lmpMax - lmpMin < 1 ? lmps[0] : null,
			// Mark legend ends when outliers clamp beyond the trimmed domain.
			lmpLo: { value: domain.lo, clamped: lmpMin < domain.lo - 0.05 },
			lmpHi: { value: domain.hi, clamped: lmpMax > domain.hi + 0.05 },
			binding: c.solution.flows.filter((f) => f.loading >= 0.999).length
		};
	});

	const displayOptions = $derived(displayOptionsFor(activeSolvable));
	$effect(() => {
		if (
			displayOptions.length > 0 &&
			!displayOptions.some((option) => option.mode === app.displayMode)
		) {
			app.displayMode = 'lmp';
		}
	});
	const activeDisplay = $derived(
		displayOptions.find((option) => option.mode === app.displayMode) ?? displayOptions[0] ?? null
	);
	const displayStats = $derived.by(() => {
		if (!activeDisplay) return null;
		const values = activeDisplay.values.map((entry) => entry.value).filter(Number.isFinite);
		if (values.length === 0) return null;
		const domain = displayDomain(activeDisplay.mode, values);
		const min = Math.min(...values);
		const max = Math.max(...values);
		const flatThreshold = activeDisplay.mode === 'lmp' ? 1 : 1e-5;
		return {
			lo: { value: domain.lo, clamped: min < domain.lo - flatThreshold / 20 },
			hi: { value: domain.hi, clamped: max > domain.hi + flatThreshold / 20 },
			uniform: max - min < flatThreshold ? values[0] : null
		};
	});

	const selectedBusData = $derived.by(() => {
		const c = activeSolvable;
		if (!c?.network || app.selectedBus === null) return null;
		return c.network.buses.find((b) => b.id === app.selectedBus) ?? null;
	});

	const committedDelta = $derived(
		activeSolvable && app.selectedBus !== null
			? (caseDeltas(activeSolvable)[app.selectedBus] ?? 0)
			: 0
	);
	const sliderValue = $derived(app.previewDeltaMw ?? committedDelta);
	const nearbyRangeCenter = $derived.by(() => {
		const c = activeSolvable;
		const bus = app.selectedBus;
		if (!c || bus === null) return committedDelta;
		if (nearbyRangeAnchor?.caseId === c.id && nearbyRangeAnchor.bus === bus) {
			return nearbyRangeAnchor.delta;
		}
		return committedDelta;
	});

	function demandBounds(
		mode: DemandRangeMode,
		bus: typeof selectedBusData,
		center: number
	): { min: number; max: number; span: number } {
		if (!bus) return { min: 0, max: 0, span: 0 };
		// Floor, not ceil: -ceil(demand) can push base + delta below zero for
		// non-integer demand, which the server rejects (400) and which is
		// physically meaningless (demand cannot go negative).
		const physicalMin = -Math.floor(bus.demand_mw);
		const physicalMax = Math.max(Math.ceil(bus.demand_mw), 50);
		if (mode === 'full')
			return { min: physicalMin, max: physicalMax, span: physicalMax - physicalMin };
		const span = Math.max(5, Math.min(25, 0.1 * Math.max(bus.demand_mw, 50)));
		return {
			min: Math.max(physicalMin, center - span),
			max: Math.min(physicalMax, center + span),
			span
		};
	}

	const sliderBounds = $derived(
		demandBounds(app.demandRangeMode, selectedBusData, nearbyRangeCenter)
	);
	const sliderMin = $derived(sliderBounds.min);
	const sliderMax = $derived(sliderBounds.max);

	function setDemandRangeMode(mode: DemandRangeMode) {
		app.demandRangeMode = mode;
		const c = activeSolvable;
		if (mode === 'local' && c && app.selectedBus !== null) {
			setNearbyRangeAnchor(c, app.selectedBus, sliderValue);
		}
		const bounds = demandBounds(
			mode,
			selectedBusData,
			mode === 'local' ? sliderValue : nearbyRangeCenter
		);
		if (app.previewDeltaMw === null) return;
		app.previewDeltaMw = Math.min(bounds.max, Math.max(bounds.min, app.previewDeltaMw));
	}

	const selectedSensitivity = $derived.by(() => {
		const c = activeSolvable;
		if (!c?.sensitivity || app.selectedBus === null) return null;
		return c.sensitivity.bus === app.selectedBus ? c.sensitivity : null;
	});

	const sensSummary = $derived.by(() =>
		selectedSensitivity ? sensitivityDomain(selectedSensitivity.values.map((v) => v.value)) : null
	);
	const flatSensBackground = $derived(sensSummary ? rgbaCss(sensFlatColor(sensSummary)) : '');

	const selectedLmp = $derived.by(() => {
		const c = activeSolvable;
		if (!c?.solution || app.selectedBus === null) return null;
		return c.solution.lmp.find((e) => e.bus === app.selectedBus)?.usd_per_mwh ?? null;
	});

	// Self-sensitivity ∂LMP_bb/∂d at the selected bus: the curvature term the
	// first-order fallback uses for a second-order objective estimate. Zero when no
	// sensitivity column is loaded for the bus.
	const selfSens = $derived.by(() => {
		if (!selectedSensitivity || app.selectedBus === null) return 0;
		return selectedSensitivity.values.find((v) => v.bus === app.selectedBus)?.value ?? 0;
	});

	// Predicted objective change vs the committed point for the live preview. The
	// engine (Study.preview) owns this for browser-solvable cases; null when no
	// engine preview applies (server/Safari path), where the first-order fallback
	// below fills in. Scoped to the case + bus it was computed for. Set by runPreview.
	let previewObjective = $state.raw<{ caseId: string; bus: number; objectiveDelta: number } | null>(
		null
	);

	// Predicted objective change vs base for the demand readout. Prefer the engine
	// preview (Study.preview at the committed point, plus the committed part); fall
	// back to a second-order gradient estimate when no Study preview is available
	// (server-only cases, or a browser that can't load the sensitivity module): the
	// exact committed part plus lmp·step + S_bb·step²/2 along the gradient.
	const predictedDeltaObj = $derived.by(() => {
		const c = activeSolvable;
		const bus = app.selectedBus;
		if (!c?.solution || !c.baseSolution || bus === null) return null;
		const committedPart = c.solution.objective - c.baseSolution.objective;
		if (previewObjective && previewObjective.caseId === c.id && previewObjective.bus === bus) {
			return committedPart + previewObjective.objectiveDelta;
		}
		if (selectedLmp === null) return null;
		const step = sliderValue - committedDelta;
		return committedPart + selectedLmp * step + 0.5 * selfSens * step * step;
	});

	const gradientScore = $derived.by(() => {
		const c = activeSolvable;
		if (!c?.solution || !c.baseSolution || c.predictedObjective == null || c.solving) return null;
		const exact = c.solution.objective - c.baseSolution.objective;
		return { pred: c.predictedObjective, exact };
	});

	const topMovers = $derived.by(() => {
		if (!selectedSensitivity || sensSummary?.flat) return [];
		return [...selectedSensitivity.values]
			.filter((v) => v.bus !== app.selectedBus)
			.sort((a, b) => Math.abs(b.value) - Math.abs(a.value))
			.slice(0, 5);
	});
	const showMoverSlot = $derived(Boolean(selectedSensitivity && !sensSummary?.flat));

	const previewing = $derived(
		Boolean(
			activeSolvable?.solving ||
			app.previewActive ||
			(app.previewDeltaMw !== null && Math.abs(sliderValue - committedDelta) >= 0.25)
		)
	);

	const fmt = new Intl.NumberFormat('en-US', { maximumFractionDigits: 1 });
	const signed = (v: number) => `${v < 0 ? '−' : '+'}${fmt.format(Math.abs(v))}`;
	const signedExp = (v: number) => `${v < 0 ? '−' : '+'}${Math.abs(v).toExponential(2)}`;
	const displayFmt = (mode: DisplayMode, value: number) =>
		mode === 'lmp' ? fmt.format(value) : value.toFixed(3);
	const SIZE_SAMPLES = [10, 100, 500];

	function sliderCurrent() {
		return sliderValue;
	}

	// Deltas the live preview would commit: the committed deltas with the slider's
	// value at the selected bus, applying commitDelta's dead zone so a tiny nudge
	// reads as "no change at this bus".
	function previewDeltas(c: SolvableCase, bus: number, value: number) {
		const deltas = { ...caseDeltas(c) };
		if (Math.abs(value) < 0.25) delete deltas[bus];
		else deltas[bus] = value;
		return deltas;
	}

	// First-order engine preview for the live drag. Uses the case's already-built
	// Study (synchronous, no re-parse, no re-solve) to paint predicted per-bus
	// ΔLMP and the predicted Δobjective. A no-op when the Study isn't built yet or
	// can't preview (Safari's relaxed-SIMD gap, server-only cases): the map then
	// falls back to the JS sensitivity-times-step preview.
	function runPreview(c: SolvableCase, bus: number, value: number) {
		const study = caseStudies.get(c)?.study;
		if (!study) return;
		try {
			const { lmp, objectiveDelta } = study.preview(previewDeltas(c, bus, value));
			const delta = new Map<number, number>();
			for (const e of lmp) delta.set(e.bus, e.usd_per_mwh);
			app.previewLmp = { caseId: c.id, bus, delta };
			previewObjective = objectiveDelta === null ? null : { caseId: c.id, bus, objectiveDelta };
		} catch {
			// Preview is best effort; on any failure leave the map on its fallback path.
			app.previewLmp = null;
			previewObjective = null;
		}
	}

	function setSliderPreview(value: number | undefined) {
		if (value === undefined) return;
		app.previewActive = true;
		app.previewDeltaMw = value;
		const c = activeSolvable;
		if (c && app.selectedBus !== null) runPreview(c, app.selectedBus, value);
	}

	function solveMetaLabel(c: SolvableCase): string {
		if ((c.iterations ?? []).length > 1) return `${c.iterations?.length} iterations`;
		if (c.solveBackend === 'clarabel-wasm-server-sensitivity') return 'server dLMP/dd';
		return c.solveBackend === 'rust-server' ? 'server solve' : 'browser solve';
	}
</script>

<svelte:window
	onkeydown={(e) => {
		if (e.key === 'Escape') clearSelection();
	}}
	ondragenter={onDragEnter}
	ondragleave={onDragLeave}
	ondragover={onDragOver}
	ondrop={onDrop}
/>

<main>
	<TellegenMap
		onbusclick={selectBus}
		onlocalbusclick={selectLocalBus}
		onplacecase={placeLocalCase}
		onmapclick={clearSelection}
	/>

	<header>
		<div class="brand">
			<svg viewBox="0 0 24 24" width="20" height="20" aria-hidden="true">
				<path d="M4 18 L12 6 L20 18" stroke="#b25e00" stroke-width="1.6" fill="none" />
				<circle cx="4" cy="18" r="2.4" fill="#b25e00" />
				<circle cx="12" cy="6" r="2.4" fill="#20242b" />
				<circle cx="20" cy="18" r="2.4" fill="#b25e00" />
			</svg>
			<h1>tellegen</h1>
		</div>
		<nav class="cases" aria-label="networks">
			{#each app.cases as c (c.id)}
				{@const [cname, cregion] = splitName(c.name)}
				<div class="case-chip" class:active={app.activeCaseId === c.id}>
					<button class="case-activate" onclick={() => activateCase(c.id)}>
						<span class="cname"
							>{cname}{#if c.perturbed}<i class="mark" title="demand perturbed"></i>{/if}</span
						>
						<span class="cregion mono">{cregion}</span>
					</button>
					<button
						class="case-remove mono"
						aria-label="remove {c.name} from this browser"
						title="remove {c.name} from this browser"
						onclick={(e) => removeBackendCase(c, e)}>&#10005;</button
					>
				</div>
			{/each}
			{#each app.localCases as c (c.id)}
				<div class="case-chip local" class:active={app.activeLocalId === c.id}>
					<button class="case-activate" onclick={() => activateLocal(c)}>
						<span class="cname">{c.label}</span>
						<span class="cregion mono">local</span>
					</button>
					<button
						class="case-remove mono"
						aria-label="remove {c.label}"
						title="remove {c.label}"
						onclick={(e) => removeLocalCase(c, e)}>&#10005;</button
					>
				</div>
			{/each}
			{#if showFileDropUi}
				<button
					class="ghost filedrop-ui"
					title="parsed in your browser; the file never uploads"
					onclick={() => fileInput?.click()}
				>
					<span class="cname"><span class="arrow">&#8675;</span>drop a case file</span>
					<span class="cregion mono">case + geographic files &mdash; or click</span>
				</button>
			{/if}
		</nav>
		<span class="kicker mono">
			<a href="https://github.com/eigenergy" target="_blank" rel="noreferrer"
				>eigenergy group @ michigan ece</a
			>
			<i class="sep"></i>
			<a href="https://eigenergy.github.io/tellegen/" target="_blank" rel="noreferrer">docs</a>
		</span>
	</header>

	<aside class="panel">
		{#if app.error}
			<p class="error mono">{app.error}</p>
		{/if}
		{#if app.parsingFile}
			<p class="dim mono blink">parsing&hellip;</p>
		{/if}
		{#if app.activeLocal}
			{@const lc = app.activeLocal}
			<h2>{lc.label} <span class="region mono">via {lc.fileName}</span></h2>
			{#if lc.substations}
				<dl class="mono">
					<div>
						<dt>substations</dt>
						<dd>{lc.substations.points.length}</dd>
					</div>
				</dl>
				<p class="footnote mono">
					display only &mdash; positions inferred from the PowerWorld diagram, not surveyed latitude
					and longitude
				</p>
				<p class="footnote mono">decoded in your browser by powerio (wasm); never uploaded</p>
			{:else if lc.summary}
				<dl class="mono">
					<div>
						<dt>buses</dt>
						<dd>{lc.summary.n_bus}</dd>
					</div>
					<div>
						<dt>branches</dt>
						<dd>{lc.summary.n_branch}</dd>
					</div>
					<div>
						<dt>generators</dt>
						<dd>{lc.summary.n_gen}</dd>
					</div>
					<div>
						<dt>load</dt>
						<dd>{fmt.format(lc.summary.load_mw)} MW</dd>
					</div>
					<div>
						<dt>gen capacity</dt>
						<dd>{fmt.format(lc.summary.gen_mw)} MW</dd>
					</div>
					<div>
						<dt>base MVA</dt>
						<dd>{fmt.format(lc.summary.base_mva)}</dd>
					</div>
				</dl>
				{#if lc.summary.warnings.length > 0}
					<ul class="warnings mono">
						{#each lc.summary.warnings.slice(0, 4) as w, i (i)}
							<li>{w}</li>
						{/each}
						{#if lc.summary.warnings.length > 4}
							<li>+{lc.summary.warnings.length - 4} more</li>
						{/if}
					</ul>
				{/if}
				{#if !lc.view}
					<p class="footnote mono">
						no coordinates in this file &mdash; click the map or drop a geographic file
					</p>
				{:else if lc.coordsKind === 'synthetic'}
					<p class="footnote mono">
						coordinates: synthetic topology layout centered where you placed it
					</p>
				{:else if lc.coordsKind === 'geofile'}
					<p class="footnote mono">
						coordinates: geographic file data from {lc.geoSource}
					</p>
				{/if}
				{#if lc.geoWarnings && lc.geoWarnings.length > 0}
					<ul class="warnings mono">
						{#each lc.geoWarnings.slice(0, 4) as w, i (i)}
							<li>{w}</li>
						{/each}
						{#if lc.geoWarnings.length > 4}
							<li>+{lc.geoWarnings.length - 4} more</li>
						{/if}
					</ul>
				{/if}
				<p class="footnote mono">parsed in your browser by powerio (wasm); never uploaded</p>
			{/if}
			{#if lc.topology && lc.coordsKind !== 'file'}
				<button class="reset mono" onclick={() => moveLocalCase(lc)}>
					{lc.coordsKind === 'synthetic_pending'
						? 'place on map'
						: lc.coordsKind === 'geofile'
							? 'place manually'
							: 'move layout'}
				</button>
			{/if}
			<button class="reset mono" onclick={() => removeLocalCase(lc)}>remove</button>
		{/if}
		{#if !networkStats}
			{#if !app.error && !app.activeLocal}
				{#if casesLoaded && app.cases.length === 0}
					<p class="dim mono">no default cases loaded</p>
					<button class="reset mono" onclick={restoreDefaultCases}>restore defaults</button>
				{:else if loadingBackendCase}
					<p class="dim mono blink">loading selected case&hellip;</p>
				{:else}
					<p class="dim mono blink">loading cases&hellip;</p>
				{/if}
			{/if}
		{:else}
			{#if !app.activeLocal}
				{@const [cname, cregion] = splitName(app.active?.name ?? '')}
				<h2>{cname} <span class="region mono">{cregion}</span></h2>
				{@const deltaObjective = stats?.deltaObjective}
				<dl class="mono">
					<div>
						<dt>buses</dt>
						<dd>{networkStats.buses}</dd>
					</div>
					<div>
						<dt>branches</dt>
						<dd>{networkStats.branches}</dd>
					</div>
					<div>
						<dt>binding lines</dt>
						<dd>{networkStats.binding ?? '…'}</dd>
					</div>
					<div>
						<dt>cost</dt>
						<dd>
							{#if networkStats.objective === null}
								<span class="blink">solving&hellip;</span>
							{:else}
								{fmt.format(networkStats.objective)} $/h
							{/if}
						</dd>
					</div>
					{#if isPerturbed(activeSolvable) && deltaObjective !== null && deltaObjective !== undefined}
						<div class="delta">
							<dt>vs base</dt>
							<dd>{signed(deltaObjective)} $/h</dd>
						</div>
					{/if}
				</dl>
			{/if}

			{#if activeSolvable}
				{@const c = activeSolvable}
				<div class="formulation">
					<label
						class="formulation-row mono"
						for="formulation-select"
						title={formulationHint(c.formulation)}
					>
						<span>formulation</span>
						<select
							id="formulation-select"
							class="mono"
							disabled={c.solving}
							value={c.formulation}
							onchange={(e) => changeFormulation(c, e.currentTarget.value as Formulation)}
						>
							{#each FORMULATIONS as f (f.id)}
								<option value={f.id} disabled={f.disabled}>
									{f.label}{f.disabled ? ' (coming soon)' : ''}
								</option>
							{/each}
						</select>
					</label>
				</div>
			{/if}

			<hr />

			{#if app.selectedBus !== null && (selectedSensitivity || app.sensitivityLoading)}
				{@const c = activeSolvable as SolvableCase}
				<div class="mode">
					<span class="chip">{previewing ? 'LMP preview' : '∂LMP/∂d'}</span>
					<span class="mono dim">bus {app.selectedBus}</span>
					<button class="mono" onclick={clearSelection}>esc&nbsp;clear</button>
				</div>
				<div class="sensitivity-readout" aria-live="polite">
					{#if previewing}
						<p class="dim small">
							{c.solving
								? 'Exact solve running; the map keeps the LMP preview.'
								: 'First order LMP preview. Release for the exact solve.'}
						</p>
					{:else}
						<p class="dim small sensitivity-copy">LMP response per MW at bus {app.selectedBus}.</p>
						{#if sensSummary?.flat}
							<div class="legend flat" style:background={flatSensBackground}></div>
							<div class="legend-labels mono single">
								<span>uniform {signedExp(sensSummary.mean)} ($/MWh)/MW</span>
							</div>
						{:else if sensSummary}
							<div class="legend" style:background={sensGradient}></div>
							<div class="legend-labels mono">
								<span>&minus;{sensSummary.scale.toExponential(1)}</span>
								<span>0</span>
								<span>+{sensSummary.scale.toExponential(1)}</span>
							</div>
						{:else if app.sensitivityLoading}
							<div class="legend" style:background="var(--line)" style:opacity="0.4"></div>
							<div class="legend-labels mono single">
								<span class="blink">computing &part;LMP/&part;d&hellip;</span>
							</div>
						{/if}
					{/if}
				</div>

				<div class="slider-block">
					<div class="slider-head mono">
						<span>&Delta; demand</span>
						<span class="val">{signed(sliderValue)} MW</span>
					</div>
					<div class="range-mode">
						<div class="segment mono" aria-label="demand range">
							<button
								type="button"
								class:active={app.demandRangeMode === 'local'}
								aria-pressed={app.demandRangeMode === 'local'}
								aria-label="nearby demand range"
								title="range near the selected demand setting"
								onclick={() => setDemandRangeMode('local')}>nearby</button
							>
							<button
								type="button"
								class:active={app.demandRangeMode === 'full'}
								aria-pressed={app.demandRangeMode === 'full'}
								aria-label="full demand range"
								title="range from zero load to the local physical limit"
								onclick={() => setDemandRangeMode('full')}>full range</button
							>
						</div>
						<span class="mono dim">{fmt.format(sliderMin)} to {fmt.format(sliderMax)} MW</span>
					</div>
					<input
						type="range"
						min={sliderMin}
						max={sliderMax}
						step="0.5"
						bind:value={sliderCurrent, setSliderPreview}
						aria-label="demand delta at selected bus"
						onpointerdown={() => setSliderPreview(sliderValue)}
						onkeydown={() => setSliderPreview(sliderValue)}
						onpointerup={(e) => finishDemandInput(Number(e.currentTarget.value))}
						onmouseup={(e) => finishDemandInput(Number(e.currentTarget.value))}
						onclick={(e) => finishDemandInput(Number(e.currentTarget.value))}
						onkeyup={(e) => finishDemandInput(Number(e.currentTarget.value))}
						onblur={(e) => finishDemandInput(Number(e.currentTarget.value))}
						onchange={(e) => finishDemandInput(Number(e.currentTarget.value))}
					/>
					<div class="demand-feedback" class:idle={!previewing && !isPerturbed(c)}>
						<p class="pred mono dim" aria-hidden={!(predictedDeltaObj !== null && previewing)}>
							{#if predictedDeltaObj !== null && previewing}
								predicted &Delta;cost {signed(predictedDeltaObj)} $/h
							{:else}
								&nbsp;
							{/if}
						</p>
						<p class="score mono" aria-hidden={!(gradientScore && isPerturbed(c))}>
							{#if gradientScore && isPerturbed(c)}
								gradient {signed(gradientScore.pred)} &middot; exact {signed(gradientScore.exact)}
								$/h
							{:else}
								&nbsp;
							{/if}
						</p>
						<div class="reset-row">
							{#if isPerturbed(c)}
								<button class="reset mono" onclick={() => resetCase(c)}>reset demand</button>
							{/if}
						</div>
					</div>
				</div>

				{#if showMoverSlot}
					<div class="movers-block">
						{#if !previewing && topMovers.length > 0}
							<table class="mono">
								<tbody>
									{#each topMovers as mover (mover.bus)}
										<tr>
											<td>bus {mover.bus}</td>
											<td class:pos={mover.value > 0} class:neg={mover.value < 0}>
												{mover.value >= 0 ? '+' : ''}{mover.value.toExponential(2)}
											</td>
										</tr>
									{/each}
								</tbody>
							</table>
						{/if}
					</div>
				{/if}
			{:else}
				<div class="mode display-mode">
					<div class="segment mono" aria-label="bus color variable">
						{#each displayOptions as option (option.mode)}
							<button
								type="button"
								class:active={app.displayMode === option.mode}
								aria-pressed={app.displayMode === option.mode}
								onclick={() => (app.displayMode = option.mode)}>{option.label}</button
							>
						{/each}
					</div>
					<span class="mono dim">{activeDisplay?.unit ?? ''}</span>
					{#if app.sensitivityLoading}
						<span class="mono dim blink">&part; loading&hellip;</span>
					{/if}
				</div>
				{#if activeDisplay && displayStats}
					<p class="dim small">{activeDisplay.copy}</p>
					<div class="legend" style:background={activeDisplay.gradient}></div>
					<div class="legend-labels mono">
						{#if displayStats.uniform !== null}
							<span>
								uniform {displayFmt(activeDisplay.mode, displayStats.uniform)}
								{activeDisplay.unit}
							</span>
						{:else}
							<span>
								{displayStats.lo.clamped ? '≤' : ''}{displayFmt(
									activeDisplay.mode,
									displayStats.lo.value
								)}
							</span>
							<span>
								{displayStats.hi.clamped ? '≥' : ''}{displayFmt(
									activeDisplay.mode,
									displayStats.hi.value
								)}
							</span>
						{/if}
					</div>
				{:else}
					<p class="dim small blink">Solving {formulationLabel(activeFormulation)}&hellip;</p>
				{/if}
			{/if}

			<hr />

			<div class="sizes">
				{#each SIZE_SAMPLES as mw (mw)}
					<span class="size mono">
						<i style:width="{2 * busRadius(mw)}px" style:height="{2 * busRadius(mw)}px"></i>
						{mw}
					</span>
				{/each}
				<span class="mono dim caption">MW, max(load,&#8201;gen)</span>
			</div>
		{/if}
	</aside>

	{#if activeSolvable && (activeSolvable.solving || activeSolvable.solveMs != null)}
		<div class="solvecard">
			<div class="solvecard-head mono">
				<span><b>OPF solve</b></span>
			</div>
			{#if (activeSolvable.iterations ?? []).length > 1}
				<Sparkline iterations={activeSolvable.iterations ?? []} />
			{/if}
			<div class="solve-meta mono dim">
				<span class="solve-formulation">{formulationLabel(activeSolvable.formulation)}</span>
				<span>{solveMetaLabel(activeSolvable)}</span>
				{#if activeSolvable.solveMs != null}<span>{activeSolvable.solveMs} ms</span>{/if}
			</div>
			{#if activeSolvable.solveBackend === 'rust-server' && activeSolvable.solveFallbackReason}
				<p class="fallback-reason mono dim" title={activeSolvable.solveFallbackReason}>
					fallback: {activeSolvable.solveFallbackReason}
				</p>
			{/if}
		</div>
	{/if}

	{#if app.dragOver}
		<div class="dropzone" aria-hidden="true">
			<div class="dropframe">
				<p class="mono">drop to parse &mdash; case files or geographic files</p>
				<p class="mono hint">parsed in your browser; the file never uploads</p>
			</div>
		</div>
	{/if}

	{#if app.placingLocalId}
		<div class="placement-cue mono">click the map to place the synthetic topology</div>
	{/if}

	{#if casesLoaded && hiddenDefaults.size > 0}
		<button class="restore-defaults mono" onclick={restoreDefaultCases}>
			&#8634; restore default cases
		</button>
	{/if}

	<footer class="mono">
		<a href="/credits">credits</a>
		<i class="sep"></i>
		<a href="/privacy">privacy</a>
		{#if showFileDropUi}
			<i class="sep filedrop-ui"></i>
			<span class="drophint filedrop-ui"
				><span class="arrow">&#8675;</span> drop a case or coordinate file anywhere</span
			>
		{/if}
	</footer>

	{#if showFileDropUi}
		<input
			type="file"
			accept=".m,.raw,.aux,.pwd,.csv,.json,.geojson"
			multiple
			hidden
			bind:this={fileInput}
			onchange={(e) => {
				const input = e.currentTarget;
				if (input.files) ingestFiles(Array.from(input.files));
				input.value = '';
			}}
		/>
	{/if}
</main>

<style>
	main {
		position: fixed;
		inset: 0;
		overflow: hidden;
	}

	header {
		position: absolute;
		top: 0;
		left: 0;
		right: 0;
		z-index: 10;
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 16px;
		padding: 10px 20px;
		background: linear-gradient(rgba(236, 233, 226, 0.95), rgba(236, 233, 226, 0));
		animation: drop 0.5s ease-out both;
	}

	.brand {
		flex: 0 0 auto;
		display: flex;
		align-items: center;
		gap: 10px;
	}

	h1 {
		margin: 0;
		font-size: 22px;
		font-weight: 600;
		letter-spacing: 0;
	}

	.cases {
		flex: 1 1 auto;
		min-width: 0;
		display: flex;
		gap: 6px;
		justify-content: center;
		overflow-x: auto;
		scrollbar-width: none;
	}

	.cases::-webkit-scrollbar {
		display: none;
	}

	.cases > button,
	.case-chip {
		display: flex;
		align-items: flex-start;
		gap: 1px;
		padding: 5px 12px 4px;
		background: rgba(252, 251, 247, 0.65);
		border: 1px solid var(--line);
		border-radius: 3px;
		cursor: pointer;
		font-family: var(--font-display);
		color: var(--ink);
		transition: border-color 0.15s ease;
	}

	.cases > button {
		flex-direction: column;
	}

	.case-chip {
		align-items: stretch;
		gap: 0;
		padding: 0;
		overflow: hidden;
		position: relative;
	}

	.case-chip button {
		font-family: var(--font-display);
		color: inherit;
		cursor: pointer;
	}

	.case-activate {
		display: flex;
		flex-direction: column;
		align-items: flex-start;
		gap: 1px;
		min-width: 0;
		width: 100%;
		padding: 5px 24px 4px 12px;
		background: transparent;
		border: 0;
	}

	.case-remove {
		position: absolute;
		top: 1px;
		right: 1px;
		display: grid;
		place-items: center;
		width: 18px;
		height: 18px;
		padding: 0;
		background: transparent;
		border: 0;
		color: var(--ink-faint);
		font-size: 9px;
		line-height: 1;
	}

	.case-remove:hover,
	.case-remove:focus-visible {
		background: var(--accent-soft);
		color: var(--red);
	}

	.cases > button:hover,
	.case-chip:hover {
		border-color: var(--accent);
	}

	.case-chip.active {
		background: var(--panel);
		border-color: var(--accent);
		box-shadow: inset 0 -2px 0 var(--accent);
	}

	.cname {
		font-size: 12.5px;
		font-weight: 600;
		line-height: 1.2;
		display: inline-flex;
		align-items: center;
		gap: 5px;
	}

	.cname .mark {
		width: 5px;
		height: 5px;
		background: var(--accent-bright);
		transform: rotate(45deg);
	}

	.cregion {
		font-size: 9.5px;
		color: var(--ink-dim);
		letter-spacing: 0;
		text-transform: uppercase;
	}

	/* Local case chips: dashed border + graphite text, topology only. */
	.case-chip.local {
		border-style: dashed;
		color: var(--ink-dim);
	}

	.case-chip.local.active {
		background: var(--panel);
		border-color: var(--accent);
		box-shadow: inset 0 -2px 0 var(--accent);
	}

	/* Ghost chip: standing invitation to drop or pick a case file. */
	.cases > button.ghost {
		background: rgba(252, 251, 247, 0.36);
		border: 1px dashed rgba(178, 94, 0, 0.55);
		color: var(--ink-dim);
		box-shadow: inset 0 0 0 1px rgba(212, 116, 34, 0.08);
	}

	.cases > button.ghost:hover {
		border-color: var(--accent);
		background: var(--accent-soft);
	}

	.cases > button.ghost:hover,
	.cases > button.ghost:hover .cregion {
		color: var(--accent);
	}

	.arrow {
		display: inline-block;
		animation: bob 1.8s ease-in-out infinite alternate;
	}

	.kicker {
		flex: 0 0 auto;
		display: flex;
		align-items: center;
		font-size: 11px;
		text-transform: uppercase;
		letter-spacing: 0;
		color: var(--ink-dim);
		white-space: nowrap;
	}

	.kicker a {
		color: var(--ink-dim);
		text-decoration: none;
	}

	.kicker a:hover {
		color: var(--accent);
	}

	.restore-defaults {
		position: absolute;
		bottom: 34px;
		left: 20px;
		z-index: 10;
		padding: 6px 11px;
		background: var(--panel);
		border: 1px solid var(--line);
		border-radius: 3px;
		color: var(--ink-dim);
		font-size: 11px;
		cursor: pointer;
		box-shadow: 0 2px 10px rgba(32, 36, 43, 0.1);
	}

	.restore-defaults:hover {
		border-color: var(--accent);
		color: var(--accent);
	}

	.panel {
		position: absolute;
		top: 64px;
		left: 20px;
		z-index: 10;
		width: 312px;
		max-height: calc(100% - 110px);
		overflow-y: auto;
		padding: 16px 18px;
		background: var(--panel);
		border: 1px solid var(--line);
		border-radius: 3px;
		backdrop-filter: blur(6px);
		box-shadow: 0 4px 24px rgba(32, 36, 43, 0.08);
		animation: rise 0.5s 0.12s ease-out both;
	}

	h2 {
		margin: 0 0 12px;
		font-size: 16px;
		font-weight: 600;
	}

	.region {
		font-size: 10px;
		font-weight: 400;
		color: var(--ink-dim);
		text-transform: uppercase;
		letter-spacing: 0;
		margin-left: 4px;
	}

	dl {
		margin: 0;
		font-size: 12.5px;
	}

	dl div {
		display: flex;
		justify-content: space-between;
		padding: 3px 0;
	}

	dl .delta dd {
		color: var(--accent);
	}

	dt {
		color: var(--ink-dim);
	}

	dd {
		margin: 0;
	}

	.footnote {
		margin: 8px 0 0;
		font-size: 10px;
		color: var(--ink-faint);
		letter-spacing: 0;
	}

	.warnings {
		margin: 8px 0 0;
		padding: 0;
		list-style: none;
		font-size: 10.5px;
		line-height: 1.5;
		color: var(--accent);
	}

	hr {
		border: 0;
		border-top: 1px solid var(--line);
		margin: 12px 0;
	}

	.mode {
		display: flex;
		align-items: center;
		gap: 10px;
		font-size: 12px;
	}

	.display-mode {
		gap: 8px;
	}

	.display-mode .segment button {
		font-size: 11px;
		padding: 2px 9px;
	}

	.chip {
		font-family: var(--font-mono);
		font-size: 11px;
		padding: 2px 8px;
		border: 1px solid var(--accent);
		color: var(--accent);
		background: var(--accent-soft);
		border-radius: 2px;
		white-space: nowrap;
	}

	.mode > button {
		margin-left: auto;
		font-size: 10.5px;
		padding: 2px 7px;
		background: none;
		border: 1px solid var(--line);
		border-radius: 2px;
		color: var(--ink-dim);
		cursor: pointer;
	}

	.mode > button:hover {
		border-color: var(--accent);
		color: var(--accent);
	}

	.small {
		font-size: 12px;
		line-height: 1.55;
	}

	.dim {
		color: var(--ink-dim);
	}

	.error {
		color: var(--red);
		font-size: 12px;
	}

	.sensitivity-readout {
		min-height: 58px;
	}

	.sensitivity-copy {
		font-size: 11.5px;
		line-height: 1.35;
		white-space: nowrap;
	}

	.legend {
		height: 6px;
		border-radius: 3px;
		margin-top: 6px;
	}

	.legend-labels {
		display: flex;
		justify-content: space-between;
		font-size: 10.5px;
		color: var(--ink-faint);
		margin-top: 4px;
	}

	.legend-labels.single {
		justify-content: center;
		text-align: center;
	}

	.slider-block {
		margin-top: 14px;
	}

	.slider-head {
		display: flex;
		justify-content: space-between;
		font-size: 11.5px;
		color: var(--ink-dim);
		margin-bottom: 4px;
	}

	.slider-head .val {
		color: var(--ink);
	}

	.formulation {
		margin-top: 10px;
	}

	.formulation-row {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 10px;
		font-size: 12px;
		color: var(--ink);
	}

	.formulation-row select {
		font-family: var(--font-mono);
		font-size: 11px;
		padding: 3px 22px 3px 8px;
		border: 1px solid var(--line);
		border-radius: 2px;
		background: rgba(252, 251, 247, 0.55);
		color: var(--ink);
		cursor: pointer;
		/* Native arrow on the right, drawn so the control reads as a control in the panel. */
		appearance: none;
		-webkit-appearance: none;
		background-image:
			linear-gradient(45deg, transparent 50%, var(--ink-dim) 50%),
			linear-gradient(135deg, var(--ink-dim) 50%, transparent 50%);
		background-position:
			right 10px center,
			right 6px center;
		background-size:
			4px 4px,
			4px 4px;
		background-repeat: no-repeat;
	}

	.formulation-row select:hover:not(:disabled) {
		border-color: var(--accent);
		color: var(--accent);
	}

	.formulation-row select:disabled {
		opacity: 0.55;
		cursor: progress;
	}

	.range-mode {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 8px;
		margin: 6px 0 7px;
		font-size: 10.5px;
	}

	.segment {
		display: inline-flex;
		border: 1px solid var(--line);
		border-radius: 2px;
		overflow: hidden;
		background: rgba(252, 251, 247, 0.55);
	}

	.segment button {
		padding: 2px 8px;
		border: 0;
		background: transparent;
		color: var(--ink-dim);
		font: inherit;
		cursor: pointer;
	}

	.segment button + button {
		border-left: 1px solid var(--line);
	}

	.segment button.active {
		background: var(--accent-soft);
		color: var(--accent);
	}

	input[type='range'] {
		-webkit-appearance: none;
		appearance: none;
		width: 100%;
		height: 4px;
		background: var(--line);
		border-radius: 2px;
		outline-offset: 4px;
		margin: 6px 0;
	}

	input[type='range']::-webkit-slider-thumb {
		-webkit-appearance: none;
		appearance: none;
		width: 14px;
		height: 14px;
		border-radius: 50%;
		background: var(--accent);
		border: 2px solid #fcfbf7;
		box-shadow: 0 1px 4px rgba(32, 36, 43, 0.3);
		cursor: ew-resize;
	}

	input[type='range']::-moz-range-thumb {
		width: 14px;
		height: 14px;
		border-radius: 50%;
		background: var(--accent);
		border: 2px solid #fcfbf7;
		box-shadow: 0 1px 4px rgba(32, 36, 43, 0.3);
		cursor: ew-resize;
	}

	.pred {
		margin: 2px 0 0;
		font-size: 11px;
		min-height: 16px;
	}

	.score {
		margin: 8px 0 0;
		font-size: 11px;
		color: var(--ink);
		min-height: 16px;
	}

	.demand-feedback {
		min-height: 78px;
	}

	/* On a fresh selection (no preview, no perturbation) the predicted/gradient/
	   reset rows are empty; collapse the reserved block so the panel isn't padded
	   with whitespace. The reservation returns during interaction to avoid jumps. */
	.demand-feedback.idle {
		min-height: 0;
	}

	.demand-feedback.idle .pred,
	.demand-feedback.idle .score {
		display: none;
	}

	.reset-row {
		min-height: 28px;
	}

	.movers-block {
		min-height: 114px;
	}

	.solvecard {
		position: absolute;
		top: 64px;
		right: 20px;
		z-index: 10;
		width: 240px;
		padding: 12px 14px 10px;
		background: var(--panel);
		border: 1px solid var(--line);
		border-radius: 3px;
		backdrop-filter: blur(6px);
		box-shadow: 0 4px 24px rgba(32, 36, 43, 0.08);
		animation: rise 0.3s ease-out both;
	}

	.solvecard-head {
		font-size: 10.5px;
		margin-bottom: 6px;
		white-space: nowrap;
	}

	.solve-meta {
		display: flex;
		align-items: center;
		flex-wrap: wrap;
		gap: 12px;
		font-size: 10px;
		margin-top: 4px;
	}

	.solve-formulation {
		color: var(--accent);
	}

	.fallback-reason {
		margin: 6px 0 0;
		font-size: 10px;
		line-height: 1.35;
		overflow-wrap: anywhere;
	}

	.reset {
		margin-top: 10px;
		font-size: 10.5px;
		padding: 3px 10px;
		background: none;
		border: 1px solid var(--line);
		border-radius: 2px;
		color: var(--ink-dim);
		cursor: pointer;
	}

	.reset:hover {
		border-color: var(--red);
		color: var(--red);
	}

	table {
		width: 100%;
		margin-top: 12px;
		border-collapse: collapse;
		font-size: 12px;
	}

	td {
		padding: 3px 0;
		border-top: 1px solid var(--line);
	}

	td:last-child {
		text-align: right;
	}

	.pos {
		color: var(--pos);
	}

	.neg {
		color: var(--neg);
	}

	.sizes {
		display: flex;
		align-items: center;
		gap: 12px;
		font-size: 10px;
		color: var(--ink-dim);
	}

	.size {
		display: inline-flex;
		align-items: center;
		gap: 5px;
	}

	.size i {
		display: inline-block;
		border-radius: 50%;
		background: rgba(212, 116, 34, 0.55);
		border: 1px solid rgba(46, 42, 34, 0.45);
	}

	.caption {
		margin-left: auto;
		font-size: 9.5px;
	}

	footer {
		position: absolute;
		bottom: 0;
		left: 0;
		right: 0;
		z-index: 10;
		display: flex;
		align-items: center;
		padding: 8px 20px;
		font-size: 10.5px;
		color: var(--ink-dim);
		background: linear-gradient(rgba(236, 233, 226, 0), rgba(236, 233, 226, 0.9));
		animation: rise 0.5s 0.24s ease-out both;
		pointer-events: none;
	}

	footer a {
		pointer-events: auto;
		color: var(--ink-dim);
	}

	footer a:hover {
		color: var(--accent);
	}

	.sep {
		width: 4px;
		height: 4px;
		margin: 0 10px;
		background: var(--accent-bright);
		opacity: 0.55;
		transform: rotate(45deg);
	}

	.drophint {
		color: var(--ink-faint);
	}

	.drophint .arrow {
		color: var(--accent);
	}

	.dropzone {
		position: fixed;
		inset: 0;
		z-index: 20;
		pointer-events: none;
		background: rgba(236, 233, 226, 0.75);
	}

	.dropframe {
		position: absolute;
		inset: 14px;
		border: 1.5px dashed var(--accent);
		border-radius: 3px;
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		gap: 6px;
	}

	.dropframe p {
		margin: 0;
		font-size: 13px;
	}

	.dropframe .hint {
		font-size: 11px;
		color: var(--ink-dim);
	}

	.placement-cue {
		position: absolute;
		left: 50%;
		bottom: 52px;
		z-index: 14;
		transform: translateX(-50%);
		padding: 8px 12px;
		background: var(--panel);
		border: 1px solid var(--accent);
		border-radius: 3px;
		color: var(--accent);
		font-size: 11px;
		box-shadow: 0 4px 18px rgba(32, 36, 43, 0.1);
		pointer-events: none;
	}

	.blink {
		animation: blink 1.2s steps(2) infinite;
	}

	@keyframes drop {
		from {
			opacity: 0;
			transform: translateY(-8px);
		}
	}

	@keyframes rise {
		from {
			opacity: 0;
			transform: translateY(8px);
		}
	}

	@keyframes blink {
		50% {
			opacity: 0.35;
		}
	}

	@keyframes bob {
		to {
			transform: translateY(2px);
		}
	}

	@media (max-width: 760px) {
		header {
			align-items: flex-start;
			flex-wrap: wrap;
			gap: 8px;
			padding: 8px 10px 12px;
			background: linear-gradient(rgba(236, 233, 226, 0.97), rgba(236, 233, 226, 0.72));
		}

		.brand {
			gap: 8px;
		}

		h1 {
			font-size: 20px;
		}

		.kicker {
			margin-left: auto;
			font-size: 9.5px;
			letter-spacing: 0;
			line-height: 2;
		}

		.cases {
			order: 3;
			width: 100%;
			overflow-x: auto;
			padding-bottom: 2px;
			scrollbar-width: none;
			scroll-padding: 10px;
			scroll-snap-type: x proximity;
			-webkit-overflow-scrolling: touch;
		}

		.cases::-webkit-scrollbar {
			display: none;
		}

		.cases > button,
		.case-chip {
			flex: 0 0 auto;
			max-width: 150px;
			min-height: 40px;
			scroll-snap-align: start;
		}

		.cases > button {
			padding: 7px 10px 6px;
		}

		.case-activate {
			padding: 7px 24px 6px 10px;
		}

		.case-remove {
			width: 22px;
			height: 22px;
		}

		.cname,
		.cregion {
			max-width: 100%;
			white-space: nowrap;
			overflow: hidden;
			text-overflow: ellipsis;
		}

		.panel {
			top: auto;
			left: 10px;
			right: 10px;
			bottom: 40px;
			width: auto;
			max-height: 44dvh;
			padding: 14px 16px;
		}

		.solvecard {
			top: 124px;
			left: auto;
			right: 10px;
			width: min(230px, calc(100% - 20px));
		}

		.mode {
			flex-wrap: wrap;
			gap: 7px;
		}

		.mode button {
			margin-left: 0;
		}

		.sizes {
			flex-wrap: wrap;
			gap: 8px 12px;
		}

		.caption {
			margin-left: 0;
			flex-basis: 100%;
		}

		footer {
			padding: 7px 10px;
			overflow-x: auto;
			font-size: 9.5px;
			white-space: nowrap;
			pointer-events: auto;
			scrollbar-width: none;
		}

		footer::-webkit-scrollbar {
			display: none;
		}

		.sep {
			margin: 0 8px;
		}

		.filedrop-ui {
			display: none;
		}
	}

	@media (hover: none), (pointer: coarse) {
		.filedrop-ui {
			display: none;
		}
	}

	@media (max-width: 420px) {
		.kicker {
			display: none;
		}

		.cases > button,
		.case-chip {
			max-width: 132px;
		}

		.panel {
			bottom: 34px;
			max-height: 46dvh;
		}

		.placement-cue {
			bottom: 38px;
			width: calc(100% - 28px);
			text-align: center;
		}
	}

	@media (prefers-reduced-motion: reduce) {
		header,
		.panel,
		footer,
		.arrow {
			animation: none;
		}
	}
</style>
