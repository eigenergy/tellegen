/// <reference types="webmcp-types" />

import { describe, expect, it, vi } from "vitest";
import {
  createTellegenPlanningTools,
  createTellegenTools,
  DEFAULT_OUTPUT_BUDGET,
  DEFAULT_TOOL_TIMEOUT_MS,
} from "./tools.js";
import type {
  ModelContextLike,
  TellegenWebMcpAdapter,
  ToolPayload,
} from "./types.js";

const payload = (operation: string): ToolPayload => ({ operation });

function adapter(
  overrides: Partial<TellegenWebMcpAdapter> = {},
): TellegenWebMcpAdapter {
  return {
    inspectCase: () => payload("inspect"),
    queryNetwork: (input) => ({ operation: "query", limit: input.limit }),
    analyzeSensitivity: () => payload("sensitivity"),
    focusNetwork: () => payload("focus"),
    previewCaseUpdate: () => payload("preview"),
    updateCase: () => payload("update"),
    resetCase: () => payload("reset"),
    ...overrides,
  };
}

const signal = () => new AbortController().signal;

function schemaDescriptions(value: unknown): string[] {
  if (!value || typeof value !== "object") return [];
  if (Array.isArray(value)) return value.flatMap(schemaDescriptions);
  const record = value as Record<string, unknown>;
  return [
    ...(typeof record.description === "string" ? [record.description] : []),
    ...Object.values(record).flatMap(schemaDescriptions),
  ];
}

describe("createTellegenTools", () => {
  it("creates a compact official WebMCP descriptor set with security annotations", () => {
    const tools = createTellegenTools(adapter());
    expect(tools.map((tool) => tool.name)).toEqual([
      "inspect_case",
      "query_network",
      "analyze_sensitivity",
      "focus_network",
      "preview_case_update",
      "update_case",
      "reset_case",
    ]);
    for (const tool of tools) {
      const official: WebMCP.ModelContextTool = tool;
      expect(official.name.length).toBeLessThanOrEqual(30);
      expect(official.description.length).toBeLessThanOrEqual(500);
      expect(official.annotations?.untrustedContentHint).toBe(true);
      expect(
        schemaDescriptions(tool.inputSchema).every(
          (text) => text.length <= 150,
        ),
      ).toBe(true);
    }
    expect(
      tools.find((tool) => tool.name === "inspect_case")?.annotations
        .readOnlyHint,
    ).toBe(true);
    expect(
      tools.find((tool) => tool.name === "preview_case_update")?.annotations
        .readOnlyHint,
    ).toBe(true);
    expect(
      tools.find((tool) => tool.name === "update_case")?.annotations
        .readOnlyHint,
    ).toBe(false);
  });

  it("accepts the official model context type at the registration boundary", () => {
    const compatible = (context: WebMCP.ModelContext): ModelContextLike =>
      context;
    expect(compatible).toBeTypeOf("function");
  });

  it("applies defaults and converts public schema names at the adapter boundary", async () => {
    const queryNetwork = vi.fn(() => payload("query"));
    const tool = createTellegenTools(adapter({ queryNetwork })).find(
      (candidate) => candidate.name === "query_network",
    );
    if (!tool) throw new Error("query tool missing");
    const response = await tool.execute(
      { case_id: "case9", element_kind: "branch" },
      { signal: signal() },
    );
    expect(response.ok).toBe(true);
    expect(queryNetwork).toHaveBeenCalledWith(
      {
        caseId: "case9",
        elementKind: "branch",
        elementIds: undefined,
        sortBy: undefined,
        direction: "desc",
        limit: 5,
      },
      expect.any(AbortSignal),
    );
  });

  it("accepts a native-shaped call with no execution options", async () => {
    const inspectCase = vi.fn(() => payload("inspect"));
    const tool = createTellegenTools(adapter({ inspectCase }))[0];
    await expect(tool.execute({})).resolves.toEqual({
      ok: true,
      data: { operation: "inspect" },
    });
    expect(inspectCase).toHaveBeenCalledWith(expect.any(AbortSignal));
  });

  it("rejects unknown fields and duplicate edits at runtime", async () => {
    const updateCase = vi.fn(() => payload("update"));
    const tool = createTellegenTools(adapter({ updateCase })).find(
      (candidate) => candidate.name === "update_case",
    );
    if (!tool) throw new Error("update tool missing");
    const unknown = await tool.execute(
      { case_id: "case9", expected_revision: "r1", surprise: true },
      { signal: signal() },
    );
    expect(unknown).toMatchObject({
      ok: false,
      error: { code: "INVALID_INPUT" },
    });

    const duplicate = await tool.execute(
      {
        case_id: "case9",
        expected_revision: "r1",
        demand: [
          { bus_id: "bus-1", delta_mw: 5 },
          { bus_id: "bus-1", delta_mw: 7 },
        ],
      },
      { signal: signal() },
    );
    expect(duplicate).toMatchObject({
      ok: false,
      error: { code: "INVALID_INPUT" },
    });
    expect(updateCase).not.toHaveBeenCalled();
  });

  it("requires a revision and at least one bounded edit for a preview", async () => {
    const previewCaseUpdate = vi.fn(() => payload("preview"));
    const tool = createTellegenTools(adapter({ previewCaseUpdate })).find(
      (candidate) => candidate.name === "preview_case_update",
    );
    if (!tool) throw new Error("preview tool missing");
    const empty = await tool.execute(
      { case_id: "case9", expected_revision: "r1" },
      { signal: signal() },
    );
    expect(empty).toMatchObject({
      ok: false,
      error: { code: "INVALID_INPUT" },
    });
    expect(previewCaseUpdate).not.toHaveBeenCalled();

    await tool.execute(
      {
        case_id: "case9",
        expected_revision: "r1",
        ratings: [{ branch_id: "branch-7", delta_mw: 5 }],
      },
      { signal: signal() },
    );
    expect(previewCaseUpdate).toHaveBeenCalledWith(
      {
        caseId: "case9",
        expectedRevision: "r1",
        mode: "set",
        demand: [],
        ratings: [{ branchId: "branch-7", deltaMw: 5 }],
        limit: 5,
      },
      expect.any(AbortSignal),
    );
  });

  it("does not call an adapter when execution was already cancelled", async () => {
    const inspectCase = vi.fn(() => payload("inspect"));
    const tool = createTellegenTools(adapter({ inspectCase }))[0];
    const controller = new AbortController();
    controller.abort();
    const response = await tool.execute({}, { signal: controller.signal });
    expect(response).toMatchObject({ ok: false, error: { code: "CANCELLED" } });
    expect(inspectCase).not.toHaveBeenCalled();
  });

  it("returns a definite success when abort arrives after a mutation commits", async () => {
    const controller = new AbortController();
    let committed = false;
    const updateCase = vi.fn(() => {
      committed = true;
      controller.abort();
      return { committed: true };
    });
    const tool = createTellegenTools(adapter({ updateCase })).find(
      (candidate) => candidate.name === "update_case",
    );
    if (!tool) throw new Error("update tool missing");

    const response = await tool.execute(
      {
        case_id: "case9",
        expected_revision: "r1",
        demand: [{ bus_id: "buses:1", delta_mw: 5 }],
      },
      { signal: controller.signal },
    );
    expect(committed).toBe(true);
    expect(response).toEqual({ ok: true, data: { committed: true } });
  });

  it("returns a definite success when abort arrives after focus commits", async () => {
    const controller = new AbortController();
    const focusNetwork = vi.fn(() => {
      controller.abort();
      return { focused: true };
    });
    const tool = createTellegenTools(adapter({ focusNetwork })).find(
      (candidate) => candidate.name === "focus_network",
    );
    if (!tool) throw new Error("focus tool missing");

    const response = await tool.execute(
      {
        case_id: "case9",
        target: { kind: "bus", element_id: "buses:1" },
      },
      { signal: controller.signal },
    );
    expect(response).toEqual({ ok: true, data: { focused: true } });
  });

  it("aborts and reports a bounded timeout", async () => {
    const observed: { signal: AbortSignal | null } = { signal: null };
    const inspectCase = vi.fn((_signal: AbortSignal) => {
      observed.signal = _signal;
      return new Promise<ToolPayload>(() => {});
    });
    const tool = createTellegenTools(adapter({ inspectCase }), {
      timeoutMs: 5,
    })[0];

    await expect(tool.execute({})).resolves.toMatchObject({
      ok: false,
      error: { code: "TIMEOUT" },
    });
    expect(observed.signal?.aborted).toBe(true);
    expect(DEFAULT_TOOL_TIMEOUT_MS).toBeGreaterThan(5);
  });

  it("reports tool lifecycle activity without exposing raw input", async () => {
    const events: unknown[] = [];
    const tool = createTellegenTools(adapter(), {
      onActivity: (event) => events.push(event),
    })[0];
    await tool.execute({}, { signal: signal() });
    expect(events).toMatchObject([
      { type: "started", toolName: "inspect_case" },
      { type: "finished", toolName: "inspect_case", response: { ok: true } },
    ]);
    expect(JSON.stringify(events)).not.toContain("input");
  });

  it("replaces oversized and unserializable output with bounded safe errors", async () => {
    const oversized = createTellegenTools(
      adapter({
        inspectCase: () => ({ content: "x".repeat(DEFAULT_OUTPUT_BUDGET) }),
      }),
    )[0];
    const response = await oversized.execute({}, { signal: signal() });
    expect(response).toMatchObject({
      ok: false,
      error: { code: "OUTPUT_TOO_LARGE" },
    });
    expect(JSON.stringify(response).length).toBeLessThanOrEqual(
      DEFAULT_OUTPUT_BUDGET,
    );

    const circular: Record<string, unknown> = {};
    circular.self = circular;
    const invalid = createTellegenTools(
      adapter({ inspectCase: () => circular as ToolPayload }),
    )[0];
    await expect(
      invalid.execute({}, { signal: signal() }),
    ).resolves.toMatchObject({
      ok: false,
      error: { code: "INVALID_OUTPUT" },
    });
  });

  it("never reports a completed mutation as an output-size failure", async () => {
    let committed = false;
    const update = createTellegenTools(
      adapter({
        updateCase: () => {
          committed = true;
          return { revision: "revision-2", detail: "x".repeat(1_500) };
        },
      }),
      { outputBudget: 256 },
    ).find((tool) => tool.name === "update_case")!;
    const response = await update.execute(
      {
        case_id: "case-1",
        expected_revision: "revision-1",
        demand: [{ bus_id: "buses:1", delta_mw: 5 }],
      },
      { signal: signal() },
    );
    expect(committed).toBe(true);
    expect(response).toMatchObject({
      ok: true,
      data: { revision: "revision-2", output_truncated: true },
    });
    expect(JSON.stringify(response).length).toBeLessThanOrEqual(256);
  });
});

describe("planning tool surface", () => {
  const capability = {
    proposeCapacityPlan: () => ({ proposal_id: "proposal-1" }),
    applyCapacityPlan: () => ({ applied: true }),
    planningAvailable: () => true,
    proposalAvailable: () => true,
    onAvailabilityChange: () => () => {},
  };

  it("classifies the planning tools honestly", () => {
    const groups = createTellegenPlanningTools(capability);
    const byName = new Map(
      [...groups.planning, ...groups.proposal].map((tool) => [tool.name, tool]),
    );
    expect(byName.get("propose_capacity_plan")?.annotations.readOnlyHint).toBe(
      false,
    );
    expect(byName.get("apply_capacity_plan")?.annotations.readOnlyHint).toBe(
      false,
    );
    expect([...byName.keys()].sort()).toEqual([
      "apply_capacity_plan",
      "propose_capacity_plan",
    ]);
    for (const tool of byName.values()) {
      expect(tool.annotations.untrustedContentHint).toBe(true);
      expect(tool.description.length).toBeLessThanOrEqual(500);
    }
  });

  it("rejects malformed propose_capacity_plan input before the adapter runs", async () => {
    const proposeCapacityPlan = vi.fn(() => ({}));
    const groups = createTellegenPlanningTools({
      ...capability,
      proposeCapacityPlan,
    });
    const tool = groups.planning.find(
      (entry) => entry.name === "propose_capacity_plan",
    )!;
    const run = (input: unknown) =>
      tool.execute(input as Record<string, unknown>, {
        signal: new AbortController().signal,
      });

    const base = {
      case_id: "case-1",
      expected_revision: "r-1",
      objective: {
        kind: "weighted_lmp",
        weights: [{ bus_id: "buses:1", weight: 1 }],
      },
      candidates: ["branches:0"],
      max_increase_per_branch_mw: 15,
      budget_mw: 20,
      increment_mw: 5,
      max_changed_lines: 2,
      exact_solve_budget: 6,
    };
    for (const bad of [
      { ...base, objective: { kind: "weighted_lmp", weights: [] } },
      {
        ...base,
        objective: {
          kind: "weighted_lmp",
          weights: [{ bus_id: "b", weight: Number.NaN }],
        },
      },
      { ...base, candidates: [] },
      { ...base, max_changed_lines: 2 },
      { ...base, increment_mw: 30 },
      { ...base, budget_mw: -1 },
      { ...base, surprise: true },
      {
        ...base,
        objective: {
          kind: "weighted_lmp",
          weights: [
            { bus_id: "buses:1", weight: 1 },
            { bus_id: "buses:1", weight: 2 },
          ],
        },
      },
    ]) {
      const response = await run(bad);
      expect(response.ok).toBe(false);
      if (!response.ok) expect(response.error.code).toBe("INVALID_INPUT");
    }
    expect(proposeCapacityPlan).not.toHaveBeenCalled();

    const good = await run({ ...base, max_changed_lines: 1 });
    expect(good.ok).toBe(true);
    expect(proposeCapacityPlan).toHaveBeenCalledTimes(1);
    const [input] = proposeCapacityPlan.mock.calls[0] as unknown[];
    expect(input).toMatchObject({
      caseId: "case-1",
      maxIncreasePerBranchMw: 15,
      incrementMw: 5,
      maxChangedLines: 1,
      exactSolveBudget: 6,
    });
  });

  it("returns bounded success receipts after oversized planning mutations", async () => {
    let staged = false;
    let applied = false;
    const escapedId = `branch-${'"\\n'.repeat(28)}`;
    const rows = Array.from({ length: 12 }, (_, index) => ({
      branch_id: `${escapedId}-${index}`,
      delta_mw: 5,
    }));
    const groups = createTellegenPlanningTools(
      {
        ...capability,
        proposeCapacityPlan: () => {
          staged = true;
          return {
            proposal_id: "proposal-1",
            revision: "revision-2",
            exact_solves: 14,
            proposal: rows,
          };
        },
        applyCapacityPlan: () => {
          applied = true;
          return {
            proposal_id: "proposal-1",
            revision: "revision-3",
            applied: rows,
            after: { detail: "x".repeat(1_500) },
          };
        },
      },
      { outputBudget: 256 },
    );
    const proposal = groups.planning[0];
    const proposed = await proposal.execute(
      {
        case_id: "case-1",
        expected_revision: "revision-1",
        objective: {
          kind: "weighted_lmp",
          weights: [{ bus_id: "buses:1", weight: 1 }],
        },
        candidates: Array.from(
          { length: 12 },
          (_, index) => `branches:${index}`,
        ),
        max_increase_per_branch_mw: 5,
        budget_mw: 60,
        increment_mw: 5,
        max_changed_lines: 12,
        exact_solve_budget: 14,
      },
      { signal: signal() },
    );
    expect(staged).toBe(true);
    expect(proposed).toMatchObject({
      ok: true,
      data: {
        proposal_id: "proposal-1",
        revision: "revision-2",
        output_truncated: true,
      },
    });
    expect(JSON.stringify(proposed).length).toBeLessThanOrEqual(256);

    const apply = groups.proposal[0];
    const committed = await apply.execute(
      {
        case_id: "case-1",
        expected_revision: "revision-2",
        proposal_id: "proposal-1",
      },
      { signal: signal() },
    );
    expect(applied).toBe(true);
    expect(committed).toMatchObject({
      ok: true,
      data: {
        proposal_id: "proposal-1",
        revision: "revision-3",
        applied_count: 12,
        output_truncated: true,
      },
    });
    expect(JSON.stringify(committed).length).toBeLessThanOrEqual(256);
  });
});
