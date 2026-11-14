import { afterEach, describe, expect, it } from "vitest";
import {
  BrowserRmsCase,
  buildRunRequest,
  openRmsCase,
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

function meta(keys: string[]) {
  return JSON.stringify({
    keys,
    notes: [],
    events: [],
    generators: [],
    sources: [],
    buses: [1, 2],
    base_mva: 100,
    base_frequency: 60,
    steps: 10,
    newton_iterations: 20,
    output_step: 0.01,
    dt: 0.0025,
    t_end: 0.02,
    screening: { verdict: "stable" },
    request: {},
  });
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
          time: new Float64Array(3),
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
