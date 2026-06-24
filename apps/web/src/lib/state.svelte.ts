import type {
	CaseSummary,
	DemandDeltas,
	Network,
	NetworkBranch,
	NetworkBus,
	SensitivityColumn,
	Solution,
	SolveIteration
} from './api';
import { DEFAULT_FORMULATION, type CaseFileSummary, type Formulation, type Topology } from './wasm';

export type SolveBackend = 'clarabel-wasm' | 'clarabel-wasm-server-sensitivity' | 'rust-server';
export type DemandRangeMode = 'local' | 'full';

/** Substations from a PowerWorld .pwd display file. Positions are inferred
 * from diagram coordinates, not surveyed latitude and longitude. */
export interface LocalSubstations {
	points: { number: number; name: string; lon: number; lat: number }[];
	approximate: true;
}

type CoordsKind = 'file' | 'synthetic_pending' | 'synthetic' | 'geofile';
type LocalView = { buses: NetworkBus[]; branches: NetworkBranch[] };

/** The fields a parsed file supplies at creation; the solve state defaults. */
export interface LocalCaseInit {
	id: string; // `local-1`, `local-2`, ...
	label: string;
	fileName: string;
	summary?: CaseFileSummary | null;
	networkJson?: string;
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
	/** Case stats; null for a .pwd display only entry. */
	summary = $state.raw<CaseFileSummary | null>(null);
	/** Raw powerio Network JSON for the browser solver branch. */
	networkJson = $state.raw<string | undefined>(undefined);
	/** Topology for synthetic placement when the file has no coordinates. */
	topology = $state.raw<Topology | undefined>(undefined);
	coordsKind = $state.raw<CoordsKind | undefined>(undefined);
	/** Map geometry when the file carried or received coordinates. */
	view = $state.raw<LocalView | null>(null);
	syntheticCenter = $state.raw<{ lon: number; lat: number } | undefined>(undefined);
	geoSource = $state.raw<string | undefined>(undefined);
	geoWarnings = $state.raw<string[] | undefined>(undefined);
	network = $state.raw<Network | null>(null);
	baseSolution = $state.raw<Solution | null>(null);
	solution = $state.raw<Solution | null>(null);
	sensitivity = $state.raw<SensitivityColumn | null>(null);
	deltas = $state.raw<DemandDeltas>({});
	/** The OPF formulation the browser Study solves for this case: DC OPF (default),
	 * full AC OPF, or the SOCWR relaxation. Changing it rebuilds the Study. */
	formulation = $state<Formulation>(DEFAULT_FORMULATION);
	iterations = $state.raw<SolveIteration[]>([]);
	solving = $state(false);
	solveMs = $state<number | null>(null);
	solveBackend = $state<SolveBackend | null>(null);
	solveFallbackReason = $state<string | null>(null);
	/** Monotone token: only the latest solve may write this case. */
	solveSeq = 0;
	/** Monotone token: only the latest sensitivity request may write this case. */
	sensitivitySeq = 0;
	predictedObjective = $state<number | null>(null);
	/** Present for a PowerWorld .pwd display only entry. */
	substations = $state.raw<LocalSubstations | undefined>(undefined);

	constructor(init: LocalCaseInit) {
		this.id = init.id;
		this.label = init.label;
		this.fileName = init.fileName;
		this.summary = init.summary ?? null;
		this.networkJson = init.networkJson;
		this.topology = init.topology;
		this.coordsKind = init.coordsKind;
		this.view = init.view ?? null;
		this.substations = init.substations;
	}
}

/** One islanded network with its own solver state on the server. API
 * payloads are reassigned wholesale, so $state.raw throughout. */
export class CaseState {
	readonly id: string;
	readonly name: string;
	network = $state.raw<Network | null>(null);
	/** Raw powerio Network JSON for the browser solver; fetched lazily. */
	networkJson = $state.raw<string | null>(null);
	/** Boot solution at base demand; never changes. */
	baseSolution = $state.raw<Solution | null>(null);
	/** Exact solution at the current committed perturbation. */
	solution = $state.raw<Solution | null>(null);
	sensitivity = $state.raw<SensitivityColumn | null>(null);
	/** Committed demand deltas (MW from base, keyed by bus). */
	deltas = $state.raw<DemandDeltas>({});
	/** The OPF formulation the browser Study solves for this case: DC OPF (default),
	 * full AC OPF, or the SOCWR relaxation. Changing it rebuilds the Study. */
	formulation = $state<Formulation>(DEFAULT_FORMULATION);
	iterations = $state.raw<SolveIteration[]>([]);
	solving = $state(false);
	solveMs = $state<number | null>(null);
	solveBackend = $state<SolveBackend | null>(null);
	solveFallbackReason = $state<string | null>(null);
	/** Monotone token: only the latest solve may write this case. */
	solveSeq = 0;
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
	}

	get perturbed(): boolean {
		return Object.values(this.deltas).some((mw) => mw !== 0);
	}
}

export class AppState {
	cases = $state.raw<CaseState[]>([]);
	activeCaseId = $state<string | null>(null);
	/** Selected bus in the active case. */
	selectedBus = $state<number | null>(null);
	/** Live slider value (MW from base) before commit; null when idle. */
	previewDeltaMw = $state<number | null>(null);
	/** True while the demand control should keep the map in LMP preview mode. */
	previewActive = $state(false);
	/** Engine first-order LMP preview for the live drag: predicted change in LMP
	 * ($/MWh) per bus at the previewed demand, scoped to the case and bus it was
	 * computed for. Set by the Study path; null when no Study preview applies (the
	 * map then falls back to the JS sensitivity-times-step preview). Reassigned
	 * wholesale, so $state.raw. */
	previewLmp = $state.raw<{ caseId: string; bus: number; delta: Map<number, number> } | null>(null);
	demandRangeMode = $state<DemandRangeMode>('local');
	sensitivityLoading = $state(false);
	error = $state<string | null>(null);

	/** Case files parsed in the browser via the powerio wasm module. */
	localCases = $state.raw<LocalCase[]>([]);
	/** Local case the panel shows; clicking a bundled case or a bus clears it. */
	activeLocalId = $state<string | null>(null);
	placingLocalId = $state<string | null>(null);
	dragOver = $state(false);
	parsingFile = $state(false);

	/** Map framing request: bump seq so repeat targets still fly. */
	frameTarget = $state<string | 'all'>('all');
	frameSeq = $state(0);

	get active(): CaseState | null {
		return this.cases.find((c) => c.id === this.activeCaseId) ?? null;
	}

	byId(id: string): CaseState | null {
		return this.cases.find((c) => c.id === id) ?? null;
	}

	get activeLocal(): LocalCase | null {
		return this.localCases.find((c) => c.id === this.activeLocalId) ?? null;
	}

	addLocal(c: LocalCase) {
		this.localCases = [...this.localCases, c];
		this.activeLocalId = c.id;
		this.placingLocalId = c.coordsKind === 'synthetic_pending' ? c.id : null;
	}

	removeCase(id: string) {
		const wasActive = this.activeCaseId === id;
		this.cases = this.cases.filter((c) => c.id !== id);
		if (!wasActive) return;

		this.selectedBus = null;
		this.previewDeltaMw = null;
		this.previewActive = false;
		this.previewLmp = null;
		this.demandRangeMode = 'local';
		this.sensitivityLoading = false;

		this.activeCaseId = this.cases[0]?.id ?? null;
		if (this.activeCaseId) {
			this.requestFrame(this.activeCaseId);
			return;
		}

		const nextLocal =
			this.localCases.find(
				(c) => c.view || c.substations || c.coordsKind === 'synthetic_pending'
			) ??
			this.localCases[0] ??
			null;
		this.activeLocalId = nextLocal?.id ?? null;
		this.placingLocalId = nextLocal?.coordsKind === 'synthetic_pending' ? nextLocal.id : null;
		if (nextLocal?.view || nextLocal?.substations) this.requestFrame(nextLocal.id);
		else this.requestFrame('all');
	}

	removeLocal(id: string) {
		this.localCases = this.localCases.filter((c) => c.id !== id);
		if (this.placingLocalId === id) this.placingLocalId = null;
		if (this.activeLocalId === id) {
			this.activeLocalId = null;
			if (this.activeCaseId === null) {
				this.activeCaseId = this.cases[0]?.id ?? null;
				if (this.activeCaseId) this.requestFrame(this.activeCaseId);
			}
		}
	}

	requestFrame(target: string | 'all') {
		this.frameTarget = target;
		this.frameSeq++;
	}
}

export const app = new AppState();
