import type { SolveResponse } from "./generated/contracts.js";
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
    let worker: WorkerLike;
    try {
      worker = workerFactory();
    } catch (error) {
      reject(error);
      return;
    }
    const finish = () => {
      signal?.removeEventListener("abort", abort);
      worker.terminate();
    };
    const abort = () => {
      finish();
      reject(new DOMException("AC OPF solve cancelled", "AbortError"));
    };
    signal?.addEventListener("abort", abort, { once: true });
    worker.onmessage = ({ data }) => {
      finish();
      if (data.type === "error") reject(new Error(data.message));
      else resolve(data);
    };
    worker.onerror = () => {
      finish();
      reject(new Error("AC OPF worker failed"));
    };
    worker.onmessageerror = () => {
      finish();
      reject(new Error("AC OPF worker returned an unreadable message"));
    };
    worker.postMessage(request);
  });
}

/** Probe the optional development asset in the same worker/WASI environment a
 * solve uses. A missing asset, unsupported browser, or instantiation failure
 * returns false and leaves the formulation disabled. */
export async function probeAcOpfWorker(
  wasmUrl: string,
  workerFactory?: AcOpfWorkerFactory,
): Promise<boolean> {
  try {
    const response = await workerCall(
      { type: "probe", wasmUrl },
      undefined,
      workerFactory,
    );
    return response.type === "ready";
  } catch {
    return false;
  }
}

/** Run one nonlinear AC OPF in a fresh, dedicated worker. Cancellation always
 * terminates that worker; there is deliberately no main-thread fallback. */
export async function solveAcOpfModule(
  wasmUrl: string,
  moduleJson: string,
  signal?: AbortSignal,
  workerFactory?: AcOpfWorkerFactory,
): Promise<SolveResponse> {
  assertEngineInputLength(moduleJson.length);
  const response = await workerCall(
    {
      type: "solve",
      wasmUrl,
      moduleJson,
      requestJson: JSON.stringify({ formulation: "acopf" }),
    },
    signal,
    workerFactory,
  );
  if (response.type !== "result") {
    throw new Error("AC OPF worker returned no solve result");
  }
  return response.response;
}
