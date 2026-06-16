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
import type { CaseFileSummary, Topology } from './wasm';

/** Substations from a PowerWorld .pwd display file. Positions are inferred
 * from diagram coordinates, not surveyed latitude and longitude. */
export interface LocalSubstations {
	points: { number: number; name: string; lon: number; lat: number }[];
	approximate: true;
}

/** A case file parsed in the browser. Topology only, no physics. A .pwd
 * display file has no case summary: substations only. */
export interface LocalCase {
	id: string; // `local-1`, `local-2`, ...
	label: string;
	fileName: string;
	/** Case stats; null for a .pwd display-only entry. */
	summary: CaseFileSummary | null;
	/** Raw powerio Network JSON for the browser solver branch. */
	networkJson?: string;
	/** Topology for synthetic placement when the file has no coordinates. */
	topology?: Topology;
	coordsKind?: 'file' | 'synthetic_pending' | 'synthetic';
	/** Map geometry when the file carried or received coordinates. */
	view: { buses: NetworkBus[]; branches: NetworkBranch[] } | null;
	syntheticCenter?: { lon: number; lat: number };
	/** Present for a PowerWorld .pwd display-only entry. */
	substations?: LocalSubstations;
}

/** One islanded network with its own solver instance on the backend. API
 * payloads are reassigned wholesale, so $state.raw throughout. */
export class CaseState {
	readonly id: string;
	readonly name: string;
	network = $state.raw<Network | null>(null);
	/** Boot solution at base demand; never changes. */
	baseSolution = $state.raw<Solution | null>(null);
	/** Exact solution at the current committed perturbation. */
	solution = $state.raw<Solution | null>(null);
	sensitivity = $state.raw<SensitivityColumn | null>(null);
	/** Committed demand deltas (MW from base, keyed by bus). */
	deltas = $state.raw<DemandDeltas>({});
	iterations = $state.raw<SolveIteration[]>([]);
	solving = $state(false);
	solveMs = $state<number | null>(null);
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
	sensitivityLoading = $state(false);
	error = $state<string | null>(null);

	/** Case files parsed in the browser via the powerio wasm module. */
	localCases = $state.raw<LocalCase[]>([]);
	/** Local case the panel shows; clicking a backend case or a bus clears it. */
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

	updateLocal(id: string, patch: Partial<LocalCase>) {
		this.localCases = this.localCases.map((c) => (c.id === id ? { ...c, ...patch } : c));
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
