/** RMS (electromechanical) dynamic simulation in the browser.
 *
 * Runs execute in a dedicated worker (rms-worker.ts) with its own wasm
 * instance: a run never blocks the page or the static-study worker, and
 * aborting one terminates that worker. A {@link BrowserRmsCase} keeps its
 * module and DYR text and reopens itself in a fresh worker on its next call.
 *
 * Numbers are the engine's: every recorded channel is the Rust run's sample
 * for sample, and the screening (verdict, COI frequency, bus frequencies) is
 * computed in Rust from the run's own samples, never from a decimated copy. */

import { parseApiResult } from "./result-validation.js";
import { errorText } from "./errors.js";
import { assertEngineInputLength } from "./input-limit.js";
import type {
  RmsRunValue,
  RmsWorkerOp,
  RmsWorkerRequest,
  RmsWorkerResponse,
} from "./rms-protocol.js";
import type {
  LoadModel,
  LowVoltageLoad,
  RmsEvent,
  RmsGenerator,
  RmsInspection,
  RmsOptions,
  RmsRequest,
  RmsScreening,
  ScreeningCriteria,
} from "./generated/rms-types.js";

export type {
  AngleScreen,
  FrequencyScreen,
  InputSignal,
  IslandScreen,
  LoadModel,
  LowVoltageLoad,
  PoleSlip,
  Reason,
  RecordSpec,
  RmsEvent,
  RmsGenerator,
  RmsInspection,
  RmsOptions,
  RmsRequest,
  RmsScreening,
  ScreeningCriteria,
  ScreeningVerdict,
  SwingTrend,
  VoltageScreen,
} from "./generated/rms-types.js";

/** How a case is prepared: the load model and the engine's options. */
export interface RmsCaseOptions extends Partial<RmsOptions> {
  load_model?: LoadModel;
}

/** A run: the events and steps; the display channels are recorded. */
export interface RmsRunRequest {
  t_end?: number;
  dt?: number;
  output_step?: number;
  events: RmsEvent[];
}

/** The drift of a run without events. */
export interface RmsFlatCheck {
  seconds: number;
  max_speed_drift: number;
  max_voltage_drift: number;
  steps: number;
}

/** The head of a run as the worker packs it. */
export interface RmsRunMeta {
  keys: string[];
  notes: string[];
  events: string[];
  generators: RmsGenerator[];
  sources: string[];
  buses: number[];
  base_mva: number;
  base_frequency: number;
  steps: number;
  newton_iterations: number;
  output_step: number;
  dt: number;
  t_end: number;
  screening: RmsScreening;
  request: RmsRequest;
}

/** A finished run. `series(key)` is a view into one shared buffer: keys are
 * the engine's channels (`vm:`, `omega:`, `delta:`, `pe:`, `pl:`, `ql:`) and
 * the derived `coi:f` (Hz), `coi:omega` (pu), `coi:delta` (rad) and
 * `f:<bus>` (Hz, estimated from the bus angle; NaN while a bus is dead). */
export interface RmsRun {
  readonly request: RmsRunRequest;
  readonly time: Float64Array;
  readonly keys: readonly string[];
  series(key: string): Float64Array | undefined;
  readonly meta: RmsRunMeta;
  readonly screening: RmsScreening;
  readonly notes: readonly string[];
  readonly events: readonly string[];
  readonly steps: number;
  readonly newtonIterations: number;
  readonly outputStep: number;
  /** Time the worker spent on the run, ms. */
  readonly workerMs: number;
  /** Bytes the run's buffers hold. */
  readonly bytes: number;
}

/** A small-signal mode with its participation factors. */
export interface RmsMode {
  re: number;
  im: number;
  freq_hz: number;
  damping: number;
  participation: { state: string; factor: number }[];
}

export interface RmsLinearResult {
  states: string[];
  modes: RmsMode[];
  bode: unknown[];
}

export interface RmsLinearRequest {
  participation_top?: number;
}

/** Thrown when an experimental tool is not in this engine build. */
export class ExperimentalUnavailable extends Error {
  constructor(what: string) {
    super(`${what} is not in this engine build`);
    this.name = "ExperimentalUnavailable";
  }
}

// ---- worker host -----------------------------------------------------------

/** The part of a Worker the host uses (tests pass a fake). */
export interface RmsWorkerLike {
  onmessage: ((ev: MessageEvent<RmsWorkerResponse>) => void) | null;
  onerror: ((ev: ErrorEvent) => void) | null;
  postMessage(msg: RmsWorkerRequest, transfer?: Transferable[]): void;
  terminate(): void;
}

export type RmsWorkerFactory = () => RmsWorkerLike;

const defaultFactory: RmsWorkerFactory = () =>
  new Worker(new URL("./rms-worker.js", import.meta.url), {
    type: "module",
  }) as unknown as RmsWorkerLike;

interface Pending {
  resolve(v: { value: unknown; workerMs: number }): void;
  reject(e: Error): void;
  progress?: (t: number) => void;
}

/** One RMS worker at a time. `generation` counts the workers: a case opened
 * in an earlier one reopens itself. */
class RmsHost {
  #worker: RmsWorkerLike | null = null;
  #pending = new Map<number, Pending>();
  #seq = 0;
  generation = 0;

  constructor(public factory: RmsWorkerFactory) {}

  #spawn(): RmsWorkerLike {
    if (this.#worker) return this.#worker;
    if (typeof Worker === "undefined" && this.factory === defaultFactory)
      throw new Error(
        "dynamic simulation needs a browser with dedicated Worker support",
      );
    const w = this.factory();
    w.onmessage = (ev) => {
      const msg = ev.data;
      const p = this.#pending.get(msg.id);
      if (!p) return;
      if ("progress" in msg) {
        p.progress?.(msg.progress.t);
        return;
      }
      this.#pending.delete(msg.id);
      if (msg.ok) p.resolve({ value: msg.value, workerMs: msg.worker_ms });
      else p.reject(new Error(msg.error));
      if (!msg.ok && msg.fatal) this.terminate(new Error(msg.error));
    };
    w.onerror = (ev) => {
      ev.preventDefault?.();
      this.terminate(new Error(ev.message || "the RMS worker failed"));
    };
    this.#worker = w;
    return w;
  }

  call(
    op: RmsWorkerOp,
    opts: { signal?: AbortSignal; progress?: (t: number) => void } = {},
  ): Promise<{ value: unknown; workerMs: number }> {
    opts.signal?.throwIfAborted();
    const w = this.#spawn();
    const id = ++this.#seq;
    return new Promise((resolve, reject) => {
      const abort = () => {
        // A wasm call cannot be interrupted: stop the worker. Every case
        // reopens itself on its next call.
        this.terminate(new DOMException("RMS run stopped", "AbortError"));
      };
      opts.signal?.addEventListener("abort", abort, { once: true });
      const done = () => opts.signal?.removeEventListener("abort", abort);
      this.#pending.set(id, {
        resolve: (v) => {
          done();
          resolve(v);
        },
        reject: (e) => {
          done();
          reject(e);
        },
        progress: opts.progress,
      });
      const req = { ...op, id } as RmsWorkerRequest;
      const transfer: Transferable[] = [];
      try {
        w.postMessage(req, transfer);
      } catch (e) {
        this.#pending.delete(id);
        done();
        reject(e instanceof Error ? e : new Error(errorText(e)));
      }
    });
  }

  /** Terminate the worker and reject what is pending with `reason`. */
  terminate(reason: Error): void {
    const w = this.#worker;
    this.#worker = null;
    // The cases opened in this worker reopen in the next.
    this.generation += 1;
    w?.terminate();
    const pending = [...this.#pending.values()];
    this.#pending.clear();
    for (const p of pending) p.reject(reason);
  }
}

let host: RmsHost | null = null;
function rmsHost(): RmsHost {
  host ??= new RmsHost(defaultFactory);
  return host;
}

/** Use `factory` for the RMS worker (tests; an embedder with its own
 * worker bundling). Terminates the current worker. */
export function setRmsWorkerFactory(factory: RmsWorkerFactory | null): void {
  host?.terminate(new Error("the RMS worker was replaced"));
  host = new RmsHost(factory ?? defaultFactory);
}

// ---- API -------------------------------------------------------------------

/** Whether this engine build carries RMS simulation and its experimental
 * tools. */
export async function rmsSupport(): Promise<{
  rms: boolean;
  experimental: boolean;
}> {
  try {
    const { value } = await rmsHost().call({ op: "support" });
    return value as { rms: boolean; experimental: boolean };
  } catch {
    return { rms: false, experimental: false };
  }
}

/** Whether `text` reads as PSS/E dynamic data (a `.txt` drop). */
export async function looksLikeDyr(text: string): Promise<boolean> {
  assertEngineInputLength(text.length);
  const { value } = await rmsHost().call({ op: "looks_like_dyr", text });
  return value === true;
}

let nextHandle = 0;

/** Prepare `moduleJson` (a stored PowerIO module) with `dyrText` in the RMS
 * worker: assembled and initialized once, run any number of times. */
export async function openRmsCase(
  moduleJson: string,
  dyrText: string,
  options: RmsCaseOptions = {},
): Promise<BrowserRmsCase> {
  assertEngineInputLength(moduleJson.length);
  assertEngineInputLength(dyrText.length);
  const c = new BrowserRmsCase(moduleJson, dyrText, options);
  await c.ready();
  return c;
}

/** A case prepared for RMS simulation in the RMS worker. */
export class BrowserRmsCase {
  readonly handle = ++nextHandle;
  #inspection: RmsInspection | null = null;
  #generation = -1;
  #freed = false;

  constructor(
    readonly moduleJson: string,
    readonly dyrText: string,
    readonly options: RmsCaseOptions,
  ) {}

  /** What the dynamic data resolves to (no run). */
  get inspection(): RmsInspection {
    if (!this.#inspection) throw new Error("the RMS case is not open yet");
    return this.#inspection;
  }

  /** Open (or reopen, after a stopped run) in the current worker. */
  async ready(signal?: AbortSignal): Promise<void> {
    if (this.#freed) throw new Error("the RMS case was freed");
    const h = rmsHost();
    if (this.#inspection && this.#generation === h.generation) return;
    const { value } = await h.call(
      {
        op: "open",
        handle: this.handle,
        module_json: this.moduleJson,
        dyr_text: this.dyrText,
        options_json: JSON.stringify(this.options),
      },
      { signal },
    );
    this.#generation = h.generation;
    this.#inspection = await parseApiResult<RmsInspection>(
      "rms_inspection",
      value,
    );
  }

  /** Run `seconds` without events: how far the case drifts from rest. */
  async flatCheck(seconds = 2): Promise<RmsFlatCheck> {
    await this.ready();
    const { value } = await rmsHost().call({
      op: "flat_check",
      handle: this.handle,
      seconds,
    });
    return parseApiResult<RmsFlatCheck>("rms_flat_check", value);
  }

  /** Run `request`, record the display channels and screen the run.
   * `signal` stops it by terminating the RMS worker. */
  async run(
    request: RmsRunRequest,
    opts: {
      signal?: AbortSignal;
      onProgress?: (p: { t: number; tEnd: number }) => void;
      criteria?: Partial<ScreeningCriteria>;
    } = {},
  ): Promise<RmsRun> {
    await this.ready(opts.signal);
    const full = buildRunRequest(request, this.options);
    const { value, workerMs } = await rmsHost().call(
      {
        op: "run",
        handle: this.handle,
        request_json: JSON.stringify({
          ...full,
          ...(opts.criteria ? { criteria: opts.criteria } : {}),
        }),
      },
      {
        signal: opts.signal,
        progress: (t) => opts.onProgress?.({ t, tEnd: full.t_end }),
      },
    );
    return unpackRun(request, value as RmsRunValue, workerMs);
  }

  /** Small-signal modes at the operating point (experimental). */
  async linear(
    request: RmsLinearRequest = {},
    opts: { signal?: AbortSignal } = {},
  ): Promise<RmsLinearResult> {
    const support = await rmsSupport();
    if (!support.experimental)
      throw new ExperimentalUnavailable("small-signal analysis");
    await this.ready(opts.signal);
    const { value } = await rmsHost().call(
      {
        op: "linear",
        handle: this.handle,
        request_json: JSON.stringify(request),
      },
      { signal: opts.signal },
    );
    return parseApiResult<RmsLinearResult>("rms_linear_result", value);
  }

  /** Release the case in the worker. */
  free(): void {
    if (this.#freed) return;
    this.#freed = true;
    const h = host;
    if (h && this.#generation === h.generation)
      h.call({ op: "free", handle: this.handle }).catch(() => {});
  }
}

/** Default dynamics for a static case (experimental): DYR text with typical
 * parameters, not the case owner's. */
export async function rmsDynamize(
  moduleJson: string,
  dyrText: string,
  request: {
    machine?: string;
    exciter?: string | null;
    governor?: string | null;
    stabilizer?: string | null;
    preset?: string | null;
    units?: [number, string][];
    replace?: boolean;
  },
): Promise<string> {
  const support = await rmsSupport();
  if (!support.experimental)
    throw new ExperimentalUnavailable("default dynamics");
  const { value } = await rmsHost().call({
    op: "dynamize",
    module_json: moduleJson,
    dyr_text: dyrText,
    request_json: JSON.stringify(request),
  });
  return value as string;
}

/** The model library (experimental). */
export async function rmsLibrary(): Promise<unknown[]> {
  const support = await rmsSupport();
  if (!support.experimental)
    throw new ExperimentalUnavailable("the model library");
  const { value } = await rmsHost().call({ op: "library" });
  return parseApiResult<unknown[]>("rms_library", value);
}

// ---- pure helpers (tested) -------------------------------------------------

/** Defaults of a run: 20 s at the engine's 2.5 ms step, recorded every 10 ms
 * for display. */
export const RMS_RUN_DEFAULTS = {
  t_end: 20,
  dt: 0.0025,
  output_step: 0.01,
} as const;

/** The full request a run sends. */
export function buildRunRequest(
  request: RmsRunRequest,
  options: RmsCaseOptions = {},
): Required<Pick<RmsRunRequest, "t_end" | "dt" | "output_step">> & {
  events: RmsEvent[];
  load_model: LoadModel;
} {
  const dt = request.dt ?? RMS_RUN_DEFAULTS.dt;
  // The output step is a multiple of the integration step.
  const want = request.output_step ?? RMS_RUN_DEFAULTS.output_step;
  const output_step = Math.max(dt, Math.round(want / dt) * dt);
  return {
    t_end: request.t_end ?? RMS_RUN_DEFAULTS.t_end,
    dt,
    output_step,
    events: request.events,
    load_model: options.load_model ?? "power_flow",
  };
}

/** Turn the worker's buffers into an {@link RmsRun} (no copy). */
export async function unpackRun(
  request: RmsRunRequest,
  value: RmsRunValue,
  workerMs: number,
): Promise<RmsRun> {
  if (
    !value ||
    typeof value !== "object" ||
    !(value.time instanceof Float64Array) ||
    !(value.values instanceof Float64Array)
  )
    throw new Error(
      "RMS run must contain Float64Array time and values buffers",
    );
  const meta = await parseApiResult<RmsRunMeta>("rms_run_meta", value.meta);
  if (new Set(meta.keys).size !== meta.keys.length)
    throw new Error("RMS run contains duplicate channel keys");
  if (
    !Number.isSafeInteger(meta.steps) ||
    !Number.isSafeInteger(meta.newton_iterations) ||
    meta.steps < 0 ||
    meta.newton_iterations < 0
  )
    throw new Error(
      "RMS run step and iteration counts must be safe nonnegative integers",
    );
  if (!Number.isFinite(workerMs) || workerMs < 0)
    throw new Error("RMS worker duration must be finite and nonnegative");
  const n = value.time.length;
  if (!n) throw new Error("RMS run must contain at least one time sample");
  for (let i = 0; i < n; i++) {
    if (
      !Number.isFinite(value.time[i]) ||
      (i > 0 && value.time[i] <= value.time[i - 1])
    )
      throw new Error(
        "RMS sample times must be finite and strictly increasing",
      );
  }
  // Channel values may contain NaN for the frequency of a de-energized bus.
  // Only their dimensions are checked here; the buffers remain shared views.
  if (
    !Number.isSafeInteger(meta.keys.length * n) ||
    value.values.length !== meta.keys.length * n
  )
    throw new Error(
      `RMS run buffers do not match: ${meta.keys.length} channels × ${n} samples ≠ ${value.values.length} values`,
    );
  const index = new Map(meta.keys.map((k, i) => [k, i] as const));
  return {
    request,
    time: value.time,
    keys: meta.keys,
    series(key: string) {
      const i = index.get(key);
      return i === undefined
        ? undefined
        : value.values.subarray(i * n, (i + 1) * n);
    },
    meta,
    screening: meta.screening,
    notes: meta.notes,
    events: meta.events,
    steps: meta.steps,
    newtonIterations: meta.newton_iterations,
    outputStep: meta.output_step,
    workerMs,
    bytes: value.values.byteLength + value.time.byteLength,
  };
}

export type { LowVoltageLoad as RmsLowVoltageLoad };
