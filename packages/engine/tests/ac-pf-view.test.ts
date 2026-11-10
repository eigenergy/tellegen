import { expect, it, vi } from "vitest";
import { BrowserStudy } from "../src/index.js";
import type { EngineHost } from "../src/host.js";

it("keeps AC power-flow voltages and omits price requests for selected equipment", async () => {
  const solution = {
    formulation: "acpf",
    status: "feasible",
    initialization: { requested: "case", used: "flat", attempts: 2 },
    objective: null,
    vm: [{ bus: 10, value: 0.98 }],
    va: [{ bus: 10, value: -0.02 }],
    flows: [{ branch: 42, pf: 12, loading: 0.5 }],
  };
  const call = vi.fn<EngineHost["call"]>(async (request) => {
    if (request.op === "study_solution") return JSON.stringify(solution);
    if (request.op === "study_replace_edits") {
      expect(request.sensitivities).toBe("[]");
      // Rust's commit envelope goes through serde_json::Value, which sorts
      // object keys; study_solution serializes the response struct directly.
      const initialization = { attempts: 2, requested: "case", used: "flat" };
      return JSON.stringify({
        solution: { ...solution, initialization },
        iterations: [],
        sensitivities: [],
      });
    }
    throw new Error(`unexpected operation ${request.op}`);
  });
  const study = new BrowserStudy({ kind: "worker", call }, 1, "acpf");
  await expect(study.currentSolution()).resolves.toMatchObject({
    objective: null,
    prices: [],
    vm: solution.vm,
    va: solution.va,
    initialization: solution.initialization,
  });
  await expect(
    study.commit("case", {}, {}, { branch: 42 }),
  ).resolves.toMatchObject({
    sensitivity: null,
    solution: {
      objective: null,
      vm: solution.vm,
      initialization: solution.initialization,
    },
  });
  // Saved-case capture compares these serialized compact results before
  // retaining the matching solved view.
  const current = await study.currentSolution();
  const committed = await study.commit("case", {}, {}, null);
  expect(JSON.stringify(current)).toBe(JSON.stringify(committed.solution));
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
