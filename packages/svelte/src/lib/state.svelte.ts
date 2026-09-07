import type {
	CaseSummary,
	DemandDeltas,
	Network,
	NetworkBranch,
	NetworkBus,
	SensitivityColumn,
	Solution,
	SolveIteration
} from './api.js';
import {
	DEFAULT_FORMULATION,
	type StudyView,
	type BranchRatingDeltas,
	type CaseFileSummary,
	type DistGraph,
	type Formulation,
	type IngestedDistCase,
	type McPfResult,
	type SensTarget,
	type Topology
} from '@tellegen/engine';
import type { MultiView } from './multiconductor.js';

export type SolveBackend = 'clarabel-wasm' | 'clarabel-wasm-server-sensitivity' | 'rust-server';
/** A map framing request: a case id, 'all', or one branch to center. */
export type FrameTarget =
	string | 'all' | { caseId: string; branchId: number } | { caseId: string; busId: number };
export interface CameraSnapshot {
	center: [number, number];
	zoom: number;
	bearing: number;
	pitch: number;
}
export interface DiagramCameraSnapshot {
	caseId: string;
	center: [number, number];
	scale: number;
}

export type DemandRangeMode = 'local' | 'full';
export type DisplayMode = 'price' | 'angle' | 'voltage';

/** The case a removal promoted to active, so the caller can hydrate it: a backend
 * case needs its network/solution loaded, a local needs a browser solve, and
 * `none` means nothing remains (or the removal left the active case untouched). */
export type FallbackTarget =
	{ kind: 'backend'; id: string } | { kind: 'local'; id: string } | { kind: 'none' };

/** Substations from a PowerWorld .pwd display file. Positions are inferred
 * from diagram coordinates, not surveyed latitude and longitude. */
export interface LocalSubstations {
	points: { number: number; name: string; lon: number; lat: number }[];
	approximate: true;
}

type CoordsKind = 'file' | 'synthetic_pending' | 'synthetic' | 'manual' | 'geofile';
type LocalView = {
	coordinate_space?: 'geographic' | 'diagram';
	buses: NetworkBus[];
	branches: NetworkBranch[];
};

/** A case is perturbed when any committed demand or rating delta is nonzero. Shared
 * by both solvable case classes so the "perturbed" rule stays single-sourced. */
const hasPerturbation = (edits: Record<number, number>): boolean =>
	Object.values(edits).some((mw) => mw !== 0);

/** The fields a parsed file supplies at creation; the solve state defaults. */
export interface LocalCaseInit {
	id: string; // `local-1`, `local-2`, ...
	label: string;
	fileName: string;
	formulation?: Formulation;
	declaredFormulation?: Formulation;
	summary?: CaseFileSummary | null;
	/** Retained PowerIO module used to construct solver studies. */
	studyInputJson?: string;
	topology?: Topology;
	coordsKind?: CoordsKind;
	view?: LocalView | null;
	substations?: LocalSubstations;
}

/** A case file parsed in the browser. Network cases can solve after they have
 * coordinates; a .pwd display file has no case summary, substations only. A
 * class with a stable identity and reactive fields, like CaseState: the solve
 * and sensitivity flows mutate fields directly (each is $state, so the panel
 * re-renders) and the seq tokens stay attached across overlapping async
 * callbacks, so a stale solve can neither freeze the UI nor clobber a newer one. */
export class LocalCase {
	readonly id: string;
	readonly label: string;
	readonly fileName: string;
	readonly declaredFormulation?: Formulation;
	/** Case stats; null for a .pwd display only entry. */
	summary: CaseFileSummary | null = $state.raw<CaseFileSummary | null>(null);
	/** Generation 2 PowerIO IR used for display edits and solver studies. */
	studyInputJson: string | undefined = $state.raw<string | undefined>(undefined);
	/** Topology for synthetic placement when the file has no coordinates. */
	topology: Topology | undefined = $state.raw<Topology | undefined>(undefined);
	coordsKind: CoordsKind | undefined = $state.raw<CoordsKind | undefined>(undefined);
	/** Map geometry when the file carried or received coordinates. */
	view: LocalView | null = $state.raw<LocalView | null>(null);
	syntheticCenter: { lon: number; lat: number } | undefined = $state.raw<
		{ lon: number; lat: number } | undefined
	>(undefined);
	diagram: { view: LocalView; layer: string; name: string; warnings: string[] } | null =
		$state.raw(null);
	displayMode: 'geographic' | 'diagram' = $state('geographic');
	geoSource: string | undefined = $state.raw<string | undefined>(undefined);
	geoWarnings: string[] | undefined = $state.raw<string[] | undefined>(undefined);
	network: Network | null = $state.raw<Network | null>(null);
	baseSolution: Solution | null = $state.raw<Solution | null>(null);
	solution: Solution | null = $state.raw<Solution | null>(null);
	sensitivity: SensitivityColumn | null = $state.raw<SensitivityColumn | null>(null);
	deltas: DemandDeltas = $state.raw<DemandDeltas>({});
	/** Committed branch rating deltas (MW from base, keyed by branch). */
	ratings: BranchRatingDeltas = $state.raw<BranchRatingDeltas>({});
	/** The selected calculation. Changing it rebuilds the numerical Study. */
	formulation = $state<Formulation>(DEFAULT_FORMULATION);
	iterations: SolveIteration[] = $state.raw<SolveIteration[]>([]);
	solving = $state(false);
	solveMs = $state<number | null>(null);
	solveBackend = $state<SolveBackend | null>(null);
	solveFallbackReason = $state<string | null>(null);
	/** Monotone token: only the latest solve may write this case. */
	solveSeq = 0;
	/** Monotone generation for external optimistic concurrency. */
	revisionGeneration = $state(0);
	/** Monotone token: only the latest sensitivity request may write this case. */
	sensitivitySeq = 0;
	predictedObjective = $state<number | null>(null);
	/** Present for a PowerWorld .pwd display only entry. */
	substations: LocalSubstations | undefined = $state.raw<LocalSubstations | undefined>(undefined);

	constructor(init: LocalCaseInit) {
		this.id = init.id;
		this.label = init.label;
		this.fileName = init.fileName;
		this.formulation = init.formulation ?? DEFAULT_FORMULATION;
		this.declaredFormulation = init.declaredFormulation;
		this.summary = init.summary ?? null;
		this.studyInputJson = init.studyInputJson;
		this.topology = init.topology;
		this.coordsKind = init.coordsKind;
		this.view = init.view ?? null;
		this.substations = init.substations;
	}

	get perturbed(): boolean {
		return hasPerturbation(this.deltas) || hasPerturbation(this.ratings);
	}
}

/** One islanded network with its own solver state on the server. API
 * payloads are reassigned wholesale, so $state.raw throughout. */
export class CaseState {
	readonly id: string;
	readonly name: string;
	readonly unavailableReason: string | null;
	network: Network | null = $state.raw<Network | null>(null);
	/** Retained PowerIO module for the browser solver; fetched lazily. */
	studyInputJson: string | null = $state.raw<string | null>(null);
	/** Boot solution at base demand; never changes. */
	baseSolution: Solution | null = $state.raw<Solution | null>(null);
	/** Exact solution at the current committed perturbation. */
	solution: Solution | null = $state.raw<Solution | null>(null);
	sensitivity: SensitivityColumn | null = $state.raw<SensitivityColumn | null>(null);
	/** Committed demand deltas (MW from base, keyed by bus). */
	deltas: DemandDeltas = $state.raw<DemandDeltas>({});
	/** Committed branch rating deltas (MW from base, keyed by branch). */
	ratings: BranchRatingDeltas = $state.raw<BranchRatingDeltas>({});
	/** The OPF formulation the browser Study solves for this case: DC OPF (default),
	 * full AC OPF, or the SOCWR relaxation. Changing it rebuilds the Study. */
	formulation = $state<Formulation>(DEFAULT_FORMULATION);
	iterations: SolveIteration[] = $state.raw<SolveIteration[]>([]);
	solving = $state(false);
	solveMs = $state<number | null>(null);
	solveBackend = $state<SolveBackend | null>(null);
	solveFallbackReason = $state<string | null>(null);
	/** Monotone token: only the latest solve may write this case. */
	solveSeq = 0;
	/** Monotone generation for external optimistic concurrency. */
	revisionGeneration = $state(0);
	/** Monotone token: only the latest sensitivity request may write this case. */
	sensitivitySeq = 0;
	/** Closer for this case's in-flight server solve stream, if any. Owned per
	 * case so closing one case's stream never strands another's solve. */
	closeStream: (() => void) | null = null;
	/** Objective change the gradient predicted for the last commit, to score
	 * the preview once the exact solve lands. */
	predictedObjective = $state<number | null>(null);

	constructor(summary: CaseSummary) {
		this.id = summary.id;
		this.name = summary.name;
		this.unavailableReason = summary.unavailable_reason ?? null;
	}

	get perturbed(): boolean {
		return hasPerturbation(this.deltas) || hasPerturbation(this.ratings);
	}
}

/** A case the solver can run: a server-backed case or a browser-parsed local case. */
export type SolvableCase = CaseState | LocalCase;

/** The multiconductor ingest payload without the graph: the summary counts,
 * connected load/generation, coordinate provenance, and diagnostics. */
export type MultiCaseSummary = Omit<IngestedDistCase, 'graph'>;

/** Geographic coordinates use the map; other positions use the diagram canvas. */
export type MultiCoordsKind = 'geographic' | 'planar' | 'synthetic';

/** A conductor-resolved case with retained electrical inputs and terminal results. */
export class MulticonductorCase {
	readonly id: string;
	readonly label: string;
	readonly fileName: string;
	moduleJson: string | null = $state.raw<string | null>(null);
	geoLayer: string | null = $state.raw<string | null>(null);
	geoWarnings: string[] = $state.raw<string[]>([]);
	result: McPfResult | null = $state.raw<McPfResult | null>(null);
	solving = $state(false);
	solveMs = $state<number | null>(null);
	solveSeq = 0;
	revisionGeneration = $state(0);
	solveAbort: AbortController | null = null;
	/** Summary counts and coordinate provenance from the parse. */
	summary: MultiCaseSummary | null = $state.raw<MultiCaseSummary | null>(null);
	/** The render-ready bus/terminal graph. */
	graph: DistGraph | null = $state.raw<DistGraph | null>(null);
	/** Placement kind resolved at ingest from the case's coordinate space. */
	coordsKind: MultiCoordsKind = $state.raw<MultiCoordsKind>('synthetic');
	/** The map or diagram positions. */
	view: MultiView | null = $state.raw<MultiView | null>(null);
	syntheticCenter: { lon: number; lat: number } | undefined = $state.raw<
		{ lon: number; lat: number } | undefined
	>(undefined);
	/** The selected bus id, whose terminal stack and incident conductors expand;
	 * null when nothing is selected. String-keyed: distribution bus ids are names. */
	selectedBusId = $state<string | null>(null);
	/** The selected edge id, whose conductor pairing expands in the panel;
	 * mutually exclusive with selectedBusId. */
	selectedEdgeId = $state<string | null>(null);

	constructor(init: {
		id: string;
		label: string;
		fileName: string;
		summary: MultiCaseSummary;
		graph: DistGraph;
		coordsKind: MultiCoordsKind;
		view?: MultiView | null;
	}) {
		this.id = init.id;
		this.label = init.label;
		this.fileName = init.fileName;
		this.summary = init.summary;
		this.moduleJson = init.summary.module_json ?? null;
		this.geoLayer = init.summary.geo_layer ?? null;
		this.graph = init.graph;
		this.coordsKind = init.coordsKind;
		this.view = init.view ?? null;
	}

	/** Whether the case is placed and ready to render. */
	get placed(): boolean {
		return this.view !== null;
	}

	/** The selected bus's placed detail, or null. */
	get selectedBus() {
		if (this.selectedBusId === null) return null;
		return this.view?.buses.find((b) => b.id === this.selectedBusId) ?? null;
	}

	/** The selected edge's placed detail, or null. */
	get selectedEdge() {
		if (this.selectedEdgeId === null) return null;
		return this.view?.edges.find((e) => e.id === this.selectedEdgeId) ?? null;
	}
}

export interface StudyDisplaySnapshot {
	id: string;
	studyId: string;
	caseId: string;
	revision: number;
	label: string;
	formulation: Formulation;
	network: Network;
	solution: StudyView | null;
	inputJson?: string;
	baseDemandMw?: Record<string, number>;
}

export class AppState {
	camera: CameraSnapshot | null = $state.raw<CameraSnapshot | null>(null);
	cameraRequest: CameraSnapshot | null = $state.raw<CameraSnapshot | null>(null);
	cameraSeq = $state(0);
	diagramCamera: DiagramCameraSnapshot | null = $state.raw<DiagramCameraSnapshot | null>(null);
	diagramCameraRequest: DiagramCameraSnapshot | null = $state.raw<DiagramCameraSnapshot | null>(
		null
	);
	diagramCameraSeq = $state(0);

	requestDiagramCamera(caseId: string, snapshot: Omit<DiagramCameraSnapshot, 'caseId'>): void {
		if (
			!Array.isArray(snapshot.center) ||
			snapshot.center.length !== 2 ||
			![...snapshot.center, snapshot.scale].every(Number.isFinite) ||
			snapshot.scale <= 0
		) {
			throw new Error('Invalid saved drawing view');
		}
		this.settleFrame();
		this.diagramCameraRequest = { caseId, center: [...snapshot.center], scale: snapshot.scale };
		this.diagramCameraSeq++;
	}

	requestCamera(snapshot: CameraSnapshot): void {
		if (
			!Array.isArray(snapshot.center) ||
			snapshot.center.length !== 2 ||
			![...snapshot.center, snapshot.zoom, snapshot.bearing, snapshot.pitch].every(
				Number.isFinite
			) ||
			Math.abs(snapshot.center[1]) > 90 ||
			snapshot.zoom < 0 ||
			snapshot.zoom > 24 ||
			snapshot.pitch < 0 ||
			snapshot.pitch > 85
		) {
			throw new Error('Invalid saved camera position');
		}
		this.settleFrame();
		this.cameraRequest = { ...snapshot, center: [...snapshot.center] };
		this.cameraSeq++;
	}

	/** Saved Study inspection is independent of the editable case and its solution. */
	studyView: StudyDisplaySnapshot | null = $state.raw<StudyDisplaySnapshot | null>(null);
	cases: CaseState[] = $state.raw<CaseState[]>([]);
	activeCaseId = $state<string | null>(null);
	/** Selected bus in the active case. */
	selectedBus = $state<number | null>(null);
	/** Selected branch in the active case; mutually exclusive with selectedBus. */
	selectedBranch = $state<number | null>(null);
	/** Live slider value (MW from base) before commit; null when idle. */
	previewDeltaMw = $state<number | null>(null);
	/** Live rating slider value (MW from base) before commit; null when idle. */
	previewRatingMw = $state<number | null>(null);
	/** True while the demand control should keep the map in nodal value preview mode. */
	previewActive = $state(false);
	/** Engine first order nodal value preview for the live drag, scoped to the case and selection
	 * target (bus or branch) it was computed for. Set by the Study path; null when
	 * no Study preview applies (the map then falls back to the JS
	 * sensitivity-times-step preview). Reassigned wholesale, so $state.raw. */
	previewPrices = $state.raw<{
		caseId: string;
		target: SensTarget;
		delta: Map<number, number>;
		units: string;
	} | null>(null);
	demandRangeMode = $state<DemandRangeMode>('local');
	displayMode = $state<DisplayMode>('price');
	sensitivityLoading = $state(false);
	#error = $state<string | null>(null);
	errorRevision = $state(0);
	/** Re-runs the operation behind the current `error`, when one applies. Every
	 * write to `error` clears it, so a retry op can never outlive its message. */
	errorRetry: (() => void) | null = $state.raw<(() => void) | null>(null);

	get error(): string | null {
		return this.#error;
	}

	set error(message: string | null) {
		this.#error = message;
		if (message) this.errorRevision++;
		this.errorRetry = null;
	}

	/** Case files parsed in the browser via the powerio wasm module. */
	localCases: LocalCase[] = $state.raw<LocalCase[]>([]);
	/** Local case the panel shows; clicking a bundled case or a bus clears it. */
	activeLocalId = $state<string | null>(null);
	placingLocalId = $state<string | null>(null);
	/** Multiconductor distribution cases parsed in the browser (viewing only). */
	multiCases: MulticonductorCase[] = $state.raw<MulticonductorCase[]>([]);
	/** Multiconductor case the panel shows; mutually exclusive with the solvable
	 * active ids. */
	activeMultiId = $state<string | null>(null);
	/** Multiconductor case awaiting placement on the map. */
	placingMultiId = $state<string | null>(null);
	dragOver = $state(false);
	parsingFile = $state(false);

	/** True under `(max-width: 760px)`; the panel renders as a bottom sheet. Set by
	 * the shell from one media query so panel and map read the same value. */
	compactLayout = $state(false);
	/** Height in px the bottom sheet covers; 0 when `compactLayout` is false.
	 * Read by the map's camera padding and by chrome anchored above the sheet. */
	sheetInset = $state(0);
	/** Height in px the header covers at the top of the map, measured by
	 * `AppHeader`. Wrapping the case tabs onto their own row roughly doubles it,
	 * so callers that need to clear the header read this rather than a constant. */
	headerInset = $state(64);
	/** `window.innerHeight`, tracked by the shell. The sheet's snaps and the map's
	 * chrome lift are both fractions of it; the fallback is a desktop window. */
	viewportHeight = $state(800);

	/** Map framing request: bump seq so repeat targets still fly. `requestFrame`
	 * returns a promise the map resolves when the camera lands (or immediately
	 * when it cannot fly), so a caller can defer heavy work until the animation
	 * finishes. */
	frameTarget: FrameTarget = $state.raw<FrameTarget>('all');
	frameSeq = $state(0);
	#frameSettled: (() => void) | null = null;

	get active(): CaseState | null {
		return this.cases.find((c) => c.id === this.activeCaseId) ?? null;
	}

	byId(id: string): CaseState | null {
		return this.cases.find((c) => c.id === id) ?? null;
	}

	get activeLocal(): LocalCase | null {
		return this.localCases.find((c) => c.id === this.activeLocalId) ?? null;
	}

	get activeMulti(): MulticonductorCase | null {
		return this.multiCases.find((c) => c.id === this.activeMultiId) ?? null;
	}

	/** Whether any case is awaiting a click-to-place on the map (local or multi). */
	get placingId(): string | null {
		return this.placingLocalId ?? this.placingMultiId;
	}

	addLocal(c: LocalCase) {
		this.localCases = [...this.localCases, c];
		this.activeLocalId = c.id;
		this.placingLocalId = c.coordsKind === 'synthetic_pending' ? c.id : null;
	}

	/** Add a multiconductor case and make it active. A case that still needs a
	 * map center (planar/synthetic, no view yet) enters placement. */
	addMulti(c: MulticonductorCase) {
		this.multiCases = [...this.multiCases, c];
		this.activeCaseId = null;
		this.activeLocalId = null;
		this.activeMultiId = c.id;
		this.placingLocalId = null;
		this.placingMultiId = c.placed ? null : c.id;
	}

	removeMulti(id: string): FallbackTarget {
		const wasActive = this.activeMultiId === id;
		this.multiCases = this.multiCases.filter((c) => c.id !== id);
		if (this.placingMultiId === id) this.placingMultiId = null;
		if (!wasActive) return { kind: 'none' };
		this.activeMultiId = null;
		if (this.activeCaseId !== null || this.activeLocalId !== null) return { kind: 'none' };
		return this.activateFallback();
	}

	removeCase(id: string): FallbackTarget {
		const wasActive = this.activeCaseId === id;
		this.cases = this.cases.filter((c) => c.id !== id);
		if (!wasActive) return { kind: 'none' };

		this.selectedBus = null;
		this.selectedBranch = null;
		this.previewDeltaMw = null;
		this.previewRatingMw = null;
		this.previewActive = false;
		this.previewPrices = null;
		this.demandRangeMode = 'local';
		this.sensitivityLoading = false;

		return this.activateFallback();
	}

	removeLocal(id: string): FallbackTarget {
		const wasActive = this.activeLocalId === id;
		this.localCases = this.localCases.filter((c) => c.id !== id);
		if (this.placingLocalId === id) this.placingLocalId = null;
		if (!wasActive) return { kind: 'none' };

		this.activeLocalId = null;
		// A backend case can still be active; only pick a fallback when nothing is.
		if (this.activeCaseId !== null) return { kind: 'none' };

		return this.activateFallback();
	}

	// Pick the next active case after a removal: the first remaining backend case,
	// else a remaining local case that can render (a view or substations) or is
	// awaiting placement, else the first remaining local. Frames whatever it picks.
	activateFallback(): FallbackTarget {
		this.activeCaseId = this.cases.find((c) => !c.unavailableReason)?.id ?? null;
		if (this.activeCaseId) {
			this.requestFrame(this.activeCaseId);
			return { kind: 'backend', id: this.activeCaseId };
		}
		const nextLocal =
			this.localCases.find(
				(c) => c.view || c.substations || c.coordsKind === 'synthetic_pending'
			) ??
			this.localCases[0] ??
			null;
		if (nextLocal) {
			this.activeLocalId = nextLocal.id;
			this.placingLocalId = nextLocal.coordsKind === 'synthetic_pending' ? nextLocal.id : null;
			if (nextLocal.view || nextLocal.substations) this.requestFrame(nextLocal.id);
			else this.requestFrame('all');
			return { kind: 'local', id: nextLocal.id };
		}
		this.activeLocalId = null;
		this.placingLocalId = null;
		// A multiconductor case is viewing only, so no hydration target: activate
		// it, frame whatever is placed, and report `none`.
		const nextMulti = this.multiCases[0] ?? null;
		this.activeMultiId = nextMulti?.id ?? null;
		this.placingMultiId = nextMulti && !nextMulti.placed ? nextMulti.id : null;
		if (nextMulti?.placed) this.requestFrame(nextMulti.id);
		else this.requestFrame('all');
		return { kind: 'none' };
	}

	requestFrame(target: FrameTarget): Promise<void> {
		// A superseded request settles immediately; the new request owns the camera.
		this.settleFrame();
		this.frameTarget = target;
		this.frameSeq++;
		return new Promise((resolve) => {
			this.#frameSettled = resolve;
		});
	}

	/** The map calls this when the requested camera move has landed. */
	settleFrame() {
		this.#frameSettled?.();
		this.#frameSettled = null;
	}
}

export function createAppState(): AppState {
	return new AppState();
}
