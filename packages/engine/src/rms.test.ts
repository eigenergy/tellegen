import { afterEach, describe, expect, it } from "vitest";
import {
  BrowserRmsCase,
  buildRunRequest,
  openRmsCase,
  rmsLibrary,
  setRmsWorkerFactory,
  unpackRun,
  type RmsWorkerLike,
} from "./rms.js";
import type {
  RmsRunValue,
  RmsWorkerRequest,
  RmsWorkerResponse,
} from "./rms-protocol.js";

const inspection = JSON.stringify({
  buses: [1, 2],
  base_mva: 100,
  base_frequency: 60,
  generators: [],
  sources: [],
  branches: [],
  load_buses: [],
  dyr_records: 0,
  eigenvalues: [],
  notes: [],
  supported_models: [],
});

// Complete serialized Rust output; malformed-response tests mutate individual fields.
const runMeta = {
  base_frequency: 60,
  base_mva: 100,
  buses: [1, 2],
  dt: 0.0025,
  events: [],
  generators: [],
  keys: [],
  newton_iterations: 20,
  notes: [],
  output_step: 0.01,
  request: {
    dt: 0.0025,
    events: [],
    load_model: "power_flow",
    output_step: 0.01,
    record: null,
    small_signal: false,
    t_end: 0.02,
  },
  screening: {
    angle: {
      first_swing_deg: 0,
      first_swing_unit: null,
      initial_separation_deg: 0.0,
      max_coi_angle_deg: 0,
      max_coi_angle_t: 0,
      max_coi_angle_unit: null,
      max_separation_deg: 0,
      pair: [],
      pole_slips: [],
      separation_t: 0,
      settled: false,
      trend: null,
    },
    criteria: {
      bus_frequency_filter_s: 0.02,
      bus_frequency_min_pu: 0.2,
      pole_slip_deg: 180.0,
      rocof_window_s: 0.5,
      runaway_pu: 0.05,
      settle_angle_deg: 1.0,
      settle_frequency_hz: 0.01,
      settle_window_s: 2.0,
      version: 1,
      voltage_recovery_pu: 0.8,
      voltage_recovery_s: 1.0,
    },
    criteria_name: "tellegen-rms-screening-1",
    frequency: {
      coi_nadir_hz: 0,
      coi_peak_hz: 0,
      final_hz: 0,
      lowest_bus: null,
      lowest_bus_hz: 0,
      lowest_bus_t: 0,
      lowest_unit: null,
      lowest_unit_hz: 0,
      lowest_unit_t: 0,
      max_rocof_hz_s: 0,
      nadir_t: 0,
      nominal_hz: 0,
      peak_t: 0,
      rocof_t: 0,
      settled: false,
    },
    held_units: 0,
    islands: [],
    reasons: [],
    reference_unit: null,
    verdict: "stable",
    voltage: {
      final_max_pu: 0,
      final_min_pu: 0,
      last_event_t: 0,
      min_bus: null,
      min_pu: 0,
      min_t: 0,
      recovered: false,
      recovered_s: null,
    },
  },
  sources: [],
  steps: 10,
  t_end: 0.02,
};

function meta(keys: string[]) {
  return JSON.stringify({ ...runMeta, keys });
}

/** A fake RMS worker: records requests, answers what `answer` says. */
class FakeWorker implements RmsWorkerLike {
  onmessage: ((ev: MessageEvent<RmsWorkerResponse>) => void) | null = null;
  onerror: ((ev: ErrorEvent) => void) | null = null;
  requests: RmsWorkerRequest[] = [];
  terminated = false;
  hold = false;
  constructor(readonly answer: (r: RmsWorkerRequest) => unknown) {}
  postMessage(msg: RmsWorkerRequest) {
    this.requests.push(msg);
    if (this.hold && msg.op === "run") return;
    queueMicrotask(() => this.reply(msg));
  }
  reply(msg: RmsWorkerRequest) {
    if (msg.op === "run") {
      for (const t of [0.005, 0.01])
        this.onmessage?.({
          data: { id: msg.id, progress: { t } },
        } as MessageEvent<RmsWorkerResponse>);
    }
    this.onmessage?.({
      data: { id: msg.id, ok: true, value: this.answer(msg), worker_ms: 1 },
    } as MessageEvent<RmsWorkerResponse>);
  }
  terminate() {
    this.terminated = true;
  }
}

function answer(r: RmsWorkerRequest): unknown {
  switch (r.op) {
    case "open":
    case "inspect":
      return inspection;
    case "support":
      return { rms: true, experimental: false };
    case "run":
      return {
        meta: meta(["vm:1", "vm:2"]),
        time: new Float64Array([0, 0.01, 0.02]),
        values: new Float64Array([1, 0.9, 0.95, 1, 0.8, 0.9]),
      } satisfies RmsRunValue;
    default:
      return null;
  }
}

afterEach(() => setRmsWorkerFactory(null));

describe("RMS run buffers", () => {
  it("unpacks channel-major values into zero-copy views", () => {
    const values = new Float64Array([1, 2, 3, 4, 5, 6]);
    const run = unpackRun(
      { events: [] },
      { meta: meta(["a", "b"]), time: new Float64Array([0, 1, 2]), values },
      5,
    );
    expect(Array.from(run.series("b")!)).toEqual([4, 5, 6]);
    expect(run.series("b")!.buffer).toBe(values.buffer);
    expect(run.series("missing")).toBeUndefined();
    expect(run.workerMs).toBe(5);
    expect(run.bytes).toBe(9 * 8);
  });

  it("refuses buffers that do not match their keys", () => {
    expect(() =>
      unpackRun(
        { events: [] },
        {
          meta: meta(["a", "b"]),
          time: new Float64Array([0, 0.01, 0.02]),
          values: new Float64Array(5),
        },
        0,
      ),
    ).toThrow(/2 channels × 3 samples/);
  });

  it("fills the run defaults and keeps the output step a multiple of dt", () => {
    expect(buildRunRequest({ events: [] })).toEqual({
      t_end: 20,
      dt: 0.0025,
      output_step: 0.01,
      events: [],
      load_model: "power_flow",
    });
    const r = buildRunRequest(
      { events: [], dt: 0.003, output_step: 0.01 },
      { load_model: "constant_power" },
    );
    expect(r.output_step / r.dt).toBeCloseTo(
      Math.round(r.output_step / r.dt),
      12,
    );
    expect(r.load_model).toBe("constant_power");
  });
});

describe("RMS worker host", () => {
  it("opens a case, runs it and reports progress against t_end", async () => {
    const workers: FakeWorker[] = [];
    setRmsWorkerFactory(() => {
      const w = new FakeWorker(answer);
      workers.push(w);
      return w;
    });
    const c = await openRmsCase("{}", "dyr", { load_model: "constant_power" });
    expect(c.inspection.base_frequency).toBe(60);
    const progress: number[] = [];
    const run = await c.run(
      { events: [], t_end: 0.02 },
      { onProgress: (p) => progress.push(p.t / p.tEnd) },
    );
    expect(progress).toEqual([0.25, 0.5]);
    expect(Array.from(run.series("vm:2")!)).toEqual([1, 0.8, 0.9]);
    const sent = workers[0].requests.find((r) => r.op === "run")!;
    expect(
      JSON.parse((sent as { request_json: string }).request_json),
    ).toMatchObject({
      t_end: 0.02,
      load_model: "constant_power",
    });
  });

  it("stops a run by terminating the worker and reopens on the next call", async () => {
    const workers: FakeWorker[] = [];
    setRmsWorkerFactory(() => {
      const w = new FakeWorker(answer);
      w.hold = workers.length === 0;
      workers.push(w);
      return w;
    });
    const c: BrowserRmsCase = await openRmsCase("{}", "dyr");
    const stop = new AbortController();
    const pending = c.run({ events: [] }, { signal: stop.signal });
    await new Promise((r) => setTimeout(r, 0));
    stop.abort();
    await expect(pending).rejects.toMatchObject({ name: "AbortError" });
    expect(workers[0].terminated).toBe(true);
    // The next run spawns a fresh worker and reopens the case first.
    const run = await c.run({ events: [] });
    expect(workers).toHaveLength(2);
    expect(workers[1].requests.map((r) => r.op)).toEqual(["open", "run"]);
    expect(run.steps).toBe(10);
  });

  it("names a missing experimental build", async () => {
    setRmsWorkerFactory(() => new FakeWorker(answer));
    const c = await openRmsCase("{}", "dyr");
    await expect(c.linear()).rejects.toThrow(/not in this engine build/);
  });
});

describe("RMS result validation", () => {
  const validRun = (): RmsRunValue => ({
    meta: meta(["vm:1"]),
    time: new Float64Array([0, 0.01]),
    values: new Float64Array([1, 1]),
  });
  it("accepts unavailable screening extrema returned by Rust", () => {
    const value = validRun();
    const parsed = JSON.parse(value.meta);
    for (const key of [
      "coi_nadir_hz",
      "coi_peak_hz",
      "final_hz",
      "lowest_bus_hz",
      "lowest_unit_hz",
    ])
      parsed.screening.frequency[key] = null;
    for (const key of ["min_pu", "final_min_pu", "final_max_pu"])
      parsed.screening.voltage[key] = null;
    value.meta = JSON.stringify(parsed);
    const run = unpackRun({ events: [] }, value, 1);
    expect(run.screening.voltage.min_pu).toBeNull();
    expect(run.screening.frequency.coi_nadir_hz).toBeNull();
    expect(run.time).toBe(value.time);
  });
  it("rejects malformed run metadata before exposing channels", () => {
    for (const change of [
      (m: any) => {
        m.keys = ["vm:1", "vm:1"];
      },
      (m: any) => {
        m.steps = "10";
      },
      (m: any) => {
        m.steps = Number.MAX_SAFE_INTEGER + 1;
      },
      (m: any) => {
        m.screening.frequency = null;
      },
      (m: any) => {
        m.request.events = false;
      },
      (m: any) => {
        delete m.base_frequency;
      },
    ]) {
      const value = validRun();
      const parsed = JSON.parse(value.meta);
      change(parsed);
      value.meta = JSON.stringify(parsed);
      expect(() => unpackRun({ events: [] }, value, 1)).toThrow(/RMS|rms/);
    }
  });
  it("rejects wrong buffer types, invalid sample times and invalid worker duration", () => {
    for (const patch of [
      { time: [0, 1] },
      { values: new Float32Array([1, 1]) },
      { time: new Float64Array() },
      { time: new Float64Array([0, 0]) },
      { time: new Float64Array([0, Infinity]) },
    ])
      expect(() =>
        unpackRun(
          { events: [] },
          { ...validRun(), ...patch } as RmsRunValue,
          1,
        ),
      ).toThrow(/RMS/);
    expect(() => unpackRun({ events: [] }, validRun(), NaN)).toThrow(
      /duration/,
    );
  });
  it("keeps unavailable frequency samples and their binary buffer intact", () => {
    const value = validRun();
    value.values[1] = NaN;
    const run = unpackRun({ events: [] }, value, 1);
    expect(run.time).toBe(value.time);
    expect(run.series("vm:1")!.buffer).toBe(value.values.buffer);
    expect(run.series("vm:1")![1]).toBeNaN();
  });
  it.each([
    [
      "flat_check",
      { seconds: 2, max_speed_drift: "bad", max_voltage_drift: 0, steps: 400 },
    ],
    [
      "linear",
      {
        states: ["omega:1"],
        modes: [
          {
            re: -1,
            im: 2,
            freq_hz: 0.3,
            damping: 0.4,
            participation: [{ state: "omega:1", factor: "bad" }],
          },
        ],
        bode: [],
      },
    ],
    [
      "library",
      [{ role: "machine", name: "GENCLS", label: "Classical", engines: false }],
    ],
  ] as const)(
    "rejects malformed %s records from the worker",
    async (op, result) => {
      setRmsWorkerFactory(
        () =>
          new FakeWorker((request) =>
            request.op === "support"
              ? { rms: true, experimental: true }
              : request.op === op
                ? JSON.stringify(result)
                : answer(request),
          ),
      );
      const c = await openRmsCase("{}", "dyr");
      const resultPromise =
        op === "flat_check"
          ? c.flatCheck()
          : op === "linear"
            ? c.linear()
            : rmsLibrary();
      await expect(resultPromise).rejects.toThrow(/Invalid engine rms_/);
      c.free();
    },
  );
  it("accepts valid flat-check, linear and library records unchanged", async () => {
    const results = {
      flat_check: {
        seconds: 2,
        max_speed_drift: 0,
        max_voltage_drift: 0,
        steps: 400,
      },
      linear: {
        states: ["omega:1"],
        modes: [
          {
            re: -1,
            im: 2,
            freq_hz: 0.3,
            damping: 0.4,
            participation: [{ state: "omega:1", factor: 1 }],
          },
        ],
        bode: [
          { label: "omega", freq_hz: [1], mag_db: [-20], phase_deg: [-90] },
        ],
      },
      library: [
        { role: "machine", name: "GENCLS", label: "Classical", engines: "RMS" },
      ],
    };
    setRmsWorkerFactory(
      () =>
        new FakeWorker((request) =>
          request.op === "support"
            ? { rms: true, experimental: true }
            : request.op in results
              ? JSON.stringify(results[request.op as keyof typeof results])
              : answer(request),
        ),
    );
    const c = await openRmsCase("{}", "dyr");
    expect(await c.flatCheck()).toEqual(results.flat_check);
    expect(await c.linear()).toEqual(results.linear);
    expect(await rmsLibrary()).toEqual(results.library);
    c.free();
  });
});
