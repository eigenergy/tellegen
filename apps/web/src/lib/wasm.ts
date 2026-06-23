/** Lazy loader for the powerio wasm module. Nothing downloads until the
 * first file is dropped; the dropped file is parsed in the browser and never
 * leaves the machine. */
import type {
	DemandDeltas,
	NetworkBranch,
	NetworkBus,
	SensitivityColumn,
	Solution,
	SolveIteration
} from './api';
import wasmUrl from './wasm-pkg/tellegen_bg.wasm?url';
import sensWasmUrl from './wasm-sens-pkg/tellegen_sens_bg.wasm?url';

export interface CaseFileSummary {
	name: string;
	base_mva: number;
	n_bus: number;
	n_branch: number;
	n_gen: number;
	load_mw: number;
	gen_mw: number;
	has_coords: boolean;
	coords_kind: 'file' | 'synthetic_pending';
	warnings: string[];
}

export interface TopologyBus {
	id: number;
	demand_mw: number;
	gen_mw: number;
}

export interface TopologyBranch {
	id: number;
	from: number;
	to: number;
	rate_mw: number;
	status: number;
}

export interface Topology {
	buses: TopologyBus[];
	branches: TopologyBranch[];
}

/** One parse per dropped file: summary stats, plus map geometry when the
 * file carries coordinates and topology for synthetic placement otherwise. */
export interface IngestedCase extends CaseFileSummary {
	network_json: string;
	topology: Topology;
	view: { buses: NetworkBus[]; branches: NetworkBranch[] } | null;
}

let ready: Promise<typeof import('./wasm-pkg/tellegen')> | null = null;
let sensitivityReady: Promise<typeof import('./wasm-sens-pkg/tellegen_sens')> | null = null;
let sensitivityUnsupported: string | null = null;

function powerio() {
	ready ??= import('./wasm-pkg/tellegen')
		.then(async (mod) => {
			await mod.default({ module_or_path: wasmUrl });
			return mod;
		})
		.catch((e) => {
			// Don't cache a rejected load: a transient failure (chunk fetch or
			// instantiate) must not disable the feature for the whole session.
			ready = null;
			throw e;
		});
	return ready;
}

function powerioSensitivity() {
	if (sensitivityUnsupported) return Promise.reject(new Error(sensitivityUnsupported));
	sensitivityReady ??= import('./wasm-sens-pkg/tellegen_sens')
		.then(async (mod) => {
			await mod.default({ module_or_path: sensWasmUrl });
			return mod;
		})
		.catch((e) => {
			const message = errorText(e);
			sensitivityReady = null;
			if (isPermanentWasmLoadFailure(message)) sensitivityUnsupported = message;
			throw e;
		});
	return sensitivityReady;
}

function isPermanentWasmLoadFailure(message: string): boolean {
	// Latch only genuine browser-capability failures the sensitivity module can
	// never recover from in this browser: no WebAssembly, or an opcode the
	// engine rejects (the sens build uses relaxed SIMD). Transient fetch or
	// instantiate failures (offline, 503, aborted navigation) routinely carry
	// the .wasm URL or "Failed to fetch" in their message, so keying on the bare
	// word "wasm"/"compile" wrongly disables the feature for the whole session.
	// Those stay retryable, matching the powerio() loader.
	if (/Failed to fetch|NetworkError|load failed|aborted|ERR_/i.test(message)) return false;
	return /CompileError|LinkError|invalid opcode|unsupported|relaxed|WebAssembly is not defined/i.test(
		message
	);
}

/** powerio format token from a file name; null for non-case files. */
export function formatOf(name: string): string | null {
	const ext = name.split('.').pop()?.toLowerCase();
	return ext === 'm' || ext === 'raw' || ext === 'aux' ? ext : null;
}

export async function ingestCase(text: string, format: string): Promise<IngestedCase> {
	return JSON.parse((await powerio()).ingest_case(text, format));
}

/** Substations from a PowerWorld .pwd display file. x/y are diagram
 * coordinates as stored (not lat/lon); the caller projects them. */
export interface DisplayPreview {
	substations: { number: number; name: string; x: number; y: number }[];
	canvas_width: number;
	canvas_height: number;
}

/** True for binary display files (PowerWorld .pwd), read via parseDisplay.
 * Kept separate from formatOf: a .pwd is display data, not a case format. */
export function isDisplayFile(name: string): boolean {
	return name.split('.').pop()?.toLowerCase() === 'pwd';
}

export async function parseDisplay(bytes: Uint8Array): Promise<DisplayPreview> {
	return JSON.parse((await powerio()).parse_display(bytes, 'pwd'));
}

/** The browser DC solve: the exact solution plus, when `sensBus` is given, its
 * dLMP/dd column — the same shapes the server serves. */
export interface BrowserSolution {
	solution: Solution;
	sensitivity: SensitivityColumn | null;
	sensitivityError?: string;
	iterations: SolveIteration[];
}

/** Solve the DC OPF in the browser at demand = base + `deltas`. `networkJson` is
 * the raw powerio Network (from the `/case` endpoint or a browser parse). When
 * `sensBus` is set, the dLMP/dd column is loaded from the sensitivity wasm
 * package when the browser supports that module. */
export async function solveDc(
	caseId: string,
	networkJson: string,
	deltas: DemandDeltas,
	sensBus: number | null
): Promise<BrowserSolution> {
	const request = JSON.stringify({ deltas, sens_bus: sensBus });
	if (sensBus !== null) {
		try {
			return parseSolveOutput(caseId, (await powerioSensitivity()).solve_dc(networkJson, request));
		} catch (e) {
			const baseRequest = JSON.stringify({ deltas, sens_bus: null });
			const message = errorText(e);
			return {
				...parseSolveOutput(caseId, (await powerio()).solve_dc(networkJson, baseRequest)),
				// A permanent capability failure (the sens build's relaxed SIMD, which
				// Safari rejects) gets a plain language note; transient errors keep detail.
				sensitivityError: isPermanentWasmLoadFailure(message)
					? 'needs SIMD this browser does not support (try Chrome or Firefox)'
					: message
			};
		}
	}
	return parseSolveOutput(caseId, (await powerio()).solve_dc(networkJson, request));
}

function parseSolveOutput(caseId: string, json: string): BrowserSolution {
	const out = JSON.parse(json);
	const solution: Solution = {
		objective: out.objective,
		lmp: out.lmp,
		flows: out.flows,
		dispatch: out.dispatch
	};
	const d = out.dlmp_dd;
	const sensitivity: SensitivityColumn | null = d
		? {
				case: caseId,
				operand: d.operand,
				parameter: d.parameter,
				bus: d.bus,
				units: d.units,
				values: d.values
			}
		: null;
	return { solution, sensitivity, iterations: out.iterations ?? [] };
}

function errorText(e: unknown): string {
	return e instanceof Error ? e.message : String(e);
}

/** True when the sensitivity wasm module has failed to load in a way it can
 * never recover from in this browser (no WebAssembly, or the relaxed-SIMD
 * opcodes the sens build needs). The Study path uses the sens module, so the
 * caller must fall back to `solveDc`/the server when this is true. */
export function isPermanentSensFailure(message: string): boolean {
	return isPermanentWasmLoadFailure(message);
}

/** One in-place network mutation for the Study handle. `bus` is the original
 * bus id; `p_mw` is the demand delta in MW. */
interface NetworkEdit {
	kind: 'add_load';
	bus: number;
	p_mw: number;
}

/** A `NetworkEdit[]` for the wasm Study, dropping zero deltas so an unchanged
 * bus is never sent. */
function deltasToEdits(deltas: DemandDeltas): NetworkEdit[] {
	return Object.entries(deltas)
		.filter(([, mw]) => mw !== 0)
		.map(([bus, p_mw]) => ({ kind: 'add_load', bus: Number(bus), p_mw }));
}

/** The SolveResponse JSON the Study returns from commit/solution. A superset of
 * formulations; DC fills objective/lmp/flows/dispatch/iterations. */
interface StudySolveResponse {
	formulation: string;
	status: string;
	objective: number;
	iterations?: SolveIteration[];
	lmp?: { bus: number; value: number }[];
	flows?: { branch: number; pf: number; loading: number }[];
	dispatch?: { gen: number; pg: number }[];
	va?: { bus: number; value: number }[];
}

/** The Preview JSON the Study returns: first-order operand changes plus the
 * predicted objective change, with no re-solve. */
interface StudyPreview {
	operands: {
		operand: unknown;
		values: { element: { Bus?: number }; index: number; value: number }[];
		units: string;
	}[];
	objective_delta: number | null;
	local_only: boolean;
}

function solveResponseToSolution(out: StudySolveResponse): Solution {
	return {
		objective: out.objective,
		lmp: (out.lmp ?? []).map((e) => ({ bus: e.bus, usd_per_mwh: e.value })),
		flows: (out.flows ?? []).map((f) => ({ branch: f.branch, mw: f.pf, loading: f.loading })),
		dispatch: (out.dispatch ?? []).map((d) => ({ gen: d.gen, mw: d.pg }))
	};
}

/** Build-once browser transport for the reactive demand drag. The network is
 * parsed and the model built when the Study is created; `commit` exact-re-solves
 * and `preview` returns a first-order linearization, neither re-parsing the
 * network (unlike `solveDc`, which rebuilds the DcNetwork on every call). */
export class BrowserStudy {
	#study: import('./wasm-sens-pkg/tellegen_sens').Study;

	constructor(study: import('./wasm-sens-pkg/tellegen_sens').Study) {
		this.#study = study;
	}

	/** Exact DC solve at demand = committed + `deltas`, advancing the committed
	 * point. Returns the UI Solution and the interior-point iterates. */
	commit(deltas: DemandDeltas): { solution: Solution; iterations: SolveIteration[] } {
		const out: StudySolveResponse = JSON.parse(
			this.#study.commit(JSON.stringify(deltasToEdits(deltas)))
		);
		return { solution: solveResponseToSolution(out), iterations: out.iterations ?? [] };
	}

	/** First-order LMP preview for `deltas` at the committed point, with no
	 * re-solve: predicted per-bus ΔLMP and the predicted Δobjective. */
	preview(deltas: DemandDeltas): { lmp: { bus: number; usd_per_mwh: number }[]; objectiveDelta: number | null } {
		const out: StudyPreview = JSON.parse(
			this.#study.preview(JSON.stringify(deltasToEdits(deltas)), '[{"Price":"Active"}]')
		);
		const lmp = (out.operands[0]?.values ?? [])
			.filter((v) => v.element.Bus !== undefined)
			.map((v) => ({ bus: v.element.Bus as number, usd_per_mwh: v.value }));
		return { lmp, objectiveDelta: out.objective_delta };
	}

	/** Release the wasm Study; call when discarding it (e.g. the case's
	 * networkJson changed, or the case was removed). */
	free() {
		this.#study.free();
	}
}

/** Construct a build-once Study over `networkJson`, parsing the network and
 * solving the base case once. Throws if the sens module can't load; the caller
 * must catch and fall back (see `isPermanentSensFailure`). */
export async function createStudy(
	networkJson: string,
	formulation = 'dcopf'
): Promise<BrowserStudy> {
	const mod = await powerioSensitivity();
	return new BrowserStudy(new mod.Study(networkJson, formulation));
}
