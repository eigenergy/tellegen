/** The engine call protocol: one request shape shared by the worker host
 * (postMessage) and the main-thread host (direct dispatch), so a request built
 * once runs on either side. Payloads are the wasm surface's own JSON strings
 * (or raw display bytes); the typed translation stays in index.ts. Study
 * handles are allocated by the caller, which keeps them valid when pending
 * requests replay against the other host. */

import type { WasmModule, WasmStudy } from "./module.js";

export type EngineRequest =
  | { op: "preload" }
  | { op: "ingest_case"; text: string; format: string }
  | { op: "ingest_dist_case"; text: string; format: string }
  | { op: "parse_display"; bytes: Uint8Array; format: string }
  | { op: "parse_geo"; bytes: Uint8Array; hint: string }
  | { op: "apply_geo"; network_json: string; layer: string }
  | { op: "apply_layout"; network_json: string; coords: string; kind: string }
  | { op: "extract_geo"; network_json: string }
  | { op: "apply_display_geo"; network_json: string; bytes: Uint8Array }
  | { op: "capabilities" }
  | { op: "solve_json"; network_json: string; request: string }
  | { op: "study_new"; study: number; network_json: string; formulation: string }
  | { op: "study_replace_edits"; study: number; edits: string; sensitivities: string }
  | { op: "study_preview"; study: number; edits: string; operands: string }
  | { op: "study_solution"; study: number }
  | { op: "study_save_package"; study: number }
  | { op: "study_apply_geo"; study: number; layer: string }
  | { op: "load_package"; text: string }
  | { op: "export_study"; package_json: string; commit: number; format: string }
  | { op: "study_free"; study: number };

export type WorkerRequest = EngineRequest & { id: number };

export type WorkerResponse =
  | { id: number; ok: true; value: string | null }
  | { id: number; ok: false; error: string };

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
    case "ingest_case":
      return mod.ingest_case(req.text, req.format);
    case "ingest_dist_case":
      return mod.ingest_dist_case(req.text, req.format);
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
    case "solve_json":
      return mod.solve_json(req.network_json, req.request);
    case "study_new":
      studies.set(req.study, new mod.Study(req.network_json, req.formulation));
      return null;
    case "study_replace_edits":
      return study(req.study).replace_edits(req.edits, req.sensitivities);
    case "study_preview":
      return study(req.study).preview_replacement(req.edits, req.operands);
    case "study_solution":
      return study(req.study).solution();
    case "study_save_package":
      return study(req.study).save_package();
    case "study_apply_geo":
      return study(req.study).apply_geo(req.layer);
    case "load_package":
      return mod.load_package(req.text);
    case "export_study":
      return mod.export_study(req.package_json, req.commit, req.format);
    case "study_free":
      studies.get(req.study)?.free();
      studies.delete(req.study);
      return null;
  }
}
