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
  | { op: "library" }
  | RmsToolOp;

/** The experimental editing tools and the live session (rms-tools.ts). */
export type RmsToolOp =
  | { op: "dyr_parameters"; module_json: string; dyr_text: string }
  | { op: "dyr_edit"; dyr_text: string; edits_json: string }
  | {
      op: "dyr_set_model";
      dyr_text: string;
      bus: number;
      machine_id: string;
      role: string;
      model: string;
    }
  | {
      op: "dyr_apply_preset";
      dyr_text: string;
      bus: number;
      machine_id: string;
      preset: string;
    }
  | { op: "unit_presets" }
  | { op: "model_template"; dyr_text: string; line: number }
  | { op: "check_graph"; graph_json: string }
  | {
      op: "live_new";
      session: number;
      module_json: string;
      dyn_text: string;
      load_model: string;
      dt: number;
      output_step: number;
    }
  | { op: "live_record"; session: number; spec_json: string }
  | { op: "live_step"; session: number; dt: number }
  | { op: "live_advance"; session: number; seconds: number }
  | { op: "live_apply"; session: number; event_json: string }
  | { op: "live_free"; session: number };

/** The value of a `live_advance`: the JSON head and the samples, transferred. */
export interface RmsLiveSliceValue {
  meta: string;
  values: Float64Array;
}

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
