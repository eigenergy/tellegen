/** Where engine requests execute. The worker host is the default in browsers:
 * the wasm instance lives in a dedicated module worker, so a multi-second
 * solve never blocks the page. The direct host runs the same requests on the
 * calling thread; it serves environments without workers (SSR, Node) and is
 * the replay target when the worker errors before its first response — the
 * failure mode of a browser that cannot run module workers. */

import { engineModule, type WasmStudy } from "./module.js";
import {
  runRequest,
  type EngineRequest,
  type WorkerRequest,
  type WorkerResponse,
} from "./protocol.js";

export interface EngineHost {
  call(req: EngineRequest): Promise<string | null>;
}

const directStudies = new Map<number, WasmStudy>();

export const directHost: EngineHost = {
  async call(req) {
    return runRequest(await engineModule(), directStudies, req);
  },
};

interface PendingCall {
  req: EngineRequest;
  resolve: (value: string | null) => void;
  reject: (error: Error) => void;
}

class WorkerHost implements EngineHost {
  #worker: Worker;
  #seq = 0;
  #pending = new Map<number, PendingCall>();
  #answered = false;
  /** Set once the worker is unusable: with a forward host when it never
   * answered (requests reroute there), without one when it crashed mid
   * session (requests reject; the next engineHost() spawns a fresh worker). */
  #failed: { forward: EngineHost | null; error: Error } | null = null;

  constructor(worker: Worker) {
    this.#worker = worker;
    worker.onmessage = (ev: MessageEvent<WorkerResponse>) => {
      this.#answered = true;
      const pending = this.#pending.get(ev.data.id);
      if (!pending) return;
      this.#pending.delete(ev.data.id);
      if (ev.data.ok) pending.resolve(ev.data.value);
      else pending.reject(new Error(ev.data.error));
    };
    worker.onerror = () => this.#fail(new Error("engine worker failed"));
    worker.onmessageerror = () =>
      this.#fail(new Error("engine worker message failed"));
  }

  #fail(error: Error) {
    if (this.#failed) return;
    const pending = [...this.#pending.values()];
    this.#pending.clear();
    this.#worker.terminate();
    if (!this.#answered) {
      // The worker never started (module workers unsupported, script blocked):
      // everything moves to the main thread, including the pending requests —
      // their study handles stay valid because the caller allocated them.
      this.#failed = { forward: directHost, error };
      if (activeHost === this) activeHost = directHost;
      for (const p of pending) directHost.call(p.req).then(p.resolve, p.reject);
      return;
    }
    // A crash mid session loses the worker's studies; callers see their next
    // call fail, drop the study handle, and rebuild against a fresh worker.
    this.#failed = { forward: null, error };
    if (activeHost === this) activeHost = null;
    for (const p of pending) p.reject(error);
  }

  call(req: EngineRequest): Promise<string | null> {
    const failed = this.#failed;
    if (failed) {
      return failed.forward
        ? failed.forward.call(req)
        : Promise.reject(failed.error);
    }
    return new Promise((resolve, reject) => {
      const id = ++this.#seq;
      this.#pending.set(id, { req, resolve, reject });
      const msg: WorkerRequest = { ...req, id };
      this.#worker.postMessage(msg);
    });
  }
}

let activeHost: EngineHost | null = null;

export function engineHost(): EngineHost {
  activeHost ??= createHost();
  return activeHost;
}

function createHost(): EngineHost {
  if (typeof Worker === "undefined") return directHost;
  try {
    return new WorkerHost(
      new Worker(new URL("./worker.js", import.meta.url), { type: "module" }),
    );
  } catch {
    return directHost;
  }
}
