import { describe, expect, it, vi } from "vitest";
import { StudyDocumentController, type StudyBackend, type StudyStore } from "./study-document.js";
import type { CreateStudy, StudyBundle, StudyOperationResult } from "./generated/study-contracts.js";

const input: CreateStudy = { id: "s", title: "Study", input: "", formulation: "dcopf", request: "Lower prices", interpretation: "Weighted prices",
  objective: { kind: "weighted_observable", operand: { Price: "Active" }, weights: [{ element: 2, weight: 1 }] },
  decisions: { variables: [{ id: "line", element: 1, intervention: "branch_rating", lower: 0, upper: 10, increment: 1, budget_weight: 1 }], total_budget: 10, max_changed_elements: 1 } };

function fixture(): StudyBundle {
  return { document: { schema: "tellegen-study", version: 1, id: "s", title: "Study", revision: 0,
    goals: {}, states: {}, experiments: { p: { kind: "planning", start_state: "base", goal: "g", rationale: "Test", evidence: [], trials: [], result_states: ["candidate"], solve_count: 1, termination: "completed" } },
    experiment_order: ["p"], decisions: {}, active_goal: "g", inspected_state: "base", applied_state: "base", recommended_state: "candidate" }, artifacts: {} };
}
class Store implements StudyStore {
  saved: StudyBundle | null = null;
  fail = false;
  async load() { return structuredClone(this.saved); }
  async create(bundle: StudyBundle) { this.saved = structuredClone(bundle); }
  async commit(expected: number, bundle: StudyBundle) {
    if (this.fail) throw new Error("QuotaExceededError: free storage");
    if (this.saved?.document.revision !== expected) throw new Error("stale stored revision");
    this.saved = structuredClone(bundle);
  }
}
function backend(): StudyBackend {
  return {
    create: async () => fixture(), validate: async (text) => JSON.parse(text) as StudyBundle,
    execute: vi.fn(async (bundle, request) => {
      bundle.document.revision++;
      if (request.operation.kind === "inspect") bundle.document.inspected_state = request.operation.state;
      if (request.operation.kind === "apply") bundle.document.applied_state = request.operation.state;
      const result: StudyOperationResult = { summary: { id: "s", title: "Study", revision: bundle.document.revision, state_count: 0, experiment_count: 1, recent_experiments: [], unavailable_historical_states: 0 }, experiment: null, comparison: null, inspected_view: null };
      return { bundle, result };
    }),
  };
}

describe("durable Study controller", () => {
  it("publishes completed mutations only after storage succeeds", async () => {
    const store = new Store(); const engine = backend();
    const controller = await StudyDocumentController.create(input, store, engine);
    store.fail = true;
    await expect(controller.execute({ expected_revision: 0, operation: { kind: "inspect", state: "candidate" } })).rejects.toThrow("QuotaExceededError");
    expect(controller.bundle.document.revision).toBe(0);
    expect(controller.bundle.document.inspected_state).toBe("base");
    expect(store.saved?.document.revision).toBe(0);
    store.fail = false;
    await controller.execute({ expected_revision: 0, operation: { kind: "inspect", state: "candidate" } });
    expect(controller.bundle.document.applied_state).toBe("base");
    expect(store.saved?.document.inspected_state).toBe("candidate");
  });
  it("serializes mutations and rejects requests based on stale state", async () => {
    const store = new Store(); const engine = backend();
    const controller = await StudyDocumentController.create(input, store, engine);
    const first = controller.execute({ expected_revision: 0, operation: { kind: "inspect", state: "candidate" } });
    const second = controller.execute({ expected_revision: 0, operation: { kind: "inspect", state: "base" } });
    await first; await expect(second).rejects.toThrow("Stale Study revision");
    expect(engine.execute).toHaveBeenCalledTimes(1);
  });
  it("binds explicit approval to a proposal and never restores approval on reload", async () => {
    const store = new Store(); const engine = backend();
    const controller = await StudyDocumentController.create(input, store, engine);
    const token = controller.recordUserApproval("p");
    const restored = await StudyDocumentController.open("s", store, engine);
    await expect(restored.applyApprovedProposal(token)).rejects.toThrow("expired");
    await expect(controller.execute({ expected_revision: 0, operation: { kind: "apply", proposal: "p", goal: "g", base_state: "base", state: "candidate" } })).rejects.toThrow("explicit user approval");
    await controller.applyApprovedProposal(token);
    expect(controller.bundle.document.applied_state).toBe("candidate");
    expect(controller.export()).not.toContain(token);
    await expect(controller.applyApprovedProposal(token)).rejects.toThrow("expired");
  });
  it("saves completed planning evidence when cancellation ends an active proposal", async () => {
    const store = new Store(); const engine = backend(); const cancel = new AbortController();
    const execute = engine.execute;
    engine.execute = async (bundle, request, signal) => {
      cancel.abort();
      const completed = await execute(bundle, request, signal);
      completed.bundle.document.experiments.p.termination = "cancelled";
      return completed;
    };
    const controller = await StudyDocumentController.create(input, store, engine);
    await controller.execute({ expected_revision: 0, operation: { kind: "propose", state: "base", goal: "g", rationale: "Explore", options: { max_solves: 5, beam_width: 2, max_iterations: 2, min_improvement: 1e-7 } } }, cancel.signal);
    expect(controller.bundle.document.revision).toBe(1);
    expect(store.saved?.document.experiments.p.termination).toBe("cancelled");
    expect(store.saved?.document.applied_state).toBe("base");
  });
  it("invalidates approval when branching and rejects pre-cancelled work", async () => {
    const store = new Store(); const engine = backend();
    const controller = await StudyDocumentController.create(input, store, engine);
    const token = controller.recordUserApproval("p");
    await controller.execute({ expected_revision: 0, operation: { kind: "branch", state: "candidate", rationale: "Explore this branch" } });
    await expect(controller.applyApprovedProposal(token)).rejects.toThrow("expired");
    const signal = AbortSignal.abort();
    await expect(controller.execute({ expected_revision: 1, operation: { kind: "inspect", state: "base" } }, signal)).rejects.toThrow();
    expect(controller.bundle.document.revision).toBe(1);
  });
  it("keeps approval through inspection and retries a failed durable apply", async () => {
    const store = new Store(), engine = backend();
    const controller = await StudyDocumentController.create(input, store, engine);
    const token = controller.recordUserApproval("p");
    await controller.execute({ expected_revision: 0, operation: { kind: "inspect", state: "candidate" } });
    expect(controller.isApprovalCurrent(token)).toBe(true);
    store.fail = true;
    await expect(controller.applyApprovedProposal(token)).rejects.toThrow("QuotaExceededError");
    expect(controller.bundle.document.applied_state).toBe("base");
    expect(controller.isApprovalCurrent(token)).toBe(true);
    store.fail = false;
    await controller.applyApprovedProposal(token);
    expect(controller.bundle.document.applied_state).toBe("candidate");
  });

});
