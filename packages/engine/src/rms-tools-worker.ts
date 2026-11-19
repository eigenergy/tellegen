/** The RMS worker's side of the experimental editing tools and the live
 * session (rms-tools.ts): the wasm exports of `rms_tools.rs`, which exist
 * only in a build with `rms-experimental`. */

import type { RmsLiveSliceValue, RmsToolOp } from "./rms-protocol.js";

interface LiveSlice {
  meta(): string;
  values(): Float64Array;
  free(): void;
}
interface LiveCase {
  set_record(spec: string): void;
  set_step(dt: number): number;
  advance(seconds: number): LiveSlice;
  apply(event: string): string;
  /** Present only in a build with `protection`. */
  set_protection?(scheme: string): void;
  free(): void;
}
type Tools = {
  rms_dyr_parameters?: (m: string, d: string) => string;
  rms_dyr_edit?: (d: string, e: string) => string;
  rms_dyr_set_model?: (d: string, bus: number, id: string, role: string, model: string) => string;
  rms_dyr_apply_preset?: (d: string, bus: number, id: string, preset: string) => string;
  rms_unit_presets?: () => string;
  rms_model_template?: (d: string, line: number) => string;
  rms_check_graph?: (g: string) => string;
  RmsLiveCase?: new (m: string, d: string, load: string, dt: number, step: number) => LiveCase;
};

const sessions = new Map<number, LiveCase>();

function tool<K extends keyof Tools>(mod: unknown, name: K, what: string): NonNullable<Tools[K]> {
  const f = (mod as Tools)[name];
  if (!f) throw new Error(`${what} is not in this engine build`);
  return f as NonNullable<Tools[K]>;
}

function session(id: number): LiveCase {
  const s = sessions.get(id);
  if (!s) throw new Error(`no live session ${id}`);
  return s;
}

/** Forget every live session (the wasm instance is gone). */
export function clearLiveSessions(): void {
  sessions.clear();
}

export function executeTool(
  mod: unknown,
  req: RmsToolOp,
): { value: unknown; transfer: Transferable[] } {
  const plain = (value: unknown) => ({ value, transfer: [] as Transferable[] });
  switch (req.op) {
    case "dyr_parameters":
      return plain(tool(mod, "rms_dyr_parameters", "DYR parameters")(req.module_json, req.dyr_text));
    case "dyr_edit":
      return plain(tool(mod, "rms_dyr_edit", "DYR editing")(req.dyr_text, req.edits_json));
    case "dyr_set_model":
      return plain(
        tool(mod, "rms_dyr_set_model", "model selection")(
          req.dyr_text,
          req.bus,
          req.machine_id,
          req.role,
          req.model,
        ),
      );
    case "dyr_apply_preset":
      return plain(
        tool(mod, "rms_dyr_apply_preset", "published units")(
          req.dyr_text,
          req.bus,
          req.machine_id,
          req.preset,
        ),
      );
    case "unit_presets":
      return plain(tool(mod, "rms_unit_presets", "published units")());
    case "model_template":
      return plain(tool(mod, "rms_model_template", "block diagrams")(req.dyr_text, req.line));
    case "check_graph":
      return plain(tool(mod, "rms_check_graph", "block diagrams")(req.graph_json));
    case "live_new": {
      const Live = tool(mod, "RmsLiveCase", "the live view");
      sessions.get(req.session)?.free();
      sessions.set(
        req.session,
        new Live(req.module_json, req.dyn_text, req.load_model, req.dt, req.output_step),
      );
      return plain(null);
    }
    case "live_record":
      session(req.session).set_record(req.spec_json);
      return plain(null);
    case "live_step":
      return plain(session(req.session).set_step(req.dt));
    case "live_advance": {
      const slice = session(req.session).advance(req.seconds);
      const value: RmsLiveSliceValue = { meta: slice.meta(), values: slice.values() };
      slice.free();
      return { value, transfer: [value.values.buffer] };
    }
    case "live_apply":
      return plain(session(req.session).apply(req.event_json));
    case "live_protection": {
      const s = session(req.session);
      if (typeof s.set_protection !== "function")
        throw new Error("protection relays are not in this engine build");
      s.set_protection(req.scheme_json);
      return plain(null);
    }
    case "live_free":
      sessions.get(req.session)?.free();
      sessions.delete(req.session);
      return plain(null);
  }
}
