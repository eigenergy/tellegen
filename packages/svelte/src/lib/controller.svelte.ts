import { ApiError, createApiClient, type TellegenApiClient } from './api.js';
import type { BranchRatingDeltas, Network, NetworkBus, SensitivityColumn, Solution } from './api.js';
import { previewScaleFor, scalarDomain, sensFlatColor, sensitivityDomain } from './colors.js';
import {
	caseDeltas as sharedCaseDeltas,
	caseRatings as sharedCaseRatings,
	displayMetaFor,
	displaySeriesFor
} from './display.js';
import { applyGeoFile, isGeoFile, mergeGeoFiles, parseGeoFile, type GeoFile } from './geo-file.js';
import {
	CaseState,
	LocalCase,
	type AppState,
	type DemandRangeMode,
	type FallbackTarget,
	type SolvableCase
} from './state.svelte.js';
import { placeSyntheticTopology } from './synthetic-layout.js';
import {
	createStudy,
	FORMULATIONS,
	formatOf,
	ingestCase,
	isDisplayFile,
	isPermanentEngineFailure,
	parseDisplay,
	type BrowserStudy,
	type Formulation,
	type SensTarget
} from '@tellegen/engine';
import { errorText, extent, formulationLabel, rgbaCss } from './format.js';

const HIDDEN_DEFAULT_CASES_KEY = 'tellegen.hiddenDefaultCases.v1';
// Open on South Carolina by default: it has the most interesting price action.
const DEFAULT_CASE_ID = 'case500';
const PWD_MERCATOR_K = 535.81608;

/** Terminal copy when nothing can solve: the browser engine is unavailable and
 * the server declines compute (403). Server compute exists in the codebase but
 * ships disabled; interest routes to email until it is offered. */
const WASM_REQUIRED_NOTICE =
	'this browser cannot run the WebAssembly engine that solves locally, and server side compute is disabled in this demo. it may be offered later; to express interest, email Samuel Talkington at talks@umich.edu';

function delayUntilFocusSettles(ms: number, signal: AbortSignal): Promise<void> {
	if (ms <= 0 || signal.aborted) return Promise.resolve();
	return new Promise((resolve) => {
		let timeout: ReturnType<typeof setTimeout>;
		const finish = () => {
			clearTimeout(timeout);
			signal.removeEventListener('abort', finish);
			resolve();
		};
		timeout = setTimeout(finish, ms);
		signal.addEventListener('abort', finish, { once: true });
	});
}

export type { SolvableCase };
type DemandRangeAnchor = {
	caseId: string;
	bus: number;
	delta: number;
};

export interface ControllerOptions {
	api?: TellegenApiClient;
	apiBase?: string;
}

export class Controller {
	app: AppState;
	api: TellegenApiClient;
	abort: AbortController | null = null;
	// While set (epoch ms), the server sensitivity fallback is rate limited: skip
	// the request and show the rate-limit copy instead of burning the budget on a
	// guaranteed rejection. Covers one rate limit window. Not $state; nothing
	// renders it.
	sensitivity429Until = 0;
	// Latched once any server fallback answers 403: compute is disabled on this
	// deploy, so later fallbacks show the notice instead of firing doomed requests.
	serverComputeOff = false;

	// Build-once browser Study per case: the network is parsed and the model built
	// when the Study is created, so a drag re-solves (commit) and previews without
	// re-parsing. Kept in a WeakMap off the reactive/raw case payloads — the wasm
	// handle is neither serialized nor part of any $state.
	caseStudies = new WeakMap<
		SolvableCase,
		{ study: BrowserStudy; networkJson: string; formulation: Formulation; baseSolution: Solution }
	>();
	// Latch a permanent sensitivity-module failure per case so we don't retry
	// createStudy — and the same permanent error — on every drag. Transient
	// failures are not latched.
	studyUnavailable = new WeakMap<SolvableCase, string>();
	// In-flight Study builds, keyed per case, so two overlapping getStudy calls for
	// the same case share one createStudy instead of each building (and leaking) a
	// wasm Study. Cleared once the build settles or the case is disposed.
	studyBuilds = new WeakMap<
		SolvableCase,
		{
			networkJson: string;
			formulation: Formulation;
			token: object;
			promise: Promise<BrowserStudy | null>;
		}
	>();
	// Local case id counter. An instance field (not a module global) so it is scoped
	// to this controller — the singleton-free shape the rest of the app relies on.
	localSeq = 0;
	// In-flight initial case-list load, so a remount during the first fetch dedupes
	// instead of firing a second concurrent load().
	loading: Promise<void> | null = null;

	nearbyRangeAnchor = $state<DemandRangeAnchor | null>(null);
	// Default cases the user has closed; drives the restore affordance. Seeded in load().
	hiddenDefaults = $state<Set<string>>(new Set());
	casesLoaded = $state(false);
	loadingBackendCase = $state<string | null>(null);
	showFileDropUi = $state(true);
	// Predicted objective change vs the committed point for the live preview. The
	// engine (Study.preview) owns this for browser-solvable cases; null when no
	// engine preview applies (server or browser fallback path), where the first-order fallback
	// below fills in. Scoped to the case + selection target it was computed for.
	// Set by runPreview / runRatingPreview.
	previewObjective = $state.raw<{
		caseId: string;
		target: SensTarget;
		objectiveDelta: number;
	} | null>(null);
	// Per-MW objective slope for the rating drag, cached per (case, branch, committed
	// point) so each drag frame stays O(buses): the engine's first-order preview at a
	// +1 MW rating step is taken once, then scaled by the live step. Invalidated on
	// commit, selection, and formulation change. Not $state; only runRatingPreview
	// reads it, and previewObjective carries the rendered value.
	ratingSlope: { caseId: string; branch: number; slope: number } | null = null;

	constructor(app: AppState, options: ControllerOptions = {}) {
		this.app = app;
		this.api = options.api ?? createApiClient({ apiBase: options.apiBase });
	}

	// ===== helpers =====

	isBackendCase(c: SolvableCase): c is CaseState {
		return c instanceof CaseState;
	}

	isActiveSolveCase(c: SolvableCase): boolean {
		return this.isBackendCase(c) ? this.app.activeCaseId === c.id : this.app.activeLocalId === c.id;
	}

	caseDeltas(c: SolvableCase) {
		return sharedCaseDeltas(c);
	}

	caseRatings(c: SolvableCase): BranchRatingDeltas {
		return sharedCaseRatings(c);
	}

	/** True when the case carries any nonzero committed rating delta. Rating edits
	 * solve only through the browser Study; the server fallback would silently
	 * drop them, so callers gate on this before falling back. */
	hasRatingEdits(c: SolvableCase): boolean {
		return Object.values(this.caseRatings(c)).some((mw) => mw !== 0);
	}

	/** True when two selection targets name the same bus or branch. */
	sameTarget(a: SensTarget, b: SensTarget): boolean {
		return 'bus' in a ? 'bus' in b && a.bus === b.bus : 'branch' in b && a.branch === b.branch;
	}

	isPerturbed(c: SolvableCase | null): boolean {
		return c?.perturbed ?? false;
	}

	setNearbyRangeAnchor(c: SolvableCase, bus: number, delta = this.caseDeltas(c)[bus] ?? 0) {
		this.nearbyRangeAnchor = { caseId: c.id, bus, delta };
	}

	// Shared reset for both bus-select entry points: cancel any in-flight request,
	// bump the per-case sensitivity token, and clear the selection/preview state so
	// the panel starts the new selection clean. Returns the live AbortController and
	// the sensitivity seq the caller guards its async writes with.
	beginBusSelection(
		c: SolvableCase,
		busId: number
	): { ac: AbortController; sensitivitySeq: number } {
		this.abort?.abort();
		const ac = new AbortController();
		this.abort = ac;
		const sensitivitySeq = (c.sensitivitySeq ?? 0) + 1;
		c.sensitivitySeq = sensitivitySeq;
		this.app.error = null;
		this.app.selectedBus = busId;
		this.app.selectedBranch = null;
		this.app.previewDeltaMw = null;
		this.app.previewRatingMw = null;
		this.app.previewActive = false;
		this.app.demandRangeMode = 'local';
		this.setNearbyRangeAnchor(c, busId);
		this.ratingSlope = null;
		this.app.sensitivityLoading = true;
		c.sensitivity = null;
		return { ac, sensitivitySeq };
	}

	// The branch-select counterpart of beginBusSelection: same abort/seq discipline,
	// but the selection lands on a branch and the demand preview fields are cleared.
	beginBranchSelection(
		c: SolvableCase,
		branchId: number
	): { ac: AbortController; sensitivitySeq: number } {
		this.abort?.abort();
		const ac = new AbortController();
		this.abort = ac;
		const sensitivitySeq = (c.sensitivitySeq ?? 0) + 1;
		c.sensitivitySeq = sensitivitySeq;
		this.app.error = null;
		this.app.selectedBranch = branchId;
		this.app.selectedBus = null;
		this.app.previewDeltaMw = null;
		this.app.previewRatingMw = null;
		this.app.previewActive = false;
		this.app.demandRangeMode = 'local';
		this.nearbyRangeAnchor = null;
		this.ratingSlope = null;
		this.app.sensitivityLoading = true;
		c.sensitivity = null;
		return { ac, sensitivitySeq };
	}

	/** The display name of a case: a backend case's `name`, a local case's `label`. */
	caseName(c: SolvableCase): string {
		return this.isBackendCase(c) ? c.name : c.label;
	}

	readHiddenDefaultCases(): Set<string> {
		if (typeof localStorage === 'undefined') return new Set();
		try {
			const parsed = JSON.parse(localStorage.getItem(HIDDEN_DEFAULT_CASES_KEY) ?? '[]');
			return new Set(Array.isArray(parsed) ? parsed.filter((id) => typeof id === 'string') : []);
		} catch {
			return new Set();
		}
	}

	writeHiddenDefaultCases(ids: Set<string>) {
		if (typeof localStorage === 'undefined') return;
		try {
			localStorage.setItem(HIDDEN_DEFAULT_CASES_KEY, JSON.stringify([...ids].sort()));
		} catch {
			// Current session removal still works; persistence is best effort.
		}
	}

	rememberHiddenDefaultCase(id: string) {
		const hidden = this.readHiddenDefaultCases();
		hidden.add(id);
		this.writeHiddenDefaultCases(hidden);
		this.hiddenDefaults = new Set(hidden);
	}

	localNetwork(c: LocalCase): Network | null {
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

	maybeStartLocalSolve(id: string) {
		const c = this.app.localCases.find((lc) => lc.id === id);
		if (!c?.networkJson || !c.view || !c.summary) return;
		c.network = this.localNetwork(c) ?? c.network ?? null;
		if (c.networkJson && c.network && !c.solution) this.runSolve(c, null);
	}

	// The case's Study, building it once for `(networkJson, formulation)` and rebuilding
	// (after free) if either changed — so picking a new formulation re-parses and re-solves
	// under that formulation. Returns null when the sens module can't load; the caller then
	// falls back to the server, surfacing solveFallbackReason.
	async getStudy(c: SolvableCase, networkJson: string): Promise<BrowserStudy | null> {
		const latched = this.studyUnavailable.get(c);
		if (latched) {
			c.solveFallbackReason ??= latched;
			return null;
		}
		const cached = this.caseStudies.get(c);
		if (cached && cached.networkJson === networkJson && cached.formulation === c.formulation)
			return cached.study;
		// Coalesce concurrent builds for the same (case, networkJson, formulation): a
		// bus-select sensitivity fetch and a solve can both reach here before either
		// createStudy resolves. Without this each builds its own wasm Study and the
		// later set() orphans the earlier one (a leaked handle), and the cache can end
		// up holding an instance the live solve never committed on.
		const inflight = this.studyBuilds.get(c);
		if (inflight && inflight.networkJson === networkJson && inflight.formulation === c.formulation)
			return inflight.promise;
		// Different params, or no build pending: drop any stale Study before rebuilding.
		if (cached) {
			cached.study.free();
			this.caseStudies.delete(c);
		}
		const formulation = c.formulation;
		// Identity for this build attempt: the post-await guards compare against it
		// (rather than the promise) so a superseding build or a disposal wins the cache.
		const token = {};
		const build = (async (): Promise<BrowserStudy | null> => {
			try {
				const study = await createStudy(networkJson, formulation);
				// The case may have switched formulation or been disposed (removed, or its
				// list reloaded) while building; don't cache a now-unwanted Study, and free
				// the orphan so its wasm memory is released.
				if (c.formulation !== formulation || this.studyBuilds.get(c)?.token !== token) {
					study.free();
					return null;
				}
				const baseSolution = study.currentSolution();
				this.caseStudies.set(c, { study, networkJson, formulation, baseSolution });
				return study;
			} catch (e) {
				const message = errorText(e);
				// Only a genuine browser-capability failure is permanent; latch it so the
				// case stays on the fallback path. Transient errors stay retryable.
				if (isPermanentEngineFailure(message)) {
					this.studyUnavailable.set(c, 'browser study wasm is not supported by this browser');
				}
				c.solveFallbackReason ??= `browser study unavailable: ${message}`;
				return null;
			} finally {
				if (this.studyBuilds.get(c)?.token === token) this.studyBuilds.delete(c);
			}
		})();
		this.studyBuilds.set(c, { networkJson, formulation, token, promise: build });
		return build;
	}

	// The ∂LMP/∂parameter column at `target` for the case's active formulation, solved in
	// the browser. Every column — bus or branch, any formulation — comes from the Study's
	// exact re-solve at the case's operating point. The column is null (with the reason on
	// `solveFallbackReason`) when the Study can't be built; the selection paths then
	// reconcile via the server where a DC endpoint exists. Throws only on a hard browser
	// error.
	async browserSensitivity(
		c: SolvableCase,
		networkJson: string,
		target: SensTarget
	): Promise<SensitivityColumn | null> {
		const study = await this.getStudy(c, networkJson);
		return study
			? study.sensitivity(c.id, this.caseDeltas(c), this.caseRatings(c), target)
			: null;
	}

	// Release a case's Study (if any) when the case is removed.
	disposeStudy(c: SolvableCase) {
		const cached = this.caseStudies.get(c);
		if (cached) {
			cached.study.free();
			this.caseStudies.delete(c);
		}
		// Cancel any in-flight build: when it resolves it sees its entry gone and frees.
		this.studyBuilds.delete(c);
	}

	acceptSensitivity(
		c: SolvableCase,
		col: SensitivityColumn | null,
		target: SensTarget | null,
		sensitivitySeq?: number
	) {
		if (!col || target === null) return;
		// The column must name the target it was requested for, and the target must
		// still be the live selection.
		const matches =
			'bus' in target
				? col.bus === target.bus && this.app.selectedBus === target.bus
				: col.branch === target.branch && this.app.selectedBranch === target.branch;
		if (!matches) return;
		if (!this.isActiveSolveCase(c)) return;
		if (sensitivitySeq !== undefined && sensitivitySeq !== (c.sensitivitySeq ?? 0)) return;
		c.sensitivity = col;
	}

	finishSolve(c: SolvableCase, seq: number, target: SensTarget | null) {
		if (seq !== (c.solveSeq ?? 0)) return;
		c.solving = false;
		const selectionLive =
			target !== null &&
			this.isActiveSolveCase(c) &&
			('bus' in target
				? this.app.selectedBus === target.bus
				: this.app.selectedBranch === target.branch);
		if (selectionLive) {
			this.app.previewActive = false;
			this.app.previewDeltaMw = null;
			this.app.previewRatingMw = null;
			// The committed solution supersedes the live engine preview.
			this.app.previewLmp = null;
			this.previewObjective = null;
		}
	}

	// Fetch and cache the raw powerio Network JSON for the browser solver.
	// Returns null when it can't be loaded, so callers fall back to the server.
	async ensureNetworkJson(c: SolvableCase): Promise<string | null> {
		if (!this.isBackendCase(c)) return c.networkJson ?? null;
		if (c.networkJson) return c.networkJson;
		try {
			const json = await this.api.getCaseNetworkJson(c.id);
			c.networkJson = json;
			return json;
		} catch (e) {
			c.solveFallbackReason = `case fetch failed: ${errorText(e)}`;
			return null;
		}
	}

	// Deltas the live preview would commit: the committed deltas with the slider's
	// value at the selected bus, applying commitDelta's dead zone so a tiny nudge
	// reads as "no change at this bus".
	previewDeltas(c: SolvableCase, bus: number, value: number) {
		const deltas = { ...this.caseDeltas(c) };
		if (Math.abs(value) < 0.25) delete deltas[bus];
		else deltas[bus] = value;
		return deltas;
	}

	demandBounds(
		mode: DemandRangeMode,
		bus: NetworkBus | null,
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

	// PowerWorld .pwd files store substation symbols at diagram coordinates,
	// not lat/lon. Auto-generated TAMU layouts are Web Mercator scaled by this
	// constant with both axes in degrees: x = K·lon and y = K·mercdeg(lat),
	// where mercdeg is the Mercator ordinate expressed in degrees. So lon = x/K,
	// and latitude is the inverse gudermannian after converting y/K back to
	// radians. Hand-edited diagrams drift from this, so positions stay
	// approximate. Verified against ACTIVSg200/2000 to within ~0.02 deg.
	pwdToLngLat(x: number, y: number): [number, number] {
		const lon = x / PWD_MERCATOR_K;
		const lat = (Math.atan(Math.sinh(((y / PWD_MERCATOR_K) * Math.PI) / 180)) * 180) / Math.PI;
		return [lon, lat];
	}

	// ===== view-model =====

	activeSolvable = $derived.by(
		(): SolvableCase | null =>
			this.app.active ?? (this.app.activeLocal?.network ? this.app.activeLocal : null)
	);
	activeFormulation = $derived(this.activeSolvable?.formulation ?? 'dcopf');

	networkStats = $derived.by(() => {
		const c = this.activeSolvable;
		if (!c?.network) return null;
		return {
			buses: c.network.buses.length,
			branches: c.network.branches.length,
			objective: c.solution?.objective ?? null,
			deltaObjective:
				c.solution && c.baseSolution ? c.solution.objective - c.baseSolution.objective : null,
			binding: c.solution ? c.solution.flows.filter((f) => f.loading >= 0.999).length : null
		};
	});

	displayOptions = $derived(displayMetaFor(this.activeSolvable));
	activeDisplay = $derived.by(() => {
		const activeMeta =
			this.displayOptions.find((option) => option.mode === this.app.displayMode) ??
			this.displayOptions[0] ??
			null;
		if (!activeMeta) return null;
		// Drive the values off the resolved mode, not app.displayMode, so the
		// displayOptions[0] fallback holds while displayMode is stale relative to the
		// formulation (e.g. leaving SOCWR before +page resets it to 'lmp').
		return { ...activeMeta, values: displaySeriesFor(this.activeSolvable, activeMeta.mode) };
	});
	displayStats = $derived.by(() => {
		if (!this.activeDisplay) return null;
		const values = this.activeDisplay.values.map((entry) => entry.value).filter(Number.isFinite);
		if (values.length === 0) return null;
		const domain = scalarDomain(this.activeDisplay.mode, values);
		const { min, max } = extent(values);
		const flatThreshold = this.activeDisplay.mode === 'lmp' ? 1 : 1e-5;
		return {
			lo: { value: domain.lo, clamped: min < domain.lo - flatThreshold / 20 },
			hi: { value: domain.hi, clamped: max > domain.hi + flatThreshold / 20 },
			uniform: max - min < flatThreshold ? values[0] : null
		};
	});

	selectedBusData = $derived.by(() => {
		const c = this.activeSolvable;
		if (!c?.network || this.app.selectedBus === null) return null;
		return c.network.buses.find((b) => b.id === this.app.selectedBus) ?? null;
	});

	selectedBranchData = $derived.by(() => {
		const c = this.activeSolvable;
		if (!c?.network || this.app.selectedBranch === null) return null;
		return c.network.branches.find((b) => b.id === this.app.selectedBranch) ?? null;
	});

	// The sensitivity target of the current selection: the selected bus (∂LMP/∂d),
	// else the selected branch (∂LMP/∂rating), else null. Selections are mutually
	// exclusive, so bus-first ordering is only a tiebreak for impossible states.
	selectionTarget = $derived.by((): SensTarget | null =>
		this.app.selectedBus !== null
			? { bus: this.app.selectedBus }
			: this.app.selectedBranch !== null
				? { branch: this.app.selectedBranch }
				: null
	);

	committedDelta = $derived.by(() =>
		this.activeSolvable && this.app.selectedBus !== null
			? (this.caseDeltas(this.activeSolvable)[this.app.selectedBus] ?? 0)
			: 0
	);
	sliderValue = $derived.by(() => this.app.previewDeltaMw ?? this.committedDelta);
	nearbyRangeCenter = $derived.by(() => {
		const c = this.activeSolvable;
		const bus = this.app.selectedBus;
		if (!c || bus === null) return this.committedDelta;
		if (this.nearbyRangeAnchor?.caseId === c.id && this.nearbyRangeAnchor.bus === bus) {
			return this.nearbyRangeAnchor.delta;
		}
		return this.committedDelta;
	});

	sliderBounds = $derived.by(() =>
		this.demandBounds(this.app.demandRangeMode, this.selectedBusData, this.nearbyRangeCenter)
	);
	sliderMin = $derived(this.sliderBounds.min);
	sliderMax = $derived(this.sliderBounds.max);

	committedRating = $derived.by(() =>
		this.activeSolvable && this.app.selectedBranch !== null
			? (this.caseRatings(this.activeSolvable)[this.app.selectedBranch] ?? 0)
			: 0
	);
	ratingSliderValue = $derived.by(() => this.app.previewRatingMw ?? this.committedRating);

	// Rating slider bounds, absolute around zero delta: ±20% of the base rating,
	// clamped to [5, 50] MW, and never below 1 - rate so the committed limit
	// rate + Δ stays at least 1 MW. Disabled when no branch is selected or the
	// line's rating is synthesized (rate_mw <= 0), which has no physical limit
	// to perturb.
	ratingBounds = $derived.by(() => {
		const b = this.selectedBranchData;
		if (!b || b.rate_mw <= 0) return { min: 0, max: 0, disabled: true };
		const span = Math.min(50, Math.max(5, 0.2 * b.rate_mw));
		return { min: Math.max(-(b.rate_mw - 1), -span), max: span, disabled: false };
	});

	selectedSensitivity = $derived.by(() => {
		const c = this.activeSolvable;
		if (!c?.sensitivity) return null;
		if (this.app.selectedBus !== null) {
			return c.sensitivity.bus === this.app.selectedBus ? c.sensitivity : null;
		}
		if (this.app.selectedBranch !== null) {
			return c.sensitivity.branch === this.app.selectedBranch ? c.sensitivity : null;
		}
		return null;
	});

	sensSummary = $derived.by(() =>
		this.selectedSensitivity
			? sensitivityDomain(this.selectedSensitivity.values.map((v) => v.value))
			: null
	);
	flatSensBackground = $derived(this.sensSummary ? rgbaCss(sensFlatColor(this.sensSummary)) : '');

	// The fixed preview normalization for a drag. Both inputs are stable while
	// dragging (slider bounds are anchored per selection; the committed value moves
	// only on commit), so the scale never shifts under the pointer and intensity is
	// linear in the step. A branch selection normalizes over the rating slider's
	// deflection instead of the demand slider's.
	previewMaxAbsStep = $derived.by(() =>
		this.app.selectedBranch !== null
			? Math.max(
					Math.abs(this.ratingBounds.min - this.committedRating),
					Math.abs(this.ratingBounds.max - this.committedRating)
				)
			: Math.max(
					Math.abs(this.sliderMin - this.committedDelta),
					Math.abs(this.sliderMax - this.committedDelta)
				)
	);
	previewScale = $derived.by(() =>
		this.sensSummary ? previewScaleFor(this.sensSummary, this.previewMaxAbsStep) : null
	);

	selectedLmp = $derived.by(() => {
		const c = this.activeSolvable;
		if (!c?.solution || this.app.selectedBus === null) return null;
		return c.solution.lmp.find((e) => e.bus === this.app.selectedBus)?.usd_per_mwh ?? null;
	});

	// Self-sensitivity ∂LMP_bb/∂d at the selected bus: the curvature term the
	// first-order fallback uses for a second-order objective estimate. Zero when no
	// sensitivity column is loaded for the bus.
	selfSens = $derived.by(() => {
		if (!this.selectedSensitivity || this.app.selectedBus === null) return 0;
		return this.selectedSensitivity.values.find((v) => v.bus === this.app.selectedBus)?.value ?? 0;
	});

	// Predicted objective change vs base for the demand/rating readout. Prefer the
	// engine preview (Study.preview at the committed point, plus the committed part);
	// fall back to a second-order gradient estimate when no Study preview is available
	// (server-only cases, or a browser that can't load the sensitivity module): the
	// exact committed part plus lmp·step + S_bb·step²/2 along the gradient. The
	// gradient fallback is demand-only — a rating step has no LMP-at-bus analogue —
	// so a branch selection without an engine slope shows no prediction.
	predictedDeltaObj = $derived.by(() => {
		const c = this.activeSolvable;
		const target = this.selectionTarget;
		if (!c?.solution || !c.baseSolution || target === null) return null;
		const committedPart = c.solution.objective - c.baseSolution.objective;
		if (
			this.previewObjective &&
			this.previewObjective.caseId === c.id &&
			this.sameTarget(this.previewObjective.target, target)
		) {
			return committedPart + this.previewObjective.objectiveDelta;
		}
		if ('branch' in target || this.selectedLmp === null) return null;
		const step = this.sliderValue - this.committedDelta;
		return committedPart + this.selectedLmp * step + 0.5 * this.selfSens * step * step;
	});

	gradientScore = $derived.by(() => {
		const c = this.activeSolvable;
		if (!c?.solution || !c.baseSolution || c.predictedObjective == null || c.solving) return null;
		const exact = c.solution.objective - c.baseSolution.objective;
		return { pred: c.predictedObjective, exact };
	});

	topMovers = $derived.by(() => {
		if (!this.selectedSensitivity || this.sensSummary?.flat) return [];
		return [...this.selectedSensitivity.values]
			.filter((v) => v.bus !== this.app.selectedBus)
			.sort((a, b) => Math.abs(b.value) - Math.abs(a.value))
			.slice(0, 5);
	});
	showMoverSlot = $derived(Boolean(this.selectedSensitivity && !this.sensSummary?.flat));

	previewing = $derived.by(() =>
		Boolean(
			this.activeSolvable?.solving ||
			this.app.previewActive ||
			(this.app.previewDeltaMw !== null &&
				Math.abs(this.sliderValue - this.committedDelta) >= 0.25) ||
			(this.app.previewRatingMw !== null &&
				Math.abs(this.ratingSliderValue - this.committedRating) >= 0.25)
		)
	);

	// ===== actions =====

	load = (): Promise<void> => {
		// A remount can call load() again before the first getCases() resolves; dedupe
		// so two concurrent loads can't double-fetch the case list and double-fit the map.
		if (this.loading) return this.loading;
		this.loading = (async () => {
			try {
				const summaries = await this.api.getCases();
				const hidden = this.readHiddenDefaultCases();
				this.hiddenDefaults = new Set(hidden);
				// Tear down the cases being replaced so their wasm Studies are freed and any
				// live server stream is closed; the bulk reload otherwise orphans them (the
				// Study WeakMaps key off the old objects, so their free() never runs).
				for (const old of this.app.cases) {
					old.closeStream?.();
					old.closeStream = null;
					old.solveSeq++;
					this.disposeStudy(old);
				}
				this.app.cases = summaries.filter((s) => !hidden.has(s.id)).map((s) => new CaseState(s));
				this.app.activeLocalId = null;
				this.app.placingLocalId = null;
				this.app.activeCaseId =
					this.app.cases.find((c) => c.id === DEFAULT_CASE_ID)?.id ?? this.app.cases[0]?.id ?? null;
				const active = this.app.active;
				if (active) await this.loadBackendCase(active, true);
				else this.app.requestFrame('all');
				// Mark loaded only on success, so a failed first load is retried on the next
				// mount (the page guards the call with `if (!ctrl.casesLoaded)`).
				this.casesLoaded = true;
			} catch (e) {
				this.fail(errorText(e), this.load);
			} finally {
				this.loading = null;
			}
		})();
		return this.loading;
	};

	loadBackendCase = async (c: CaseState, frame = false) => {
		if (c.network && c.solution && c.baseSolution) {
			if (frame && this.app.activeCaseId === c.id) this.app.requestFrame(c.id);
			return;
		}
		this.loadingBackendCase = c.id;
		const requestedFormulation = c.formulation;
		try {
			const [network, dcBaseSolution] = await Promise.all([
				c.network ? Promise.resolve(c.network) : this.api.getNetwork(c.id),
				// The server caches only the DC OPF base solution. Other formulations
				// must hydrate from the browser Study so their cost, prices, flows, and
				// voltage fields all come from the selected formulation.
				requestedFormulation === 'dcopf' ? this.api.getSolution(c.id) : Promise.resolve(null)
			]);
			if (!this.app.byId(c.id)) return;
			c.network = network;
			if (dcBaseSolution && c.formulation === requestedFormulation) {
				c.baseSolution = dcBaseSolution;
				c.solution = dcBaseSolution;
			}
			if (frame && this.app.activeCaseId === c.id) this.app.requestFrame(c.id);
			if (this.isActiveSolveCase(c) && c.formulation !== 'dcopf' && !c.solution && !c.solving) {
				this.runSolve(c, this.selectionTarget);
			}
		} catch (e) {
			if (this.app.byId(c.id)) {
				this.fail(`${c.name}: ${errorText(e)}`, () => this.loadBackendCase(c, true));
			}
		} finally {
			if (this.loadingBackendCase === c.id) this.loadingBackendCase = null;
		}
	};

	activateCase = async (id: string) => {
		this.app.activeLocalId = null;
		this.app.placingLocalId = null;
		if (this.app.activeCaseId !== id) {
			this.clearSelection();
			this.app.activeCaseId = id;
		}
		const c = this.app.byId(id);
		if (c) await this.loadBackendCase(c, true);
	};

	// Hydrate whichever case a removal promoted to active: a backend case needs its
	// network/solution loaded (re-framing the map onto it), a local needs a browser
	// solve. `none` (no case promoted, or the active case was untouched) is a no-op.
	private hydrateFallback = async (t: FallbackTarget) => {
		if (t.kind === 'backend') {
			const c = this.app.byId(t.id);
			if (c) await this.loadBackendCase(c, true);
		} else if (t.kind === 'local') {
			this.maybeStartLocalSolve(t.id);
		}
	};

	removeBackendCase = async (c: CaseState, event?: MouseEvent) => {
		event?.stopPropagation();
		this.rememberHiddenDefaultCase(c.id);
		// Tear down this case's own in-flight server stream whether or not it is the
		// active case (a non-active case can still hold a live stream), and bump the
		// seq so any detached handler no-ops.
		c.closeStream?.();
		c.closeStream = null;
		c.solveSeq++;
		this.disposeStudy(c);
		if (this.app.activeCaseId === c.id) this.clearSelection();
		await this.hydrateFallback(this.app.removeCase(c.id));
	};

	activateLocal = (c: LocalCase) => {
		this.clearSelection();
		// Mirror activateCase's reset: a local and a backend case are mutually
		// exclusive, so drop the backend selection. Otherwise app.active (derived
		// from activeCaseId) stays set and its solve card keeps hovering over the
		// local view.
		this.app.activeCaseId = null;
		this.app.activeLocalId = c.id;
		this.app.placingLocalId = c.coordsKind === 'synthetic_pending' ? c.id : null;
		if (c.view || c.substations) this.app.requestFrame(c.id);
		this.maybeStartLocalSolve(c.id);
	};

	removeLocalCase = async (c: LocalCase, event?: MouseEvent) => {
		event?.stopPropagation();
		if (this.app.activeLocalId === c.id) {
			// Local cases solve in the browser only (no server stream); the seq bump
			// invalidates any in-flight browser solve.
			c.solveSeq = (c.solveSeq ?? 0) + 1;
			this.clearSelection();
		}
		this.disposeStudy(c);
		await this.hydrateFallback(this.app.removeLocal(c.id));
	};

	addAndActivateLocal = (c: LocalCase) => {
		this.clearSelection();
		this.app.activeCaseId = null;
		this.app.addLocal(c);
		if (c.view || c.substations) this.app.requestFrame(c.id);
		this.maybeStartLocalSolve(c.id);
	};

	placeLocalCase = (lon: number, lat: number) => {
		const id = this.app.placingLocalId;
		const c = id ? this.app.localCases.find((lc) => lc.id === id) : null;
		if (!c?.topology) return;
		c.view = placeSyntheticTopology(c.topology, { lon, lat });
		c.coordsKind = 'synthetic';
		c.syntheticCenter = { lon, lat };
		this.app.placingLocalId = null;
		this.app.activeLocalId = c.id;
		this.app.requestFrame(c.id);
		this.maybeStartLocalSolve(c.id);
	};

	moveLocalCase = (c: LocalCase) => {
		this.app.activeCaseId = null;
		this.app.activeLocalId = c.id;
		this.app.placingLocalId = c.id;
	};

	withGeoFile = (c: LocalCase, geoFiles: GeoFile[]): LocalCase => {
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
	};

	applyGeoFilesToExisting = (geoFiles: GeoFile[]) => {
		const target =
			(this.app.activeLocal?.topology ? this.app.activeLocal : null) ??
			this.app.localCases.find((c) => c.coordsKind === 'synthetic_pending') ??
			[...this.app.localCases].reverse().find((c) => c.topology);
		if (!target?.topology) {
			this.app.error =
				'drop a case file with the geographic file, or select a parsed local case first';
			return;
		}
		try {
			this.withGeoFile(target, geoFiles);
			this.app.activeCaseId = null;
			this.app.activeLocalId = target.id;
			this.app.placingLocalId = null;
			this.app.requestFrame(target.id);
			this.maybeStartLocalSolve(target.id);
			this.app.error = null;
		} catch (e) {
			this.app.error = `${geoFiles.map((s) => s.sourceNames.join(' + ')).join(' + ')}: ${
				e instanceof Error ? e.message : e
			}; use place on map for manual placement`;
		}
	};

	selectBus = async (caseId: string, busId: number) => {
		this.app.activeLocalId = null;
		this.app.placingLocalId = null;
		if (this.app.activeCaseId !== caseId) {
			this.clearSelection();
			this.app.activeCaseId = caseId;
		}
		const c = this.app.byId(caseId);
		if (!c) return;
		const { ac, sensitivitySeq } = this.beginBusSelection(c, busId);
		// True once the browser path fully settled the selection (column accepted or
		// a terminal message shown); false leaves DC OPF one server reconciliation.
		let handled = false;
		try {
			try {
				// The dLMP/dd column from the browser solver (under the case's formulation). DC OPF
				// may reconcile a null column via the server; AC OPF / SOCWR are browser-only, so a
				// null column there is reported, never sent to the DC-only server.
				const networkJson = await this.ensureNetworkJson(c);
				if (networkJson) {
					const sensitivity = await this.browserSensitivity(c, networkJson, { bus: busId });
					if (!ac.signal.aborted && sensitivity) {
						this.acceptSensitivity(c, sensitivity, { bus: busId }, sensitivitySeq);
						handled = true;
					} else if (!ac.signal.aborted && c.formulation !== 'dcopf') {
						this.app.error = `${c.name}: ${formulationLabel(c.formulation)} sensitivity unavailable in the browser${c.solveFallbackReason ? `: ${c.solveFallbackReason}` : ''}`;
						handled = true;
					}
				} else if (c.formulation !== 'dcopf') {
					if (!ac.signal.aborted) {
						this.app.error = `${c.name}: ${formulationLabel(c.formulation)} needs the browser network JSON, which is unavailable`;
					}
					handled = true;
				}
			} catch {
				// The browser path threw; the DC OPF server reconciliation below still
				// applies. AC OPF / SOCWR have no server fallback (nothing is solved on
				// the server), so the column stays absent.
				handled = c.formulation !== 'dcopf';
			}
			if (!handled && !ac.signal.aborted && c.formulation === 'dcopf') {
				if (this.hasRatingEdits(c)) {
					// The server solves at base ratings, so its column would come from the
					// wrong operating point once rating edits are committed. Fail instead
					// of silently reconciling there.
					this.fail(`${c.name}: line rating edits solve only in the browser engine`, () =>
						this.selectBus(c.id, busId)
					);
				} else {
					await this.fetchServerSensitivity(c, busId, ac, sensitivitySeq);
				}
			}
		} finally {
			if (this.abort === ac) this.app.sensitivityLoading = false;
		}
	};

	/** The one server dLMP/dd request a bus selection may make — the reconciliation
	 * path when the browser solver produced no column for a DC OPF case. Never
	 * retries on its own: a second request inside the same rate limit window is a
	 * guaranteed rejection, and the retry button covers the user need. A 429 opens
	 * a cooldown for the rest of the window so further selections skip the server
	 * instead of feeding it. */
	private fetchServerSensitivity = async (
		c: CaseState,
		busId: number,
		ac: AbortController,
		sensitivitySeq: number
	) => {
		const retryOp = () => this.selectBus(c.id, busId);
		if (this.serverComputeOff) {
			this.fail(WASM_REQUIRED_NOTICE, retryOp);
			return;
		}
		if (Date.now() < this.sensitivity429Until) {
			this.fail('rate limited; wait a few seconds and try again', retryOp);
			return;
		}
		try {
			const col = await this.api.getSensitivity(c.id, busId, c.deltas, ac.signal);
			if (!ac.signal.aborted) this.acceptSensitivity(c, col, { bus: busId }, sensitivitySeq);
		} catch (e) {
			if (e instanceof DOMException) return;
			if (e instanceof ApiError && e.status === 403) {
				this.serverComputeOff = true;
				if (!ac.signal.aborted && sensitivitySeq === (c.sensitivitySeq ?? 0)) {
					this.fail(WASM_REQUIRED_NOTICE, retryOp);
				}
				return;
			}
			if (e instanceof ApiError && e.status === 429) {
				this.sensitivity429Until = Date.now() + 10_000;
			}
			// A late failure must not clobber a newer selection's state.
			if (!ac.signal.aborted && sensitivitySeq === (c.sensitivitySeq ?? 0)) {
				this.fail(errorText(e), retryOp);
			}
		}
	};

	/** The one server ∂LMP/∂rating request a branch selection may make — the
	 * reconciliation path when the browser Study produced no column for a DC OPF
	 * case, the branch counterpart of `fetchServerSensitivity`. Shares the 429
	 * cooldown so selections back off the server together. */
	private fetchServerBranchSensitivity = async (
		c: CaseState,
		branchId: number,
		ac: AbortController,
		sensitivitySeq: number
	) => {
		const retryOp = () => this.selectBranch(c.id, branchId);
		if (this.serverComputeOff) {
			this.fail(WASM_REQUIRED_NOTICE, retryOp);
			return;
		}
		if (Date.now() < this.sensitivity429Until) {
			this.fail('rate limited; wait a few seconds and try again', retryOp);
			return;
		}
		try {
			const col = await this.api.getBranchSensitivity(c.id, branchId, c.deltas, ac.signal);
			if (!ac.signal.aborted) this.acceptSensitivity(c, col, { branch: branchId }, sensitivitySeq);
		} catch (e) {
			if (e instanceof DOMException) return;
			if (e instanceof ApiError && e.status === 403) {
				this.serverComputeOff = true;
				if (!ac.signal.aborted && sensitivitySeq === (c.sensitivitySeq ?? 0)) {
					this.fail(WASM_REQUIRED_NOTICE, retryOp);
				}
				return;
			}
			if (e instanceof ApiError && e.status === 429) {
				this.sensitivity429Until = Date.now() + 10_000;
			}
			if (!ac.signal.aborted && sensitivitySeq === (c.sensitivitySeq ?? 0)) {
				this.fail(errorText(e), retryOp);
			}
		}
	};

	/** Show an error with an optional one-click re-run of the failed operation. */
	private fail(message: string, retry: (() => void) | null = null) {
		this.app.error = message;
		this.app.errorRetry = retry;
	}

	/** The panel's retry button: re-run the failed operation when one is known,
	 * else reload the case list. An explicit retry always ends a 429 cooldown. */
	retryError = () => {
		const op = this.app.errorRetry;
		this.app.error = null;
		this.sensitivity429Until = 0;
		(op ?? this.load)();
	};

	selectLocalBus = async (localId: string, busId: number) => {
		const c = this.app.localCases.find((lc) => lc.id === localId);
		if (!c?.networkJson || !c.network) return;
		this.app.activeCaseId = null;
		this.app.activeLocalId = localId;
		this.app.placingLocalId = null;
		const { ac, sensitivitySeq } = this.beginBusSelection(c, busId);
		try {
			const sensitivity = await this.browserSensitivity(c, c.networkJson, { bus: busId });
			if (!ac.signal.aborted) this.acceptSensitivity(c, sensitivity, { bus: busId }, sensitivitySeq);
			if (!ac.signal.aborted && !sensitivity) {
				// A null column means the solve ran but produced no dLMP/dd for this bus (or the
				// Study could not be built); local cases have no server fallback, so say so
				// instead of leaving the panel in LMP view with no explanation.
				this.app.error = `${c.label}: ${formulationLabel(c.formulation)} sensitivity unavailable in the browser${
					c.solveFallbackReason ? `: ${c.solveFallbackReason}` : ' (no dLMP/dd column for this bus)'
				}`;
			}
		} catch (e) {
			if (!ac.signal.aborted) {
				this.fail(`${c.label}: ${errorText(e)}`, () => this.selectLocalBus(localId, busId));
			}
		} finally {
			if (this.abort === ac) this.app.sensitivityLoading = false;
		}
	};

	/** The terminal message when a case with committed rating edits cannot use the
	 * browser Study: the server fallback solves at base ratings, so there is no
	 * correct fallback to take. */
	private ratingEditsFallbackError(c: SolvableCase): string {
		return `${this.caseName(c)}: line rating edits solve only in the browser engine${
			c.solveFallbackReason ? `: ${c.solveFallbackReason}` : ''
		}`;
	}

	/** The terminal message for a branch selection that produced no ∂LMP/∂rating
	 * column after the browser or server path had a chance to provide one. */
	private failBranchSensitivity(c: SolvableCase, retry: () => void) {
		this.fail(
			`${this.caseName(c)}: ∂LMP/∂rating sensitivity unavailable${
				c.solveFallbackReason ? `: ${c.solveFallbackReason}` : ''
			}`,
			retry
		);
	}

	selectBranch = async (caseId: string, branchId: number, sensitivityDelayMs = 0) => {
		this.app.activeLocalId = null;
		this.app.placingLocalId = null;
		if (this.app.activeCaseId !== caseId) {
			this.clearSelection();
			this.app.activeCaseId = caseId;
		}
		const c = this.app.byId(caseId);
		if (!c) return;
		const { ac, sensitivitySeq } = this.beginBranchSelection(c, branchId);
		const retry = () => this.selectBranch(caseId, branchId);
		// True once the browser path fully settled the selection (column accepted or
		// a terminal message shown); false leaves DC OPF one server reconciliation.
		let handled = false;
		try {
			await delayUntilFocusSettles(sensitivityDelayMs, ac.signal);
			if (ac.signal.aborted) return;
			try {
				// The ∂LMP/∂rating column from the browser Study (under the case's
				// formulation). DC OPF may reconcile a null column via the server; AC OPF /
				// SOCWR are browser-only, so a null column there is terminal.
				const networkJson = await this.ensureNetworkJson(c);
				if (networkJson) {
					const sensitivity = await this.browserSensitivity(c, networkJson, { branch: branchId });
					if (!ac.signal.aborted && sensitivity) {
						this.acceptSensitivity(c, sensitivity, { branch: branchId }, sensitivitySeq);
						handled = true;
					} else if (!ac.signal.aborted && c.formulation !== 'dcopf') {
						this.failBranchSensitivity(c, retry);
						handled = true;
					}
				} else if (c.formulation !== 'dcopf') {
					if (!ac.signal.aborted) this.failBranchSensitivity(c, retry);
					handled = true;
				}
			} catch {
				// The browser path threw; the DC OPF server reconciliation below still
				// applies. AC OPF / SOCWR have no server fallback (nothing is solved on
				// the server), so the column stays absent.
				handled = c.formulation !== 'dcopf';
			}
			if (!handled && !ac.signal.aborted && c.formulation === 'dcopf') {
				if (this.hasRatingEdits(c)) {
					// The server solves at base ratings, so its column would come from the
					// wrong operating point once rating edits are committed. Fail instead
					// of silently reconciling there.
					this.fail(this.ratingEditsFallbackError(c), retry);
				} else {
					await this.fetchServerBranchSensitivity(c, branchId, ac, sensitivitySeq);
				}
			}
		} finally {
			if (this.abort === ac) this.app.sensitivityLoading = false;
		}
	};

	selectLocalBranch = async (localId: string, branchId: number, sensitivityDelayMs = 0) => {
		const c = this.app.localCases.find((lc) => lc.id === localId);
		if (!c?.networkJson || !c.network) return;
		this.app.activeCaseId = null;
		this.app.activeLocalId = localId;
		this.app.placingLocalId = null;
		const { ac, sensitivitySeq } = this.beginBranchSelection(c, branchId);
		try {
			await delayUntilFocusSettles(sensitivityDelayMs, ac.signal);
			if (ac.signal.aborted) return;
			const sensitivity = await this.browserSensitivity(c, c.networkJson, { branch: branchId });
			if (ac.signal.aborted) return;
			if (sensitivity) {
				this.acceptSensitivity(c, sensitivity, { branch: branchId }, sensitivitySeq);
			} else if (sensitivitySeq === (c.sensitivitySeq ?? 0)) {
				this.failBranchSensitivity(c, () => this.selectLocalBranch(localId, branchId));
			}
		} catch (e) {
			if (!ac.signal.aborted && sensitivitySeq === (c.sensitivitySeq ?? 0)) {
				this.fail(`${c.label}: ${errorText(e)}`, () => this.selectLocalBranch(localId, branchId));
			}
		} finally {
			if (this.abort === ac) this.app.sensitivityLoading = false;
		}
	};

	clearSelection = () => {
		this.abort?.abort();
		const c = this.app.active;
		if (c) {
			c.sensitivitySeq++;
			c.sensitivity = null;
		}
		const lc = this.app.activeLocal;
		if (lc) {
			lc.sensitivitySeq = (lc.sensitivitySeq ?? 0) + 1;
			lc.sensitivity = null;
		}
		this.app.selectedBus = null;
		this.app.selectedBranch = null;
		this.app.previewDeltaMw = null;
		this.app.previewRatingMw = null;
		this.app.previewActive = false;
		this.app.previewLmp = null;
		this.previewObjective = null;
		this.ratingSlope = null;
		this.app.demandRangeMode = 'local';
		this.nearbyRangeAnchor = null;
		this.app.sensitivityLoading = false;
	};

	// Exact solve in the browser (wasm). The build-once Study commits the new
	// operating point without re-parsing, returning the ∂LMP/∂parameter column for
	// the selection target in the same solve; on a Study failure or missing network
	// JSON it reconciles via the server stream (backend cases). Rating edits never
	// fall back: the server solves at base ratings, so a Study failure there is
	// terminal.
	runSolve = (c: SolvableCase, target: SensTarget | null) => {
		// Cancel this case's own previous server stream, if any (backend only).
		if (this.isBackendCase(c)) {
			c.closeStream?.();
			c.closeStream = null;
		}
		c.solveSeq = (c.solveSeq ?? 0) + 1;
		const seq = c.solveSeq;
		this.app.error = null;
		c.solving = true;
		c.solveBackend = null;
		c.solveFallbackReason = null;
		c.iterations = [];
		c.solveMs = null;
		this.ensureNetworkJson(c).then(async (networkJson) => {
			if (seq !== (c.solveSeq ?? 0)) return;
			if (!networkJson) {
				c.solveFallbackReason ??= 'browser network JSON unavailable';
				if (this.hasRatingEdits(c)) {
					c.solving = false;
					this.app.error = this.ratingEditsFallbackError(c);
					return;
				}
				if (this.isBackendCase(c)) return this.serverSolve(c, target, seq);
				c.solving = false;
				this.app.error = `${c.label}: local case has no browser network JSON`;
				return;
			}
			const t0 = performance.now();
			c.solveBackend = 'clarabel-wasm';

			// Build-once Study path: commit the new operating point (no re-parse). The
			// ∂LMP/∂parameter column for the selection target is computed in the same
			// solve and comes back with it, so there is no second solve to reconcile it.
			const study = await this.getStudy(c, networkJson);
			if (seq !== (c.solveSeq ?? 0)) return;
			if (study) {
				try {
					const cached = this.caseStudies.get(c);
					if (!c.baseSolution && cached?.study === study) c.baseSolution = cached.baseSolution;
					const { solution, iterations, sensitivity } = study.commit(
						c.id,
						this.caseDeltas(c),
						this.caseRatings(c),
						target
					);
					if (seq !== (c.solveSeq ?? 0)) return;
					c.solution = solution;
					c.iterations = iterations;
					if (!c.baseSolution && Object.keys(this.caseDeltas(c)).length === 0)
						c.baseSolution = solution;
					c.solveMs = Math.round(performance.now() - t0);
					// The commit carried the sensitivity column; accept it for the selection
					// through the same seq-guarded setter every sensitivity source goes through.
					if (target !== null && sensitivity) this.acceptSensitivity(c, sensitivity, target);
					this.finishSolve(c, seq, target);
					return;
				} catch (e) {
					if (seq !== (c.solveSeq ?? 0)) return;
					const msg = errorText(e);
					// For AC OPF / SOCWR (browser-only) an infeasible operating point is an
					// expected outcome — demand pushed past what the network can serve — not a
					// broken Study. Keep the Study (it solves other points) and say so plainly,
					// instead of the misleading "study unavailable".
					if (c.formulation !== 'dcopf' && /infeasible/i.test(msg)) {
						c.solving = false;
						this.app.previewActive = false;
						this.app.previewDeltaMw = null;
						this.app.previewRatingMw = null;
						this.app.error = `${this.caseName(c)}: ${formulationLabel(c.formulation)} has no feasible solution at this demand`;
						return;
					}
					// Any other commit failure is unexpected; drop the Study so the next solve
					// rebuilds, then fall through to the server fallback.
					this.disposeStudy(c);
					c.solveFallbackReason ??= `browser study commit failed: ${msg}`;
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
				this.app.error = `${this.caseName(c)}: ${formulationLabel(c.formulation)} runs only in the browser Study, which is unavailable: ${why}`;
				return;
			}

			// Neither the server stream nor any other path can apply rating edits;
			// falling through would silently solve at base ratings, so stop here instead.
			if (this.hasRatingEdits(c)) {
				if (seq !== (c.solveSeq ?? 0)) return;
				c.solving = false;
				this.app.error = this.ratingEditsFallbackError(c);
				return;
			}

			// Fallback: the server stream for backend cases. The Study is the only
			// browser solver, so a local case with a failed Study is terminal.
			if (seq !== (c.solveSeq ?? 0)) return;
			if (this.isBackendCase(c)) {
				this.serverSolve(c, target, seq);
				return;
			}
			c.solving = false;
			this.app.error = `${c.label}: browser solve unavailable${
				c.solveFallbackReason ? `: ${c.solveFallbackReason}` : ''
			}`;
		});
	};

	serverSolve = (c: CaseState, target: SensTarget | null, seq = c.solveSeq) => {
		// Known-disabled server compute: show the notice instead of opening a
		// stream that answers 403 without a readable body (EventSource hides it).
		if (this.serverComputeOff) {
			if (seq !== c.solveSeq) return;
			c.solving = false;
			if (this.isActiveSolveCase(c)) {
				this.app.previewActive = false;
				this.app.previewDeltaMw = null;
				this.app.previewRatingMw = null;
			}
			this.fail(WASM_REQUIRED_NOTICE);
			return;
		}
		c.solveBackend = 'rust-server';
		c.solveFallbackReason ??= 'browser solve unavailable';
		// The server computes bus columns only; a branch target streams no sensitivity.
		const sensBus = target && 'bus' in target ? target.bus : null;
		c.closeStream = this.api.openSolveStream(c.id, c.deltas, sensBus, {
			onsolution: (sol) => {
				if (seq !== c.solveSeq) return;
				c.solution = sol;
				c.iterations = sol.iterations ?? [];
				c.solveMs = sol.solve_ms;
			},
			onsensitivity: (col) => {
				if (seq !== c.solveSeq) return;
				this.acceptSensitivity(c, col, target);
			},
			onfail: (msg) => {
				if (seq !== c.solveSeq) return;
				c.solving = false;
				// Only the active case owns the global preview fields (mirror finishSolve); a
				// background case's fallback failure must not collapse the active case's preview.
				if (this.isActiveSolveCase(c)) {
					this.app.previewActive = false;
					this.app.previewDeltaMw = null;
					this.app.previewRatingMw = null;
				}
				this.fail(`${this.caseName(c)}: ${formulationLabel(c.formulation)} ${msg}`, () =>
					this.runSolve(c, target)
				);
			},
			ondone: () => {
				this.finishSolve(c, seq, target);
			}
		});
	};

	commitDelta = (value: number) => {
		const c = this.activeSolvable;
		const bus = this.app.selectedBus;
		if (!c || bus === null) return;
		// Refresh the engine preview at the commit value (a typed value may not have
		// driven a drag), then score the commit with the engine's predicted Δobjective.
		this.runPreview(c, bus, value);
		c.predictedObjective = this.predictedDeltaObj;
		c.deltas = this.previewDeltas(c, bus, value);
		this.app.previewDeltaMw = value;
		this.app.previewActive = true;
		this.runSolve(c, { bus });
	};

	finishDemandInput = (value: number) => {
		if (Math.abs(value - this.committedDelta) < 0.25) {
			if (!this.activeSolvable?.solving) {
				this.app.previewActive = false;
				this.app.previewDeltaMw = null;
			}
			return;
		}
		this.commitDelta(value);
	};

	resetCase = (c: SolvableCase) => {
		c.deltas = {};
		c.ratings = {};
		c.predictedObjective = null;
		this.app.previewLmp = null;
		this.previewObjective = null;
		this.ratingSlope = null;
		this.app.previewDeltaMw = this.app.selectedBus === null ? null : 0;
		this.app.previewRatingMw = this.app.selectedBranch === null ? null : 0;
		this.app.previewActive = this.app.selectedBus !== null || this.app.selectedBranch !== null;
		this.app.demandRangeMode = 'local';
		if (this.app.selectedBus !== null) this.setNearbyRangeAnchor(c, this.app.selectedBus, 0);
		if (c.baseSolution) c.solution = c.baseSolution;
		this.runSolve(c, this.selectionTarget);
	};

	// Switch the active case to a new OPF formulation. Solving every formulation stays
	// entirely in the browser via the Study (nothing is routed to the server), so this
	// disposes the old Study — `getStudy` rebuilds it for the new formulation, re-parsing
	// and re-solving the base — then re-solves at the committed demand. The base solution
	// is dropped so it is recaptured under the new formulation (a DC and an AC objective
	// are not comparable). A no-op when the choice is unchanged.
	changeFormulation = (c: SolvableCase, next: Formulation) => {
		if (c.formulation === next) return;
		// Disabled menu items (e.g. AC OPF, coming soon) are not selectable in the engine yet.
		if (FORMULATIONS.find((f) => f.id === next)?.disabled) return;
		c.formulation = next;
		// The committed point carries over (same demand), but the model and its solution do
		// not; drop the Study and the cached solutions so they rebuild under `next`.
		this.disposeStudy(c);
		c.baseSolution = null;
		c.solution = null;
		c.iterations = [];
		c.solveMs = null;
		c.predictedObjective = null;
		this.app.previewLmp = null;
		this.previewObjective = null;
		this.ratingSlope = null;
		this.app.error = null;
		// The current sensitivity column was computed under the old formulation; clear it so
		// the overlay recomputes (the Study returns the new column with the re-solve below).
		c.sensitivity = null;
		c.sensitivitySeq = (c.sensitivitySeq ?? 0) + 1;
		this.runSolve(c, this.selectionTarget);
	};

	/** Parse dropped files in the browser via the powerio wasm module. Case
	 * files (.m, .raw, .aux) become local networks; geographic files can
	 * place those networks; a PowerWorld .pwd becomes a substation point
	 * preview. Files run serially; nothing uploads. */
	ingestFiles = async (files: FileList | File[]) => {
		const list = Array.from(files);
		const geoFiles: GeoFile[] = [];
		for (const file of list.filter((f) => isGeoFile(f.name))) {
			this.app.parsingFile = true;
			try {
				geoFiles.push(parseGeoFile(file.name, await file.text()));
				this.app.error = null;
			} catch (e) {
				this.app.error = `${file.name}: ${e instanceof Error ? e.message : e}`;
			} finally {
				this.app.parsingFile = false;
			}
		}

		let parsedCaseCount = 0;
		for (const file of list.filter((f) => !isGeoFile(f.name))) {
			if (isDisplayFile(file.name)) {
				this.app.parsingFile = true;
				try {
					const bytes = new Uint8Array(await file.arrayBuffer());
					const display = await parseDisplay(bytes);
					const points = display.substations.map((s) => {
						const [lon, lat] = this.pwdToLngLat(s.x, s.y);
						return { number: s.number, name: s.name, lon, lat };
					});
					const id = `local-${++this.localSeq}`;
					this.addAndActivateLocal(
						new LocalCase({
							id,
							label: file.name.replace(/\.[^.]+$/, ''),
							fileName: file.name,
							summary: null,
							view: null,
							substations: { points, approximate: true }
						})
					);
					this.app.error = null;
				} catch (e) {
					this.app.error = `${file.name}: ${e instanceof Error ? e.message : e}`;
				} finally {
					this.app.parsingFile = false;
				}
				continue;
			}
			const format = formatOf(file.name);
			if (!format) {
				this.app.error = `${file.name}: not a case or geographic file (.m, .raw, .aux, .pwd, .csv, .json, .geojson)`;
				continue;
			}
			this.app.parsingFile = true;
			try {
				const text = await file.text();
				const { network_json, topology, view, ...summary } = await ingestCase(text, format);
				if (format === 'aux' && (summary.n_branch === 0 || summary.n_gen === 0)) {
					this.app.error = `${file.name}: aux parsed, but no complete network; drop the matching .m or .raw case file`;
					continue;
				}
				const id = `local-${++this.localSeq}`;
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
				// A co-dropped geographic file places (or repositions) the network. Apply it
				// for any parsed case that carries topology, not just synthetic ones, so a geo
				// overlay dropped alongside a case that already has coordinates takes effect
				// instead of being silently discarded.
				if (geoFiles.length > 0 && local.topology) {
					this.withGeoFile(local, geoFiles);
				}
				this.addAndActivateLocal(local);
				parsedCaseCount++;
				this.app.error = null; // a successful parse clears a prior file's error
			} catch (e) {
				this.app.error = `${file.name}: ${e instanceof Error ? e.message : e}`;
			} finally {
				this.app.parsingFile = false;
			}
		}

		if (geoFiles.length > 0 && parsedCaseCount === 0) this.applyGeoFilesToExisting(geoFiles);
	};

	setDemandRangeMode = (mode: DemandRangeMode) => {
		this.app.demandRangeMode = mode;
		const c = this.activeSolvable;
		if (mode === 'local' && c && this.app.selectedBus !== null) {
			this.setNearbyRangeAnchor(c, this.app.selectedBus, this.sliderValue);
		}
		const bounds = this.demandBounds(
			mode,
			this.selectedBusData,
			mode === 'local' ? this.sliderValue : this.nearbyRangeCenter
		);
		if (this.app.previewDeltaMw === null) return;
		this.app.previewDeltaMw = Math.min(bounds.max, Math.max(bounds.min, this.app.previewDeltaMw));
	};

	// First-order engine preview for the live drag. Uses the case's already-built
	// Study (synchronous, no re-parse, no re-solve) to paint predicted per-bus
	// ΔLMP and the predicted Δobjective. A no-op when the Study isn't built yet or
	// can't preview (browser fallback path, server-only cases): the map then
	// falls back to the JS sensitivity-times-step preview.
	runPreview = (c: SolvableCase, bus: number, value: number) => {
		// Fast path: the committed ∂LMP/∂d column (already solved at the committed point),
		// scaled by the demand step, is the same first-order linearization the engine preview
		// returns — without rebuilding the differentiable KKT every drag frame. That engine
		// preview is ~80 ms/frame on the largest case (CATS, ~8870 buses), which blocked the
		// main thread and made dragging choppy; reusing the column is O(buses).
		const col = c.sensitivity;
		if (col && col.bus === bus) {
			const committedAtBus = this.caseDeltas(c)[bus] ?? 0;
			const step = (Math.abs(value) < 0.25 ? 0 : value) - committedAtBus;
			const delta = new Map<number, number>();
			for (const v of col.values) delta.set(v.bus, v.value * step);
			this.app.previewLmp = { caseId: c.id, target: { bus }, delta };
			const lmpAtBus = c.solution?.lmp.find((l) => l.bus === bus)?.usd_per_mwh ?? null;
			this.previewObjective =
				lmpAtBus === null
					? null
					: { caseId: c.id, target: { bus }, objectiveDelta: lmpAtBus * step };
			return;
		}
		// Fallback (no committed column yet): the engine's first-order preview, which rebuilds
		// the differentiable system. Best effort — on any failure the map keeps its own path.
		const study = this.caseStudies.get(c)?.study;
		if (!study) return;
		try {
			const { lmp, objectiveDelta } = study.preview(
				this.previewDeltas(c, bus, value),
				this.caseRatings(c)
			);
			const delta = new Map<number, number>();
			for (const e of lmp) delta.set(e.bus, e.usd_per_mwh);
			this.app.previewLmp = { caseId: c.id, target: { bus }, delta };
			this.previewObjective =
				objectiveDelta === null
					? null
					: { caseId: c.id, target: { bus }, objectiveDelta };
		} catch {
			this.app.previewLmp = null;
			this.previewObjective = null;
		}
	};

	setSliderPreview = (value: number | undefined) => {
		if (value === undefined) return;
		this.app.previewActive = true;
		this.app.previewDeltaMw = value;
		const c = this.activeSolvable;
		if (c && this.app.selectedBus !== null) this.runPreview(c, this.app.selectedBus, value);
	};

	// Ratings the live preview would commit: the committed rating deltas with the
	// slider's value at the selected branch, applying commitRating's dead zone so a
	// tiny nudge reads as "no change at this line".
	previewRatings(c: SolvableCase, branch: number, value: number): BranchRatingDeltas {
		const ratings = { ...this.caseRatings(c) };
		if (Math.abs(value) < 0.25) delete ratings[branch];
		else ratings[branch] = value;
		return ratings;
	}

	// First-order engine preview for the live rating drag, mirroring runPreview: the
	// committed ∂LMP/∂rating column scaled by the rating step paints predicted per-bus
	// ΔLMP without touching the engine per frame. The predicted Δobjective comes from
	// a per-MW slope taken once from Study.preview at a +1 MW step off the committed
	// point (preview is replacement-absolute, so the absolute ratings map is built),
	// then scaled by the live step; null when no Study slope is available.
	runRatingPreview = (c: SolvableCase, branch: number, value: number) => {
		const committedAtBranch = this.caseRatings(c)[branch] ?? 0;
		const step = (Math.abs(value) < 0.25 ? 0 : value) - committedAtBranch;
		const col = c.sensitivity;
		if (col && col.branch === branch) {
			const delta = new Map<number, number>();
			for (const v of col.values) delta.set(v.bus, v.value * step);
			this.app.previewLmp = { caseId: c.id, target: { branch }, delta };
		}
		if (this.ratingSlope?.caseId !== c.id || this.ratingSlope.branch !== branch) {
			this.ratingSlope = null;
			const study = this.caseStudies.get(c)?.study;
			if (study) {
				try {
					const { objectiveDelta } = study.preview(
						this.caseDeltas(c),
						this.previewRatings(c, branch, committedAtBranch + 1)
					);
					if (objectiveDelta !== null) {
						this.ratingSlope = { caseId: c.id, branch, slope: objectiveDelta };
					}
				} catch {
					// Best effort; the readout just shows no prediction.
				}
			}
		}
		this.previewObjective =
			this.ratingSlope === null
				? null
				: {
						caseId: c.id,
						target: { branch },
						objectiveDelta: this.ratingSlope.slope * step
					};
	};

	setRatingPreview = (value: number | undefined) => {
		if (value === undefined) return;
		this.app.previewActive = true;
		this.app.previewRatingMw = value;
		const c = this.activeSolvable;
		if (c && this.app.selectedBranch !== null) {
			this.runRatingPreview(c, this.app.selectedBranch, value);
		}
	};

	commitRating = (value: number) => {
		const c = this.activeSolvable;
		const branch = this.app.selectedBranch;
		if (!c || branch === null) return;
		// Refresh the engine preview at the commit value (a typed value may not have
		// driven a drag), then score the commit with the engine's predicted Δobjective.
		this.runRatingPreview(c, branch, value);
		c.predictedObjective = this.predictedDeltaObj;
		c.ratings = this.previewRatings(c, branch, value);
		// The slope was taken at the old committed point; the next drag re-derives it.
		this.ratingSlope = null;
		this.app.previewRatingMw = value;
		this.app.previewActive = true;
		this.runSolve(c, this.selectionTarget);
	};

	finishRatingInput = (value: number) => {
		if (Math.abs(value - this.committedRating) < 0.25) {
			if (!this.activeSolvable?.solving) {
				this.app.previewActive = false;
				this.app.previewRatingMw = null;
			}
			return;
		}
		this.commitRating(value);
	};

	restoreDefaultCases = () => {
		try {
			if (typeof localStorage !== 'undefined') localStorage.removeItem(HIDDEN_DEFAULT_CASES_KEY);
		} catch {
			// Ignore storage failures and reload from the server.
		}
		this.hiddenDefaults = new Set();
		this.load();
	};

	sliderCurrent = () => this.sliderValue;
	ratingSliderCurrent = () => this.ratingSliderValue;
}

export function createController(app: AppState, options: ControllerOptions = {}): Controller {
	return new Controller(app, options);
}
