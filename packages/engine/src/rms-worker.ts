/** Dedicated RMS worker: its own wasm instance (the same, HTTP-cached
 * module the engine worker loads) and the prepared RmsCase handles. The
 * host terminates it to stop a run; the static-study worker is untouched.
 * A trap (a Rust panic or a failed allocation) leaves the instance
 * undefined, so the worker answers and closes itself, as worker.ts does.
 * @block RMS worker */

import { errorText } from "./errors.js";
import type {
  RmsRunValue,
  RmsWorkerRequest,
  RmsWorkerResponse,
} from "./rms-protocol.js";
import { clearLiveSessions, executeTool } from "./rms-tools-worker.js";
import * as wasm from "./wasm-pkg/tellegen.js";

type Wasm = typeof wasm;

/** The parts of the threaded engine build (`wasm-pkg-mt`) this worker
 * calls. That build is optional and made apart from `npm run wasm`, so it is
 * loaded by URL at run time and not type-checked against. */
interface ThreadedBuild {
  default(init: { module_or_path: WebAssembly.Module }): Promise<{ memory: WebAssembly.Memory }>;
  rms_threads_init(global: number, relays: number): void;
}

/** Experimental: the engine build with SIMD and threads (shared memory):
 * the module, its memory shared with one Web Worker per pool thread, and
 * the pools started before the first call. */
async function threaded(): Promise<Wasm> {
  const mt = (await import(
    /* @vite-ignore */ new URL("./wasm-pkg-mt/tellegen.js", import.meta.url).href
  )) as ThreadedBuild;
  const url = new URL("./wasm-pkg-mt/tellegen_bg.wasm", import.meta.url);
  const module = await WebAssembly.compileStreaming(fetch(url));
  const out = await mt.default({ module_or_path: module });
  const memory = out.memory;
  const starting: Promise<void>[] = [];
  (globalThis as unknown as { __tellegenSpawnRmsThread: (ptr: number) => void })
    .__tellegenSpawnRmsThread = (ptr: number) => {
    const w = new Worker(new URL("./rms-thread.js", import.meta.url), {
      type: "module",
      name: "tellegen-rms-thread",
    });
    starting.push(
      new Promise<void>((resolve, reject) => {
        w.onmessage = () => resolve();
        w.onerror = (e) => reject(new Error(e.message || "an RMS thread failed to start"));
      }),
    );
    w.postMessage({ module, memory, ptr });
  };
  const cores = navigator.hardwareConcurrency || 4;
  mt.rms_threads_init(Math.max(0, Math.min(3, cores - 3)), 0);
  await Promise.all(starting);
  return mt as unknown as Wasm;
}

/** The build this worker runs: the threaded one when it was started for it
 * (`tellegen-rms-mt`) on a cross-origin isolated page, else the plain one. */
async function load(url?: string): Promise<Wasm> {
  const g = globalThis as unknown as { name?: string; crossOriginIsolated?: boolean };
  if (!url && g.name === "tellegen-rms-mt" && g.crossOriginIsolated) {
    try {
      return await threaded();
    } catch (e) {
      console.warn("the RMS engine build with threads is unavailable:", e);
    }
  }
  await wasm.default({
    module_or_path: url ?? new URL("./wasm-pkg/tellegen_bg.wasm", import.meta.url).href,
  });
  return wasm;
}
type Case = InstanceType<Wasm["RmsCase"]>;

let ready: Promise<Wasm> | null = null;
const cases = new Map<number, Case>();

function module(url?: string): Promise<Wasm> {
  ready ??= load(url)
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
          protection: (() => {
            const f = (mod as unknown as { protection_supported?: () => boolean })
              .protection_supported;
            return typeof f === "function" && f();
          })(),
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
