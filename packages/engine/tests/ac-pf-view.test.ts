import { expect, it, vi } from "vitest";
import { BrowserStudy, solveResponseToSolution } from "../src/index.js";
import type { EngineHost } from "../src/host.js";

it("preserves AC endpoint quantities, utilization metadata and source identities", () => {
  const loading_info = {
    basis: "mva",
    rating: 80,
    rating_source: "stated",
    limiting_end: "to",
  } as const;
  const projected = solveResponseToSolution({
    formulation: "acpf",
    status: "feasible",
    flows: [
      {
        branch: 42,
        uid: "line-a",
        pf: 30,
        qf: 40,
        pt: -29,
        qt: -42,
        loading: 0.64,
        loading_info,
      },
    ],
  });
  expect(projected).toMatchObject({ formulation: "acpf", status: "feasible" });
  expect(projected.flows).toEqual([
    {
      branch: 42,
      uid: "line-a",
      mw: 30,
      qf: 40,
      pt: -29,
      qt: -42,
      loading: 0.64,
      loading_info,
    },
  ]);
  expect(
    solveResponseToSolution({ flows: [{ branch: 91, pf: -10, loading: 0.1 }] })
      .flows,
  ).toEqual([{ branch: 91, mw: -10, loading: 0.1 }]);
});

it("keeps AC power-flow voltages and omits price requests for selected equipment", async () => {
  const solution = {
    formulation: "acpf",
    status: "feasible",
    objective: null,
    vm: [{ bus: 10, value: 0.98 }],
    va: [{ bus: 10, value: -0.02 }],
    flows: [{ branch: 42, pf: 12, loading: 0.5 }],
  };
  const call = vi.fn<EngineHost["call"]>(async (request) => {
    if (request.op === "study_solution") return JSON.stringify(solution);
    if (request.op === "study_replace_edits") {
      expect(request.sensitivities).toBe("[]");
      return JSON.stringify({ solution, iterations: [], sensitivities: [] });
    }
    throw new Error(`unexpected operation ${request.op}`);
  });
  const study = new BrowserStudy({ kind: "worker", call }, 1, "acpf");
  await expect(study.currentSolution()).resolves.toMatchObject({
    objective: null,
    prices: [],
    vm: solution.vm,
    va: solution.va,
  });
  await expect(
    study.commit("case", {}, {}, { branch: 42 }),
  ).resolves.toMatchObject({
    sensitivity: null,
    solution: { objective: null, vm: solution.vm },
  });
  call.mockClear();
  await expect(
    study.sensitivity("case", {}, {}, { bus: 10 }),
  ).resolves.toBeNull();
  await expect(study.preview({ 10: 1 })).resolves.toEqual({
    prices: [],
    objectiveDelta: null,
    units: null,
  });
  expect(call).not.toHaveBeenCalled();
});
