/** Messages between the page and the dedicated RMS worker (rms-worker.ts).
 * The worker owns its own wasm instance and the prepared `RmsCase` handles,
 * so a run never blocks the page or the static-study worker, and Stop can
 * terminate it outright. */

export type RmsWorkerOp =
  | { op: "support" }
  | { op: "looks_like_dyr"; text: string }
  | {
      op: "open";
      handle: number;
      module_json: string;
      dyr_text: string;
      options_json: string;
    }
  | { op: "inspect"; handle: number }
  | { op: "flat_check"; handle: number; seconds: number }
  | { op: "run"; handle: number; request_json: string }
  | { op: "linear"; handle: number; request_json: string }
  | { op: "free"; handle: number }
  | {
      op: "dynamize";
      module_json: string;
      dyr_text: string;
      request_json: string;
    }
  | { op: "library" };

export type RmsWorkerRequest = RmsWorkerOp & { id: number; wasm_url?: string };

/** The value of a `run`: the JSON head and the two buffers, transferred. */
export interface RmsRunValue {
  meta: string;
  time: Float64Array;
  values: Float64Array;
}

export type RmsWorkerResponse =
  | { id: number; progress: { t: number } }
  | { id: number; ok: true; value: unknown; worker_ms: number }
  | { id: number; ok: false; error: string; fatal?: boolean };
