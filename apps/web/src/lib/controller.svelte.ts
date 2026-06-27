import {
	getCaseNetworkJson,
	getCases,
	getNetwork,
	getSensitivity,
	getSolution,
	openSolveStream
} from '$lib/api';
import type { Network, NetworkBus, SensitivityColumn, Solution } from '$lib/api';
import { lmpDomain, lmpGradient, sensFlatColor, sensitivityDomain } from '$lib/colors';
import {
	applyGeoFile,
	isGeoFile,
	mergeGeoFiles,
	parseGeoFile,
	type GeoFile
} from '$lib/geo-file';
import {
	CaseState,
	LocalCase,
	type AppState,
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
import { errorText, formulationLabel, priceCopy, rgbaCss } from '$lib/format';

// Counter for local case ids; module level so ids stay unique across remounts.
let localSeq = 0;

const HIDDEN_DEFAULT_CASES_KEY = 'tellegen.hiddenDefaultCases.v1';
// Open on South Carolina by default: it has the most interesting price action.
const DEFAULT_CASE_ID = 'case500';
const PWD_MERCATOR_K = 535.81608;

export type SolvableCase = CaseState | LocalCase;
type DemandRangeAnchor = {
	caseId: string;
	bus: number;
	delta: number;
};
export type DisplayOption = {
	mode: DisplayMode;
	label: string;
	unit: string;
	copy: string;
	gradient: string;
	values: { bus: number; value: number }[];
};

export class Controller {
	app: AppState;
	abort: AbortController | null = null;

	// Build-once browser Study per case: the network is parsed and the model built
	// when the Study is created, so a drag re-solves (commit) and previews without
	// re-parsing. Kept in a WeakMap off the reactive/raw case payloads — the wasm
	// handle is neither serialized nor part of any $state.
	caseStudies = new WeakMap<
		SolvableCase,
		{ study: BrowserStudy; networkJson: string; formulation: Formulation; baseSolution: Solution }
	>();
	// Latch a permanent sensitivity-module failure (the sens build's relaxed SIMD,
	// which Safari rejects) per case so we don't retry createStudy — and the same
	// permanent error — on every drag. Transient failures are not latched.
	studyUnavailable = new WeakMap<SolvableCase, string>();

	nearbyRangeAnchor = $state<DemandRangeAnchor | null>(null);
	// Default cases the user has closed; drives the restore affordance. Seeded in load().
	hiddenDefaults = $state<Set<string>>(new Set());
	casesLoaded = $state(false);
	loadingBackendCase = $state<string | null>(null);
	showFileDropUi = $state(true);
	// Predicted objective change vs the committed point for the live preview. The
	// engine (Study.preview) owns this for browser-solvable cases; null when no
	// engine preview applies (server/Safari path), where the first-order fallback
	// below fills in. Scoped to the case + bus it was computed for. Set by runPreview.
	previewObjective = $state.raw<{ caseId: string; bus: number; objectiveDelta: number } | null>(
		null
	);

	constructor(app: AppState) {
		this.app = app;
	}

	// ===== helpers =====

	isBackendCase(c: SolvableCase): c is CaseState {
		return c instanceof CaseState;
	}

	isActiveSolveCase(c: SolvableCase): boolean {
		return this.isBackendCase(c) ? this.app.activeCaseId === c.id : this.app.activeLocalId === c.id;
	}

	caseDeltas(c: SolvableCase) {
		return this.isBackendCase(c) ? c.deltas : (c.deltas ?? {});
	}

	isPerturbed(c: SolvableCase | null): boolean {
		return c ? Object.values(this.caseDeltas(c)).some((mw) => mw !== 0) : false;
	}

	setNearbyRangeAnchor(c: SolvableCase, bus: number, delta = this.caseDeltas(c)[bus] ?? 0) {
		this.nearbyRangeAnchor = { caseId: c.id, bus, delta };
	}

	displayOptionsFor(c: SolvableCase | null): DisplayOption[] {
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

	displayDomain(mode: DisplayMode, values: number[]): { lo: number; hi: number } {
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
	// falls back to solveDc/the server, surfacing solveFallbackReason.
	async getStudy(c: SolvableCase, networkJson: string): Promise<BrowserStudy | null> {
		const latched = this.studyUnavailable.get(c);
		if (latched) {
			c.solveFallbackReason ??= latched;
			return null;
		}
		const cached = this.caseStudies.get(c);
		if (cached && cached.networkJson === networkJson && cached.formulation === c.formulation)
			return cached.study;
		if (cached) {
			cached.study.free();
			this.caseStudies.delete(c);
		}
		try {
			const study = await createStudy(networkJson, c.formulation);
			const baseSolution = study.currentSolution();
			this.caseStudies.set(c, { study, networkJson, formulation: c.formulation, baseSolution });
			return study;
		} catch (e) {
			const message = errorText(e);
			// Only a genuine browser-capability failure is permanent; latch it so the
			// case stays on the fallback path. Transient errors stay retryable.
			if (isPermanentSensFailure(message)) {
				this.studyUnavailable.set(c, 'browser study needs SIMD this browser does not support');
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
	async browserSensitivity(
		c: SolvableCase,
		networkJson: string,
		busId: number
	): Promise<SensitivityColumn | null> {
		if (c.formulation === 'dcopf') {
			return (await solveDc(c.id, networkJson, this.caseDeltas(c), busId)).sensitivity;
		}
		const study = await this.getStudy(c, networkJson);
		return study ? study.sensitivity(c.id, this.caseDeltas(c), busId) : null;
	}

	// Release a case's Study (if any) when the case is removed.
	disposeStudy(c: SolvableCase) {
		const cached = this.caseStudies.get(c);
		if (cached) {
			cached.study.free();
			this.caseStudies.delete(c);
		}
	}

	acceptSensitivity(
		c: SolvableCase,
		col: SensitivityColumn | null,
		busId: number | null,
		sensitivitySeq?: number
	) {
		if (!col || busId === null) return;
		if (col.bus !== busId) return;
		if (!this.isActiveSolveCase(c) || this.app.selectedBus !== busId) return;
		if (sensitivitySeq !== undefined && sensitivitySeq !== (c.sensitivitySeq ?? 0)) return;
		c.sensitivity = col;
	}

	finishSolve(c: SolvableCase, seq: number, sensBus: number | null) {
		if (seq !== (c.solveSeq ?? 0)) return;
		c.solving = false;
		if (this.isActiveSolveCase(c) && this.app.selectedBus === sensBus) {
			this.app.previewActive = false;
			this.app.previewDeltaMw = null;
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
			const json = await getCaseNetworkJson(c.id);
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
			binding: c.solution ? c.solution.flows.filter((f) => f.loading >= 0.999).length : null
		};
	});

	stats = $derived.by(() => {
		const c = this.activeSolvable;
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

	displayOptions = $derived(this.displayOptionsFor(this.activeSolvable));
	activeDisplay = $derived(
		this.displayOptions.find((option) => option.mode === this.app.displayMode) ??
			this.displayOptions[0] ??
			null
	);
	displayStats = $derived.by(() => {
		if (!this.activeDisplay) return null;
		const values = this.activeDisplay.values.map((entry) => entry.value).filter(Number.isFinite);
		if (values.length === 0) return null;
		const domain = this.displayDomain(this.activeDisplay.mode, values);
		const min = Math.min(...values);
		const max = Math.max(...values);
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

	selectedSensitivity = $derived.by(() => {
		const c = this.activeSolvable;
		if (!c?.sensitivity || this.app.selectedBus === null) return null;
		return c.sensitivity.bus === this.app.selectedBus ? c.sensitivity : null;
	});

	sensSummary = $derived.by(() =>
		this.selectedSensitivity
			? sensitivityDomain(this.selectedSensitivity.values.map((v) => v.value))
			: null
	);
	flatSensBackground = $derived(this.sensSummary ? rgbaCss(sensFlatColor(this.sensSummary)) : '');

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

	// Predicted objective change vs base for the demand readout. Prefer the engine
	// preview (Study.preview at the committed point, plus the committed part); fall
	// back to a second-order gradient estimate when no Study preview is available
	// (server-only cases, or a browser that can't load the sensitivity module): the
	// exact committed part plus lmp·step + S_bb·step²/2 along the gradient.
	predictedDeltaObj = $derived.by(() => {
		const c = this.activeSolvable;
		const bus = this.app.selectedBus;
		if (!c?.solution || !c.baseSolution || bus === null) return null;
		const committedPart = c.solution.objective - c.baseSolution.objective;
		if (
			this.previewObjective &&
			this.previewObjective.caseId === c.id &&
			this.previewObjective.bus === bus
		) {
			return committedPart + this.previewObjective.objectiveDelta;
		}
		if (this.selectedLmp === null) return null;
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
					Math.abs(this.sliderValue - this.committedDelta) >= 0.25)
		)
	);

	// ===== actions =====

	load = async () => {
		try {
			const summaries = await getCases();
			const hidden = this.readHiddenDefaultCases();
			this.hiddenDefaults = new Set(hidden);
			this.app.cases = summaries.filter((s) => !hidden.has(s.id)).map((s) => new CaseState(s));
			this.app.activeLocalId = null;
			this.app.placingLocalId = null;
			this.app.activeCaseId =
				this.app.cases.find((c) => c.id === DEFAULT_CASE_ID)?.id ?? this.app.cases[0]?.id ?? null;
			const active = this.app.active;
			if (active) await this.loadBackendCase(active, true);
			else this.app.requestFrame('all');
		} catch (e) {
			this.app.error = `server unreachable: ${e instanceof Error ? e.message : e}`;
		} finally {
			this.casesLoaded = true;
		}
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
				c.network ? Promise.resolve(c.network) : getNetwork(c.id),
				// The server caches only the DC OPF base solution. Other formulations
				// must hydrate from the browser Study so their cost, prices, flows, and
				// voltage fields all come from the selected formulation.
				requestedFormulation === 'dcopf' ? getSolution(c.id) : Promise.resolve(null)
			]);
			if (!this.app.byId(c.id)) return;
			c.network = network;
			if (dcBaseSolution && c.formulation === requestedFormulation) {
				c.baseSolution = dcBaseSolution;
				c.solution = dcBaseSolution;
			}
			if (frame && this.app.activeCaseId === c.id) this.app.requestFrame(c.id);
			if (this.isActiveSolveCase(c) && c.formulation !== 'dcopf' && !c.solution && !c.solving) {
				this.runSolve(c, this.app.selectedBus);
			}
		} catch (e) {
			if (this.app.byId(c.id)) this.app.error = `${c.name}: ${errorText(e)}`;
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
		this.app.removeCase(c.id);
		const active = this.app.active;
		if (active) await this.loadBackendCase(active, true);
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

	removeLocalCase = (c: LocalCase, event?: MouseEvent) => {
		event?.stopPropagation();
		if (this.app.activeLocalId === c.id) {
			// Local cases solve in the browser only (no server stream); the seq bump
			// invalidates any in-flight browser solve.
			c.solveSeq = (c.solveSeq ?? 0) + 1;
			this.clearSelection();
		}
		this.disposeStudy(c);
		this.app.removeLocal(c.id);
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
		this.abort?.abort();
		const ac = new AbortController();
		this.abort = ac;
		const sensitivitySeq = ++c.sensitivitySeq;
		this.app.error = null;
		this.app.selectedBus = busId;
		this.app.previewDeltaMw = null;
		this.app.previewActive = false;
		this.app.demandRangeMode = 'local';
		this.setNearbyRangeAnchor(c, busId);
		this.app.sensitivityLoading = true;
		c.sensitivity = null;
		try {
			// The dLMP/dd column from the browser solver (under the case's formulation). DC OPF
			// may reconcile a null column via the server; AC OPF / SOCWR are browser-only, so a
			// null column there is reported, never sent to the DC-only server.
			const networkJson = await this.ensureNetworkJson(c);
			if (networkJson) {
				const sensitivity = await this.browserSensitivity(c, networkJson, busId);
				if (!ac.signal.aborted && sensitivity)
					this.acceptSensitivity(c, sensitivity, busId, sensitivitySeq);
				else if (!ac.signal.aborted && c.formulation === 'dcopf') {
					const col = await getSensitivity(caseId, busId, c.deltas, ac.signal);
					if (!ac.signal.aborted) this.acceptSensitivity(c, col, busId, sensitivitySeq);
				} else if (!ac.signal.aborted && !sensitivity) {
					this.app.error = `${c.name}: ${formulationLabel(c.formulation)} sensitivity unavailable in the browser${c.solveFallbackReason ? `: ${c.solveFallbackReason}` : ''}`;
				}
			} else if (c.formulation === 'dcopf') {
				const col = await getSensitivity(caseId, busId, c.deltas, ac.signal);
				if (!ac.signal.aborted) this.acceptSensitivity(c, col, busId, sensitivitySeq);
			} else if (!ac.signal.aborted) {
				this.app.error = `${c.name}: ${formulationLabel(c.formulation)} needs the browser network JSON, which is unavailable`;
			}
		} catch {
			// The browser path threw. DC OPF reconciles via the server; AC OPF / SOCWR have no
			// server fallback (nothing is solved on the server), so the column stays absent.
			if (c.formulation === 'dcopf') {
				try {
					const col = await getSensitivity(caseId, busId, c.deltas, ac.signal);
					if (!ac.signal.aborted) this.acceptSensitivity(c, col, busId, sensitivitySeq);
				} catch (e2) {
					if (!ac.signal.aborted && !(e2 instanceof DOMException)) this.app.error = String(e2);
				}
			}
		} finally {
			if (this.abort === ac) this.app.sensitivityLoading = false;
		}
	};

	selectLocalBus = async (localId: string, busId: number) => {
		const c = this.app.localCases.find((lc) => lc.id === localId);
		if (!c?.networkJson || !c.network) return;
		this.app.activeCaseId = null;
		this.app.activeLocalId = localId;
		this.app.placingLocalId = null;
		this.abort?.abort();
		const ac = new AbortController();
		this.abort = ac;
		c.sensitivitySeq = (c.sensitivitySeq ?? 0) + 1;
		const sensitivitySeq = c.sensitivitySeq;
		this.app.error = null;
		this.app.selectedBus = busId;
		this.app.previewDeltaMw = null;
		this.app.previewActive = false;
		this.app.demandRangeMode = 'local';
		this.setNearbyRangeAnchor(c, busId);
		this.app.sensitivityLoading = true;
		c.sensitivity = null;
		try {
			const sensitivity = await this.browserSensitivity(c, c.networkJson, busId);
			if (!ac.signal.aborted) this.acceptSensitivity(c, sensitivity, busId, sensitivitySeq);
			if (!ac.signal.aborted && !sensitivity) {
				// A null column means the solve ran but produced no dLMP/dd for this bus (or the
				// Study could not be built); local cases have no server fallback, so say so
				// instead of leaving the panel in LMP view with no explanation.
				this.app.error = `${c.label}: ${formulationLabel(c.formulation)} sensitivity unavailable in the browser${
					c.solveFallbackReason ? `: ${c.solveFallbackReason}` : ' (no dLMP/dd column for this bus)'
				}`;
			}
		} catch (e) {
			if (!ac.signal.aborted) this.app.error = `${c.label}: ${e instanceof Error ? e.message : e}`;
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
		this.app.previewDeltaMw = null;
		this.app.previewActive = false;
		this.app.previewLmp = null;
		this.previewObjective = null;
		this.app.demandRangeMode = 'local';
		this.nearbyRangeAnchor = null;
		this.app.sensitivityLoading = false;
	};

	// Exact DC solve in the browser (wasm). The build-once Study commits the new
	// operating point without re-parsing, returning the dLMP/dd column in the same
	// solve; on a Study failure it falls back to solveDc, and on any browser failure
	// or missing network JSON it reconciles via the server stream (backend cases).
	runSolve = (c: SolvableCase, sensBus: number | null) => {
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
				if (this.isBackendCase(c)) return this.serverSolve(c, sensBus, seq);
				c.solving = false;
				this.app.error = `${c.label}: local case has no browser network JSON`;
				return;
			}
			const t0 = performance.now();
			c.solveBackend = 'clarabel-wasm';

			// Build-once Study path: commit the new operating point (no re-parse). The
			// dLMP/dd column for the selected bus is computed in the same solve and comes
			// back with it, so there is no second solve to reconcile it.
			const study = await this.getStudy(c, networkJson);
			if (seq !== (c.solveSeq ?? 0)) return;
			if (study) {
				try {
					const cached = this.caseStudies.get(c);
					if (!c.baseSolution && cached?.study === study) c.baseSolution = cached.baseSolution;
					const { solution, iterations, sensitivity } = study.commit(
						c.id,
						this.caseDeltas(c),
						sensBus
					);
					if (seq !== (c.solveSeq ?? 0)) return;
					c.solution = solution;
					c.iterations = iterations;
					if (!c.baseSolution && Object.keys(this.caseDeltas(c)).length === 0)
						c.baseSolution = solution;
					c.solveMs = Math.round(performance.now() - t0);
					// The commit carried the dLMP/dd column; accept it for the selected bus
					// through the same seq-guarded setter every sensitivity source goes through.
					if (sensBus !== null && sensitivity) this.acceptSensitivity(c, sensitivity, sensBus);
					this.finishSolve(c, seq, sensBus);
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
						this.app.error = `${this.caseName(c)}: ${formulationLabel(c.formulation)} has no feasible solution at this demand`;
						return;
					}
					// Any other commit failure is unexpected; drop the Study so the next solve
					// rebuilds, then fall through to the solveDc fallback.
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

			// Fallback: the original per-call browser solve (re-parses each time). Used
			// when the Study can't be built (e.g. Safari's relaxed-SIMD gap) or a built
			// Study failed to commit; itself falls to the server for backend cases.
			solveDc(c.id, networkJson, this.caseDeltas(c), sensBus)
				.then(async ({ solution, sensitivity, sensitivityError, iterations }) => {
					if (seq !== (c.solveSeq ?? 0)) return;
					c.solution = solution;
					c.iterations = iterations;
					if (!c.baseSolution && Object.keys(this.caseDeltas(c)).length === 0)
						c.baseSolution = solution;
					c.solveMs = Math.round(performance.now() - t0);
					if (sensitivity || sensBus === null) {
						this.acceptSensitivity(c, sensitivity, sensBus);
					} else if (this.isBackendCase(c)) {
						// No browser sensitivity column (whether the solve threw or just
						// produced none): reconcile via the server for backend cases.
						try {
							const col = await getSensitivity(c.id, sensBus, c.deltas);
							if (seq !== c.solveSeq) return;
							c.solveBackend = 'clarabel-wasm-server-sensitivity';
							this.acceptSensitivity(c, col, sensBus);
						} catch (e) {
							if (seq !== c.solveSeq) return;
							c.solveFallbackReason = `server sensitivity failed: ${errorText(e)}`;
							this.serverSolve(c, sensBus, seq);
							return;
						}
					} else {
						// Local case: no server fallback, so report the gap (including a null
						// column with no error) instead of silently staying in LMP view.
						this.app.error = `${c.label}: browser sensitivity unavailable${sensitivityError ? `: ${sensitivityError}` : ' (no dLMP/dd column for this bus)'}`;
					}
					this.finishSolve(c, seq, sensBus);
				})
				.catch((e) => {
					if (seq !== (c.solveSeq ?? 0)) return;
					c.solveFallbackReason = `browser solve failed: ${errorText(e)}`;
					if (this.isBackendCase(c)) this.serverSolve(c, sensBus, seq);
					else {
						c.solving = false;
						this.app.error = `${c.label}: ${c.solveFallbackReason}`;
					}
				});
		});
	};

	serverSolve = (c: CaseState, sensBus: number | null, seq = c.solveSeq) => {
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
				this.acceptSensitivity(c, col, sensBus);
			},
			onfail: (msg) => {
				if (seq !== c.solveSeq) return;
				c.solving = false;
				this.app.previewActive = false;
				this.app.previewDeltaMw = null;
				this.app.error = msg;
			},
			ondone: () => {
				this.finishSolve(c, seq, sensBus);
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
		this.runSolve(c, bus);
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
		c.predictedObjective = null;
		this.app.previewLmp = null;
		this.previewObjective = null;
		this.app.previewDeltaMw = this.app.selectedBus === null ? null : 0;
		this.app.previewActive = this.app.selectedBus !== null;
		this.app.demandRangeMode = 'local';
		if (this.app.selectedBus !== null) this.setNearbyRangeAnchor(c, this.app.selectedBus, 0);
		if (c.baseSolution) c.solution = c.baseSolution;
		this.runSolve(c, this.app.selectedBus);
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
		this.app.error = null;
		// The current sensitivity column was computed under the old formulation; clear it so
		// the overlay recomputes (the Study returns the new column with the re-solve below).
		c.sensitivity = null;
		c.sensitivitySeq = (c.sensitivitySeq ?? 0) + 1;
		this.runSolve(c, this.app.selectedBus);
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
					const id = `local-${++localSeq}`;
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
				this.app.error = `${file.name}: not a case or coordinate file (.m, .raw, .aux, .pwd, .csv, .json, .geojson)`;
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
	// can't preview (Safari's relaxed-SIMD gap, server-only cases): the map then
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
			this.app.previewLmp = { caseId: c.id, bus, delta };
			const lmpAtBus = c.solution?.lmp.find((l) => l.bus === bus)?.usd_per_mwh ?? null;
			this.previewObjective =
				lmpAtBus === null ? null : { caseId: c.id, bus, objectiveDelta: lmpAtBus * step };
			return;
		}
		// Fallback (no committed column yet): the engine's first-order preview, which rebuilds
		// the differentiable system. Best effort — on any failure the map keeps its own path.
		const study = this.caseStudies.get(c)?.study;
		if (!study) return;
		try {
			const { lmp, objectiveDelta } = study.preview(this.previewDeltas(c, bus, value));
			const delta = new Map<number, number>();
			for (const e of lmp) delta.set(e.bus, e.usd_per_mwh);
			this.app.previewLmp = { caseId: c.id, bus, delta };
			this.previewObjective = objectiveDelta === null ? null : { caseId: c.id, bus, objectiveDelta };
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
}

export function createController(app: AppState): Controller {
	return new Controller(app);
}
