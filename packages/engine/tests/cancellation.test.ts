import { describe, expect, it, vi } from "vitest";
import {
  BrowserStudy,
  type CapacityPlanSpecJson,
} from "../src/index.js";
import type { EngineHost } from "../src/host.js";

const planSpec: CapacityPlanSpecJson = {
  objective: {
    kind: "weighted_lmp",
    weights: [{ bus: 1, weight: 1 }],
  },
  candidates: ["branches:0"],
  max_increase_per_branch_mw: 10,
  budget_mw: 10,
  increment_mw: 5,
  max_changed_lines: 1,
  exact_solve_budget: 2,
};

describe("isolated study cancellation", () => {
  it("cancels only the disposable planning host and keeps the retained Study usable", async () => {
    let rejectCall: ((error: Error) => void) | null = null;
    let planningStartedResolve: (() => void) | null = null;
    const planningStarted = new Promise<void>((resolve) => {
      planningStartedResolve = resolve;
    });
    const isolatedCancel = vi.fn((reason: Error) => {
      rejectCall?.(reason);
      return true;
    });
    const isolatedHost: EngineHost = {
      call: (request) => {
        if (request.op === "study_new" || request.op === "study_free") {
          return Promise.resolve(null);
        }
        if (request.op === "study_plan") {
          planningStartedResolve?.();
          return new Promise((_resolve, reject) => {
            rejectCall = reject;
          });
        }
        return Promise.reject(new Error(`unexpected isolated call ${request.op}`));
      },
      cancel: isolatedCancel,
    };
    const sharedCancel = vi.fn(() => true);
    const sharedCall = vi.fn<EngineHost["call"]>((request) => {
      if (request.op === "study_save_module") {
        return Promise.resolve('{"contract":"powerio.module/1"}');
      }
      if (request.op === "study_solution") {
        return Promise.resolve(
          JSON.stringify({
            formulation: "dcopf",
            status: "solved",
            objective: 7,
            lmp: [],
            flows: [],
            dispatch: [],
          }),
        );
      }
      return Promise.reject(new Error(`unexpected shared call ${request.op}`));
    });
    const sharedHost: EngineHost = {
      call: sharedCall,
      cancel: sharedCancel,
    };
    const study = new BrowserStudy(sharedHost, 1, "dcopf", () => isolatedHost);
    const controller = new AbortController();

    const pending = study.plan(planSpec, controller.signal);
    await planningStarted;
    controller.abort();

    await expect(pending).rejects.toMatchObject({ name: "AbortError" });
    expect(isolatedCancel).toHaveBeenCalledOnce();
    expect(sharedCancel).not.toHaveBeenCalled();
    await expect(study.currentSolution()).resolves.toMatchObject({ objective: 7 });
  });

  it("does not dispatch when the signal is already aborted", async () => {
    const call = vi.fn<EngineHost["call"]>();
    const study = new BrowserStudy({ call }, 1);
    const controller = new AbortController();
    controller.abort();

    await expect(study.plan(planSpec, controller.signal)).rejects.toMatchObject({
      name: "AbortError",
    });
    expect(call).not.toHaveBeenCalled();
  });
});
