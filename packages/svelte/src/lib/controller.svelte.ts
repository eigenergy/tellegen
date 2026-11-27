import { NOTICES, caseLabel } from './controller/messages.js';
export { NOTICES, caseLabel } from './controller/messages.js';
import type { DcFormula } from '@tellegen/engine';
import {
	applyGeo,
	applyLayout,
	browserWasmTransport,
	calculationAvailable,
	COLLECTION_UNAVAILABLE,
	collectionSupported,
	extractGeo,
	formatPowerIoDiagnostic,
	ingestDistCaseBytes,
	ingestJsonDrop,
	probeAcOpfWorker,
	type AppliedGeoCase,
	type BrowserStudy,
	type EngineTransport,
	type Formulation,
	type IngestedCase,
	type IngestedDistCase,
	type McLoadPowerEdit,
	type McModuleInput,
	type PowerIoDiagnostic,
	type ResolvedSettings,
	type SensTarget
} from '@tellegen/engine';
import type { BranchRatingDeltas, Network, NetworkBus, Solution } from './api.js';
import { ApiError, createApiClient, RATE_LIMITED, type TellegenApiClient } from './api.js';
import { CollectionImport } from './collection-import.svelte.js';
import { previewScaleFor, sensitivityDomain } from './colors.js';
import { BalancedSolveActions } from './controller/balanced-solve.js';
import {
	CaseImportActions,
	type DisplayFile,
	type GeoLayerFile
} from './controller/case-imports.js';
import {
	CaseSettingsActions,
	defaultCalculationSettings,
	hasCustomCalculationSettings
} from './controller/case-settings.js';
import { DistributionResultsActions } from './controller/distribution-results.js';
import { DistributionSolveActions, releaseMcInput } from './controller/distribution-solve.js';
import { limitWords } from './diagram/verdict.js';
import {
	displayMetaFor,
	displaySeriesFor,
	dollarUnits,
	caseDeltas as sharedCaseDeltas,
	caseRatings as sharedCaseRatings
} from './display.js';
import { DynamicsController } from './dynamics/controller.svelte.js';
import { PIO_MODULE_EXPORT } from './export-formats.js';
import { atLimit, downloadText, errorText, formulationLabel } from './format.js';
import { errorTitle, powerIoEntry, type ErrorContext, type LogEntryInput } from './log.svelte.js';
import { displayRange } from './map-legend.js';
import { buildDiagramView, buildGeographicView, placeMultiView } from './multiconductor.js';
import { recordedCalculationSettings } from './settings.svelte.js';
import { DEAD_ZONE_MW } from './slider.js';
import {
	CaseState,
	LocalCase,
	MulticonductorCase,
	type AppState,
	type DemandRangeMode,
	type FallbackTarget,
	type SolvableCase
} from './state.svelte.js';
import { placeSyntheticTopology } from './synthetic-layout.js';

/** PowerIO IR text for a case input, built by the engine for a kept module. */
function mcInputJson(input: McModuleInput): Promise<string> {
	return typeof input === 'string' ? Promise.resolve(input) : input.moduleJson();
}

/** Terminal copy when nothing can solve. Two facts decide the wording: whether
 * this browser's engine failure is known permanent, and that the server
 * declines compute. */
const WASM_REQUIRED_NOTICE = 'Engine unavailable';
const COMPUTE_OFF_NOTICE = 'Solve failed';

/** Upper bound on waiting for a camera fly-to before the sensitivity solve
 * starts anyway: a missing map or a cue that never appears must not stall the
 * selection. */
const FOCUS_SETTLE_CAP_MS = 1200;

function previewUnits(sensitivityUnits: string): string {
	const numerator = sensitivityUnits.replace(/\/(?:MW|MVA)$/, '').trim();
	const unwrapped =
		numerator.startsWith('(') && numerator.endsWith(')') ? numerator.slice(1, -1) : numerator;
	return dollarUnits(unwrapped);
}

/** A lowered analysis row is view-only only when a current producer says so;
 * absence keeps payloads from older compatible engine/server versions editable. */
export function isDisplayOnlyElement(element: { editable?: boolean } | null | undefined): boolean {
	return element?.editable === false;
}

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

/** `writeCaseFile` found no Study to write; the error banner already says why. */
class CaseFileUnavailable extends Error {
	override name = 'CaseFileUnavailable';
}

export type { SolvableCase };
type DemandRangeAnchor = {
	caseId: string;
	bus: number;
	delta: number;
};

export interface ControllerOptions {
	api?: TellegenApiClient;
	/** Backend base path. `null` means there is no backend: no client is
	 * created and nothing is ever fetched. */
	apiBase?: string | null;
	/** Whether `load()` fetches the bundled cases. Default `true`. With
	 * `false`, `load()` only marks the shell loaded and frames the map. */
	loadDefaultCases?: boolean;
	acOpfWasmUrl?: string;
	mcTransport?: Pick<EngineTransport, 'solveMcModule' | 'applyMcGeo'> &
		Partial<
			Pick<
				EngineTransport,
				| 'solveMcStudy'
				| 'applyMcStudyGeo'
				| 'createMcPfSession'
				| 'ingestDistCaseBytes'
				| 'prepareMcEnergizedNetwork'
				| 'prepareMcFixedControls'
			>
		>;
}

/** Input to `Controller.openBalancedModule` and `replaceLocalCase`: a
 * balanced (transmission) powerio module, either as its JSON text or as the
 * payload the engine's `ingestCase` already produced for it. */
export interface OpenBalancedModuleInput {
	/** Case label; also the stem of the `.pio.json` file name the shell shows. */
	name: string;
	/** Module JSON, ingested through the engine. Ignored when `payload` is set. */
	moduleJson?: string;
	/** An already ingested balanced payload; skips the engine round trip. */
	payload?: IngestedCase;
	/** A `.geo.json` layer document, for a module that carries no coordinates. */
	geoLayer?: string;
	/** Overrides the formulation the module declares. */
	formulation?: Formulation;
	/** A Study already building from this module for the case's formulation,
	 * such as the one `BrowserSynthSession.ingestWithStudy` starts. The case's
	 * first solve uses it instead of building its own. The controller owns it
	 * from this call on and frees it when it cannot use it. */
	study?: Promise<BrowserStudy>;
	/** Default `true`: add the case, make it active, and start its browser
	 * solve. With `false` the prepared case is returned and not added. */
	activate?: boolean;
	/** Default `true`: fly the map to the case when it is activated. */
	frame?: boolean;
}

/** Free a Study nobody will use, once its build settles. */
function releaseWhenBuilt(study: Promise<BrowserStudy>) {
	void study.then(
		(built) => built.free(),
		() => {}
	);
}

/** A case's name for the header and panels: the name the file gives, unless that is a
 * description (a PSS/E RAW header line, say), in which case the file name. */

/** A case that needs the server when the host configured none. */
const noApiNotice = (name: string) => `${name} needs the tellegen server`;

const HIDDEN_DEFAULT_CASES_KEY = 'tellegen.hiddenDefaultCases.v1';
// Open on South Carolina by default: it has the most interesting price action.
const DEFAULT_CASE_ID = 'case500';

/** Commit and snapshot responses can order object keys differently (including nested loading
 * metadata). Compare the exact JSON values, retaining array order and numerical equality. */
function sameSolution(a: Solution, b: Solution): boolean {
	const ordered = (_key: string, value: unknown) => {
		if (value === null || typeof value !== 'object' || Array.isArray(value)) return value;
		const record = value as Record<string, unknown>;
		return Object.fromEntries(
			Object.keys(record)
				.sort()
				.map((key) => [key, record[key]])
		);
	};
	return JSON.stringify(a, ordered) === JSON.stringify(b, ordered);
}

/** Whether a local case renders or will render a bus with this id. */
function localHasBus(c: LocalCase, busId: number): boolean {
	const buses = c.view?.buses ?? c.diagram?.view.buses ?? c.topology?.buses ?? [];
	return buses.some((b) => b.id === busId);
}

type SavedCaseCapture = {
	input: string;
	solution?: string;
	view?: import('@tellegen/engine').StudyView;
};

export class Controller {
	app: AppState;
	/** `null` when the controller was created with `apiBase: null`; every
	 * server path then reports "compute off" instead of fetching. */
	api: TellegenApiClient | null;
	readonly loadDefaultCases: boolean;
	mcTransport: Pick<EngineTransport, 'solveMcModule' | 'applyMcGeo'> &
		Partial<
			Pick<
				EngineTransport,
				| 'solveMcStudy'
				| 'applyMcStudyGeo'
				| 'createMcPfSession'
				| 'ingestDistCaseBytes'
				| 'prepareMcEnergizedNetwork'
				| 'prepareMcFixedControls'
			>
		>;
	private hostedLoadAbort: AbortController | null = null;
	loadingHostedDistribution = $state<string | null>(null);
	abort: AbortController | null = null;
	// While set (epoch ms), the server sensitivity fallback is rate limited: skip
	// the request and show the rate-limit copy instead of burning the budget on a
	// guaranteed rejection. Covers one rate limit window. Not $state; nothing
	// renders it.
	sensitivity429Until = 0;
	// Latched once any server fallback answers 403: compute is disabled on this
	// deploy, so later fallbacks show the notice instead of firing doomed requests.
	serverComputeOff = false;
	/** The optional EPL-bearing WASI asset is never inferred or bundled. */
	private readonly acOpfWasmUrl: string | null;
	private acOpfProbe: Promise<boolean> | null = null;
	acOpfLoading = $state(false);
	private readonly acOpfProbeAbort = new AbortController();
	private readonly acOpfSolves = new Map<SolvableCase, AbortController>();
	acOpfAvailable = $state(false);

	// Build-once browser Study per case: the retained PowerIO module is parsed and the model built
	// when the Study is created, so a drag re-solves (commit) and previews without
	// re-parsing. Kept in a WeakMap off the reactive/raw case payloads — the wasm
	// handle is neither serialized nor part of any $state.
	caseStudies = new WeakMap<
		SolvableCase,
		{
			study: BrowserStudy;
			studyInputJson: string;
			formulation: Formulation;
			baseSolution: Solution;
			settings?: ResolvedSettings;
		}
	>();
	private detachedCaseCaptures = new WeakMap<
		LocalCase,
		{
			baseInput: string;
			edits: string;
			solution: Solution | null;
			captured: SavedCaseCapture;
		}
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
			studyInputJson: string;
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
	// File reads and worker messages retain whole buffers. Admit one validated
	// batch at a time so repeated drops cannot multiply the aggregate byte cap.
	collectionsAvailable = $state(false);
	private collectionProbe: Promise<boolean> | null = null;
	probeCollections = (): Promise<boolean> => {
		return (this.collectionProbe ??= collectionSupported()
			.then((available) => {
				if (!this.disposed) this.collectionsAvailable = available;
				return available;
			})
			.catch(() => false));
	};
	private openCollection = async (read: Parameters<CollectionImport['start']>[0]) => {
		if (!(await this.probeCollections()))
			this.fail(COLLECTION_UNAVAILABLE, null, {
				title: 'Imports unavailable',
				stage: 'Open'
			});
		else if (!this.disposed) await this.collectionImport.start(read);
	};
	readonly collectionImport = new CollectionImport((label, result) => {
		if (this.disposed) return;
		if (result.kind === 'multiconductor') {
			this.addMultiCase(label, result.payload, undefined, label);
		} else {
			const local = this.localFromBalancedPayload(label, {
				...result.payload,
				name: label
			});
			// Saved modules restore their calculation and overrides in LocalCase.
			// Only a raw network without recorded settings defaults to power flow.
			if (!result.payload.formulation && !local.settings.calculation) local.formulation = 'acpf';
			this.placeTopologyAsDiagram(local);
			this.addAndActivateLocal(local, { solve: false });
		}
	});
	// Set by dispose(). Not $state; nothing renders it.
	disposed = false;

	nearbyRangeAnchor = $state<DemandRangeAnchor | null>(null);
	// Default cases the user has closed; drives the restore affordance. Seeded in load().
	hiddenDefaults = $state<Set<string>>(new Set());
	casesLoaded = $state(false);
	/** The last attempt to fetch the case list failed. */
	casesFailed = $state(false);
	/** That failure's message, for the panel's Details. */
	casesError = $state<string | null>(null);
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
	ratingSlope: { caseId: string; branch: number; slope: number | null } | null = null;

	/** Dynamics mode: RMS simulation of a local case with its .dyr data. */
	readonly dynamics: DynamicsController;

	constructor(app: AppState, options: ControllerOptions = {}) {
		this.app = app;
		this.dynamics = new DynamicsController(app, (files) => this.ingestFiles(files));
		this.dynamics.onBusClick = (bus) => {
			const local = this.app.activeLocal;
			if (local) void this.selectLocalBus(local.id, bus);
		};
		this.api = options.api ?? createApiClient({ apiBase: options.apiBase });
		this.loadDefaultCases = options.loadDefaultCases ?? true;
		this.mcTransport = options.mcTransport ?? browserWasmTransport;
		this.acOpfWasmUrl = options.acOpfWasmUrl ?? null;
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

	/** Whether the engine can run `formulation` on this case; see `calculationAvailable`. */
	formulationAvailable(c: SolvableCase, formulation: Formulation): boolean {
		return calculationAvailable(formulation, {
			edited: c.perturbed,
			acOpf: this.acOpfAvailable
		});
	}

	/** Whether a Calculation menu offers `formulation` for this case as it stands. */
	calculationOffered(c: SolvableCase, formulation: Formulation): boolean {
		return calculationAvailable(formulation, {
			declared: c instanceof LocalCase ? c.declaredFormulation : null,
			edited: c.perturbed,
			ratingsEdited: c.ratingsEdited,
			acOpf: this.acOpfAvailable
		});
	}

	/** Whether this host supplied the optional solver asset. It loads only on request. */
	get acOpfConfigured(): boolean {
		return Boolean(this.acOpfWasmUrl);
	}

	/** Share one probe across the shell and embedded controls, and cancel it on disposal. */
	loadAcOpf = (): Promise<boolean> => {
		if (this.disposed || !this.acOpfWasmUrl) return Promise.resolve(false);
		if (this.acOpfAvailable) return Promise.resolve(true);
		if (this.acOpfProbe) return this.acOpfProbe;
		this.acOpfLoading = true;
		this.acOpfProbe = probeAcOpfWorker(this.acOpfWasmUrl, undefined, this.acOpfProbeAbort.signal)
			.then((available) => {
				if (this.disposed) return false;
				this.acOpfAvailable = available;
				return available;
			})
			.catch(() => false)
			.finally(() => {
				this.acOpfLoading = false;
				if (!this.acOpfAvailable) this.acOpfProbe = null;
			});
		return this.acOpfProbe;
	};

	probeAcOpf = () => {
		void this.loadAcOpf().then((available) => {
			if (this.disposed) return;
			// A declared instance can be dropped while the optional asset is probing.
			const c = this.app.activeLocal;
			if (available && c?.formulation === 'acopf' && !c.solving) {
				this.maybeStartLocalSolve(c.id);
			}
		});
	};

	caseExportUnavailableReason(c: SolvableCase): string | null {
		return c.formulation === 'acopf'
			? 'Saving and exporting AC OPF cases is not supported yet.'
			: null;
	}

	bumpRevision(c: SolvableCase): void {
		c.revisionGeneration += 1;
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
		this.bumpRevision(c);
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
		this.bumpRevision(c);
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
		const view = c.displayMode === 'diagram' ? c.diagram?.view : c.view;
		if (!c.summary || !view) return null;
		return {
			id: c.id,
			name: c.label,
			base_mva: c.summary.base_mva,
			synthetic_coords: c.coordsKind !== 'file' && c.coordsKind !== 'geofile',
			...(c.network?.model_details ? { model_details: c.network.model_details } : {}),
			coordinate_space: view.coordinate_space,
			buses: view.buses,
			branches: view.branches,
			equipment: view.equipment,
			...(view.substations ? { substations: view.substations } : {})
		};
	}
	private createBalancedSolve(): BalancedSolveActions {
		const controller = this;
		return new BalancedSolveActions(this, {
			get disposed() {
				return controller.disposed;
			},
			get acOpfWasmUrl() {
				return controller.acOpfWasmUrl;
			},
			get acOpfSolves() {
				return controller.acOpfSolves;
			},
			computeOffNotice: (...args) => this.computeOffNotice(...args),
			ratingEditsFallbackError: (...args) => this.ratingEditsFallbackError(...args),
			fail: (...args) => this.fail(...args),
			failed: (...args) => this.failed(...args)
		});
	}
	private readonly balancedSolve: BalancedSolveActions = this.createBalancedSolve();

	maybeStartLocalSolve = this.balancedSolve.maybeStartLocalSolve;

	getStudy = this.balancedSolve.getStudy;

	browserSensitivity = this.balancedSolve.browserSensitivity;

	/** Release every engine handle this controller holds: each case's browser
	 * Study, including one still being built, its AC OPF solve, each
	 * multiconductor session, with their pending edits and in-flight requests,
	 * and the AC OPF asset probe. `TellegenProvider` calls it when it unmounts;
	 * the controller builds no Study and keeps no new session afterwards. */
	dispose() {
		if (this.disposed) return;
		this.disposed = true;
		this.collectionImport.close();
		this.hostedLoadAbort?.abort();
		this.hostedLoadAbort = null;
		this.loadingHostedDistribution = null;
		this.abort?.abort();
		this.abort = null;
		this.acOpfProbeAbort.abort();
		for (const cancellation of this.acOpfSolves.values()) cancellation.abort();
		this.acOpfSolves.clear();
		for (const c of [...this.app.cases, ...this.app.localCases]) {
			c.solveSeq = (c.solveSeq ?? 0) + 1;
			this.disposeStudy(c);
		}
		for (const c of this.app.multiCases) {
			c.solveAbort?.abort();
			if (c.mcEditTimer !== null) clearTimeout(c.mcEditTimer);
			c.mcEditTimer = null;
			c.mcSession?.free();
			c.mcSession = null;
			releaseMcInput(c.mcInput);
			c.mcInput = null;
		}
	}
	disposeStudy = this.balancedSolve.disposeStudy;

	acceptSensitivity = this.balancedSolve.acceptSensitivity;

	finishSolve = this.balancedSolve.finishSolve;

	ensureStudyInputJson = this.balancedSolve.ensureStudyInputJson;

	// Deltas the live preview would commit: the committed deltas with the slider's
	// value at the selected bus, applying commitDelta's dead zone so a tiny nudge
	// reads as "no change at this bus".
	previewDeltas(c: SolvableCase, bus: number, value: number) {
		const deltas = { ...this.caseDeltas(c) };
		if (Math.abs(value) < DEAD_ZONE_MW) delete deltas[bus];
		else deltas[bus] = value;
		return deltas;
	}

	demandBounds(
		mode: DemandRangeMode,
		bus: NetworkBus | null,
		center: number
	): { min: number; max: number; span: number } {
		if (!bus || isDisplayOnlyElement(bus)) return { min: 0, max: 0, span: 0 };
		// Floor, not ceil: -ceil(demand) can push base + delta below zero for
		// non-integer demand, which the server rejects (400) and which is
		// physically meaningless (demand cannot go negative).
		const physicalMin = -Math.floor(bus.demand_mw);
		const physicalMax = Math.max(Math.ceil(bus.demand_mw), 50);
		if (mode === 'full')
			return {
				min: physicalMin,
				max: physicalMax,
				span: physicalMax - physicalMin
			};
		const span = Math.max(5, Math.min(25, 0.1 * Math.max(bus.demand_mw, 50)));
		return {
			min: Math.max(physicalMin, center - span),
			max: Math.min(physicalMax, center + span),
			span
		};
	}

	// ===== view-model =====

	activeSolvable = $derived.by(
		(): SolvableCase | null =>
			this.app.active ?? (this.app.activeLocal?.network ? this.app.activeLocal : null)
	);
	activeFormulation = $derived(this.activeSolvable?.formulation ?? 'dcopf');
	/** The balanced case that can open as a single-line diagram: none in a saved study or for a
	 * multiconductor case. */
	singleLineCase = $derived.by(() =>
		!this.app.studyView && !this.app.activeMulti && this.activeSolvable?.network
			? this.activeSolvable
			: null
	);
	/** That case while its single-line diagram replaces the map. */
	singleLineShown = $derived(this.singleLineCase?.schematic ? this.singleLineCase : null);

	networkStats = $derived.by(() => {
		const c = this.activeSolvable;
		if (!c?.network) return null;
		return {
			buses: c.network.buses.length,
			branches: c.network.branches.length,
			objective: c.solution?.objective ?? null,
			deltaObjective:
				c.solution?.objective != null && c.baseSolution?.objective != null
					? c.solution.objective - c.baseSolution.objective
					: null,
			binding: c.solution ? (this.bindingRule?.count ?? null) : null
		};
	});

	/** Which lines the active result treats as at their limit: the solver's own binding flags
	 * when it reports them (`flagged`), else lines loaded to 99.9% of rating (legacy and non-DC
	 * results). */
	bindingRule = $derived.by(() => {
		const solution = this.activeSolvable?.solution;
		if (!solution) return null;
		const constraints = solution.branch_constraints;
		const flagged = constraints
			? new Set(constraints.filter((row) => row.binding).map((row) => row.branch))
			: null;
		const binds = (flow: { branch: number; loading: number }) =>
			flagged ? flagged.has(flow.branch) : atLimit(flow);
		return {
			label: limitWords(this.activeFormulation).lines,
			flagged: !!constraints,
			binds,
			count: constraints
				? constraints.filter((row) => row.binding).length
				: solution.flows.filter(binds).length
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
		// formulation (e.g. leaving SOCWR before +page resets it to price).
		return {
			...activeMeta,
			values: displaySeriesFor(this.activeSolvable, activeMeta.mode)
		};
	});
	displayStats = $derived.by(() => {
		if (!this.activeDisplay) return null;
		return displayRange(
			this.activeDisplay.mode,
			this.activeDisplay.values.map((entry) => entry.value)
		);
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

	// The sensitivity target of the current selection: the selected bus (price/demand),
	// else the selected branch (price/rating), else null. Selections are mutually
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
	sliderDisabled = $derived(
		this.activeFormulation === 'acopf' ||
			!this.selectedBusData ||
			isDisplayOnlyElement(this.selectedBusData)
	);

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
		if (this.activeFormulation === 'acopf' || !b || isDisplayOnlyElement(b) || b.rate_mw <= 0)
			return { min: 0, max: 0, disabled: true };
		const span = Math.min(50, Math.max(5, 0.2 * b.rate_mw));
		return {
			min: Math.max(-(b.rate_mw - 1), -span),
			max: span,
			disabled: false
		};
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
		return c.solution.prices.find((e) => e.bus === this.app.selectedBus)?.value ?? null;
	});

	// Self-sensitivity of the selected bus value to demand: the curvature term the
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
	// exact committed part plus price·step + S_bb·step²/2 along the gradient. The
	// gradient fallback is demand-only — a rating step has no corresponding bus term —
	// so a branch selection without an engine slope shows no prediction.
	predictedDeltaObj = $derived.by(() => {
		const c = this.activeSolvable;
		const target = this.selectionTarget;
		if (c?.solution?.objective == null || c.baseSolution?.objective == null || target === null)
			return null;
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
		if (
			c?.solution?.objective == null ||
			c.baseSolution?.objective == null ||
			c.predictedObjective == null ||
			c.solving
		)
			return null;
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
				Math.abs(this.sliderValue - this.committedDelta) >= DEAD_ZONE_MW) ||
			(this.app.previewRatingMw !== null &&
				Math.abs(this.ratingSliderValue - this.committedRating) >= DEAD_ZONE_MW)
		)
	);

	// ===== actions =====

	load = (): Promise<void> => {
		// Default cases off, or no backend at all: nothing to fetch. The shell
		// still needs its loaded state and a framed map, and a host that opens
		// its own cases must never see a request go out.
		const api = this.api;
		if (!this.loadDefaultCases || api === null) {
			this.casesLoaded = true;
			void this.app.requestFrame('all');
			return Promise.resolve();
		}
		// A remount can call load() again before the first getCases() resolves; dedupe
		// so two concurrent loads can't double-fetch the case list and double-fit the map.
		if (this.loading) return this.loading;
		this.loading = (async () => {
			this.hostedLoadAbort?.abort();
			this.probeAcOpf();
			// Learn the deploy's compute gate up front so fallback paths pick honest
			// copy and skip doomed requests (the SSE stream cannot see a 403). On
			// failure assume enabled; the 403 latch in fetchServerColumn still
			// catches a disabled deploy.
			void api
				.getComputeStatus()
				.then((s) => {
					this.serverComputeOff = !s.enabled;
				})
				.catch(() => {});
			try {
				let summaries: Awaited<ReturnType<typeof api.getCases>>;
				this.casesFailed = false;
				this.casesError = null;
				try {
					summaries = await api.getCases();
				} catch (e) {
					this.casesFailed = true;
					// The status and the server's own words, which the short message leaves out.
					this.casesError =
						e instanceof ApiError && e.status
							? `${errorText(e)} (HTTP ${e.status})${e.serverMessage ? `: ${e.serverMessage}` : ''}`
							: errorText(e);
					throw new Error(`Case list: ${errorText(e)}`, { cause: e });
				}
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
				for (const c of this.app.multiCases.filter((c) => c.hosted)) {
					c.solveAbort?.abort();
					if (c.mcEditTimer !== null) clearTimeout(c.mcEditTimer);
					c.mcSession?.free();
					c.mcSession = null;
					releaseMcInput(c.mcInput);
					c.mcInput = null;
				}
				this.app.multiCases = this.app.multiCases.filter((c) => !c.hosted);
				this.leaveMulti();
				const visible = summaries.filter((s) => !hidden.has(s.id));
				this.app.hostedDistributionCases = visible.filter((s) => s.model === 'multiconductor');
				this.app.cases = visible
					.filter((s) => s.model !== 'multiconductor')
					.map((s) => new CaseState(s));
				this.app.activeLocalId = null;
				this.app.placingLocalId = null;
				this.app.activeCaseId =
					this.app.cases.find((c) => c.id === DEFAULT_CASE_ID && !c.unavailableReason)?.id ??
					this.app.cases.find((c) => !c.unavailableReason)?.id ??
					null;
				const active = this.app.active;
				if (active) await this.loadBackendCase(active, true);
				else {
					const distribution = this.app.hostedDistributionCases.find((s) => !s.unavailable_reason);
					if (distribution) await this.activateHostedDistribution(distribution.id);
					else this.app.requestFrame('all');
				}
				// Mark loaded only on success, so a failed first load is retried on the next
				// mount (the page guards the call with `if (!ctrl.casesLoaded)`).
				this.casesLoaded = true;
			} catch (e) {
				this.failed(
					this.casesFailed ? 'Could not load the case list' : 'Could not load the cases',
					'Open',
					e,
					{ retry: this.load, quiet: this.casesFailed }
				);
			} finally {
				this.loading = null;
			}
		})();
		return this.loading;
	};

	loadBackendCase = async (c: CaseState, frame = false) => {
		if (c.unavailableReason) {
			this.fail(`${c.name}: ${c.unavailableReason}`);
			return;
		}
		if (c.network && c.solution && c.baseSolution) {
			if (frame && this.app.activeCaseId === c.id) this.app.requestFrame(c.id);
			return;
		}
		const api = this.api;
		if (!api) {
			this.fail(noApiNotice(c.name));
			return;
		}
		this.loadingBackendCase = c.id;
		const requestedFormulation = c.formulation;
		try {
			const [network, dcBaseSolution] = await Promise.all([
				c.network ? Promise.resolve(c.network) : api.getNetwork(c.id),
				// The server caches only the DC OPF base solution. Other formulations
				// must hydrate from the browser Study so their objective, nodal values, flows, and
				// voltage fields all come from the selected formulation.
				requestedFormulation === 'dcopf' &&
				(!c.resultDcFormula || c.resultDcFormula === 'series_susceptance')
					? api.getSolution(c.id)
					: Promise.resolve(null)
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
				this.failed(`Could not load ${c.name}`, 'Open', e, {
					message: `${c.name}: ${errorText(e)}`,
					retry: () => this.loadBackendCase(c, true)
				});
			}
		} finally {
			if (this.loadingBackendCase === c.id) this.loadingBackendCase = null;
		}
	};

	/** Leave the multiconductor view: a backend or local case becoming active is
	 * mutually exclusive with a multi case, so drop the multi's active/pending ids.
	 * The multi's per-bus expansion stays on the case for when it is reactivated. */
	private leaveMulti() {
		this.app.activeMultiId = null;
		this.app.placingMultiId = null;
	}

	activateCase = async (id: string) => {
		this.hostedLoadAbort?.abort();
		const candidate = this.app.byId(id);
		if (!candidate) return;
		if (candidate.unavailableReason) {
			this.fail(`${candidate.name}: ${candidate.unavailableReason}`);
			return;
		}
		const inspectedSavedState = this.app.studyView !== null;
		this.app.studyView = null;
		this.app.activeLocalId = null;
		this.app.placingLocalId = null;
		this.leaveMulti();
		if (inspectedSavedState || this.app.activeCaseId !== id) {
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
		} else if (!this.app.activeCaseId && !this.app.activeLocalId && !this.app.activeMultiId) {
			const next = this.app.hostedDistributionCases.find((s) => !s.unavailable_reason);
			if (next) await this.activateHostedDistribution(next.id);
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
		this.hostedLoadAbort?.abort();
		this.app.studyView = null;
		this.clearSelection();
		// Mirror activateCase's reset: a local and a backend case are mutually
		// exclusive, so drop the backend selection. Otherwise app.active (derived
		// from activeCaseId) stays set and its solve card keeps hovering over the
		// local view.
		this.app.activeCaseId = null;
		this.leaveMulti();
		this.app.activeLocalId = c.id;
		if (
			(c.formulation === 'acpf' || (c.formulation === 'acopf' && !c.solution?.prices.length)) &&
			this.app.displayMode === 'price'
		)
			this.app.displayMode = 'voltage';
		this.app.placingLocalId = c.coordsKind === 'synthetic_pending' && !c.diagram ? c.id : null;
		if (c.view || c.diagram || c.substations) this.app.requestFrame(c.id);
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
		this.dynamics.forget(c.id);
		await this.hydrateFallback(this.app.removeLocal(c.id));
	};

	/** Add a prepared local case, make it active, frame it (unless `frame` is
	 * false) and start its browser solve. */
	addAndActivateLocal = (c: LocalCase, options: { frame?: boolean; solve?: boolean } = {}) => {
		c.autoSolve = options.solve ?? true;
		this.app.studyView = null;
		this.clearSelection();
		this.app.activeCaseId = null;
		this.leaveMulti();
		this.app.addLocal(c);
		if (
			(c.formulation === 'acpf' || (c.formulation === 'acopf' && !c.solution?.prices.length)) &&
			this.app.displayMode === 'price'
		)
			this.app.displayMode = 'voltage';
		if (c.diagram) this.app.placingLocalId = null;
		if ((options.frame ?? true) && (c.view || c.diagram || c.substations))
			this.app.requestFrame(c.id);
		// Prepare the visible network even for import-only cases so settings and
		// explicit calculations are available immediately. autoSolve gates work.
		this.maybeStartLocalSolve(c.id);
	};

	placeLocalCase = async (lon: number, lat: number) => {
		const id = this.app.placingLocalId;
		const c = id ? this.app.localCases.find((lc) => lc.id === id) : null;
		if (!c?.topology) return;
		c.view = placeSyntheticTopology(c.topology, { lon, lat });
		c.coordsKind = 'synthetic';
		c.syntheticCenter = { lon, lat };
		this.app.placingLocalId = null;
		this.app.activeLocalId = c.id;
		this.app.requestFrame(c.id);
		// Persist the layout before the solve kicks off, so the Study builds from
		// the stamped payload and a save carries the placement.
		await this.stampLayout(c);
		this.maybeStartLocalSolve(c.id);
	};

	/** Write the case's on-screen layout into its network payload
	 * (`Bus.location` with `synthetic` provenance — the engine also accepts
	 * `manual`, which round trips through saved PowerIO modules), so saves,
	 * exports, and the `.geo.json` download carry it. Best effort: on an
	 * engine failure the view still renders, only the layout's persistence is
	 * lost. */
	private stampLayout = async (c: LocalCase) => {
		if (!c.studyInputJson || !c.view) return;
		try {
			const coords = Object.fromEntries(
				c.view.buses.map((b) => [b.id, [b.lon, b.lat] as [number, number]])
			);
			const stamped = await applyLayout(c.studyInputJson, coords, 'synthetic');
			c.studyInputJson = stamped.module_json;
			await this.syncStudyGeo(c, [stamped.layer]);
		} catch {
			// The on-screen view is unaffected; only persistence was lost.
		}
	};

	/** Download the case's coordinates as a `.geo.json` layer: canonical
	 * GeoJSON with provenance stamped (`synthetic`/`manual` for layouts), the
	 * document powerio and tellegen both read back. */
	downloadGeoLayer = async (c: LocalCase): Promise<void> => {
		if (!c.studyInputJson) return;
		try {
			downloadText(
				c.displayMode === 'diagram' && c.diagram
					? c.diagram.layer
					: await extractGeo(c.studyInputJson),
				`${this.caseFileStem(c)}.geo.json`,
				'application/geo+json'
			);
			this.app.error = null;
		} catch (e) {
			this.failed('Could not export the layout', 'Export', e, {
				message: `${this.caseName(c)}: could not export the layout: ${errorText(e)}`
			});
		}
	};

	moveLocalCase = (c: LocalCase) => {
		this.app.activeCaseId = null;
		this.app.activeLocalId = c.id;
		this.app.placingLocalId = c.id;
	};

	/** Apply parsed geo layers onto a local case, in drop order (a later file
	 * wins on shared elements). The engine owns matching and the network becomes
	 * the single owner of the coordinates: matched points land in
	 * `Bus.location`, routes in `Branch.route`, and the refreshed
	 * `module_json` replaces the case IR — so a saved module or export
	 * carries exactly what is on screen. A live Study is synced the same way.
	 * Throws when a layer matches nothing. */
	applyGeoLayers = async (c: LocalCase, layers: GeoLayerFile[]): Promise<void> => {
		if (!c.studyInputJson || layers.length === 0) return;
		let payload: AppliedGeoCase | null = null;
		const geographic = [];
		for (const entry of layers) {
			const document = JSON.parse(entry.layer);
			const space = document.powerio_geo?.space;
			if (space === 'diagram' || space?.kind === 'diagram') {
				const payload = await applyGeo(c.studyInputJson, entry.layer);
				if (payload.view) {
					c.diagram = {
						view: payload.view,
						layer: entry.layer,
						name: entry.name,
						warnings: payload.report.notes
					};
					if (!c.view) c.displayMode = 'diagram';
				}
			} else geographic.push(entry);
		}
		layers = geographic;
		if (!layers.length) return;
		let moduleJson = c.studyInputJson;
		const warnings: string[] = [];
		let routes = 0;
		let unmatched = 0;
		for (const l of layers) {
			payload = await applyGeo(moduleJson, l.layer);
			moduleJson = payload.module_json;
			routes += payload.report.matched_branches;
			unmatched += payload.report.unmatched_features;
			warnings.push(
				...l.diagnostics.map((d) => `${l.name}: ${formatPowerIoDiagnostic(d)}`),
				...payload.report.notes.map((n) => `${l.name}: ${n}`)
			);
			this.logCoordinates(l.name, l.diagnostics, payload.report.notes);
		}
		if (!payload) return;
		const sourceLabel = layers.map((l) => l.name).join(' + ');
		const allPlaced = this.adoptGeoPayload(c, payload, sourceLabel, sourceLabel, [
			...(routes > 0 ? [`${routes} branch route(s) matched from geographic file data`] : []),
			...(unmatched > 0 ? [`${unmatched} feature(s) matched no case element`] : []),
			...warnings
		]);
		this.logPlacement(
			c,
			c.geoWarnings ?? [],
			allPlaced && !unmatched && !warnings.length && !payload.warnings.length
		);
		await this.syncStudyGeo(
			c,
			layers.map((l) => l.layer)
		);
	};

	/** Adopt an engine geo apply onto the case: the refreshed payload becomes
	 * the case's network and view, provenance flips to the geographic source,
	 * and the warnings panel gets the placement summary. Returns whether every bus was placed. */
	private adoptGeoPayload(
		c: LocalCase,
		payload: AppliedGeoCase,
		sourceLabel: string,
		placedFrom: string,
		extraWarnings: string[]
	) {
		c.studyInputJson = payload.module_json;
		c.view = payload.view;
		c.coordsKind = 'geofile';
		c.displayMode = payload.view?.coordinate_space === 'diagram' ? 'diagram' : 'geographic';
		c.syntheticCenter = undefined;
		c.geoSource = sourceLabel;
		const placedCanonicalBuses =
			payload.view?.buses.filter((bus) => bus.editable !== false).length ?? 0;
		c.geoWarnings = [
			`${placedCanonicalBuses} of ${payload.n_bus} buses placed from ${placedFrom}`,
			...extraWarnings,
			...payload.warnings
		];
		return placedCanonicalBuses === payload.n_bus;
	}

	/** Update an existing numerical Study after the base input receives geographic data. */
	private syncStudyGeo = async (c: LocalCase, layers: string[]) => {
		const cached = this.caseStudies.get(c);
		if (!cached) return;
		if (cached.formulation !== c.formulation) {
			this.disposeStudy(c);
			return;
		}
		try {
			for (const layer of layers) await cached.study.applyGeoLayer(layer);
			cached.studyInputJson = c.studyInputJson!;
		} catch (error) {
			this.disposeStudy(c);
			throw error;
		}
	};

	private caseEditKey(c: SolvableCase): string {
		return JSON.stringify([
			c.formulation,
			c.resultDcFormula,
			this.caseDeltas(c),
			this.caseRatings(c)
		]);
	}

	private async localSidecarCopy(source: CaseState): Promise<LocalCase> {
		if (source.solving) throw new Error('Wait for the current solve before attaching coordinates.');
		const revision = source.revisionGeneration;
		const captured = await this.captureSavedCase(source);
		const base = await this.ensureStudyInputJson(source);
		if (!base) throw new Error('The selected case input is unavailable.');
		const parsed = await ingestJsonDrop(new TextEncoder().encode(base));
		if (!parsed.payload || !('topology' in parsed.payload))
			throw new Error('The selected case has no balanced network data.');
		const copy = this.localFromBalancedPayload(`${source.name}.pio.json`, {
			...parsed.payload,
			name: `${source.name} (local copy)`
		});
		copy.formulation = source.formulation;
		copy.settings.calculation = source.settings.calculation;
		copy.settings.accepted = source.settings.accepted;
		copy.settings.baselines = $state.snapshot(source.settings.baselines);
		copy.settings.options = $state.snapshot(source.settings.options);
		copy.settings.resolved = $state.snapshot(source.settings.resolved);
		copy.deltas = { ...source.deltas };
		copy.ratings = { ...source.ratings };
		copy.solution = source.solution;
		copy.baseSolution = source.baseSolution;
		copy.iterations = source.iterations;
		copy.solveMs = source.solveMs;
		copy.solveBackend = source.solveBackend;
		copy.solveDetail = source.solveDetail;
		copy.network = source.network;
		if (source.network) {
			copy.view = {
				buses: source.network.buses,
				branches: source.network.branches,
				coordinate_space: source.network.coordinate_space
			};
			copy.coordsKind = source.network.synthetic_coords ? 'synthetic' : 'file';
			if (!source.network.synthetic_coords && source.network.coordinate_space !== 'diagram') {
				const layer = JSON.stringify({
					type: 'FeatureCollection',
					powerio_geo: { space: 'geographic' },
					features: [
						...source.network.buses.map((bus) => ({
							type: 'Feature',
							properties: { target: 'bus', id: String(bus.id) },
							geometry: { type: 'Point', coordinates: [bus.lon, bus.lat] }
						})),
						...source.network.branches
							.filter((branch) => branch.path.length > 1)
							.map((branch) => ({
								type: 'Feature',
								properties: {
									target: 'branch',
									branch_id: String(branch.id),
									from: String(branch.from),
									to: String(branch.to)
								},
								geometry: { type: 'LineString', coordinates: branch.path }
							}))
					]
				});
				const placed = await applyGeo(copy.studyInputJson!, layer);
				copy.studyInputJson = placed.module_json;
				copy.view = placed.view;
			}
		}
		if (source.revisionGeneration !== revision || source.solving)
			throw new Error(NOTICES.caseChangedWhileLoading);
		this.detachedCaseCaptures.set(copy, {
			baseInput: copy.studyInputJson!,
			edits: this.caseEditKey(copy),
			solution: copy.solution,
			captured
		});
		return copy;
	}

	private stageLocalSidecars(source: LocalCase): LocalCase {
		const copy = new LocalCase({
			id: source.id,
			label: source.label,
			fileName: source.fileName,
			summary: source.summary,
			studyInputJson: source.studyInputJson,
			topology: source.topology,
			coordsKind: source.coordsKind,
			view: source.view,
			substations: source.substations
		});
		for (const key of [
			'diagram',
			'displayMode',
			'geoSource',
			'geoWarnings',
			'syntheticCenter',
			'formulation',
			'deltas',
			'ratings',
			'solution',
			'baseSolution',
			'iterations',
			'solveMs',
			'solveBackend',
			'solveDetail'
		] as const)
			Object.assign(copy, { [key]: source[key] });
		const captured = this.detachedCaseCaptures.get(source);
		if (captured) this.detachedCaseCaptures.set(copy, { ...captured });
		return copy;
	}

	private async attachSidecarsToSelected(
		layers: GeoLayerFile[],
		displays: DisplayFile[] = [],
		expected?: SolvableCase | MulticonductorCase | null
	) {
		if (this.app.studyView) {
			this.app.error = NOTICES.returnToLiveCase;
			return;
		}
		const source = this.app.activeMulti ?? this.app.activeLocal ?? this.app.active;
		if (expected !== undefined && source !== expected) {
			this.app.error = NOTICES.caseChanged;
			return;
		}
		if (!source) {
			this.app.error = NOTICES.selectMatchingCase;
			return;
		}
		if (source instanceof MulticonductorCase) {
			try {
				await this.applyMultiGeoLayers(source, layers);
			} catch (error) {
				this.failed('Could not attach coordinates', 'Coordinates', error);
			}
			if (displays.length) this.app.error = 'Multiconductor cases need GeoJSON or CSV coordinates';
			return;
		}
		const selectedId = source.id;
		const revision = source.revisionGeneration;
		let target: LocalCase | null = null;
		try {
			target =
				source instanceof CaseState
					? await this.localSidecarCopy(source)
					: this.stageLocalSidecars(source);
			if ((this.app.activeLocal ?? this.app.active)?.id !== selectedId || this.app.studyView)
				throw new Error(NOTICES.caseChanged);
			const placement = await this.prepareDroppedLocal(target, layers, displays);
			const applied = placement.geoLayersConsumed || displays.some((display) => display.consumed);
			if (!applied) throw new Error(placement.error ?? 'No coordinates matched the selected case');
			if (
				(this.app.activeLocal ?? this.app.active)?.id !== selectedId ||
				this.app.studyView ||
				source.revisionGeneration !== revision
			)
				throw new Error(NOTICES.caseChanged);
			const saved = this.detachedCaseCaptures.get(target);
			if (saved) saved.baseInput = target.studyInputJson!;
			if (
				(this.app.activeLocal ?? this.app.active)?.id !== selectedId ||
				this.app.studyView ||
				source.revisionGeneration !== revision ||
				source.solving
			)
				throw new Error(NOTICES.caseChanged);
			if (source instanceof LocalCase) {
				const cached = this.caseStudies.get(source);
				if (cached && target.studyInputJson && target.view) {
					const layer = await extractGeo(target.studyInputJson);
					try {
						await cached.study.applyGeoLayer(layer);
					} catch (error) {
						this.disposeStudy(source);
						throw error;
					}
					if (
						(this.app.activeLocal ?? this.app.active)?.id !== selectedId ||
						this.app.studyView ||
						source.revisionGeneration !== revision ||
						source.solving
					) {
						this.disposeStudy(source);
						throw new Error(NOTICES.caseChanged);
					}
					cached.studyInputJson = target.studyInputJson;
				}
				for (const key of [
					'studyInputJson',
					'view',
					'diagram',
					'displayMode',
					'coordsKind',
					'geoSource',
					'geoWarnings',
					'syntheticCenter'
				] as const)
					Object.assign(source, { [key]: target[key] });
				if (saved) this.detachedCaseCaptures.set(source, saved);
				target = source;
			}
			this.app.studyView = null;
			this.clearSelection();
			this.app.activeCaseId = null;
			this.leaveMulti();
			if (target !== source) this.app.addLocal(target);
			this.app.activeLocalId = target.id;
			this.app.placingLocalId = null;
			target.network = this.localNetwork(target);
			this.bumpRevision(target);
			this.app.requestFrame(target.id);
			this.maybeStartLocalSolve(target.id);
			this.app.error = placement.error;
		} catch (error) {
			if (target && target !== source) this.disposeStudy(target);
			this.failed('Could not attach coordinates', 'Coordinates', error);
		}
	}

	applyGeoLayersToExisting = async (layers: GeoLayerFile[]) => {
		await this.attachSidecarsToSelected(layers);
	};

	/** Ask the map to frame the selected branch and wait for the camera to land,
	 * capped by FOCUS_SETTLE_CAP_MS and cut short by the selection's abort. */
	private awaitFocus(caseId: string, branchId: number, ac: AbortController): Promise<void> {
		return Promise.race([
			this.app.requestFrame({ caseId, branchId }),
			delayUntilFocusSettles(FOCUS_SETTLE_CAP_MS, ac.signal)
		]);
	}

	/** One selection flow for bus and branch targets on backend cases: optional
	 * camera focus first (so the fly-to is not janked by the solve), the browser
	 * Study column, then the one server reconciliation for DC OPF without rating
	 * edits. */
	private selectTarget = async (caseId: string, target: SensTarget, focus: boolean) => {
		this.app.activeLocalId = null;
		this.app.placingLocalId = null;
		this.leaveMulti();
		if (this.app.activeCaseId !== caseId) {
			this.clearSelection();
			this.app.activeCaseId = caseId;
		}
		const c = this.app.byId(caseId);
		if (!c) return;
		const { ac, sensitivitySeq } =
			'bus' in target
				? this.beginBusSelection(c, target.bus)
				: this.beginBranchSelection(c, target.branch);
		const retry = () =>
			'bus' in target
				? this.selectBus(caseId, target.bus)
				: this.selectBranch(caseId, target.branch);
		// True once the browser path fully settled the selection (column accepted or
		// a terminal message shown); false leaves DC OPF one server reconciliation.
		let handled = false;
		try {
			if (focus && 'branch' in target) {
				await this.awaitFocus(caseId, target.branch, ac);
				if (ac.signal.aborted) return;
			}
			if (c.formulation === 'acpf') return;
			try {
				// The column from the browser Study (under the case's formulation). DC OPF
				// may reconcile a null column via the server; AC OPF / SOCWR are browser
				// only, so a null column there is terminal.
				const studyInputJson = await this.ensureStudyInputJson(c);
				if (studyInputJson) {
					const sensitivity = await this.browserSensitivity(c, studyInputJson, target, ac.signal);
					if (!ac.signal.aborted && sensitivity) {
						this.acceptSensitivity(c, sensitivity, target, sensitivitySeq);
						handled = true;
					} else if (!ac.signal.aborted && c.formulation !== 'dcopf') {
						this.failColumnUnavailable(c, target, retry);
						handled = true;
					}
				} else if (c.formulation !== 'dcopf') {
					if (!ac.signal.aborted) this.failColumnUnavailable(c, target, retry);
					handled = true;
				}
			} catch (e) {
				// The browser path threw; the DC OPF server reconciliation below still
				// applies. AC OPF / SOCWR have no server fallback (nothing is solved on
				// the server), so surface the error there instead of leaving the click
				// looking dead.
				if (c.formulation !== 'dcopf') {
					if (!ac.signal.aborted && sensitivitySeq === (c.sensitivitySeq ?? 0)) {
						this.failed('No price sensitivity', 'Solve', e, {
							message: `${this.caseName(c)}: ${errorText(e)}`,
							retry
						});
					}
					handled = true;
				}
			}
			if (
				!handled &&
				!ac.signal.aborted &&
				c.formulation === 'dcopf' &&
				(!c.resultDcFormula || c.resultDcFormula === 'series_susceptance')
			) {
				if (this.hasRatingEdits(c)) {
					// The server solves at base ratings, so its column would come from the
					// wrong operating point once rating edits are committed. Fail instead
					// of silently reconciling there.
					this.fail(this.ratingEditsFallbackError(c), retry);
				} else {
					await this.fetchServerColumn(c, target, ac, sensitivitySeq);
				}
			}
		} finally {
			if (this.abort === ac) this.app.sensitivityLoading = false;
		}
	};

	selectBus = (caseId: string, busId: number) => this.selectTarget(caseId, { bus: busId }, false);

	/** The compute-off terminal copy. Name the browser engine as the cause only
	 * when its failure is known permanent; otherwise the miss may have been
	 * transient (a wasm chunk or case JSON fetch), so say what is known and keep
	 * the retry meaningful. */
	private computeOffNotice(c: SolvableCase): string {
		return this.studyUnavailable.has(c) ? WASM_REQUIRED_NOTICE : COMPUTE_OFF_NOTICE;
	}

	/** The one server nodal-value/parameter request a selection may make: the
	 * reconciliation path when the browser Study produced no column for a DC OPF
	 * case. Never retries on its own: a second request inside the same rate limit
	 * window is a guaranteed rejection, and the retry button covers the user
	 * need. A 429 opens a cooldown for the rest of the window; the server's own
	 * compute-disabled 403 latches serverComputeOff for the session (matched on
	 * `serverMessage` so a proxy or WAF 403 cannot latch a false diagnosis). */
	private fetchServerColumn = async (
		c: CaseState,
		target: SensTarget,
		ac: AbortController,
		sensitivitySeq: number
	) => {
		if (c.resultDcFormula && c.resultDcFormula !== 'series_susceptance') {
			this.fail('Needs browser engine');
			return;
		}
		const retryOp =
			'bus' in target
				? () => this.selectBus(c.id, target.bus)
				: () => this.selectBranch(c.id, target.branch);
		if (hasCustomCalculationSettings(c)) {
			this.fail("Server can't apply settings", retryOp);
			return;
		}
		const api = this.api;
		if (this.serverComputeOff || !api) {
			this.fail(this.computeOffNotice(c), retryOp);
			return;
		}
		if (Date.now() < this.sensitivity429Until) {
			this.fail(NOTICES.rateLimited, retryOp);
			return;
		}
		try {
			const col =
				'bus' in target
					? await api.getSensitivity(c.id, target.bus, c.deltas, ac.signal)
					: await api.getBranchSensitivity(c.id, target.branch, c.deltas, ac.signal);
			if (!ac.signal.aborted) {
				// The solution on screen came from the browser or the served base
				// solve; the column came from the server. Label the mix.
				c.solveBackend = 'clarabel-wasm-server-sensitivity';
				this.acceptSensitivity(c, col, target, sensitivitySeq);
			}
		} catch (e) {
			if (e instanceof DOMException) return;
			if (
				e instanceof ApiError &&
				e.status === 403 &&
				e.serverMessage === 'server compute is disabled'
			) {
				this.serverComputeOff = true;
				if (!ac.signal.aborted && sensitivitySeq === (c.sensitivitySeq ?? 0)) {
					this.fail(this.computeOffNotice(c), retryOp);
				}
				return;
			}
			if (e instanceof ApiError && e.status === 429) {
				this.sensitivity429Until = Date.now() + 10_000;
			}
			// A late failure must not clobber a newer selection's state.
			if (!ac.signal.aborted && sensitivitySeq === (c.sensitivitySeq ?? 0)) {
				this.failed('No price sensitivity', 'Solve', e, { retry: retryOp });
			}
		}
	};

	/** Show an error with an optional one-click re-run of the failed operation. */
	private fail(message: string, retry: (() => void) | null = null, known: ErrorContext = {}) {
		this.failed(known.title, known.stage, message, { ...known, retry });
	}

	/** Report a caught error: `title` names what failed and `stage` the step. A
	 * `message` replaces the error's own text, and `quiet` keeps it out of the toast. */
	private failed(
		title: string | undefined,
		stage: string | undefined,
		cause: unknown,
		known: Omit<ErrorContext, 'retry'> & {
			quiet?: boolean;
			retry?: (() => void) | null;
		} = {}
	) {
		const { retry } = known;
		this.app.reportError(cause, {
			...known,
			stage,
			// The server's status says more than any step's own title.
			title: cause instanceof ApiError && cause.status === 429 ? RATE_LIMITED : title,
			// An explicit retry always ends a 429 cooldown.
			retry: retry
				? () => {
						this.sensitivity429Until = 0;
						return retry();
					}
				: undefined
		});
	}

	/** A file that could not be read. */
	private openFailed(name: string, cause: unknown) {
		this.failed(`Could not open ${name}`, 'Open', cause, {
			message: `${name}: ${errorText(cause)}`
		});
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
		if (!c?.studyInputJson || !c.network) return;
		this.app.activeCaseId = null;
		this.leaveMulti();
		this.app.activeLocalId = localId;
		this.app.placingLocalId = null;
		const { ac, sensitivitySeq } = this.beginBusSelection(c, busId);
		try {
			// Dynamics mode selects a bus to pick events, not to price it.
			if (c.formulation === 'acpf' || this.app.mode === 'dynamics') return;
			const sensitivity = await this.browserSensitivity(
				c,
				c.studyInputJson,
				{
					bus: busId
				},
				ac.signal
			);
			if (!ac.signal.aborted)
				this.acceptSensitivity(c, sensitivity, { bus: busId }, sensitivitySeq);
			if (!ac.signal.aborted && !sensitivity) {
				// A null column means the solve ran but produced no price/demand column for this bus (or the
				// Study could not be built); local cases have no server fallback, so say so
				// instead of leaving the panel in nodal value view with no explanation.
				this.app.error = `${c.label}: ${formulationLabel(c.formulation)} sensitivity unavailable in the browser${
					c.solveFallbackReason
						? `: ${c.solveFallbackReason}`
						: ' (no price sensitivity column for this bus)'
				}`;
			}
		} catch (e) {
			if (!ac.signal.aborted) {
				this.failed('No price sensitivity', 'Solve', e, {
					message: `${c.label}: ${errorText(e)}`,
					retry: () => this.selectLocalBus(localId, busId)
				});
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

	/** The terminal message when the browser produced no column and the server
	 * cannot reconcile (non-DC formulation, or a local case with no server). */
	private failColumnUnavailable(c: SolvableCase, target: SensTarget, retry: () => void) {
		const what =
			'bus' in target
				? `${formulationLabel(c.formulation)} sensitivity`
				: 'price/rating sensitivity';
		this.fail(
			`${this.caseName(c)}: ${what} unavailable${
				c.solveFallbackReason ? `: ${c.solveFallbackReason}` : ''
			}`,
			retry
		);
	}

	/** `focus` frames the selected line on the map first; the sensitivity solve
	 * waits for the camera to land so the fly-to stays smooth. */
	selectBranch = (caseId: string, branchId: number, opts: { focus?: boolean } = {}) =>
		this.selectTarget(caseId, { branch: branchId }, opts.focus ?? false);

	selectLocalBranch = async (localId: string, branchId: number, opts: { focus?: boolean } = {}) => {
		const c = this.app.localCases.find((lc) => lc.id === localId);
		if (!c?.studyInputJson || !c.network) return;
		this.app.activeCaseId = null;
		this.leaveMulti();
		this.app.activeLocalId = localId;
		this.app.placingLocalId = null;
		const { ac, sensitivitySeq } = this.beginBranchSelection(c, branchId);
		const retry = () => this.selectLocalBranch(localId, branchId);
		if (this.app.mode === 'dynamics') {
			if (this.abort === ac) this.app.sensitivityLoading = false;
			return;
		}
		try {
			if (opts.focus) {
				await this.awaitFocus(localId, branchId, ac);
				if (ac.signal.aborted) return;
			}
			if (c.formulation === 'acpf') return;
			const sensitivity = await this.browserSensitivity(
				c,
				c.studyInputJson,
				{
					branch: branchId
				},
				ac.signal
			);
			if (ac.signal.aborted) return;
			if (sensitivity) {
				this.acceptSensitivity(c, sensitivity, { branch: branchId }, sensitivitySeq);
			} else if (sensitivitySeq === (c.sensitivitySeq ?? 0)) {
				this.failColumnUnavailable(c, { branch: branchId }, retry);
			}
		} catch (e) {
			if (!ac.signal.aborted && sensitivitySeq === (c.sensitivitySeq ?? 0)) {
				this.failed('No price sensitivity', 'Solve', e, {
					message: `${c.label}: ${errorText(e)}`,
					retry
				});
			}
		} finally {
			if (this.abort === ac) this.app.sensitivityLoading = false;
		}
	};

	clearSelection = () => {
		this.abort?.abort();
		const c = this.app.active;
		if (c) {
			this.bumpRevision(c);
			c.sensitivitySeq++;
			c.sensitivity = null;
		}
		const lc = this.app.activeLocal;
		if (lc) {
			this.bumpRevision(lc);
			lc.sensitivitySeq = (lc.sensitivitySeq ?? 0) + 1;
			lc.sensitivity = null;
		}
		// Multiconductor cases carry their own bus/edge selection (string ids);
		// clear both so an Escape or empty-map click collapses the expanded detail.
		const mc = this.app.activeMulti;
		if (mc) {
			mc.selectedBusId = null;
			mc.selectedEdgeId = null;
		}
		this.app.selectedBus = null;
		this.app.selectedBranch = null;
		this.app.previewDeltaMw = null;
		this.app.previewRatingMw = null;
		this.app.previewActive = false;
		this.app.previewPrices = null;
		this.previewObjective = null;
		this.ratingSlope = null;
		this.app.demandRangeMode = 'local';
		this.nearbyRangeAnchor = null;
		this.app.sensitivityLoading = false;
	};
	runSolve = this.balancedSolve.runSolve;

	serverSolve = this.balancedSolve.serverSolve;

	commitDelta = (value: number) => {
		const c = this.activeSolvable;
		const bus = this.app.selectedBus;
		if (!c || bus === null || this.sliderDisabled) return;
		// Refresh the engine preview at the commit value (a typed value may not have
		// driven a drag), then score the commit with the engine's predicted Δobjective.
		this.runPreview(c, bus, value);
		c.predictedObjective = this.predictedDeltaObj;
		c.deltas = this.previewDeltas(c, bus, value);
		this.bumpRevision(c);
		this.app.previewDeltaMw = value;
		this.app.previewActive = true;
		this.runSolve(c, { bus });
	};

	finishDemandInput = (value: number) => {
		if (this.sliderDisabled) return;
		if (Math.abs(value - this.committedDelta) < DEAD_ZONE_MW) {
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
		this.bumpRevision(c);
		c.predictedObjective = null;
		this.app.previewPrices = null;
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

	/** Undo the selected bus's demand change. When it is the case's only edit, the base
	 * result returns at once, as a whole-case reset does. */
	resetDemand = (c: SolvableCase) => {
		const bus = this.app.selectedBus;
		if (bus === null) return;
		if (this.onlyEdit(c, 'deltas', bus)) this.resetCase(c);
		else this.commitDelta(0);
	};

	/** Undo the selected line's rating change, as `resetDemand` does for a bus. */
	resetRating = (c: SolvableCase) => {
		const branch = this.app.selectedBranch;
		if (branch === null) return;
		if (this.onlyEdit(c, 'ratings', branch)) this.resetCase(c);
		else void this.commitRating(0);
	};

	/** The case's edits are this one element's and nothing else. */
	private onlyEdit(c: SolvableCase, kind: 'deltas' | 'ratings', id: number): boolean {
		const others = (edits: Record<number, number>, skip: number | null) =>
			Object.entries(edits).some(([k, mw]) => mw !== 0 && Number(k) !== skip);
		return (
			!others(c.deltas, kind === 'deltas' ? id : null) &&
			!others(c.ratings, kind === 'ratings' ? id : null)
		);
	}

	/** Change the convention through the same settings path as WebMCP. */
	changeDcFormula = async (c: SolvableCase, next: DcFormula) => {
		if (c.solution?.branch_susceptance_formula === next) return;
		if (c instanceof LocalCase && c.declaredFormulation) {
			this.fail('The case file sets its own DC line model, so Settings cannot change it.');
			return;
		}
		try {
			await this.updateSettings(
				c,
				'dcopf',
				{ branch_susceptance_formula: next },
				c.settings.revision
			);
			await this.runSettings(c);
		} catch (error) {
			this.failed('DC line model change failed', 'Solve', error);
		}
	};

	/** Choose a calculation and run it: the Network panel's Calculation select.
	 * The current result stays on screen until the new one succeeds. */
	chooseCalculation = async (c: SolvableCase, next: Formulation) => {
		try {
			if (next === 'acopf' && !(await this.loadAcOpf()))
				throw new Error('AC OPF solver failed to load');
			await this.updateSettings(c, next, {}, c.settings.revision);
			await this.runSettings(c);
		} catch (error) {
			if ((error as Error)?.name === 'AbortError') return;
			if (!c.solving) c.failedCalculation = next;
			this.failed(errorTitle(error, `${formulationLabel(next)} failed`), 'Solve', error, {
				retry: () => void this.chooseCalculation(c, next)
			});
		}
	};

	private readonly settingsActions = new CaseSettingsActions(this, {
		url: () => this.acOpfWasmUrl,
		solves: this.acOpfSolves
	});
	inspectSettings = this.settingsActions.inspectSettings;
	updateSettings = this.settingsActions.updateSettings;
	canCancelSettings = this.settingsActions.canCancelSettings;
	cancelSettings = this.settingsActions.cancelSettings;
	runSettings = this.settingsActions.runSettings;

	/** Rebuild the numerical Study for a selected calculation at the current demand. */
	changeFormulation = (c: SolvableCase, next: Formulation) => {
		if (c.formulation === next) return;
		if (c instanceof LocalCase && c.declaredFormulation && c.declaredFormulation !== next) {
			this.fail('This PowerIO instance requires its declared calculation.');
			return;
		}
		if (!this.formulationAvailable(c, next)) return;
		c.formulation = next;
		c.failedCalculation = null;
		// Demand does not depend on the calculation, so a demand colouring stays.
		if (this.app.displayMode !== 'demand')
			this.app.displayMode = next === 'acpf' || next === 'acopf' ? 'voltage' : 'price';
		this.bumpRevision(c);
		// The committed point carries over (same demand), but the model and its solution do
		// not; drop the Study and the cached solutions so they rebuild under `next`.
		this.disposeStudy(c);
		c.baseSolution = null;
		c.solution = null;
		c.iterations = [];
		c.solveMs = null;
		c.solveDetail = null;
		c.predictedObjective = null;
		this.app.previewPrices = null;
		this.previewObjective = null;
		this.ratingSlope = null;
		this.app.error = null;
		// The current sensitivity column was computed under the old formulation; clear it so
		// the overlay recomputes (the Study returns the new column with the re-solve below).
		c.sensitivity = null;
		c.sensitivitySeq = (c.sensitivitySeq ?? 0) + 1;
		this.runSolve(c, this.selectionTarget);
	};

	private readonly caseImports = new CaseImportActions(this, {
		addMultiCase: (...args) => this.addMultiCase(...args),
		attachSidecarsToSelected: (...args) => this.attachSidecarsToSelected(...args),
		fail: (...args) => this.fail(...args),
		failed: (...args) => this.failed(...args),
		localFromBalancedPayload: (...args) => this.localFromBalancedPayload(...args),
		logImport: (...args) => this.logImport(...args),
		logPlacement: (...args) => this.logPlacement(...args),
		openCollection: (...args) => this.openCollection(...args),
		openFailed: (...args) => this.openFailed(...args)
	});
	ingestFiles = this.caseImports.ingestFiles;
	ingestFolder = this.caseImports.ingestFolder;
	ingestDrop = this.caseImports.ingestDrop;
	private prepareDroppedLocal = this.caseImports.prepareDroppedLocal;
	private placeTopologyAsDiagram = this.caseImports.placeTopologyAsDiagram;
	setCaseDisplayMode = this.caseImports.setCaseDisplayMode;

	private createDistributionSolve() {
		const controller = this;
		return new DistributionSolveActions(this, {
			get disposed() {
				return controller.disposed;
			},
			get hostedLoadAbort() {
				return controller.hostedLoadAbort;
			},
			get mcTransport() {
				return controller.mcTransport;
			},
			failed: (...args) => this.failed(...args),
			logImport: (...args) => this.logImport(...args)
		});
	}
	private readonly distributionSolve = this.createDistributionSolve();
	private addMultiCase = this.distributionSolve.addMultiCase;
	useWrittenEquipmentSettings = this.distributionSolve.useWrittenEquipmentSettings;
	solveMultiCase = this.distributionSolve.solveMultiCase;
	restoreMultiSnapshot = this.distributionSolve.restoreMultiSnapshot;
	snapshotMultiCase = this.distributionSolve.snapshotMultiCase;

	private readonly distributionResults = new DistributionResultsActions();
	static readonly AUTO_OVERVIEW_MAX_BUSES = DistributionResultsActions.AUTO_OVERVIEW_MAX_BUSES;
	multiResultDetail = this.distributionResults.multiResultDetail;
	overviewOnRequest = this.distributionResults.overviewOnRequest;
	readOverview = this.distributionResults.readOverview;
	followOverview = this.distributionResults.followOverview;
	multiOverviewResult = this.distributionResults.multiOverviewResult;
	multiFullResult = this.distributionResults.multiFullResult;
	multiTerminalTable = this.distributionResults.multiTerminalTable;

	/** Apply coordinates atomically to the explicitly selected conductor-resolved case. */
	applyMultiGeoLayers = async (c: MulticonductorCase, layers: GeoLayerFile[]) => {
		if (!layers.length) return;
		if (this.app.studyView || this.app.activeMulti !== c)
			throw new Error(NOTICES.selectMatchingCase);
		if (c.solving) throw new Error('Wait for the calculation before attaching coordinates.');
		if (!c.mcInput || !this.mcTransport.applyMcGeo)
			throw new Error('Coordinate attachment is unavailable for this case.');
		const baseInput = c.mcInput;
		const retainedAtStart = c.mcSession;
		let snapshot = c.mcSnapshot;
		if (!snapshot && c.mcSession && c.result) snapshot = await this.snapshotMultiCase(c);
		// Coordinates attach to IR text, so a kept module is written out here.
		const input = snapshot?.input_module ?? (await mcInputJson(baseInput));
		const revision = c.revisionGeneration;
		let next = input;
		let payload: Awaited<ReturnType<NonNullable<EngineTransport['applyMcGeo']>>> | undefined;
		const notes: string[] = [];
		let matched = 0;
		let complete = true;
		for (const layer of layers) {
			payload = await this.mcTransport.applyMcGeo(next, layer.layer);
			if (snapshot) {
				if (!this.mcTransport.applyMcStudyGeo)
					throw new Error('Saved result coordinates are unavailable in this build.');
				snapshot = await this.mcTransport.applyMcStudyGeo(snapshot, layer.layer);
			}
			if (!payload.module_json)
				throw new Error('Coordinate attachment returned no electrical input.');
			next = payload.module_json;
			matched += payload.report.matched_buses + payload.report.matched_branches;
			complete &&=
				!payload.report.unmatched_features &&
				!payload.report.notes.length &&
				!layer.diagnostics.length;
			notes.push(
				`${payload.report.matched_buses} buses, ${payload.report.matched_branches} routes matched; ${payload.report.unmatched_features} unmatched objects`,
				...payload.report.notes,
				...layer.diagnostics.map(formatPowerIoDiagnostic)
			);
			this.logCoordinates(layer.name, layer.diagnostics, payload.report.notes);
		}
		if (!payload || !matched) throw new Error('No coordinates matched the selected case.');
		if (
			this.app.studyView ||
			this.app.activeMulti !== c ||
			c.mcInput !== baseInput ||
			c.mcSession !== retainedAtStart ||
			c.revisionGeneration !== revision ||
			c.solving
		)
			throw new Error(NOTICES.caseChanged);
		const { graph, ...summary } = payload;
		if (c.mcEditTimer !== null) {
			clearTimeout(c.mcEditTimer);
			c.mcEditTimer = null;
		}
		c.mcSession?.free();
		c.mcSession = null;
		// Without a session, result detail comes from the saved result.
		c.mcFullResult = snapshot?.result ?? c.mcFullResult;
		c.mcLoadBranches = [];
		c.mcLoadEdits = [];
		releaseMcInput(c.mcInput);
		c.mcInput = next;
		c.mcSnapshot = snapshot ? { ...snapshot, id: crypto.randomUUID() } : null;
		c.mcSavedAt = null;
		c.geoLayer = payload.geo_layer ?? null;
		c.graph = graph;
		c.summary = summary;
		c.coordsKind = payload.coords_kind;
		c.view =
			payload.coords_kind === 'geographic'
				? buildGeographicView(graph, payload.geo_layer)
				: buildDiagramView(graph, payload.geo_layer);
		c.geoWarnings = notes;
		this.logPlacement(c, notes, complete);
		c.revisionGeneration++;
		this.app.placingMultiId = null;
		this.app.error = null;
		this.app.requestFrame(c.id);
	};

	/** Stage an absolute load-branch P/Q edit and coalesce rapid UI changes into
	 * one retained-factor warm solve. */
	queueMultiLoadPower = (
		c: MulticonductorCase,
		load: string,
		branch: number,
		p_w: number,
		q_var: number
	) => {
		if (!c.mcSession) {
			this.app.error = 'Run power flow first';
			return;
		}
		if (!Number.isFinite(p_w) || !Number.isFinite(q_var)) {
			this.app.error = 'Load P and Q must be finite numbers.';
			return;
		}
		const row = c.mcLoadBranches.find((entry) => entry.load === load && entry.branch === branch);
		if (!row) {
			this.app.error = `Load ${load} branch ${branch} is unavailable.`;
			return;
		}
		c.mcLoadBranches = c.mcLoadBranches.map((entry) =>
			entry.load === load && entry.branch === branch ? { ...entry, p_w, q_var } : entry
		);
		const next: McLoadPowerEdit = { load, branch, p_w, q_var };
		const without = c.mcLoadEdits.filter((edit) => edit.load !== load || edit.branch !== branch);
		c.mcLoadEdits = p_w === row.base_p_w && q_var === row.base_q_var ? without : [...without, next];
		c.mcEditRevision++;
		c.mcSnapshot = null;
		c.mcSavedAt = null;
		c.solving = true;
		if (c.mcEditTimer !== null) clearTimeout(c.mcEditTimer);
		c.mcEditTimer = setTimeout(() => {
			c.mcEditTimer = null;
			void this.flushMultiLoadPowers(c);
		}, 180);
	};

	private flushMultiLoadPowers = async (c: MulticonductorCase): Promise<void> => {
		// A running flush picks the new revision up in its own loop.
		if (c.mcEditRunning) return;
		const session = c.mcSession;
		if (!session || !this.app.multiCases.includes(c)) {
			// `queueMultiLoadPower` marked the case as solving on behalf of
			// this flush; release it, or Save and Export stay disabled.
			c.solving = false;
			return;
		}
		c.mcEditRunning = true;
		c.solving = true;
		this.app.error = null;
		try {
			for (;;) {
				const revision = c.mcEditRevision;
				const edits = c.mcLoadEdits.map((edit) => ({ ...edit }));
				const started = performance.now();
				const summary = await session.replaceLoadPowers(edits);
				if (session !== c.mcSession || !this.app.multiCases.includes(c)) return;
				if (revision !== c.mcEditRevision) continue;
				// One round trip: the queued edit already set the branch state
				// this successful solve confirms. Only a failure refetches it.
				c.result = summary;
				c.mcFullResult = null;
				c.mcSnapshot = null;
				c.mcSavedAt = null;
				const total = performance.now() - started;
				c.solveMs = total;
				c.mcTiming = session.lastTiming ? { ...session.lastTiming, total_ms: total } : null;
				c.revisionGeneration++;
				break;
			}
		} catch (error) {
			if (session === c.mcSession) {
				try {
					c.mcLoadBranches = await session.loadBranches();
				} catch {
					// The session itself no longer answers (the worker trapped
					// or the handle is gone). Retrying against it would fail
					// forever, so drop it and let the next run rebuild the
					// session against a fresh worker.
					session.free();
					c.mcSession = null;
					c.mcLoadEdits = [];
				}
			}
			this.failed(errorTitle(error, 'Power flow failed'), 'Solve', error, {
				retry: c.mcSession
					? () => void this.flushMultiLoadPowers(c)
					: () => void this.solveMultiCase(c).catch(() => {})
			});
		} finally {
			// The running flag belongs to this call regardless of whether the
			// session was replaced meanwhile; leaving it set would silently
			// ignore every later edit. The solving flag is shared with an
			// explicit solve, which clears it itself when it owns it.
			c.mcEditRunning = false;
			if (session === c.mcSession || c.mcSession === null) c.solving = false;
		}
	};

	/** Make a multiconductor case active, framing it when it is placed. */
	activateMulti = (c: MulticonductorCase) => {
		this.hostedLoadAbort?.abort();
		this.app.studyView = null;
		this.clearSelection();
		this.app.activeCaseId = null;
		this.app.activeLocalId = null;
		this.app.activeMultiId = c.id;
		this.app.placingLocalId = null;
		this.app.placingMultiId = c.placed ? null : c.id;
		if (c.placed) this.app.requestFrame(c.id);
	};

	/** Select a bus in a multiconductor case: its terminal stack and incident
	 * conductors expand. Selecting is viewing detail only — no solve. */
	selectMultiBus = (caseId: string, busId: string) => {
		this.hostedLoadAbort?.abort();
		const c = this.app.multiCases.find((mc) => mc.id === caseId);
		if (!c) return;
		this.app.activeCaseId = null;
		this.app.activeLocalId = null;
		this.app.activeMultiId = caseId;
		this.app.placingMultiId = null;
		c.selectedEdgeId = null;
		c.selectedBusId = c.selectedBusId === busId ? null : busId;
	};

	/** Select an edge in a multiconductor case: its conductor pairing expands in
	 * the panel. Mutually exclusive with the bus selection. */
	selectMultiEdge = (caseId: string, edgeId: string) => {
		this.hostedLoadAbort?.abort();
		const c = this.app.multiCases.find((mc) => mc.id === caseId);
		if (!c) return;
		this.app.activeCaseId = null;
		this.app.activeLocalId = null;
		this.app.activeMultiId = caseId;
		this.app.placingMultiId = null;
		c.selectedBusId = null;
		c.selectedEdgeId = c.selectedEdgeId === edgeId ? null : edgeId;
	};

	/** Place a multiconductor case awaiting a center: fit its provided planar
	 * coordinates when it carries them, else lay it out synthetically. */
	placeMultiCase = (lon: number, lat: number) => {
		const id = this.app.placingMultiId;
		const c = id ? this.app.multiCases.find((mc) => mc.id === id) : null;
		if (!c?.graph) return;
		c.view = placeMultiView(c.graph, { lon, lat });
		c.syntheticCenter = { lon, lat };
		this.app.placingMultiId = null;
		this.app.activeMultiId = c.id;
		this.app.requestFrame(c.id);
	};

	/** Re-enter placement for a multiconductor case, to move its layout. */
	moveMultiCase = (c: MulticonductorCase) => {
		this.app.activeCaseId = null;
		this.app.activeLocalId = null;
		this.app.activeMultiId = c.id;
		this.app.placingMultiId = c.id;
	};

	removeMultiCase = async (c: MulticonductorCase, event?: MouseEvent) => {
		event?.stopPropagation();
		if (c.hosted) {
			this.rememberHiddenDefaultCase(c.id);
			this.app.hostedDistributionCases = this.app.hostedDistributionCases.filter(
				(s) => s.id !== c.id
			);
		}
		c.solveAbort?.abort();
		if (c.mcEditTimer !== null) clearTimeout(c.mcEditTimer);
		c.mcSession?.free();
		c.mcSession = null;
		releaseMcInput(c.mcInput);
		c.mcInput = null;
		if (this.app.activeMultiId === c.id) {
			c.selectedBusId = null;
			c.selectedEdgeId = null;
		}
		await this.hydrateFallback(this.app.removeMulti(c.id));
	};

	/** Hosted feeders use the same typed-module ingest and calculation path as local feeders. */
	activateHostedDistribution = async (id: string) => {
		if (this.disposed) return;
		const summary = this.app.hostedDistributionCases.find((s) => s.id === id);
		if (!summary || summary.unavailable_reason) return;
		const existing = this.app.multiCases.find((c) => c.hosted && c.id === id);
		if (existing) {
			this.activateMulti(existing);
			return;
		}
		const api = this.api;
		if (!api) {
			this.fail(noApiNotice(summary.name));
			return;
		}
		this.hostedLoadAbort?.abort();
		const abort = new AbortController();
		this.hostedLoadAbort = abort;
		this.loadingHostedDistribution = id;
		let payload: IngestedDistCase | null = null;
		try {
			const text = await api.getCaseModuleJson(id, abort.signal);
			abort.signal.throwIfAborted();
			payload = await (this.mcTransport.ingestDistCaseBytes ?? ingestDistCaseBytes)(
				new TextEncoder().encode(text),
				'pio',
				{ retain: true }
			);
			abort.signal.throwIfAborted();
			if (!this.app.hostedDistributionCases.includes(summary)) return;
			this.addMultiCase(`${id}.pio.json`, payload, summary);
			payload = null; // The case now owns the retained input.
			this.app.error = null;
			this.app.errorRetry = null;
		} catch (error) {
			if (!abort.signal.aborted)
				this.failed(`Could not load ${summary.name}`, 'Open', error, {
					message: `${summary.name}: ${errorText(error)}`,
					retry: () => this.activateHostedDistribution(id)
				});
		} finally {
			releaseMcInput(payload?.input);
			if (this.hostedLoadAbort === abort) {
				this.hostedLoadAbort = null;
				this.loadingHostedDistribution = null;
			}
		}
	};

	removeHostedDistribution = (id: string) => {
		this.rememberHiddenDefaultCase(id);
		this.app.hostedDistributionCases = this.app.hostedDistributionCases.filter((s) => s.id !== id);
		if (this.loadingHostedDistribution === id) this.hostedLoadAbort?.abort();
	};

	/** The unified map click-to-place: a pending multiconductor case takes the
	 * click first, otherwise a pending local case does. */
	placeCase = (lon: number, lat: number) => {
		if (this.app.placingMultiId) this.placeMultiCase(lon, lat);
		else this.placeLocalCase(lon, lat);
	};

	async projectStudyInput(input: string, layer?: string): Promise<Network> {
		const parsed = layer
			? { payload: await applyGeo(input, layer) }
			: await ingestJsonDrop(new TextEncoder().encode(input));
		if (!parsed.payload || !('topology' in parsed.payload))
			throw new Error('Study requires a balanced network.');
		const c = parsed.payload;
		const view = c.view ?? {
			...placeSyntheticTopology(c.topology, { lon: 0, lat: 0 }),
			coordinate_space: 'diagram' as const
		};
		return {
			id: 'study',
			name: c.name,
			base_mva: c.base_mva,
			synthetic_coords: !c.view,
			...view
		};
	}

	async caseGeographyLayers(c: LocalCase): Promise<string[]> {
		if (!c.studyInputJson || !c.view) return [];
		return [await extractGeo(c.studyInputJson)];
	}

	/** Capture a matching cached result without constructing or solving a model. */
	async captureSavedCase(c: SolvableCase): Promise<SavedCaseCapture> {
		const base = await this.ensureStudyInputJson(c);
		if (!base) throw new Error('The case input is unavailable.');
		const detached = c instanceof LocalCase ? this.detachedCaseCaptures.get(c) : undefined;
		if (
			detached?.baseInput === base &&
			detached.edits === this.caseEditKey(c) &&
			detached.solution === c.solution
		)
			return detached.captured;
		const cached = this.caseStudies.get(c);
		if (c.solution && cached?.studyInputJson === base && cached.formulation === c.formulation) {
			const current = await cached.study.currentSolution();
			if (sameSolution(current, c.solution)) {
				return {
					input: await cached.study.saveInstanceModule(),
					solution: await cached.study.saveSolutionModule(),
					view: await cached.study.currentView()
				};
			}
		}
		if (
			Object.values(this.caseDeltas(c)).some((v) => v !== 0) ||
			Object.values(this.caseRatings(c)).some((v) => v !== 0)
		) {
			throw new Error('Wait for the solve');
		}
		const api = this.api;
		if (
			c instanceof CaseState &&
			c.solution &&
			c.formulation === 'dcopf' &&
			(!c.resultDcFormula || c.resultDcFormula === 'series_susceptance') &&
			api?.getSavedCase
		) {
			const snapshot = await api.getSavedCase(c.id);
			if (snapshot.base_input !== base || !sameSolution(snapshot.display_solution, c.solution))
				throw new Error('Server case changed; reload before saving');
			return {
				input: snapshot.input,
				solution: snapshot.solution,
				view: snapshot.view
			};
		}
		if (
			c.formulation === 'dcopf' &&
			c.resultDcFormula &&
			c.resultDcFormula !== 'series_susceptance'
		) {
			throw new Error('Wait for the solve');
		}
		return { input: base };
	}

	/** Record what PowerIO and the engine said while reading a case file, and
	 * any saved calculation settings in `moduleJson` this engine cannot apply. */
	private logImport(
		fileName: string,
		summary: { diagnostics?: PowerIoDiagnostic[]; warnings?: string[] },
		moduleJson?: string,
		/** Log only: the dynamics demo counts its notes in the Dynamics panel. */
		quiet = false
	) {
		const recorded = recordedCalculationSettings(moduleJson);
		const ignored = recorded?.ignored.map((key) => `${recorded.calculation}.${key}`) ?? [];
		const entries: LogEntryInput[] = [
			...(summary.diagnostics ?? []).map((d) =>
				powerIoEntry(d, { stage: 'Open', subject: fileName })
			),
			...(summary.warnings ?? []).map((message) => ({
				severity: 'note' as const,
				stage: 'Open',
				source: 'tellegen',
				subject: fileName,
				message
			})),
			...ignored.map((setting) => ({
				severity: 'warning' as const,
				stage: 'Open',
				source: 'tellegen',
				code: 'settings_ignored',
				subject: fileName,
				message: `saved setting ${setting} is not supported by this engine and was ignored`
			}))
		];
		// Notes alone are not a toast: the Log keeps them.
		const problems = entries.filter((entry) => entry.severity !== 'note').length;
		if (quiet || !problems) this.app.log.record(entries, { repeat: false });
		else
			this.app.log.notify(entries, {
				repeat: false,
				title: `${problems} import ${problems === 1 ? 'warning' : 'warnings'}`
			});
	}

	/** Record what attaching coordinates to a case reported. `complete` is false
	 * when something was left unplaced or unmatched, which is worth a toast. */
	private logPlacement(c: LocalCase | MulticonductorCase, notes: string[], complete: boolean) {
		const entries = notes.map((message) => ({
			severity: 'note' as const,
			stage: 'Coordinates',
			subject: c.fileName,
			message
		}));
		if (complete) this.app.log.record(entries, { repeat: false });
		else
			this.app.log.notify(entries, {
				repeat: false,
				title: `${notes.length} coordinate ${notes.length === 1 ? 'note' : 'notes'}`
			});
	}

	/** Record a geographic layer's reader findings and placement notes. */
	private logCoordinates(layer: string, diagnostics?: PowerIoDiagnostic[], notes?: string[]) {
		this.app.log.record([
			...(diagnostics ?? []).map((d) => powerIoEntry(d, { stage: 'Coordinates', subject: layer })),
			...(notes ?? []).map((message) => ({
				severity: 'note' as const,
				stage: 'Coordinates',
				source: 'tellegen',
				subject: layer,
				message
			}))
		]);
	}

	/** Build a local case from a balanced PowerIO ingest payload without activating
	 * it yet; co-dropped placement data is applied before the first solve. */
	private localFromBalancedPayload = (fileName: string, payload: IngestedCase): LocalCase => {
		const { module_json, topology, view, formulation, ...summary } = payload;
		const label = caseLabel(summary.name, fileName);
		this.logImport(fileName, summary, module_json);
		return new LocalCase({
			id: `local-${++this.localSeq}`,
			label,
			fileName,
			formulation,
			declaredFormulation: formulation,
			summary,
			studyInputJson: module_json,
			topology,
			coordsKind: summary.coords_kind,
			view
		});
	};

	/** Ingest module text through the engine and require a balanced network. */
	private async ingestBalancedModule(name: string, moduleJson: string | undefined) {
		if (moduleJson === undefined)
			throw new Error(`${name}: openBalancedModule needs moduleJson or payload`);
		const result = await ingestJsonDrop(new TextEncoder().encode(moduleJson));
		if (result.kind === 'transmission') return result.payload;
		if (result.kind === 'module' && !('model' in result.payload)) return result.payload;
		throw new Error(`${name}: not a balanced network module (${result.kind})`);
	}

	/** Open a balanced powerio module from memory, the programmatic twin of
	 * dropping a `.pio.json` file: ingest (unless `payload` is given), apply an
	 * optional geographic layer, lay out a module without coordinates as a
	 * diagram, then add and activate it. Resolves to the new case; throws when
	 * the module is not a balanced network or the geographic layer matches
	 * nothing. */
	openBalancedModule = async (input: OpenBalancedModuleInput): Promise<LocalCase> => {
		this.app.parsingFile = true;
		let study = input.study;
		try {
			const payload =
				input.payload ?? (await this.ingestBalancedModule(input.name, input.moduleJson));
			const local = this.localFromBalancedPayload(`${input.name}.pio.json`, {
				...payload,
				name: input.name
			});
			if (input.formulation) local.formulation = input.formulation;
			if (input.geoLayer) {
				await this.applyGeoLayers(local, [
					{
						name: `${input.name}.geo.json`,
						layer: input.geoLayer,
						diagnostics: []
					}
				]);
			}
			this.placeTopologyAsDiagram(local);
			if (study) {
				this.adoptStudy(local, study);
				study = undefined;
			}
			if (input.activate !== false) this.addAndActivateLocal(local, { frame: input.frame });
			return local;
		} finally {
			// The open failed before the case took the Study.
			if (study) releaseWhenBuilt(study);
			this.app.parsingFile = false;
		}
	};

	/** Make a Study that was started before the case existed the case's
	 * pending build, so `getStudy` waits for it instead of building another.
	 * It is used only as built: for the case's module and formulation, with no
	 * calculation settings. Anything else, or a failed build, leaves the case
	 * to build its own. */
	private adoptStudy(c: LocalCase, pending: Promise<BrowserStudy>) {
		const studyInputJson = c.studyInputJson;
		const formulation = c.formulation;
		if (
			this.disposed ||
			!studyInputJson ||
			c.settings.accepted ||
			c.settings.baselines[formulation]
		) {
			releaseWhenBuilt(pending);
			return;
		}
		const token = {};
		const current = () => this.studyBuilds.get(c)?.token === token;
		const promise = (async (): Promise<BrowserStudy | null> => {
			let study: BrowserStudy | null = null;
			try {
				study = await pending;
				if (study.formulation !== formulation)
					throw new Error(`Study built for ${study.formulation}, not ${formulation}`);
				const baseSolution = await study.currentSolution();
				// As in `getStudy`: the case may have changed or gone while this built.
				if (c.formulation !== formulation || !current()) {
					study.free();
					return null;
				}
				this.caseStudies.set(c, {
					study,
					studyInputJson,
					formulation,
					baseSolution,
					settings: defaultCalculationSettings(formulation)
				});
				this.studyBuilds.delete(c);
				return study;
			} catch {
				study?.free();
				if (!current()) return null;
				this.studyBuilds.delete(c);
				return this.getStudy(c, studyInputJson);
			}
		})();
		this.studyBuilds.set(c, { studyInputJson, formulation, token, promise });
	}

	/** Swap a local case for a new module in place: same slot in the case list,
	 * same formulation unless `input.formulation` says otherwise, same display
	 * mode when the new case supports it, and the selected bus kept when the
	 * new case has it. The previous case's Study is freed. */
	replaceLocalCase = async (
		previous: LocalCase,
		input: OpenBalancedModuleInput
	): Promise<LocalCase> => {
		const wasActive = this.app.activeLocalId === previous.id;
		const selectedBus = wasActive ? this.app.selectedBus : null;
		const displayMode = previous.displayMode;
		const next = await this.openBalancedModule({
			...input,
			formulation: input.formulation ?? previous.formulation,
			activate: false
		});
		if (displayMode === 'diagram' ? next.diagram : next.view) next.displayMode = displayMode;
		const slot = this.app.localCases.findIndex((c) => c.id === previous.id);
		// A seq bump invalidates any in-flight browser solve of the old case.
		previous.solveSeq = (previous.solveSeq ?? 0) + 1;
		this.disposeStudy(previous);
		if (wasActive) {
			this.addAndActivateLocal(next, { frame: input.frame });
		} else {
			this.app.localCases = [...this.app.localCases, next];
			this.maybeStartLocalSolve(next.id);
		}
		const cases = this.app.localCases.filter((c) => c.id !== previous.id && c.id !== next.id);
		cases.splice(slot < 0 ? cases.length : slot, 0, next);
		this.app.localCases = cases;
		if (this.app.placingLocalId === previous.id) this.app.placingLocalId = null;
		if (selectedBus !== null && localHasBus(next, selectedBus)) {
			void this.selectLocalBus(next.id, selectedBus);
		}
		return next;
	};

	/** Build (or reuse) the case's Study and commit it at the current edit state, so a
	 * save or export captures exactly what is on screen. Null (with an error set) when no
	 * PowerIO module or Study is available. */
	public syncedStudy = async (c: SolvableCase): Promise<BrowserStudy | null> => {
		const unavailable = this.caseExportUnavailableReason(c);
		if (unavailable) {
			this.app.error = `${this.caseName(c)}: ${unavailable}`;
			return null;
		}
		const studyInputJson = await this.ensureStudyInputJson(c);
		if (!studyInputJson) {
			this.app.error = `${this.caseName(c)}: re-add the case`;
			return null;
		}
		const study = await this.getStudy(c, studyInputJson);
		if (!study) {
			this.app.error = `${this.caseName(c)}: solve again before saving`;
			return null;
		}
		await study.commit(c.id, this.caseDeltas(c), this.caseRatings(c), null);
		return study;
	};

	/** Save the materialized case as a retained PowerIO module. */
	saveCaseModule = async (c: SolvableCase): Promise<void> => {
		const study = await this.syncedStudy(c);
		if (!study) return;
		try {
			downloadText(
				await study.saveModule(),
				`${this.caseFileStem(c)}.pio.json`,
				'application/json'
			);
			this.app.error = null;
		} catch (e) {
			this.failed('Could not save the PowerIO module', 'Export', e, {
				message: `${this.caseName(c)}: could not save PowerIO module: ${errorText(e)}`
			});
		}
	};

	/** Save the exact DC OPF result with the materialized problem instance it solved. */
	saveSolutionModule = async (c: SolvableCase): Promise<void> => {
		if (c.formulation !== 'dcopf') {
			this.app.error = `${this.caseName(c)}: needs a DC OPF result`;
			return;
		}
		const study = await this.syncedStudy(c);
		if (!study) return;
		try {
			downloadText(
				await study.saveSolutionModule(),
				`${this.caseFileStem(c)}.solution.pio.json`,
				'application/json'
			);
			this.app.error = null;
		} catch (e) {
			this.failed('Could not save the solution', 'Export', e, {
				message: `${this.caseName(c)}: could not save exact solution: ${errorText(e)}`
			});
		}
	};

	/** Write the active study's committed state to `format` (a PowerIO token, or
	 * `pio-module` for the retained module) and download it. Throws when the case
	 * cannot be written; the error banner then names the reason. */
	writeCaseFile = async (
		c: SolvableCase,
		format: string
	): Promise<{
		fileName: string;
		bytes: number;
		diagnostics: PowerIoDiagnostic[];
	}> => {
		const study = await this.syncedStudy(c);
		if (!study)
			throw new CaseFileUnavailable(this.app.error ?? `${this.caseName(c)}: export is unavailable`);
		const stem = this.caseFileStem(c);
		let text: string;
		let fileName: string;
		let diagnostics: PowerIoDiagnostic[] = [];
		if (format === PIO_MODULE_EXPORT.token) {
			text = await study.saveModule();
			fileName = `${stem}.pio.json`;
			downloadText(text, fileName, 'application/json');
		} else {
			const exported = await study.export(format);
			text = exported.text;
			fileName = `${stem}.${exported.extension}`;
			diagnostics = exported.diagnostics ?? [];
			downloadText(text, fileName, 'text/plain');
			this.app.log.record(
				diagnostics.map((d) => powerIoEntry(d, { stage: 'Export', subject: fileName })),
				{ repeat: false }
			);
		}
		this.app.error = null;
		return {
			fileName,
			bytes: new TextEncoder().encode(text).length,
			diagnostics
		};
	};

	/** Export the active study's committed state to `format` and download it.
	 * Returns this output operation's structured diagnostics. */
	exportCaseAs = async (c: SolvableCase, format: string): Promise<PowerIoDiagnostic[]> => {
		try {
			return (await this.writeCaseFile(c, format)).diagnostics;
		} catch (e) {
			// An unavailable study has already put its reason in the banner.
			if (!(e instanceof CaseFileUnavailable))
				this.failed('Could not export the case', 'Export', e, {
					message: `${this.caseName(c)}: could not export study: ${errorText(e)}`
				});
			return [];
		}
	};

	/** A filesystem-safe base name for a downloaded case file. */
	private caseFileStem(c: SolvableCase): string {
		const base = this.isBackendCase(c) ? c.name : c.label || c.fileName.replace(/\.[^.]+$/, '');
		return base.replace(/[^\w.-]+/g, '_') || 'case';
	}

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
	// Study (no re-parse, no re-solve) to paint predicted per-bus
	// nodal value changes and the predicted objective change. A no-op when the Study isn't built yet or
	// can't preview (browser fallback path, server-only cases): the map then
	// falls back to the JS sensitivity-times-step preview.
	runPreview = (c: SolvableCase, bus: number, value: number) => {
		if (c.formulation === 'acpf' || c.formulation === 'acopf') return;
		// Fast path: the committed price/demand column (already solved at the committed point),
		// scaled by the demand step, is the same first-order linearization the engine preview
		// returns — without rebuilding the differentiable KKT every drag frame. That engine
		// preview is ~80 ms/frame on the largest case (CATS, ~8870 buses), which blocked the
		// main thread and made dragging choppy; reusing the column is O(buses).
		const col = c.sensitivity;
		if (col && col.bus === bus) {
			const committedAtBus = this.caseDeltas(c)[bus] ?? 0;
			const step = (Math.abs(value) < DEAD_ZONE_MW ? 0 : value) - committedAtBus;
			const delta = new Map<number, number>();
			for (const v of col.values) delta.set(v.bus, v.value * step);
			this.app.previewPrices = {
				caseId: c.id,
				target: { bus },
				delta,
				units: previewUnits(col.units)
			};
			const priceAtBus = c.solution?.prices.find((price) => price.bus === bus)?.value ?? null;
			this.previewObjective =
				priceAtBus === null
					? null
					: {
							caseId: c.id,
							target: { bus },
							objectiveDelta: priceAtBus * step
						};
			return;
		}
		// Fallback (no committed column yet): the engine's first-order preview, which
		// rebuilds the differentiable system. DC OPF only: the conic rebuild takes
		// seconds on the large cases. The preview is async (the engine runs in a
		// worker), so drag events queue at most one point: the newest requested
		// point runs next, intermediate points are dropped. Best effort; on any
		// failure the map keeps its own path.
		if (c.formulation !== 'dcopf' || !this.caseStudies.get(c)) return;
		this.enginePreviewNext = { c, bus, value };
		void this.pumpEnginePreview();
	};

	// The newest demand drag point waiting on an engine preview, and whether a
	// preview call is in flight. Written by runPreview per input event; drained
	// one call at a time so previews never stack up behind a slow solve.
	private enginePreviewNext: {
		c: SolvableCase;
		bus: number;
		value: number;
	} | null = null;
	private enginePreviewBusy = false;

	private async pumpEnginePreview() {
		if (this.enginePreviewBusy) return;
		this.enginePreviewBusy = true;
		try {
			while (this.enginePreviewNext) {
				const { c, bus, value } = this.enginePreviewNext;
				this.enginePreviewNext = null;
				const study = this.caseStudies.get(c)?.study;
				if (!study) continue;
				try {
					const { prices, objectiveDelta, units } = await study.preview(
						this.previewDeltas(c, bus, value),
						this.caseRatings(c)
					);
					// Apply only while this point is still the live drag target: a
					// preview that resolves after a re-selection or a commit is stale.
					if (!this.app.previewActive || this.activeSolvable !== c || this.app.selectedBus !== bus)
						continue;
					const delta = new Map<number, number>();
					for (const e of prices) delta.set(e.bus, e.value);
					this.app.previewPrices = {
						caseId: c.id,
						target: { bus },
						delta,
						units: dollarUnits(units ?? '$/MWh')
					};
					this.previewObjective =
						objectiveDelta === null ? null : { caseId: c.id, target: { bus }, objectiveDelta };
				} catch {
					if (this.activeSolvable === c && this.app.selectedBus === bus) {
						this.app.previewPrices = null;
						this.previewObjective = null;
					}
				}
			}
		} finally {
			this.enginePreviewBusy = false;
		}
	}

	setSliderPreview = (value: number | undefined) => {
		if (value === undefined || this.sliderDisabled) return;
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
		if (Math.abs(value) < DEAD_ZONE_MW) delete ratings[branch];
		else ratings[branch] = value;
		return ratings;
	}

	// First-order engine preview for the live rating drag, mirroring runPreview: the
	// committed price/rating column scaled by the rating step paints predicted per-bus
	// nodal value changes without touching the engine per frame. The predicted objective change comes from
	// a per-MW slope taken once from Study.preview at a +1 MW step off the committed
	// point (preview is replacement-absolute, so the absolute ratings map is built),
	// then scaled by the live step; null when no Study slope is available.
	runRatingPreview = (c: SolvableCase, branch: number, value: number) => {
		if (c.formulation === 'acpf' || c.formulation === 'acopf') return;
		const committedAtBranch = this.caseRatings(c)[branch] ?? 0;
		const step = (Math.abs(value) < DEAD_ZONE_MW ? 0 : value) - committedAtBranch;
		const col = c.sensitivity;
		if (col && col.branch === branch) {
			const delta = new Map<number, number>();
			for (const v of col.values) delta.set(v.bus, v.value * step);
			this.app.previewPrices = {
				caseId: c.id,
				target: { branch },
				delta,
				units: previewUnits(col.units)
			};
		}
		let cached = this.ratingSlope;
		if (cached?.caseId !== c.id || cached.branch !== branch) {
			// One slope attempt per (case, branch, committed point), cached even
			// when the preview returns nothing: the engine preview rebuilds the
			// differentiable system (~100 ms on the large cases), and a null
			// result retried on every input event froze the drag. A drag before
			// the Study exists caches nothing, so the prediction appears once
			// the Study lands.
			// The slope preview runs only for DC OPF: the conic differentiable
			// preview takes seconds on the large cases, and freezing the first
			// drag frame is not worth the Δ$ readout. SOCWR shows the column
			// preview without an objective prediction.
			const study = c.formulation === 'dcopf' ? this.caseStudies.get(c)?.study : undefined;
			if (study) {
				// Cache the entry before the async preview resolves so per-event calls
				// don't stack engine calls; the slope fills in when it lands and the
				// readout refreshes at the current drag position.
				const entry = { caseId: c.id, branch, slope: null as number | null };
				cached = entry;
				this.ratingSlope = entry;
				const pending = study
					.preview(this.caseDeltas(c), this.previewRatings(c, branch, committedAtBranch + 1))
					.then(({ objectiveDelta }) => {
						if (this.ratingSlope !== entry || objectiveDelta === null) return;
						this.ratingSlope = { caseId: c.id, branch, slope: objectiveDelta };
						const live = this.app.previewRatingMw;
						if (live !== null && this.activeSolvable === c && this.app.selectedBranch === branch) {
							this.runRatingPreview(c, branch, live);
						}
					})
					.catch(() => {
						// Best effort; the readout just shows no prediction.
					})
					.finally(() => {
						if (this.ratingSlopePending === pending) this.ratingSlopePending = null;
					});
				this.ratingSlopePending = pending;
			} else {
				cached = null;
			}
		}
		this.previewObjective =
			cached === null || cached.slope === null
				? null
				: {
						caseId: c.id,
						target: { branch },
						objectiveDelta: cached.slope * step
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

	// The in-flight slope preview for the rating drag, so a commit that arrives
	// before it resolves (typed value: input and change in the same tick) can
	// wait for the slope instead of scoring without a prediction.
	private ratingSlopePending: Promise<void> | null = null;

	commitRating = async (value: number) => {
		const c = this.activeSolvable;
		const branch = this.app.selectedBranch;
		if (!c || branch === null || isDisplayOnlyElement(this.selectedBranchData)) return;
		// Refresh the engine preview at the commit value (a typed value may not have
		// driven a drag), then score the commit with the engine's predicted Δobjective.
		this.runRatingPreview(c, branch, value);
		if (this.ratingSlopePending) {
			await this.ratingSlopePending;
			if (this.activeSolvable !== c || this.app.selectedBranch !== branch) return;
			this.runRatingPreview(c, branch, value);
		}
		c.predictedObjective = this.predictedDeltaObj;
		c.ratings = this.previewRatings(c, branch, value);
		this.bumpRevision(c);
		// The slope was taken at the old committed point; the next drag re-derives it.
		this.ratingSlope = null;
		this.app.previewRatingMw = value;
		this.app.previewActive = true;
		this.runSolve(c, this.selectionTarget);
	};

	finishRatingInput = (value: number) => {
		if (Math.abs(value - this.committedRating) < DEAD_ZONE_MW) {
			if (!this.activeSolvable?.solving) {
				this.app.previewActive = false;
				this.app.previewRatingMw = null;
			}
			return;
		}
		this.commitRating(value);
	};

	restoreDefaultCases = () => {
		if (!this.loadDefaultCases) return;
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
