import type {
  TellegenPlanningAdapter,
  TellegenToolDefinition,
  TellegenToolActivityEvent,
  TellegenWebMcpAdapter,
  ToolPayload,
  ToolResponse,
} from "./types.js";
import { TellegenToolError } from "./types.js";
import {
  validateAnalyzeSensitivity,
  validateApplyCapacityPlan,
  validateEmpty,
  validateFocusNetwork,
  validatePreviewCaseUpdate,
  validateProposeCapacityPlan,
  validateQueryNetwork,
  validateResetCase,
  validateUpdateCase,
} from "./validation.js";

export const DEFAULT_OUTPUT_BUDGET = 1_450;
export const DEFAULT_TOOL_TIMEOUT_MS = 120_000;
const MAX_ERROR_LENGTH = 320;

const objectSchema = (
  properties: Record<string, unknown>,
  required: string[] = [],
): Record<string, unknown> => ({
  type: "object",
  properties,
  ...(required.length > 0 ? { required } : {}),
  additionalProperties: false,
});

const targetSchema = objectSchema(
  {
    kind: {
      type: "string",
      enum: ["bus", "branch"],
      description: "Element type.",
    },
    element_id: {
      type: "string",
      description: "Stable element ID returned by query_network.",
    },
  },
  ["kind", "element_id"],
);

const annotations = (readOnlyHint: boolean) => ({
  readOnlyHint,
  untrustedContentHint: true,
});

function safeError(error: unknown): ToolResponse {
  if (error instanceof TellegenToolError) {
    return {
      ok: false,
      error: { code: error.code, message: cleanMessage(error.message) },
    };
  }
  if (error instanceof DOMException && error.name === "AbortError") {
    return {
      ok: false,
      error: { code: "CANCELLED", message: "tool execution was cancelled" },
    };
  }
  if (error instanceof DOMException && error.name === "TimeoutError") {
    return {
      ok: false,
      error: { code: "TIMEOUT", message: "tool execution timed out" },
    };
  }
  const message =
    error instanceof Error ? error.message : "tool execution failed";
  return {
    ok: false,
    error: { code: "TOOL_FAILED", message: cleanMessage(message) },
  };
}

function cleanMessage(message: string): string {
  const cleaned = message.replace(/[\u0000-\u001f\u007f]+/g, " ").trim();
  return cleaned.length <= MAX_ERROR_LENGTH
    ? cleaned
    : `${cleaned.slice(0, MAX_ERROR_LENGTH - 1)}…`;
}

function withinBudget(
  response: ToolResponse,
  outputBudget: number,
): ToolResponse {
  let serialized: string;
  try {
    serialized = JSON.stringify(response);
  } catch {
    return {
      ok: false,
      error: {
        code: "INVALID_OUTPUT",
        message: "tool returned a value that is not JSON serializable",
      },
    };
  }
  if (serialized.length <= outputBudget) return response;
  return {
    ok: false,
    error: {
      code: "OUTPUT_TOO_LARGE",
      message: `tool output exceeded ${outputBudget} characters; narrow the query or lower its limit`,
    },
  };
}

function copyPrimitive(
  target: ToolPayload,
  source: ToolPayload,
  key: string,
): void {
  const value = source[key];
  if (
    value === null ||
    typeof value === "string" ||
    typeof value === "boolean" ||
    (typeof value === "number" && Number.isFinite(value))
  ) {
    target[key] = value;
  }
}

/**
 * A mutation has already committed when an adapter marked `commit-aware`
 * resolves. If
 * its detailed payload is too large or not serializable, report a bounded
 * success instead of claiming that the mutation failed.
 */
function committedSuccess(
  toolName: string,
  result: ToolPayload,
  outputBudget: number,
): ToolResponse {
  const summary: ToolPayload = {
    completed: true,
    output_truncated: true,
  };
  if (toolName === "propose_capacity_plan") {
    copyPrimitive(summary, result, "proposal_id");
    copyPrimitive(summary, result, "revision");
    copyPrimitive(summary, result, "exact_solves");
  } else if (toolName === "apply_capacity_plan") {
    copyPrimitive(summary, result, "proposal_id");
    copyPrimitive(summary, result, "revision");
    copyPrimitive(summary, result, "exact_phi");
    const applied = result.applied;
    if (Array.isArray(applied)) summary.applied_count = applied.length;
  } else if (toolName === "update_case" || toolName === "reset_case") {
    copyPrimitive(summary, result, "revision");
    copyPrimitive(summary, result, "formulation");
  } else if (toolName === "focus_network") {
    copyPrimitive(summary, result, "revision");
  }
  const compact = withinBudget({ ok: true, data: summary }, outputBudget);
  if (compact.ok) return compact;
  return { ok: true, data: { completed: true, output_truncated: true } };
}

function execute<T>(
  toolName: string,
  title: string,
  validate: (input: unknown) => T,
  run: (input: T, signal: AbortSignal) => Promise<ToolPayload> | ToolPayload,
  outputBudget: number,
  timeoutMs: number,
  onActivity: ((event: TellegenToolActivityEvent) => void) | undefined,
  nextActivityId: () => string,
  completion: "abortable" | "commit-aware" = "abortable",
): TellegenToolDefinition["execute"] {
  return async (input, options) => {
    const id = nextActivityId();
    const startedAt = Date.now();
    emitActivity(onActivity, {
      type: "started",
      id,
      toolName,
      title,
      startedAt,
    });
    let response: ToolResponse;
    const parentSignal = options?.signal;
    const controller = new AbortController();
    const relayAbort = () => {
      const reason = parentSignal?.reason;
      controller.abort(
        reason instanceof DOMException &&
          (reason.name === "AbortError" || reason.name === "TimeoutError")
          ? reason
          : new DOMException("tool execution was cancelled", "AbortError"),
      );
    };
    if (parentSignal?.aborted) relayAbort();
    else parentSignal?.addEventListener("abort", relayAbort, { once: true });
    const timer = setTimeout(
      () =>
        controller.abort(
          new DOMException("tool execution timed out", "TimeoutError"),
        ),
      timeoutMs,
    );
    let rejectAbort: (() => void) | undefined;
    try {
      controller.signal.throwIfAborted();
      const running = Promise.resolve(run(validate(input), controller.signal));
      let result: ToolPayload;
      if (completion === "commit-aware") {
        // The adapter checks the signal immediately before its commit point.
        // Once it resolves, the state change is final and a later abort must
        // not turn the response into CANCELLED.
        result = await running;
      } else {
        const rejectOnAbort = new Promise<never>((_resolve, reject) => {
          rejectAbort = () => reject(controller.signal.reason);
          controller.signal.addEventListener("abort", rejectAbort, {
            once: true,
          });
        });
        result = await Promise.race([running, rejectOnAbort]);
        controller.signal.throwIfAborted();
      }
      response = withinBudget({ ok: true, data: result }, outputBudget);
      if (!response.ok && completion === "commit-aware") {
        response = committedSuccess(toolName, result, outputBudget);
      }
    } catch (error) {
      response = withinBudget(safeError(error), outputBudget);
    } finally {
      clearTimeout(timer);
      parentSignal?.removeEventListener("abort", relayAbort);
      if (rejectAbort) {
        controller.signal.removeEventListener("abort", rejectAbort);
      }
    }
    emitActivity(onActivity, {
      type: "finished",
      id,
      toolName,
      title,
      startedAt,
      finishedAt: Date.now(),
      response,
    });
    return response;
  };
}

function emitActivity(
  onActivity: ((event: TellegenToolActivityEvent) => void) | undefined,
  event: TellegenToolActivityEvent,
): void {
  try {
    onActivity?.(event);
  } catch {
    // Observability must never change a tool's behavior.
  }
}

export interface CreateTellegenToolsOptions {
  outputBudget?: number;
  timeoutMs?: number;
  onActivity?: (event: TellegenToolActivityEvent) => void;
}

function executionLimits(options: CreateTellegenToolsOptions): {
  outputBudget: number;
  timeoutMs: number;
} {
  const outputBudget = options.outputBudget ?? DEFAULT_OUTPUT_BUDGET;
  const timeoutMs = options.timeoutMs ?? DEFAULT_TOOL_TIMEOUT_MS;
  if (
    !Number.isInteger(outputBudget) ||
    outputBudget < 256 ||
    outputBudget > 1_500
  ) {
    throw new TellegenToolError(
      "INVALID_CONFIGURATION",
      "outputBudget must be an integer from 256 to 1500",
    );
  }
  if (!Number.isInteger(timeoutMs) || timeoutMs < 1 || timeoutMs > 300_000) {
    throw new TellegenToolError(
      "INVALID_CONFIGURATION",
      "timeoutMs must be an integer from 1 to 300000",
    );
  }
  return { outputBudget, timeoutMs };
}

/** Build plain tool descriptors that can be registered in a tab or invoked by a headless test. */
export function createTellegenTools(
  adapter: TellegenWebMcpAdapter,
  options: CreateTellegenToolsOptions = {},
): TellegenToolDefinition[] {
  const { outputBudget, timeoutMs } = executionLimits(options);
  let activitySequence = 0;
  const nextActivityId = () => `tellegen-tool-${++activitySequence}`;
  const tracked = <T>(
    name: string,
    title: string,
    validate: (input: unknown) => T,
    run: (input: T, signal: AbortSignal) => Promise<ToolPayload> | ToolPayload,
    completion: "abortable" | "commit-aware" = "abortable",
  ) =>
    execute(
      name,
      title,
      validate,
      run,
      outputBudget,
      timeoutMs,
      options.onActivity,
      nextActivityId,
      completion,
    );

  return [
    {
      name: "inspect_case",
      title: "Inspect active case",
      description:
        "Inspect the active tellegen case, its network and solve state, committed edits, selection, and revision. Call before a mutation to obtain case_id and expected_revision.",
      inputSchema: objectSchema({}),
      execute: tracked(
        "inspect_case",
        "Inspect active case",
        validateEmpty,
        (_input, signal) => adapter.inspectCase(signal),
      ),
      annotations: annotations(true),
    },
    {
      name: "query_network",
      title: "Query network elements",
      description:
        "Query a bounded list of buses or branches in the active case by stable ID or metric. Returns current solved values and IDs that other tellegen tools accept.",
      inputSchema: objectSchema(
        {
          case_id: {
            type: "string",
            description: "Active case ID from inspect_case.",
          },
          element_kind: {
            type: "string",
            enum: ["bus", "branch"],
            description: "Element type.",
          },
          element_ids: {
            type: "array",
            items: { type: "string" },
            maxItems: 10,
            description:
              "Specific stable IDs to return. Omit to rank elements.",
          },
          sort_by: {
            type: "string",
            enum: [
              "id",
              "demand_mw",
              "generation_mw",
              "price",
              "loading",
              "flow_mw",
              "rating_mw",
            ],
            description:
              "Metric for ranking. Defaults to demand_mw for buses and loading for branches.",
          },
          direction: {
            type: "string",
            enum: ["asc", "desc"],
            description: "Sort direction. Defaults to desc.",
          },
          limit: {
            type: "integer",
            minimum: 1,
            maximum: 10,
            description: "Maximum rows. Defaults to 5.",
          },
        },
        ["case_id", "element_kind"],
      ),
      execute: tracked(
        "query_network",
        "Query network elements",
        validateQueryNetwork,
        (input, signal) => adapter.queryNetwork(input, signal),
      ),
      annotations: annotations(true),
    },
    {
      name: "analyze_sensitivity",
      title: "Analyze sensitivity",
      description:
        "Compute the current formulation's nodal price sensitivity for one bus demand or branch rating and return the largest responses. Leaves the visible selection unchanged.",
      inputSchema: objectSchema(
        {
          case_id: {
            type: "string",
            description: "Active case ID from inspect_case.",
          },
          target: targetSchema,
          limit: {
            type: "integer",
            minimum: 1,
            maximum: 8,
            description: "Maximum response rows. Defaults to 5.",
          },
        },
        ["case_id", "target"],
      ),
      execute: tracked(
        "analyze_sensitivity",
        "Analyze sensitivity",
        validateAnalyzeSensitivity,
        (input, signal) => adapter.analyzeSensitivity(input, signal),
      ),
      annotations: annotations(true),
    },
    {
      name: "focus_network",
      title: "Focus network element",
      description:
        "Focus a bus or branch in the tellegen interface and load its computed sensitivity view. The returned result reflects the interface after the selection has settled.",
      inputSchema: objectSchema(
        {
          case_id: {
            type: "string",
            description: "Active case ID from inspect_case.",
          },
          target: targetSchema,
        },
        ["case_id", "target"],
      ),
      execute: tracked(
        "focus_network",
        "Focus network element",
        validateFocusNetwork,
        (input, signal) => adapter.focusNetwork(input, signal),
        "commit-aware",
      ),
      annotations: annotations(false),
    },
    {
      name: "preview_case_update",
      title: "Preview case update",
      description:
        "Predict objective and nodal value changes for bounded demand or branch rating edits at the current formulation. Leaves the case and visible interface unchanged and requires the current revision.",
      inputSchema: objectSchema(
        {
          case_id: {
            type: "string",
            description: "Active case ID from inspect_case.",
          },
          expected_revision: {
            type: "string",
            description:
              "Current revision from inspect_case; protects against stale analysis.",
          },
          mode: {
            type: "string",
            enum: ["set", "increment"],
            description:
              "Set final deltas or add the supplied MW changes. Defaults to set.",
          },
          demand: {
            type: "array",
            maxItems: 24,
            description: "Bus demand delta edits in MW from the base case.",
            items: objectSchema(
              {
                bus_id: {
                  type: "string",
                  description: "Stable bus ID from query_network.",
                },
                delta_mw: {
                  type: "number",
                  description: "Demand delta in MW.",
                },
              },
              ["bus_id", "delta_mw"],
            ),
          },
          ratings: {
            type: "array",
            maxItems: 24,
            description: "Branch rating delta edits in MW from the base case.",
            items: objectSchema(
              {
                branch_id: {
                  type: "string",
                  description: "Stable branch ID from query_network.",
                },
                delta_mw: {
                  type: "number",
                  description: "Branch rating delta in MW.",
                },
              },
              ["branch_id", "delta_mw"],
            ),
          },
          limit: {
            type: "integer",
            minimum: 1,
            maximum: 8,
            description:
              "Maximum predicted nodal value changes. Defaults to 5.",
          },
        },
        ["case_id", "expected_revision"],
      ),
      execute: tracked(
        "preview_case_update",
        "Preview case update",
        validatePreviewCaseUpdate,
        (input, signal) => adapter.previewCaseUpdate(input, signal),
      ),
      annotations: annotations(true),
    },
    {
      name: "update_case",
      title: "Update and solve case",
      description:
        "Atomically set or increment demand and branch-rating deltas, optionally choose a formulation, then run the exact solve and update the visible interface. Requires the current revision.",
      inputSchema: objectSchema(
        {
          case_id: {
            type: "string",
            description: "Active case ID from inspect_case.",
          },
          expected_revision: {
            type: "string",
            description:
              "Current revision from inspect_case; protects against stale writes.",
          },
          mode: {
            type: "string",
            enum: ["set", "increment"],
            description:
              "Set final deltas or add the supplied MW changes. Defaults to set.",
          },
          demand: {
            type: "array",
            maxItems: 24,
            description: "Bus demand delta edits in MW from the base case.",
            items: objectSchema(
              {
                bus_id: {
                  type: "string",
                  description: "Stable bus ID from query_network.",
                },
                delta_mw: {
                  type: "number",
                  description: "Demand delta in MW.",
                },
              },
              ["bus_id", "delta_mw"],
            ),
          },
          ratings: {
            type: "array",
            maxItems: 24,
            description: "Branch rating delta edits in MW from the base case.",
            items: objectSchema(
              {
                branch_id: {
                  type: "string",
                  description: "Stable branch ID from query_network.",
                },
                delta_mw: {
                  type: "number",
                  description: "Branch rating delta in MW.",
                },
              },
              ["branch_id", "delta_mw"],
            ),
          },
          formulation: {
            type: "string",
            description: "Available formulation ID shown by inspect_case.",
          },
        },
        ["case_id", "expected_revision"],
      ),
      execute: tracked(
        "update_case",
        "Update and solve case",
        validateUpdateCase,
        (input, signal) => adapter.updateCase(input, signal),
        "commit-aware",
      ),
      annotations: annotations(false),
    },
    {
      name: "reset_case",
      title: "Reset case edits",
      description:
        "Clear all committed demand and branch-rating edits in the active case, keep its formulation, run the exact solve, and update the visible interface. Requires the current revision.",
      inputSchema: objectSchema(
        {
          case_id: {
            type: "string",
            description: "Active case ID from inspect_case.",
          },
          expected_revision: {
            type: "string",
            description:
              "Current revision from inspect_case; protects against stale writes.",
          },
        },
        ["case_id", "expected_revision"],
      ),
      execute: tracked(
        "reset_case",
        "Reset case edits",
        validateResetCase,
        (input, signal) => adapter.resetCase(input, signal),
        "commit-aware",
      ),
      annotations: annotations(false),
    },
  ];
}

/** Build the two capacity planning tools. They are registered dynamically so
 * proposal is available only when the active case supports it, and apply is
 * available only while a matching proposal awaits human approval. */
export function createTellegenPlanningTools(
  planning: TellegenPlanningAdapter,
  options: CreateTellegenToolsOptions = {},
): {
  planning: TellegenToolDefinition[];
  proposal: TellegenToolDefinition[];
} {
  const { outputBudget, timeoutMs } = executionLimits(options);
  let activitySequence = 0;
  const nextActivityId = () => `tellegen-plan-${++activitySequence}`;
  const tracked = <T>(
    name: string,
    title: string,
    validate: (input: unknown) => T,
    run: (input: T, signal: AbortSignal) => Promise<ToolPayload> | ToolPayload,
    completion: "abortable" | "commit-aware" = "abortable",
  ) =>
    execute(
      name,
      title,
      validate,
      run,
      outputBudget,
      timeoutMs,
      options.onActivity,
      nextActivityId,
      completion,
    );

  const proposeCapacityPlan: TellegenToolDefinition = {
    name: "propose_capacity_plan",
    title: "Propose capacity increases",
    description:
      "Use implicit derivatives to choose bounded branch capacity increase trials. Exact solves verify the staged proposal. The electrical case stays unchanged.",
    inputSchema: objectSchema(
      {
        case_id: {
          type: "string",
          description: "Active case ID from inspect_case.",
        },
        expected_revision: {
          type: "string",
          description:
            "Current revision from inspect_case; a stale revision refuses the run.",
        },
        objective: {
          ...objectSchema(
            {
              kind: {
                type: "string",
                enum: ["weighted_lmp"],
                description: "Implicit objective kind.",
              },
              weights: {
                type: "array",
                minItems: 1,
                maxItems: 16,
                description:
                  "Weighted LMP terms defining the minimized objective.",
                items: objectSchema(
                  {
                    bus_id: {
                      type: "string",
                      description: "Stable bus ID from query_network.",
                    },
                    weight: {
                      type: "number",
                      description: "Coefficient applied to this bus LMP.",
                    },
                  },
                  ["bus_id", "weight"],
                ),
              },
            },
            ["kind", "weights"],
          ),
          description: "Objective evaluated at each exact OPF solution.",
        },
        candidates: {
          type: "array",
          minItems: 1,
          maxItems: 12,
          items: { type: "string" },
          description: "Candidate branches whose capacities may increase.",
        },
        max_increase_per_branch_mw: {
          type: "number",
          exclusiveMinimum: 0,
          description: "Largest proposed increase for one branch, MW.",
        },
        budget_mw: {
          type: "number",
          exclusiveMinimum: 0,
          description: "Maximum sum of proposed capacity increases, MW.",
        },
        increment_mw: {
          type: "number",
          exclusiveMinimum: 0,
          description: "Capacity increment used to construct trials, MW.",
        },
        max_changed_lines: {
          type: "integer",
          minimum: 1,
          maximum: 12,
          description: "Maximum branches changed in the final proposal.",
        },
        exact_solve_budget: {
          type: "integer",
          minimum: 2,
          maximum: 32,
          description: "Total exact solves, including baseline and all trials.",
        },
      },
      [
        "case_id",
        "expected_revision",
        "objective",
        "candidates",
        "max_increase_per_branch_mw",
        "budget_mw",
        "increment_mw",
        "max_changed_lines",
        "exact_solve_budget",
      ],
    ),
    execute: tracked(
      "propose_capacity_plan",
      "Propose capacity increases",
      validateProposeCapacityPlan,
      (input, signal) => planning.proposeCapacityPlan(input, signal),
      "commit-aware",
    ),
    annotations: { readOnlyHint: false, untrustedContentHint: true },
  };

  const applyCapacityPlan: TellegenToolDefinition = {
    name: "apply_capacity_plan",
    title: "Apply capacity proposal",
    description:
      "After visible human approval, verify and apply the staged capacity proposal with an exact solve. A failed solve leaves the case, proposal, and approval unchanged.",
    inputSchema: objectSchema(
      {
        case_id: {
          type: "string",
          description: "Active case ID from inspect_case.",
        },
        expected_revision: {
          type: "string",
          description:
            "Current revision from inspect_case; a stale revision refuses the apply.",
        },
        proposal_id: {
          type: "string",
          description: "Proposal ID returned by propose_capacity_plan.",
        },
      },
      ["case_id", "expected_revision", "proposal_id"],
    ),
    execute: tracked(
      "apply_capacity_plan",
      "Apply capacity proposal",
      validateApplyCapacityPlan,
      (input, signal) => planning.applyCapacityPlan(input, signal),
      "commit-aware",
    ),
    annotations: { readOnlyHint: false, untrustedContentHint: true },
  };

  return {
    planning: [proposeCapacityPlan],
    proposal: [applyCapacityPlan],
  };
}
