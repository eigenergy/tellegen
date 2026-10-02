/** Dedicated benchmark worker: drives the wasm `McPfSession` directly, so
 * each wasm call, the result-string transfer, and the main-thread parse can
 * be timed separately. Every reply carries `post_abs`, the worker's
 * `performance.timeOrigin + performance.now()` immediately before
 * `postMessage`, so the main thread can subtract its own aligned receive time,
 * and `worker_ms`, the worker-clock time from receipt to post, so the main
 * thread can also derive an offset-free messaging overhead.
 *
 * A wasm call's time includes the numerical solve, today's network clone and
 * instance rebuild, result construction, JSON serialization inside wasm, and
 * the UTF-8 decode into a JS string; these are inseparable from JS. */

import init, { McPfSession } from '../../../../../packages/engine/src/wasm-pkg/tellegen.js';

type Request =
	| { id: number; op: 'init'; sent_abs: number }
	| { id: number; op: 'create'; url: string; sent_abs: number }
	| { id: number; op: 'result'; sent_abs: number }
	| { id: number; op: 'load_branches'; sent_abs: number }
	| { id: number; op: 'edit'; edits: string; sent_abs: number }
	| { id: number; op: 'input_module'; sent_abs: number }
	| { id: number; op: 'snapshot'; sent_abs: number }
	| { id: number; op: 'memory'; sent_abs: number }
	| { id: number; op: 'free'; sent_abs: number };

const scope = globalThis as unknown as {
	onmessage: ((event: MessageEvent<Request>) => void) | null;
	postMessage(message: unknown): void;
};

let memory: WebAssembly.Memory | null = null;
let session: McPfSession | null = null;

const now = () => performance.timeOrigin + performance.now();

function jsHeap(): number | null {
	const value = (performance as unknown as { memory?: { usedJSHeapSize: number } }).memory;
	return value ? value.usedJSHeapSize : null;
}

function live(): McPfSession {
	if (!session) throw new Error('no benchmark session');
	return session;
}

/** Time one wasm call that returns a string. */
function timed(call: () => string): { text: string; wasm_ms: number } {
	const started = performance.now();
	const text = call();
	return { text, wasm_ms: performance.now() - started };
}

scope.onmessage = async (event: MessageEvent<Request>) => {
	const request = event.data;
	const received_abs = now();
	const base = { id: request.id, request_transfer_ms: received_abs - request.sent_abs };
	try {
		let reply: Record<string, unknown>;
		switch (request.op) {
			case 'init': {
				const started = performance.now();
				const output = await init();
				memory = output.memory;
				reply = { init_ms: performance.now() - started };
				break;
			}
			case 'create': {
				const fetched = performance.now();
				const text = await (await fetch(request.url)).text();
				const started = performance.now();
				session?.free();
				session = new McPfSession(text, '{}');
				reply = {
					fetch_ms: started - fetched,
					module_chars: text.length,
					create_ms: performance.now() - started
				};
				break;
			}
			case 'result':
				reply = timed(() => live().result());
				break;
			case 'load_branches':
				reply = timed(() => live().load_branches());
				break;
			case 'edit':
				reply = timed(() => live().replace_load_powers(request.edits));
				break;
			case 'input_module':
				reply = timed(() => live().input_module());
				break;
			case 'snapshot':
				reply = timed(() => live().snapshot('mc-pf-bench', 'Browser session benchmark'));
				break;
			case 'memory':
				reply = {};
				break;
			case 'free':
				session?.free();
				session = null;
				reply = {};
				break;
		}
		const wasm_memory_bytes = memory?.buffer.byteLength ?? null;
		const worker_js_heap_bytes = jsHeap();
		const post_abs = now();
		scope.postMessage({
			...base,
			ok: true,
			...reply,
			wasm_memory_bytes,
			worker_js_heap_bytes,
			worker_ms: post_abs - received_abs,
			post_abs
		});
	} catch (error) {
		scope.postMessage({ ...base, ok: false, error: String(error), post_abs: now() });
	}
};
