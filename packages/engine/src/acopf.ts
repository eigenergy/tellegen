import { engineError, errorCode } from "./errors.js";
import { validateApiResult } from "./result-validation.js";
import type { SensRequest, SolveResponse } from "./generated/api-types.js";
import type {
  AcOpfWorkerRequest,
  AcOpfWorkerResponse,
} from "./acopf-protocol.js";
import { assertEngineInputLength } from "./input-limit.js";

interface WorkerLike {
  onmessage: ((event: MessageEvent<AcOpfWorkerResponse>) => void) | null;
  onerror: ((event: ErrorEvent) => void) | null;
  onmessageerror: ((event: MessageEvent) => void) | null;
  postMessage(message: AcOpfWorkerRequest): void;
  terminate(): void;
}

export type AcOpfWorkerFactory = () => WorkerLike;

const defaultWorkerFactory: AcOpfWorkerFactory = () =>
  new Worker(new URL("./acopf-worker.js", import.meta.url), { type: "module" });

/** The one worker kept alive between calls. It holds the compiled wasm module,
 * so a later solve skips the worker start, the fetch and the compile. `busy`
 * is true while it answers a request. */
interface KeptWorker {
  factory: AcOpfWorkerFactory;
  worker: WorkerLike;
  busy: boolean;
}

let keptWorker: KeptWorker | null = null;

function detach(worker: WorkerLike) {
  worker.onmessage = null;
  worker.onerror = null;
  worker.onmessageerror = null;
}

/** Terminate the kept worker if it is idle, freeing the compiled module. The
 * next call starts a fresh one. Returns whether a worker was terminated. */
export function releaseAcOpfWorker(): boolean {
  const kept = keptWorker;
  if (!kept || kept.busy) return false;
  keptWorker = null;
  detach(kept.worker);
  kept.worker.terminate();
  return true;
}

function workerCall(
  request: AcOpfWorkerRequest,
  signal?: AbortSignal,
  workerFactory: AcOpfWorkerFactory = defaultWorkerFactory,
): Promise<AcOpfWorkerResponse> {
  signal?.throwIfAborted();
  if (typeof Worker === "undefined" && workerFactory === defaultWorkerFactory) {
    return Promise.reject(
      new Error("AC OPF requires a browser with dedicated Worker support"),
    );
  }
  return new Promise((resolve, reject) => {
    // Reuse the idle kept worker when it came from the same factory, and
    // replace it when it came from another. A call made while it is busy runs
    // in a worker of its own, which is terminated when that call ends.
    if (keptWorker && !keptWorker.busy && keptWorker.factory !== workerFactory) {
      releaseAcOpfWorker();
    }
    let kept: KeptWorker | null = null;
    let worker: WorkerLike;
    if (keptWorker && !keptWorker.busy) {
      kept = keptWorker;
      worker = kept.worker;
    } else {
      try {
        worker = workerFactory();
      } catch (error) {
        reject(error);
        return;
      }
      if (!keptWorker) {
        kept = { factory: workerFactory, worker, busy: false };
        keptWorker = kept;
      }
    }
    if (kept) kept.busy = true;

    /** End the call. A worker that answered stays alive for the next call;
     * one that was cancelled or failed is terminated, because a synchronous
     * solve cannot be interrupted any other way. */
    let ended = false;
    const finish = (keep: boolean): boolean => {
      if (ended) return false;
      ended = true;
      signal?.removeEventListener("abort", abort);
      detach(worker);
      if (kept && keep) {
        kept.busy = false;
        return true;
      }
      if (kept && keptWorker === kept) keptWorker = null;
      worker.terminate();
      return true;
    };
    const abort = () => {
      finish(false);
      reject(new DOMException("AC OPF solve cancelled", "AbortError"));
    };
    signal?.addEventListener("abort", abort, { once: true });
    if (signal?.aborted) {
      abort();
      return;
    }
    // Validation may first import its validators, so a cancellation can
    // arrive while it runs; `finish` then reports that the call has ended.
    const settle = async (data: AcOpfWorkerResponse) => {
      try {
        if (!data || typeof data !== "object")
          throw new Error("AC OPF worker returned an invalid message");
        switch (data.type) {
          case "result":
            await validateApiResult<SolveResponse>("solve", data.response);
            break;
          case "joint_result":
            await validateApiResult("ac_dc", data.response);
            break;
          case "ready":
            break;
          case "error":
            if (typeof data.message !== "string" ||
                (data.code !== undefined && errorCode(data) === undefined))
              throw new Error("AC OPF worker returned an invalid error");
            break;
          default:
            throw new Error("AC OPF worker returned an unknown message type");
        }
      } catch (error) {
        if (finish(false)) reject(error);
        return;
      }
      if (!finish(true)) return;
      if (data.type === "error") reject(engineError(data.message, data.code));
      else resolve(data);
    };
    worker.onmessage = ({ data }) => void settle(data);
    worker.onerror = () => {
      finish(false);
      reject(new Error("AC OPF worker failed"));
    };
    worker.onmessageerror = () => {
      finish(false);
      reject(new Error("AC OPF worker returned an unreadable message"));
    };
    try {
      worker.postMessage(request);
    } catch (error) {
      finish(false);
      reject(error);
    }
  });
}

/** Probe the optional solver asset in the same worker/WASI environment a
 * solve uses. A missing asset, unsupported browser, or instantiation failure
 * returns false and leaves the formulation disabled. A successful probe leaves
 * its worker, with the compiled module, ready for the first solve. */
export async function probeAcOpfWorker(
  wasmUrl: string,
  workerFactory?: AcOpfWorkerFactory,
  signal?: AbortSignal,
): Promise<boolean> {
  try {
    const response = await workerCall(
      { type: "probe", wasmUrl },
      signal,
      workerFactory,
    );
    if (response.type === "ready") return true;
  } catch {
    // Fall through: the formulation stays disabled.
  }
  releaseAcOpfWorker();
  return false;
}

/** Run one nonlinear AC OPF in a dedicated worker, which is kept for later
 * solves. Cancellation always terminates that worker, and the next solve
 * starts a fresh one; there is deliberately no main-thread fallback. */
export async function solveAcOpfModule(
  wasmUrl: string,
  moduleJson: string,
  signal?: AbortSignal,
  workerFactory?: AcOpfWorkerFactory,
  sensitivities?: SensRequest[],
): Promise<SolveResponse> {
  assertEngineInputLength(moduleJson.length);
  const response = await workerCall(
    {
      type: "solve",
      wasmUrl,
      moduleJson,
      requestJson: JSON.stringify({ formulation: "acopf", sensitivities }),
    },
    signal,
    workerFactory,
  );
  if (response.type !== "result") {
    throw new Error("AC OPF worker returned no solve result");
  }
  return response.response;
}

/** Joint AC/DC economic dispatch in the separate optional nonlinear worker. */
export async function solveJointAcDcModule(
  wasmUrl: string,
  moduleJson: string,
  signal?: AbortSignal,
  workerFactory?: AcOpfWorkerFactory,
): Promise<import("./dc-grid.js").AcDcSolution> {
  assertEngineInputLength(moduleJson.length);
  const response = await workerCall(
    {
      type: "solve",
      wasmUrl,
      moduleJson,
      requestJson: JSON.stringify({ formulation: "acopf", ac_dc: true }),
    },
    signal,
    workerFactory,
  );
  if (response.type !== "joint_result")
    throw new Error("AC/DC worker returned no joint result");
  return response.response;
}
