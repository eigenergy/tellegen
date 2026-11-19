/** Dedicated RMS worker: its own wasm instance (the same, HTTP-cached
 * module the engine worker loads) and the prepared RmsCase handles. The
 * host terminates it to stop a run; the static-study worker is untouched.
 * A trap (a Rust panic or a failed allocation) leaves the instance
 * undefined, so the worker answers and closes itself, as worker.ts does. */

import { errorText } from "./errors.js";
import type {
  RmsRunValue,
  RmsWorkerRequest,
  RmsWorkerResponse,
} from "./rms-protocol.js";
import { clearLiveSessions, executeTool } from "./rms-tools-worker.js";
import * as wasm from "./wasm-pkg/tellegen.js";

type Wasm = typeof wasm;
type Case = InstanceType<Wasm["RmsCase"]>;

let ready: Promise<Wasm> | null = null;
const cases = new Map<number, Case>();

function module(url?: string): Promise<Wasm> {
  ready ??= wasm
    .default({
      module_or_path:
        url ?? new URL("./wasm-pkg/tellegen_bg.wasm", import.meta.url).href,
    })
    .then(() => wasm)
    .catch((e) => {
      ready = null;
      throw e;
    });
  return ready;
}

const scope = globalThis as unknown as {
  onmessage: ((ev: MessageEvent<RmsWorkerRequest>) => void) | null;
  postMessage(msg: RmsWorkerResponse, transfer?: Transferable[]): void;
  close(): void;
};

function handle(id: number): Case {
  const c = cases.get(id);
  if (!c) throw new Error(`no open RMS case ${id}`);
  return c;
}

// Experimental exports exist only in a build with rms-experimental.
type Experimental = {
  rms_dynamize?: (m: string, d: string, r: string) => string;
  rms_library?: () => string;
};

async function execute(
  mod: Wasm,
  req: RmsWorkerRequest,
): Promise<{ value: unknown; transfer: Transferable[] }> {
  switch (req.op) {
    case "support":
      return {
        value: {
          rms: typeof mod.rms_supported === "function" && mod.rms_supported(),
          experimental:
            typeof mod.rms_experimental_supported === "function" &&
            mod.rms_experimental_supported(),
        },
        transfer: [],
      };
    case "looks_like_dyr":
      return { value: mod.rms_looks_like_dyr(req.text), transfer: [] };
    case "open": {
      cases.get(req.handle)?.free();
      cases.set(
        req.handle,
        new mod.RmsCase(req.module_json, req.dyr_text, req.options_json),
      );
      return { value: handle(req.handle).inspection(), transfer: [] };
    }
    case "inspect":
      return { value: handle(req.handle).inspection(), transfer: [] };
    case "flat_check":
      return {
        value: handle(req.handle).flat_check(req.seconds),
        transfer: [],
      };
    case "run": {
      const buffers = handle(req.handle).run(req.request_json, (t: number) =>
        scope.postMessage({ id: req.id, progress: { t } }),
      );
      const value: RmsRunValue = {
        meta: buffers.meta(),
        time: buffers.time(),
        values: buffers.values(),
      };
      buffers.free();
      return { value, transfer: [value.time.buffer, value.values.buffer] };
    }
    case "linear": {
      const c = handle(req.handle) as Case & {
        linear?: (r: string) => string;
      };
      if (typeof c.linear !== "function")
        throw new Error("small-signal analysis is not in this engine build");
      return { value: c.linear(req.request_json), transfer: [] };
    }
    case "free":
      cases.get(req.handle)?.free();
      cases.delete(req.handle);
      return { value: null, transfer: [] };
    case "dynamize": {
      const x = mod as unknown as Experimental;
      if (typeof x.rms_dynamize !== "function")
        throw new Error("default dynamics are not in this engine build");
      return {
        value: x.rms_dynamize(req.module_json, req.dyr_text, req.request_json),
        transfer: [],
      };
    }
    case "library": {
      const x = mod as unknown as Experimental;
      if (typeof x.rms_library !== "function")
        throw new Error("the model library is not in this engine build");
      return { value: x.rms_library(), transfer: [] };
    }
    default:
      return executeTool(mod, req);
  }
}

scope.onmessage = async (ev: MessageEvent<RmsWorkerRequest>) => {
  const req = ev.data;
  try {
    const mod = await module(req.wasm_url);
    const started = performance.now();
    const { value, transfer } = await execute(mod, req);
    scope.postMessage(
      { id: req.id, ok: true, value, worker_ms: performance.now() - started },
      transfer,
    );
  } catch (e) {
    const fatal = e instanceof WebAssembly.RuntimeError;
    scope.postMessage({ id: req.id, ok: false, error: errorText(e), fatal });
    if (fatal) {
      cases.clear();
      clearLiveSessions();
      ready = null;
      scope.close();
    }
  }
};
