import { describe, expect, it, vi } from "vitest";
import { BrowserSynthSession, SynthEngineError } from "../src/index.js";
import type { EngineHost } from "../src/host.js";
import type { EngineRequest, EngineValue, SynthPreviewValue } from "../src/protocol.js";

const STAGES = [
  ["context", false],
  ["site_buses", true],
  ["topology", true],
  ["electrify", true],
  ["dc_opf", false],
  ["calibrate", false],
  ["ac_pf", false],
  ["finalize", false],
] as const;

function preview(buses: number): SynthPreviewValue {
  return {
    kind: "synth_preview",
    busPositions: new Float64Array(buses * 2),
    busKinds: new Uint8Array(buses),
    busKv: new Float32Array(buses),
    branchPositions: new Float64Array(6),
    branchStartIndices: new Uint32Array([0, 3]),
    branchKv: new Float32Array(1),
  };
}

/** A host that walks the stage list, one stage per advance. */
function stagedHost(kind: "worker" | "direct" = "worker") {
  let index = 0;
  const calls: EngineRequest[] = [];
  const call = vi.fn<EngineHost["call"]>(async (request): Promise<EngineValue> => {
    calls.push(request);
    switch (request.op) {
      case "synth_session_advance": {
        const [stage, previewChanged] = STAGES[index];
        index += 1;
        const done = index === STAGES.length;
        return JSON.stringify({
          stage,
          next: done ? null : STAGES[index][0],
          progress: index / STAGES.length,
          done,
          warnings: [],
          preview_changed: previewChanged,
        });
      }
      case "synth_session_preview":
        return preview(4);
      case "synth_session_report":
        return JSON.stringify({ schema: "tellegen.synth-report/1", stages: [], warnings: [] });
      case "synth_session_free":
        return null;
      case "synth_session_export_gridfm":
        return JSON.stringify({ format: "gridfm-graphkit", n_bus: 4 });
      default:
        throw new Error(`unexpected operation ${request.op}`);
    }
  });
  const cancel = vi.fn<NonNullable<EngineHost["cancel"]>>(() => kind === "worker");
  const host: EngineHost = { kind, call, cancel };
  return { host, call, cancel, calls };
}

describe("BrowserSynthSession", () => {
  it("runs every stage and fetches the preview only when it changed", async () => {
    const { host, calls } = stagedHost();
    const session = new BrowserSynthSession(host, 9);
    const seen: Array<[string, boolean]> = [];
    const report = await session.run((outcome, view) => {
      seen.push([outcome.stage, view !== undefined]);
      if (view) expect(view.busPositions).toBeInstanceOf(Float64Array);
    });
    expect(report.schema).toBe("tellegen.synth-report/1");
    expect(seen).toEqual(STAGES.map(([stage, changed]) => [stage, changed]));
    expect(session.done).toBe(true);
    const ops = calls.map((c) => c.op);
    expect(ops.filter((op) => op === "synth_session_advance")).toHaveLength(STAGES.length);
    expect(ops.filter((op) => op === "synth_session_preview")).toHaveLength(3);
    expect(calls.every((c) => !("session" in c) || c.session === 9)).toBe(true);
  });

  it("exports GridFM graphkit JSON", async () => {
    const { host } = stagedHost();
    const text = await new BrowserSynthSession(host, 6).exportGridfm();
    expect(JSON.parse(text)).toMatchObject({ format: "gridfm-graphkit", n_bus: 4 });
  });

  it("returns the preview as typed arrays without the transport tag", async () => {
    const { host } = stagedHost();
    const view = await new BrowserSynthSession(host, 1).preview();
    expect(view.busKinds).toBeInstanceOf(Uint8Array);
    expect(view.branchStartIndices).toEqual(new Uint32Array([0, 3]));
    expect("kind" in view).toBe(false);
  });

  it("cancels the synthetic worker on abort and rejects with the reason", async () => {
    const { host, cancel, calls } = stagedHost("worker");
    const session = new BrowserSynthSession(host, 2);
    const controller = new AbortController();
    const reason = new DOMException("user moved on", "AbortError");
    const run = session.run((outcome) => {
      if (outcome.stage === "topology") controller.abort(reason);
    }, controller.signal);
    await expect(run).rejects.toBe(reason);
    expect(cancel).toHaveBeenCalledTimes(1);
    // The terminated worker took the session with it: nothing more is sent.
    expect(calls.some((c) => c.op === "synth_session_free")).toBe(false);
    await expect(session.report()).rejects.toBe(reason);
  });

  it("frees the session when the host cannot interrupt (direct host)", async () => {
    const { host, cancel, calls } = stagedHost("direct");
    const session = new BrowserSynthSession(host, 3);
    expect(session.hostKind).toBe("direct");
    await session.advance();
    session.cancel();
    expect(cancel).toHaveBeenCalledTimes(1);
    await Promise.resolve();
    expect(calls.at(-1)).toEqual({ op: "synth_session_free", session: 3 });
    await expect(session.advance()).rejects.toThrow(/cancelled/);
  });

  it("turns generator refusals into SynthEngineError with their code", async () => {
    const payload = {
      code: "osm_insufficient",
      message: "OpenStreetMap data is insufficient (2 bus(es), 1 branch(es)): too few lines",
      buses: 2,
      branches: 1,
    };
    const host: EngineHost = {
      kind: "worker",
      call: async () => {
        throw new Error(JSON.stringify(payload));
      },
    };
    const error = await new BrowserSynthSession(host, 4).advance().catch((e: unknown) => e);
    expect(error).toBeInstanceOf(SynthEngineError);
    expect(error).toMatchObject({ code: "osm_insufficient", buses: 2, branches: 1 });
    // A failure that is not a generator payload passes through untouched.
    const plain: EngineHost = {
      kind: "worker",
      call: async () => {
        throw new Error("engine worker failed");
      },
    };
    await expect(new BrowserSynthSession(plain, 5).report()).rejects.toThrow("engine worker failed");
  });
});
