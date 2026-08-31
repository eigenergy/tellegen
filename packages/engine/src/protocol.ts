/** The engine call protocol: one request shape shared by the worker host
 * (postMessage) and the main-thread host (direct dispatch), so a request built
 * once runs on either side. Payloads are the wasm surface's own JSON strings
 * (or raw display bytes); the typed translation stays in index.ts. Study
 * handles are allocated by the caller, which keeps them valid when pending
 * requests replay against the other host. */

import type { WasmModule, WasmStudy } from "./module.js";

export type EngineRequest =
  | { op: "preload" }
  | { op: "classify_json"; bytes: Uint8Array }
  | { op: "ingest_json_drop"; bytes: Uint8Array }
  | { op: "ingest_case"; bytes: Uint8Array; format: string }
  | { op: "ingest_model_json"; network_json: string }
  | { op: "ingest_model_json_bytes"; bytes: Uint8Array }
  | { op: "ingest_dist_case"; text: string; format: string }
  | { op: "ingest_dist_case_bytes"; bytes: Uint8Array; format: string }
  | { op: "parse_display"; bytes: Uint8Array; format: string }
  | { op: "parse_geo"; bytes: Uint8Array; hint: string }
  | { op: "apply_geo"; network_json: string; layer: string }
  | { op: "apply_layout"; network_json: string; coords: string; kind: string }
  | { op: "extract_geo"; network_json: string }
  | { op: "apply_display_geo"; network_json: string; bytes: Uint8Array }
  | { op: "capabilities" }
  | { op: "solve_module"; module_json: string; request: string }
  | {
      op: "study_new";
      study: number;
      module_json: string;
      formulation: string;
    }
  | {
      op: "study_replace_edits";
      study: number;
      edits: string;
      sensitivities: string;
    }
  | { op: "study_preview"; study: number; edits: string; operands: string }
  | { op: "study_solution"; study: number }
  | { op: "study_plan"; study: number; spec: string }
  | { op: "study_save_module"; study: number }
  | { op: "study_save_solution_module"; study: number }
  | { op: "study_export"; study: number; format: string }
  | { op: "study_apply_geo"; study: number; layer: string }
  | { op: "study_free"; study: number };

export type WorkerRequest = EngineRequest & { id: number };

export type WorkerResponse =
  | { id: number; ok: true; value: string | null }
  /** `fatal` marks an error the wasm instance cannot be trusted after — a trap
   * leaves linear memory, the allocator, and every live Study undefined. The
   * host tears the worker down rather than serving the next request from it. */
  | { id: number; ok: false; error: string; fatal?: boolean };

/** Run one request against a loaded wasm module. `studies` maps caller
 * allocated handles to live wasm Study instances on this side of the
 * boundary. */
export function runRequest(
  mod: WasmModule,
  studies: Map<number, WasmStudy>,
  req: EngineRequest,
): string | null {
  const study = (handle: number): WasmStudy => {
    const s = studies.get(handle);
    if (!s) throw new Error(`unknown study handle ${handle}`);
    return s;
  };
  switch (req.op) {
    case "preload":
      return null; // loading the module was the work
    case "classify_json":
      return mod.classify_json(req.bytes);
    case "ingest_json_drop":
      return mod.ingest_json_drop(req.bytes);
    case "ingest_case":
      return mod.ingest_case(req.bytes, req.format);
    case "ingest_model_json":
      return mod.ingest_model_json(req.network_json);
    case "ingest_model_json_bytes":
      return mod.ingest_model_json_bytes(req.bytes);
    case "ingest_dist_case":
      return mod.ingest_dist_case(req.text, req.format);
    case "ingest_dist_case_bytes":
      return mod.ingest_dist_case_bytes(req.bytes, req.format);
    case "parse_display":
      return mod.parse_display(req.bytes, req.format);
    case "parse_geo":
      return mod.parse_geo(req.bytes, req.hint);
    case "apply_geo":
      return mod.apply_geo(req.network_json, req.layer);
    case "apply_layout":
      return mod.apply_layout(req.network_json, req.coords, req.kind);
    case "extract_geo":
      return mod.extract_geo(req.network_json);
    case "apply_display_geo":
      return mod.apply_display_geo(req.network_json, req.bytes);
    case "capabilities":
      return mod.capabilities_json();
    case "solve_module":
      return mod.solve_module(req.module_json, req.request);
    case "study_new":
      studies.set(req.study, new mod.Study(req.module_json, req.formulation));
      return null;
    case "study_replace_edits":
      return study(req.study).replace_edits(req.edits, req.sensitivities);
    case "study_preview":
      return study(req.study).preview_replacement(req.edits, req.operands);
    case "study_solution":
      return study(req.study).solution();
    case "study_plan":
      return study(req.study).plan(req.spec);
    case "study_save_module":
      return study(req.study).save_module();
    case "study_save_solution_module":
      return study(req.study).save_solution_module();
    case "study_export":
      return study(req.study).export(req.format);
    case "study_apply_geo":
      return study(req.study).apply_geo(req.layer);
    case "study_free":
      studies.get(req.study)?.free();
      studies.delete(req.study);
      return null;
  }
}
