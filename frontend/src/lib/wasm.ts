/** Lazy loader for the powerio wasm module. Nothing downloads until the
 * first file is dropped; the dropped file is parsed in the browser and never
 * leaves the machine. */
import type { DemandDeltas, NetworkBranch, NetworkBus, SensitivityColumn, Solution } from './api';
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
			return {
				...parseSolveOutput(caseId, (await powerio()).solve_dc(networkJson, baseRequest)),
				sensitivityError: errorText(e)
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
	return { solution, sensitivity };
}

function errorText(e: unknown): string {
	return e instanceof Error ? e.message : String(e);
}
