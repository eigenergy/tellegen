import { describe, expect, it, vi } from "vitest";
import { BrowserStudy } from "../src/index.js";
import type { EngineHost } from "../src/host.js";

// A freshly built Study has already solved its base case (`study_new`). The
// first commit of a case opened without edits, the generated-grid hand-off of
// #261, must answer that solution instead of solving the same problem again.
const base = {
  formulation: "dcopf",
  status: "optimal",
  objective: 10,
  iterations: [{ iter: 0 }, { iter: 1 }],
  lmp: [
    { bus: 1, value: 20 },
    { bus: 2, value: 21 },
  ],
  flows: [{ branch: 7, pf: 5, loading: 0.1 }],
  dispatch: [],
};
const edited = { ...base, objective: 12, iterations: [{ iter: 0 }] };

function fakeHost(failReplace = false) {
  const call = vi.fn<EngineHost["call"]>(async (request) => {
    if (request.op === "study_solution") return JSON.stringify(base);
    if (request.op === "study_replace_edits") {
      if (failReplace) throw new Error("infeasible");
      const solution = JSON.parse(request.edits).length ? edited : base;
      return JSON.stringify({
        solution,
        iterations: solution.iterations,
        sensitivities: [],
      });
    }
    throw new Error(`unexpected operation ${request.op}`);
  });
  return {
    call,
    study: new BrowserStudy({ kind: "worker", call }, 1, "dcopf"),
  };
}
const ops = (call: ReturnType<typeof fakeHost>["call"]) =>
  call.mock.calls.map(([request]) => request.op);

describe("BrowserStudy commit at the base case", () => {
  it("answers the retained base solution without a second solve", async () => {
    const { call, study } = fakeHost();
    const out = await study.commit("case", {}, {}, null);
    expect(ops(call)).toEqual(["study_solution"]);
    expect(out.solution.objective).toBe(10);
    expect(out.iterations).toEqual(base.iterations);
    expect(out.sensitivity).toBeNull();
    // Zero deltas are no edits.
    await study.commit("case", { 1: 0 }, { 7: 0 }, null);
    expect(ops(call)).not.toContain("study_replace_edits");
  });

  it("solves again once edits moved the committed point, then not after returning to base", async () => {
    const { call, study } = fakeHost();
    await expect(
      study.commit("case", { 1: 5 }, {}, null),
    ).resolves.toMatchObject({
      solution: { objective: 12 },
    });
    // Back to base has to replace the committed edits in the engine.
    await expect(study.commit("case", {}, {}, null)).resolves.toMatchObject({
      solution: { objective: 10 },
    });
    expect(ops(call)).toEqual(["study_replace_edits", "study_replace_edits"]);
    call.mockClear();
    await study.commit("case", {}, {}, null);
    expect(ops(call)).toEqual(["study_solution"]);
  });

  it("asks the engine when a sensitivity column is wanted", async () => {
    const { call, study } = fakeHost();
    await study.commit("case", {}, {}, { bus: 2 });
    const replace = call.mock.calls.find(
      ([r]) => r.op === "study_replace_edits",
    );
    expect(replace?.[0]).toMatchObject({ edits: "[]" });
    expect(
      JSON.parse((replace?.[0] as { sensitivities: string }).sensitivities),
    ).toHaveLength(1);
  });

  it("does not trust the base point after a failed replacement", async () => {
    const { call, study } = fakeHost(true);
    await expect(study.commit("case", { 1: 5 }, {}, null)).rejects.toThrow(
      "infeasible",
    );
    call.mockClear();
    await expect(study.commit("case", {}, {}, null)).rejects.toThrow(
      "infeasible",
    );
    expect(ops(call)).toEqual(["study_replace_edits"]);
  });
});
