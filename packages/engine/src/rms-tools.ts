/** Experimental RMS tools in the browser: a case's DYR records as named
 * parameters and their edits, the models of a unit, controller block graphs,
 * and a live session that advances on demand and takes events at its current
 * time. They run in the RMS worker (rms-worker.ts) and need an engine build
 * with `rms-experimental`; the numbers are the engine's. */

import { assertEngineInputLength } from "./input-limit.js";
import type { RmsLiveSliceValue, RmsToolOp } from "./rms-protocol.js";
import { ExperimentalUnavailable, rmsCall, rmsGeneration, rmsSupport } from "./rms.js";
import type { LoadModel, RecordSpec, RmsEvent } from "./generated/rms-types.js";
import type { ProtectionScheme, RelayOperation } from "./protection.js";

async function call(op: RmsToolOp, what: string): Promise<unknown> {
  const support = await rmsSupport();
  if (!support.experimental) throw new ExperimentalUnavailable(what);
  return (await rmsCall(op)).value;
}

// ---- DYR records and models ------------------------------------------------

/** One parameter of a DYR record. */
export interface DyrParameter {
  name: string;
  /** The token as written in the file. */
  text: string;
  value: number;
  group?: string;
  unit?: string;
  description?: string;
}

/** A DYR record linked to a generator. An inverter record's role is its part
 * of the plant: `converter` (REGC*), `electrical` (REEC*), `plant` (REPC*),
 * or `inverter` for another inverter record. */
export interface DyrRecordView {
  /** First line of the record in the file (1-based); edits name it. */
  line: number;
  model: string;
  role: "machine" | "exciter" | "governor" | "stabilizer" | InverterPart | "inverter";
  parameters: DyrParameter[];
}

/** A generating unit of the case: its linked DYR records (none for a static
 * unit) and its inverter records (by name, and as parts with parameters). */
export interface DyrGeneratorView {
  bus: number;
  machine_id: string;
  mbase: number;
  pg: number;
  pmax: number;
  qmax: number;
  qmin: number;
  vg: number;
  records: DyrRecordView[];
  inverter: string[];
  /** Converter, electrical control and plant controller, in that order;
   * a model without named parameters has none. */
  inverter_records?: DyrRecordView[];
}

/** A part of an inverter-based unit, swapped on its own. */
export type InverterPart = "converter" | "electrical" | "plant";

/** Role of a dynamic model in a generating unit: a synchronous machine and
 * its controls, a whole inverter-based unit, or one part of one. */
export type ModelRole =
  | "machine"
  | "exciter"
  | "governor"
  | "stabilizer"
  | "oel"
  | "uel"
  | "inverter"
  | InverterPart;

/** One model of the library. */
export interface LibraryModel {
  role: ModelRole;
  name: string;
  label: string;
  engines: string;
}

/** A generating unit from published data, with its source. */
export interface UnitPreset {
  id: string;
  kind: string;
  label: string;
  rated_mva: number | null;
  source: string;
  notes: string;
  machine: { model: string; params: string };
  exciter: { model: string; params: string } | null;
  governor: { model: string; params: string } | null;
  stabilizer: { model: string; params: string } | null;
}

/** One parameter edit: record line, parameter index (PSS/E order), value. */
export interface DyrEdit {
  line: number;
  index: number;
  value: number;
}

/** Every generating unit of the case with its linked records as named
 * parameters (static units have none). */
export async function dyrParameters(moduleJson: string, dyrText: string): Promise<DyrGeneratorView[]> {
  assertEngineInputLength(moduleJson.length);
  assertEngineInputLength(dyrText.length);
  const v = await call({ op: "dyr_parameters", module_json: moduleJson, dyr_text: dyrText }, "DYR parameters");
  return JSON.parse(v as string) as DyrGeneratorView[];
}

/** DYR text with single parameter values replaced; every other byte stays. */
export async function editDyr(dyrText: string, edits: DyrEdit[]): Promise<string> {
  assertEngineInputLength(dyrText.length);
  return (await call({ op: "dyr_edit", dyr_text: dyrText, edits_json: JSON.stringify(edits) }, "DYR editing")) as string;
}

/** DYR text with the `role` model of unit `(bus, machineId)` set to `model`
 * (typical parameters), or removed with `null`. */
export async function setUnitModel(
  dyrText: string,
  bus: number,
  machineId: string,
  role: ModelRole,
  model: string | null,
): Promise<string> {
  assertEngineInputLength(dyrText.length);
  return (await call(
    { op: "dyr_set_model", dyr_text: dyrText, bus, machine_id: machineId, role, model: model ?? "" },
    "model selection",
  )) as string;
}

/** DYR text giving unit `(bus, machineId)` the models of a published unit. */
export async function applyUnitPreset(
  dyrText: string,
  bus: number,
  machineId: string,
  preset: string,
): Promise<string> {
  assertEngineInputLength(dyrText.length);
  return (await call(
    { op: "dyr_apply_preset", dyr_text: dyrText, bus, machine_id: machineId, preset },
    "published units",
  )) as string;
}

/** Generating units from published data. */
export async function unitPresets(): Promise<UnitPreset[]> {
  return JSON.parse((await call({ op: "unit_presets" }, "published units")) as string) as UnitPreset[];
}

// ---- controller block graphs -----------------------------------------------

/** Controller slot a block graph fills. */
export type GraphSlot = "exciter" | "governor" | "stabilizer";

/** One block of a controller graph. Time constants sit in the mass matrix;
 * zero makes the state algebraic. */
export type GraphBlock =
  | { type: "input"; port: string }
  | { type: "const"; value: number }
  | { type: "gain"; k: number }
  | { type: "sum"; signs: number[] }
  | { type: "product" }
  | { type: "divide" }
  | { type: "clamp"; min: number; max: number }
  | { type: "clamp_var" }
  | { type: "min"; tie?: number }
  | { type: "max" }
  | { type: "rectifier" }
  | { type: "sat_quadratic"; a: number; b: number }
  | { type: "gate"; lo: number; hi: number }
  | { type: "lag"; k: number; t: number }
  | { type: "lag_limited"; k: number; t: number; min: number; max: number; clamp_output?: boolean }
  | { type: "lead_lag"; k: number; t1: number; t2: number }
  | { type: "washout"; k: number; t: number }
  | { type: "integrator"; k: number }
  | { type: "pi_limited"; kp: number; ki: number; min: number; max: number; rate?: number }
  | { type: "lag2"; k: number; t1: number; t2: number }
  | { type: "lead_lag2"; t1: number; t2: number; t3: number; t4: number };

/** A node: id, block, input node ids in the block's order. */
export type GraphNode = GraphBlock & {
  id: string;
  inputs: string[];
  /** State names (default: the node id). */
  states?: string[];
  /** Start `[a, b]` of an integrating state for the initialization: a·target + b. */
  start?: [number, number];
  /** Editor position. */
  pos?: [number, number];
};

/** A controller as a block diagram. */
export interface BlockGraph {
  slot: GraphSlot;
  name: string;
  nodes: GraphNode[];
  /** Node whose output is the slot's output. */
  output: string;
}

/** A graph replacing (or adding) a controller of one generator. */
export interface ModelOverride {
  bus: number;
  machine_id: string;
  graph: BlockGraph;
}

/** Ports each slot's graph can read. */
export const GRAPH_PORTS: Record<GraphSlot, string[]> = {
  exciter: ["vt", "vs", "xad_ifd", "v_ref", "p", "q"],
  governor: ["omega", "p_ref"],
  stabilizer: ["omega", "pe", "vt", "pe_m"],
};

/** The dynamic data the engine reads: plain DYR, or DYR with controller
 * graphs as `{"dyr", "models"}` JSON. */
export function dynamicInput(dyrText: string, models: ModelOverride[] | undefined): string {
  return models && models.length ? JSON.stringify({ dyr: dyrText, models }) : dyrText;
}

/** The controller record starting on `line` as an editable block graph. */
export async function modelTemplate(dyrText: string, line: number): Promise<BlockGraph> {
  assertEngineInputLength(dyrText.length);
  return JSON.parse(
    (await call({ op: "model_template", dyr_text: dyrText, line }, "block diagrams")) as string,
  ) as BlockGraph;
}

/** Check a graph: its states, or why it does not compile. */
export async function checkGraph(
  graph: BlockGraph,
): Promise<{ ok: true; states: string[] } | { ok: false; error: string }> {
  return JSON.parse((await call({ op: "check_graph", graph_json: JSON.stringify(graph) }, "block diagrams")) as string);
}

// ---- live session ----------------------------------------------------------

/** New samples of a live session: `series(key)` views one shared buffer. */
export interface RmsLiveSlice {
  /** Simulated time reached, s. */
  t: number;
  time: number[];
  keys: string[];
  values: Float64Array;
  /** Events applied since the previous slice, as the engine describes them. */
  events: string[];
  /** Relay operations since the previous slice (with relays set). */
  relays: RelayOperation[];
  /** Time the worker spent computing the slice, ms (wasm only, no transfer). */
  workerMs: number;
  series(key: string): Float64Array | undefined;
}

let liveSeq = 0;

/** A running simulation in the RMS worker: advance it in small slices and
 * apply events at its current time. It ends when the worker is replaced
 * (a stopped run); {@link alive} says so. */
export class BrowserRmsLive {
  readonly #session = ++liveSeq;
  readonly #generation: number;
  #freed = false;

  private constructor(generation: number) {
    this.#generation = generation;
  }

  /** Start at the initial operating point of `moduleJson` with `dynText`
   * (DYR, or {@link dynamicInput} JSON). */
  static async open(
    moduleJson: string,
    dynText: string,
    options: { loadModel?: LoadModel; dt?: number; outputStep?: number } = {},
  ): Promise<BrowserRmsLive> {
    assertEngineInputLength(moduleJson.length);
    assertEngineInputLength(dynText.length);
    const support = await rmsSupport();
    if (!support.experimental) throw new ExperimentalUnavailable("the live view");
    const live = new BrowserRmsLive(rmsGeneration());
    await rmsCall({
      op: "live_new",
      session: live.#session,
      module_json: moduleJson,
      dyn_text: dynText,
      load_model: options.loadModel ?? "power_flow",
      dt: options.dt ?? 0.005,
      output_step: options.outputStep ?? 0.02,
    });
    return live;
  }

  /** Whether the session still exists in the worker. */
  get alive(): boolean {
    return !this.#freed && this.#generation === rmsGeneration();
  }

  #check(): void {
    if (!this.alive) throw new Error("the live session ended; start it again");
  }

  /** Record only `spec` from now on (null: every channel). */
  async setRecord(spec: RecordSpec | null): Promise<void> {
    this.#check();
    await rmsCall({ op: "live_record", session: this.#session, spec_json: spec ? JSON.stringify(spec) : "" });
  }

  /** Integrate with a step of about `dt` s; returns the step taken. */
  async setStep(dt: number): Promise<number> {
    this.#check();
    return (await rmsCall({ op: "live_step", session: this.#session, dt })).value as number;
  }

  /** Advance `seconds` of simulated time. */
  async advance(seconds: number): Promise<RmsLiveSlice> {
    this.#check();
    const { value, workerMs } = await rmsCall({ op: "live_advance", session: this.#session, seconds });
    const v = value as RmsLiveSliceValue;
    const meta = JSON.parse(v.meta) as {
      t: number;
      time: number[];
      keys: string[];
      events: string[];
      relays?: RelayOperation[];
    };
    const n = meta.time.length;
    const index = new Map(meta.keys.map((k, i) => [k, i] as const));
    return {
      ...meta,
      relays: meta.relays ?? [],
      workerMs,
      values: v.values,
      series(key: string) {
        const i = index.get(key);
        return i === undefined ? undefined : v.values.subarray(i * n, (i + 1) * n);
      },
    };
  }

  /** Apply `event` now; its `time` is replaced by the session's. Returns
   * the engine's description. */
  async apply(event: RmsEvent): Promise<string> {
    this.#check();
    return (await rmsCall({ op: "live_apply", session: this.#session, event_json: JSON.stringify(event) }))
      .value as string;
  }

  /** Let the relays of `scheme` act from now on (null: none). Their
   * operations come with the slices ({@link RmsLiveSlice.relays}). */
  async setProtection(scheme: ProtectionScheme | null): Promise<void> {
    this.#check();
    await rmsCall({
      op: "live_protection",
      session: this.#session,
      scheme_json: scheme ? JSON.stringify(scheme) : "",
    });
  }

  free(): void {
    if (this.#freed) return;
    const alive = this.alive;
    this.#freed = true;
    if (alive) rmsCall({ op: "live_free", session: this.#session }).catch(() => {});
  }
}
