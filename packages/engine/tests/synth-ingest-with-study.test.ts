import { beforeEach, describe, expect, it, vi } from "vitest";
import type { EngineRequest, EngineValue } from "../src/protocol.js";

/** The shared study host, replaced so the test sees what reaches it. */
const studyHost = vi.hoisted(() => ({
  kind: "worker" as const,
  call: vi.fn(),
}));
vi.mock("../src/host.js", async (importOriginal) => ({
  ...(await importOriginal<typeof import("../src/host.js")>()),
  engineHost: () => studyHost,
}));

import { BrowserSynthSession } from "../src/index.js";
import type { EngineHost } from "../src/host.js";

const MODULE = '{"schema":"pio-ir"}';
const VIEW = { name: "generated", n_bus: 3, coords_kind: "synthetic" };

/** A synthetic host that answers the module text and the payload without it,
 * recording the order of everything both hosts were asked. */
function hosts(view: () => EngineValue = () => JSON.stringify(VIEW)) {
  const order: string[] = [];
  const synth: EngineHost = {
    kind: "worker",
    call: vi.fn(async (request: EngineRequest): Promise<EngineValue> => {
      order.push(request.op);
      if (request.op === "synth_session_module") return MODULE;
      if (request.op === "synth_session_ingest_view") return view();
      throw new Error(`unexpected operation ${request.op}`);
    }),
  };
  studyHost.call.mockImplementation(async (request: EngineRequest) => {
    order.push(request.op);
    return null;
  });
  return { synth, order };
}

describe("BrowserSynthSession.ingestWithStudy", () => {
  beforeEach(() => {
    studyHost.call.mockReset();
  });

  it("starts the Study from the module text before it asks for the payload", async () => {
    const { synth, order } = hosts();
    const { payload, study } = await new BrowserSynthSession(synth, 7).ingestWithStudy("dcopf");
    expect(order).toEqual(["synth_session_module", "study_new", "synth_session_ingest_view"]);
    expect(studyHost.call).toHaveBeenCalledWith(
      expect.objectContaining({ op: "study_new", module_json: MODULE, formulation: "dcopf" }),
    );
    // The payload is the one `ingest()` returns: the view plus the module text.
    expect(payload).toEqual({ ...VIEW, module_json: MODULE });
    expect((await study).formulation).toBe("dcopf");
  });

  it("frees the Study when the payload fails", async () => {
    const { synth, order } = hosts(() => {
      throw new Error("no payload");
    });
    await expect(new BrowserSynthSession(synth, 7).ingestWithStudy()).rejects.toThrow("no payload");
    await vi.waitFor(() => expect(order).toContain("study_free"));
  });

  it("returns the payload when the Study fails, and the failure with the Study", async () => {
    const { synth } = hosts();
    studyHost.call.mockImplementation(async (request: EngineRequest) => {
      if (request.op === "study_new") throw new Error("infeasible");
      return null;
    });
    const { payload, study } = await new BrowserSynthSession(synth, 7).ingestWithStudy();
    expect(payload.module_json).toBe(MODULE);
    await expect(study).rejects.toThrow("infeasible");
  });
});
