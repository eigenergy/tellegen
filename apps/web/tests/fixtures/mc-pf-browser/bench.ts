/** Browser benchmark for retained multiconductor PF sessions (#132).
 *
 * `mc-pf-bench.spec.ts` drives these steps one at a time so it can sample
 * CDP heap metrics between them. The end-to-end steps use the public engine
 * exactly as the Svelte controller does; the breakdown steps use a dedicated
 * worker that calls the wasm `McPfSession` directly. */

import {
	createMcPfSession,
	preloadEngine,
	type BrowserMcPfSession,
	type McLoadBranchState,
	type McLoadPowerEdit,
	type McPfResult
} from '@tellegen/engine';

/** The Svelte controller's load-edit debounce, excluded from every timing. */
const CONTROLLER_DEBOUNCE_MS = 180;

/** SplitMix64, identical to `benchmarks::mc_feeder::SplitMix64`. */
class SplitMix64 {
	private state: bigint;
	constructor(seed: bigint) {
		this.state = BigInt.asUintN(64, seed);
	}
	nextU64(): bigint {
		this.state = BigInt.asUintN(64, this.state + 0x9e3779b97f4a7c15n);
		let z = this.state;
		z = BigInt.asUintN(64, (z ^ (z >> 30n)) * 0xbf58476d1ce4e5b9n);
		z = BigInt.asUintN(64, (z ^ (z >> 27n)) * 0x94d049bb133111ebn);
		return z ^ (z >> 31n);
	}
	nextF64(): number {
		return Number(this.nextU64() >> 11n) / 2 ** 53;
	}
	uniform(lo: number, hi: number): number {
		return lo + (hi - lo) * this.nextF64();
	}
	below(n: number): number {
		return Number((this.nextU64() * BigInt(n)) >> 64n);
	}
	chance(probability: number): boolean {
		return this.nextF64() < probability;
	}
}

/** The native benchmark's deterministic single-branch edit sequence. */
function editSequence(branches: McLoadBranchState[], count: number, seed: bigint) {
	const rng = new SplitMix64(seed);
	const factor = () => {
		const magnitude = rng.uniform(0.05, 0.2);
		return rng.chance(0.5) ? 1 + magnitude : 1 - magnitude;
	};
	const edits: McLoadPowerEdit[] = [];
	for (let i = 0; i < count; i++) {
		const branch = branches[rng.below(branches.length)];
		const p = factor();
		const q = factor();
		edits.push({
			load: branch.load,
			branch: branch.branch,
			p_w: branch.base_p_w * p,
			q_var: branch.base_q_var * q
		});
	}
	return edits;
}

/** Accumulate single edits into the absolute set the UI sends. */
class EditSet {
	private readonly position = new Map<string, number>();
	readonly edits: McLoadPowerEdit[] = [];
	apply(edit: McLoadPowerEdit) {
		const key = `${edit.load}\u0000${edit.branch}`;
		const at = this.position.get(key);
		if (at === undefined) {
			this.position.set(key, this.edits.length);
			this.edits.push({ ...edit });
		} else {
			this.edits[at] = { ...edit };
		}
	}
}

const nextFrame = () => new Promise<void>((resolve) => requestAnimationFrame(() => resolve()));

interface State {
	moduleUrl: string;
	moduleJson: string | null;
	editCount: number;
	sequence: McLoadPowerEdit[] | null;
	session: BrowserMcPfSession | null;
	/** Retained like the controller's `c.result` and `c.mcLoadBranches`. */
	result: McPfResult | null;
	branches: McLoadBranchState[];
	e2eEdits: EditSet;
	breakdownEdits: EditSet;
}

const state: State = {
	moduleUrl: '',
	moduleJson: null,
	editCount: 0,
	sequence: null,
	session: null,
	result: null,
	branches: [],
	e2eEdits: new EditSet(),
	breakdownEdits: new EditSet()
};

function sequence(): McLoadPowerEdit[] {
	state.sequence ??= editSequence(state.branches, state.editCount, 132n);
	return state.sequence;
}

// --- Dedicated breakdown worker -------------------------------------------

interface WorkerReply {
	id: number;
	ok: boolean;
	error?: string;
	text?: string;
	wasm_ms?: number;
	post_abs: number;
	worker_ms: number;
	request_transfer_ms: number;
	wasm_memory_bytes: number | null;
	worker_js_heap_bytes: number | null;
	[key: string]: unknown;
}

let worker: Worker | null = null;
let workerSeq = 0;
type TimedReply = WorkerReply & { sent_abs: number; recv_abs: number };
const pending = new Map<number, (reply: WorkerReply & { recv_abs: number }) => void>();

function callWorker(op: string, payload: Record<string, unknown> = {}) {
	worker ??= (() => {
		const created = new Worker(new URL('./bench-worker.ts', import.meta.url), { type: 'module' });
		created.onmessage = (event: MessageEvent<WorkerReply>) => {
			const recv_abs = performance.timeOrigin + performance.now();
			const resolve = pending.get(event.data.id);
			pending.delete(event.data.id);
			resolve?.({ ...event.data, recv_abs });
		};
		return created;
	})();
	const id = ++workerSeq;
	return new Promise<TimedReply>((resolve, reject) => {
		const sent_abs = performance.timeOrigin + performance.now();
		pending.set(id, (reply) =>
			reply.ok ? resolve({ ...reply, sent_abs }) : reject(new Error(reply.error))
		);
		worker!.postMessage({ id, op, ...payload, sent_abs });
	});
}

/** One string-returning wasm call: wasm time, size, transfer, and parse.
 *
 * `transfer_ms` subtracts timeOrigin-aligned timestamps from two clocks, so
 * it carries their alignment error (about a millisecond; small negative values
 * mean "below resolution"). `messaging_ms` is offset free: the main-thread
 * round trip minus the worker-clock processing time, i.e. the request and
 * reply postMessage costs together. */
async function measuredCall(op: string, payload: Record<string, unknown> = {}, parse = true) {
	const started = performance.now();
	const reply = await callWorker(op, payload);
	const text = reply.text ?? '';
	const transfer_ms = reply.recv_abs - reply.post_abs;
	const messaging_ms = reply.recv_abs - reply.sent_abs - reply.worker_ms;
	let parse_ms: number | null = null;
	let parsed: unknown = null;
	if (parse) {
		const parseStart = performance.now();
		parsed = JSON.parse(text);
		parse_ms = performance.now() - parseStart;
	}
	return {
		parsed,
		timing: {
			wasm_ms: reply.wasm_ms ?? null,
			chars: text.length,
			request_transfer_ms: reply.request_transfer_ms,
			transfer_ms,
			messaging_ms,
			parse_ms,
			total_ms: performance.now() - started,
			wasm_memory_bytes: reply.wasm_memory_bytes,
			worker_js_heap_bytes: reply.worker_js_heap_bytes
		}
	};
}

// --- Steps ----------------------------------------------------------------

const bench = {
	controllerDebounceMs: CONTROLLER_DEBOUNCE_MS,

	async load(moduleUrl: string, editsUrl: string | null, editCount: number) {
		const started = performance.now();
		const response = await fetch(moduleUrl);
		if (!response.ok) throw new Error(`cannot fetch ${moduleUrl}: ${response.status}`);
		state.moduleJson = await response.text();
		const fetch_ms = performance.now() - started;
		state.moduleUrl = moduleUrl;
		state.editCount = editCount;
		state.sequence = null;
		let edits_source = 'splitmix64';
		if (editsUrl) {
			const document = (await (await fetch(editsUrl)).json()) as { edits: McLoadPowerEdit[] };
			state.sequence = document.edits.slice(0, editCount);
			edits_source = 'native edits file';
		}
		// Spawn the engine worker and instantiate wasm up front, as an app that
		// has already ingested the case would have; cold creation excludes it.
		const preloadStart = performance.now();
		await preloadEngine();
		const engine_preload_ms = performance.now() - preloadStart;
		return { fetch_ms, module_chars: state.moduleJson.length, edits_source, engine_preload_ms };
	},

	/** Cold: exactly the controller's `createMcPfSession` + `result`/`loadBranches`. */
	async e2eCreate() {
		if (state.moduleJson === null) throw new Error('load a module first');
		const started = performance.now();
		const session = await createMcPfSession(state.moduleJson);
		const created = performance.now();
		const [result, branches] = await Promise.all([session.result(), session.loadBranches()]);
		const fetched = performance.now();
		await nextFrame();
		state.session = session;
		state.result = result;
		state.branches = branches;
		state.e2eEdits = new EditSet();
		sequence();
		return {
			create_ms: created - started,
			result_and_load_branches_ms: fetched - created,
			ui_visible_ms: performance.now() - started,
			iterations: result.iterations,
			converged: result.converged,
			factorization_count: result.factorization_count,
			matrix_dimension: result.matrix_dimension,
			matrix_nonzeros: result.matrix_nonzeros,
			min_voltage_pu: result.min_voltage_pu,
			terminals: result.terminals.length,
			load_branches: branches.length
		};
	},

	/** One debounced flush, as `flushMultiLoadPowers` runs it, plus one frame. */
	async e2eEdit(index: number) {
		const session = state.session;
		if (!session) throw new Error('create the session first');
		state.e2eEdits.apply(sequence()[index]);
		const edits = state.e2eEdits.edits.map((edit) => ({ ...edit }));
		const started = performance.now();
		const result = await session.replaceLoadPowers(edits);
		const replaced = performance.now();
		const branches = await session.loadBranches();
		const listed = performance.now();
		state.result = result;
		state.branches = branches;
		await nextFrame();
		return {
			index,
			accumulated_edits: edits.length,
			replace_load_powers_ms: replaced - started,
			load_branches_ms: listed - replaced,
			frame_ms: performance.now() - listed,
			ui_visible_ms: performance.now() - started,
			iterations: result.iterations,
			factorization_count: result.factorization_count
		};
	},

	async e2eFeederWide() {
		const session = state.session;
		if (!session) throw new Error('create the session first');
		const edits = state.branches.map((branch) => ({
			load: branch.load,
			branch: branch.branch,
			p_w: branch.base_p_w * 1.05,
			q_var: branch.base_q_var * 1.05
		}));
		const started = performance.now();
		const result = await session.replaceLoadPowers(edits);
		const replaced = performance.now();
		const branches = await session.loadBranches();
		const listed = performance.now();
		state.result = result;
		state.branches = branches;
		await nextFrame();
		return {
			edited_branches: edits.length,
			replace_load_powers_ms: replaced - started,
			load_branches_ms: listed - replaced,
			ui_visible_ms: performance.now() - started,
			iterations: result.iterations,
			factorization_count: result.factorization_count
		};
	},

	async e2eMaterialize() {
		const session = state.session;
		if (!session) throw new Error('create the session first');
		let started = performance.now();
		const input = await session.inputModule();
		const input_module_ms = performance.now() - started;
		started = performance.now();
		const snapshot = await session.snapshot('mc-pf-bench', 'Browser session benchmark');
		const snapshot_ms = performance.now() - started;
		return {
			input_module_ms,
			input_module_chars: input.length,
			snapshot_ms,
			snapshot_input_module_chars: snapshot.input_module.length,
			snapshot_solution_module_chars: snapshot.solution_module.length
		};
	},

	e2eFree() {
		state.session?.free();
		state.session = null;
		state.result = null;
		return true;
	},

	async breakdownInit() {
		const reply = await callWorker('init');
		return {
			init_ms: reply.init_ms,
			wasm_memory_bytes: reply.wasm_memory_bytes,
			worker_js_heap_bytes: reply.worker_js_heap_bytes
		};
	},

	async breakdownCreate() {
		const started = performance.now();
		const reply = await callWorker('create', { url: state.moduleUrl });
		const created_ms = performance.now() - started;
		const result = await measuredCall('result');
		const branches = await measuredCall('load_branches');
		state.result = result.parsed as McPfResult;
		state.breakdownEdits = new EditSet();
		return {
			worker_fetch_ms: reply.fetch_ms,
			wasm_create_ms: reply.create_ms,
			create_roundtrip_ms: created_ms,
			wasm_memory_bytes_after_create: reply.wasm_memory_bytes,
			result: result.timing,
			load_branches: branches.timing
		};
	},

	async breakdownEdit(index: number) {
		state.breakdownEdits.apply(sequence()[index]);
		const editsJson = JSON.stringify(state.breakdownEdits.edits);
		const started = performance.now();
		const replaced = await measuredCall('edit', { edits: editsJson });
		const branches = await measuredCall('load_branches');
		const result = replaced.parsed as McPfResult;
		state.result = result;
		return {
			index,
			accumulated_edits: state.breakdownEdits.edits.length,
			edits_json_chars: editsJson.length,
			replace: replaced.timing,
			load_branches: branches.timing,
			total_ms: performance.now() - started,
			iterations: result.iterations,
			factorization_count: result.factorization_count
		};
	},

	async breakdownMaterialize() {
		const input = await measuredCall('input_module', {}, false);
		const snapshot = await measuredCall('snapshot');
		const memory = await callWorker('memory');
		return {
			input_module: input.timing,
			snapshot: snapshot.timing,
			wasm_memory_bytes: memory.wasm_memory_bytes,
			worker_js_heap_bytes: memory.worker_js_heap_bytes
		};
	},

	async breakdownFree() {
		await callWorker('free');
		worker?.terminate();
		worker = null;
		state.result = null;
		return true;
	}
};

(window as unknown as { mcBench: typeof bench }).mcBench = bench;
const status = document.querySelector<HTMLElement>('#status');
if (status) status.textContent = 'ready';
