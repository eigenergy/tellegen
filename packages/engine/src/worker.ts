/** Dedicated worker entry: owns the wasm instance and the live Study handles,
 * so solves, sensitivity columns, and previews run off the main thread. The
 * message handler attaches synchronously at module evaluation, so requests
 * posted while the wasm still loads queue instead of dropping. Handlers all
 * await the same init promise, so requests execute in arrival order.
 *
 * The wasm glue is imported statically: the worker only spawns on first
 * engine use, so laziness buys nothing here, and a static graph keeps the
 * worker a single chunk (Vite's default worker format cannot code-split). */

import { errorText, isPermanentWasmLoadFailure } from "./errors.js";
import type { WasmModule, WasmStudy } from "./module.js";
import {
  runRequest,
  type WorkerRequest,
  type WorkerResponse,
} from "./protocol.js";
import * as wasm from "./wasm-pkg/tellegen.js";

let wasmReady: Promise<WasmModule> | null = null;
let wasmUnsupported: string | null = null;

function wasmModule(): Promise<WasmModule> {
  if (wasmUnsupported) return Promise.reject(new Error(wasmUnsupported));
  const wasmUrl = new URL("./wasm-pkg/tellegen_bg.wasm", import.meta.url).href;
  wasmReady ??= wasm
    .default({ module_or_path: wasmUrl })
    .then(() => wasm)
    .catch((e) => {
      const message = errorText(e);
      // Don't cache a rejected init: a transient failure must not disable the
      // engine for the whole session. A capability failure is permanent.
      wasmReady = null;
      if (isPermanentWasmLoadFailure(message)) wasmUnsupported = message;
      throw e;
    });
  return wasmReady;
}

const studies = new Map<number, WasmStudy>();

// Typed view of the dedicated worker global; the package compiles against the
// DOM lib, so DedicatedWorkerGlobalScope is not in scope.
const scope = globalThis as unknown as {
  onmessage: ((ev: MessageEvent<WorkerRequest>) => void) | null;
  postMessage(msg: WorkerResponse): void;
  close(): void;
};

scope.onmessage = async (ev: MessageEvent<WorkerRequest>) => {
  const req = ev.data;
  try {
    const value = runRequest(await wasmModule(), studies, req);
    scope.postMessage({ id: req.id, ok: true, value });
  } catch (e) {
    // A Rust panic or a failed allocation is a wasm trap, and a trapped
    // instance is not recoverable: linear memory, the allocator, and every
    // live Study are undefined afterwards. Reporting it as an ordinary error
    // would leave the next solve running against that state and returning
    // numbers that look fine. Answer this request, then take the worker down
    // so the host spawns a clean one.
    const fatal = e instanceof WebAssembly.RuntimeError;
    scope.postMessage({ id: req.id, ok: false, error: errorText(e), fatal });
    if (fatal) {
      studies.clear();
      wasmReady = null;
      scope.close();
    }
  }
};
