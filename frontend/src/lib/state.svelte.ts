import type {
	CaseSummary,
	DemandDeltas,
	Network,
	NetworkBranch,
	NetworkBus,
	SensitivityColumn,
	Solution
} from './api';
import type { CaseFileSummary, Topology } from './wasm';

export type SolveBackend = 'clarabel-wasm' | 'clarabel-wasm-server-sensitivity' | 'rust-server';
export type DemandRangeMode = 'local' | 'full';

/** Substations from a PowerWorld .pwd display file. Positions are inferred
 * from diagram coordinates, not surveyed latitude and longitude. */
export interface LocalSubstations {
	points: { number: number; name: string; lon: number; lat: number }[];
	approximate: true;
}

/** A case file parsed in the browser. Network cases can solve after they have
 * coordinates. A .pwd display file has no case summary: substations only. */
export interface LocalCase {
	id: string; // `local-1`, `local-2`, ...
	label: string;
	fileName: string;
	/** Case stats; null for a .pwd display only entry. */
	summary: CaseFileSummary | null;
	/** Raw powerio Network JSON for the browser solver branch. */
	networkJson?: string;
	/** Topology for synthetic placement when the file has no coordinates. */
	topology?: Topology;
	coordsKind?: 'file' | 'synthetic_pending' | 'synthetic' | 'geofile';
	/** Map geometry when the file carried or received coordinates. */
	view: { buses: NetworkBus[]; branches: NetworkBranch[] } | null;
	syntheticCenter?: { lon: number; lat: number };
	geoSource?: string;
	geoWarnings?: string[];
	/** Local solve state. Present only for parsed case files, never for .pwd display entries. */
	network?: Network | null;
	baseSolution?: Solution | null;
	solution?: Solution | null;
	sensitivity?: SensitivityColumn | null;
	deltas?: DemandDeltas;
	solving?: boolean;
	solveMs?: number | null;
	solveBackend?: SolveBackend | null;
	solveFallbackReason?: string | null;
	solveSeq?: number;
	sensitivitySeq?: number;
	predictedObjective?: number | null;
	/** Present for a PowerWorld .pwd display only entry. */
	substations?: LocalSubstations;
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
	solving = $state(false);
	solveMs = $state<number | null>(null);
	solveBackend = $state<SolveBackend | null>(null);
	solveFallbackReason = $state<string | null>(null);
	/** Monotone token: only the latest solve may write this case. */
	solveSeq = 0;
	/** Monotone token: only the latest sensitivity request may write this case. */
	sensitivitySeq = 0;
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

	updateLocal(id: string, patch: Partial<LocalCase>) {
		// Mutate the existing entry in place so its object identity stays stable.
		// The solve and sensitivity seq tokens live on the LocalCase object, and an
		// in-flight solve closure holds that same reference; replacing the object
		// here would detach those closures from the live seq, letting a stale solve
		// clobber a newer one. Reassign the array (raw state) to fire reactivity.
		const existing = this.localCases.find((c) => c.id === id);
		if (!existing) return;
		Object.assign(existing, patch);
		this.localCases = [...this.localCases];
	}

	removeCase(id: string) {
		const wasActive = this.activeCaseId === id;
		this.cases = this.cases.filter((c) => c.id !== id);
		if (!wasActive) return;

		this.selectedBus = null;
		this.previewDeltaMw = null;
		this.previewActive = false;
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
