import type {
  AnalyzeSensitivityInput,
  ApplyCapacityPlanInput,
  CapacityPlanBusWeight,
  DemandEdit,
  EditMode,
  ElementKind,
  ElementTarget,
  FocusNetworkInput,
  ProposeCapacityPlanInput,
  PreviewCaseUpdateInput,
  QueryNetworkInput,
  RatingEdit,
  ResetCaseInput,
  SortDirection,
  UpdateCaseInput,
} from "./types.js";
import { TellegenToolError } from "./types.js";

const MAX_ID_LENGTH = 128;
const MAX_EDITS = 24;
const SORT_FIELDS = new Set([
  "id",
  "demand_mw",
  "generation_mw",
  "price",
  "loading",
  "flow_mw",
  "rating_mw",
]);

function object(value: unknown, label: string): Record<string, unknown> {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    throw new TellegenToolError("INVALID_INPUT", `${label} must be an object`);
  }
  return value as Record<string, unknown>;
}

function exactKeys(
  value: Record<string, unknown>,
  allowed: readonly string[],
  label: string,
): void {
  const extra = Object.keys(value).find((key) => !allowed.includes(key));
  if (extra)
    throw new TellegenToolError(
      "INVALID_INPUT",
      `${label} has unknown field ${extra}`,
    );
}

function string(value: unknown, label: string): string {
  if (
    typeof value !== "string" ||
    value.length === 0 ||
    value.length > MAX_ID_LENGTH
  ) {
    throw new TellegenToolError(
      "INVALID_INPUT",
      `${label} must be a nonempty string of at most ${MAX_ID_LENGTH} characters`,
    );
  }
  return value;
}

function number(value: unknown, label: string): number {
  if (typeof value !== "number" || !Number.isFinite(value)) {
    throw new TellegenToolError(
      "INVALID_INPUT",
      `${label} must be a finite number`,
    );
  }
  return value;
}

function integer(
  value: unknown,
  label: string,
  min: number,
  max: number,
): number {
  const parsed = number(value, label);
  if (!Number.isInteger(parsed) || parsed < min || parsed > max) {
    throw new TellegenToolError(
      "INVALID_INPUT",
      `${label} must be an integer from ${min} to ${max}`,
    );
  }
  return parsed;
}

function kind(value: unknown): ElementKind {
  if (value !== "bus" && value !== "branch") {
    throw new TellegenToolError("INVALID_INPUT", "kind must be bus or branch");
  }
  return value;
}

function target(value: unknown): ElementTarget {
  const input = object(value, "target");
  exactKeys(input, ["kind", "element_id"], "target");
  return {
    kind: kind(input.kind),
    elementId: string(input.element_id, "target.element_id"),
  };
}

function editArray<T>(
  value: unknown,
  label: string,
  idKey: "bus_id" | "branch_id",
  map: (id: string, deltaMw: number) => T,
): T[] {
  if (value === undefined) return [];
  if (!Array.isArray(value) || value.length > MAX_EDITS) {
    throw new TellegenToolError(
      "INVALID_INPUT",
      `${label} must be an array with at most ${MAX_EDITS} entries`,
    );
  }
  const seen = new Set<string>();
  return value.map((entry, index) => {
    const row = object(entry, `${label}[${index}]`);
    exactKeys(row, [idKey, "delta_mw"], `${label}[${index}]`);
    const id = string(row[idKey], `${label}[${index}].${idKey}`);
    if (seen.has(id)) {
      throw new TellegenToolError(
        "INVALID_INPUT",
        `${label} repeats element ${id}`,
      );
    }
    seen.add(id);
    return map(id, number(row.delta_mw, `${label}[${index}].delta_mw`));
  });
}

export function validateEmpty(input: unknown): Record<string, never> {
  const value = object(input ?? {}, "input");
  exactKeys(value, [], "input");
  return {};
}

export function validateQueryNetwork(input: unknown): QueryNetworkInput {
  const value = object(input, "input");
  exactKeys(
    value,
    ["case_id", "element_kind", "element_ids", "sort_by", "direction", "limit"],
    "input",
  );
  let elementIds: string[] | undefined;
  if (value.element_ids !== undefined) {
    if (!Array.isArray(value.element_ids) || value.element_ids.length > 10) {
      throw new TellegenToolError(
        "INVALID_INPUT",
        "element_ids must contain at most 10 strings",
      );
    }
    elementIds = value.element_ids.map((id, index) =>
      string(id, `element_ids[${index}]`),
    );
  }
  let sortBy: QueryNetworkInput["sortBy"];
  if (value.sort_by !== undefined) {
    const candidate = string(value.sort_by, "sort_by");
    if (!SORT_FIELDS.has(candidate)) {
      throw new TellegenToolError(
        "INVALID_INPUT",
        `unsupported sort_by ${candidate}`,
      );
    }
    sortBy = candidate as QueryNetworkInput["sortBy"];
  }
  const direction: SortDirection =
    value.direction === undefined ? "desc" : (value.direction as SortDirection);
  if (direction !== "asc" && direction !== "desc") {
    throw new TellegenToolError(
      "INVALID_INPUT",
      "direction must be asc or desc",
    );
  }
  return {
    caseId: string(value.case_id, "case_id"),
    elementKind: kind(value.element_kind),
    elementIds,
    sortBy,
    direction,
    limit: value.limit === undefined ? 5 : integer(value.limit, "limit", 1, 10),
  };
}

export function validateAnalyzeSensitivity(
  input: unknown,
): AnalyzeSensitivityInput {
  const value = object(input, "input");
  exactKeys(value, ["case_id", "target", "limit"], "input");
  return {
    caseId: string(value.case_id, "case_id"),
    target: target(value.target),
    limit: value.limit === undefined ? 5 : integer(value.limit, "limit", 1, 8),
  };
}

export function validateFocusNetwork(input: unknown): FocusNetworkInput {
  const value = object(input, "input");
  exactKeys(value, ["case_id", "target"], "input");
  return {
    caseId: string(value.case_id, "case_id"),
    target: target(value.target),
  };
}

export function validatePreviewCaseUpdate(
  input: unknown,
): PreviewCaseUpdateInput {
  const value = object(input, "input");
  exactKeys(
    value,
    ["case_id", "expected_revision", "mode", "demand", "ratings", "limit"],
    "input",
  );
  const mode: EditMode =
    value.mode === undefined ? "set" : (value.mode as EditMode);
  if (mode !== "set" && mode !== "increment") {
    throw new TellegenToolError(
      "INVALID_INPUT",
      "mode must be set or increment",
    );
  }
  const demand = editArray<DemandEdit>(
    value.demand,
    "demand",
    "bus_id",
    (busId, deltaMw) => ({ busId, deltaMw }),
  );
  const ratings = editArray<RatingEdit>(
    value.ratings,
    "ratings",
    "branch_id",
    (branchId, deltaMw) => ({ branchId, deltaMw }),
  );
  if (demand.length === 0 && ratings.length === 0) {
    throw new TellegenToolError(
      "INVALID_INPUT",
      "provide at least one demand or rating edit",
    );
  }
  return {
    caseId: string(value.case_id, "case_id"),
    expectedRevision: string(value.expected_revision, "expected_revision"),
    mode,
    demand,
    ratings,
    limit: value.limit === undefined ? 5 : integer(value.limit, "limit", 1, 8),
  };
}

export function validateUpdateCase(input: unknown): UpdateCaseInput {
  const value = object(input, "input");
  exactKeys(
    value,
    [
      "case_id",
      "expected_revision",
      "mode",
      "demand",
      "ratings",
      "formulation",
    ],
    "input",
  );
  const mode: EditMode =
    value.mode === undefined ? "set" : (value.mode as EditMode);
  if (mode !== "set" && mode !== "increment") {
    throw new TellegenToolError(
      "INVALID_INPUT",
      "mode must be set or increment",
    );
  }
  const demand = editArray<DemandEdit>(
    value.demand,
    "demand",
    "bus_id",
    (busId, deltaMw) => ({
      busId,
      deltaMw,
    }),
  );
  const ratings = editArray<RatingEdit>(
    value.ratings,
    "ratings",
    "branch_id",
    (branchId, deltaMw) => ({ branchId, deltaMw }),
  );
  const formulation =
    value.formulation === undefined
      ? undefined
      : string(value.formulation, "formulation");
  if (
    demand.length === 0 &&
    ratings.length === 0 &&
    formulation === undefined
  ) {
    throw new TellegenToolError(
      "INVALID_INPUT",
      "provide at least one demand edit, rating edit, or formulation",
    );
  }
  return {
    caseId: string(value.case_id, "case_id"),
    expectedRevision: string(value.expected_revision, "expected_revision"),
    mode,
    demand,
    ratings,
    formulation,
  };
}

export function validateResetCase(input: unknown): ResetCaseInput {
  const value = object(input, "input");
  exactKeys(value, ["case_id", "expected_revision"], "input");
  return {
    caseId: string(value.case_id, "case_id"),
    expectedRevision: string(value.expected_revision, "expected_revision"),
  };
}

const MAX_PLAN_WEIGHTS = 16;
const MAX_PLAN_CANDIDATES = 12;
const MAX_PLAN_MW = 10_000;

function positiveMw(value: unknown, label: string): number {
  const parsed = number(value, label);
  if (parsed <= 0 || parsed > MAX_PLAN_MW) {
    throw new TellegenToolError(
      "INVALID_INPUT",
      `${label} must be greater than 0 and at most ${MAX_PLAN_MW} MW`,
    );
  }
  return parsed;
}

export function validateProposeCapacityPlan(
  input: unknown,
): ProposeCapacityPlanInput {
  const value = object(input, "input");
  exactKeys(
    value,
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
    "input",
  );
  const objective = object(value.objective, "objective");
  exactKeys(objective, ["kind", "weights"], "objective");
  if (objective.kind !== "weighted_lmp") {
    throw new TellegenToolError(
      "INVALID_INPUT",
      "objective.kind must be weighted_lmp",
    );
  }
  if (!Array.isArray(objective.weights) || objective.weights.length === 0) {
    throw new TellegenToolError(
      "INVALID_INPUT",
      `objective.weights must be a nonempty array with at most ${MAX_PLAN_WEIGHTS} entries`,
    );
  }
  if (objective.weights.length > MAX_PLAN_WEIGHTS) {
    throw new TellegenToolError(
      "INVALID_INPUT",
      `objective.weights must contain at most ${MAX_PLAN_WEIGHTS} entries`,
    );
  }
  const seenBuses = new Set<string>();
  const weights: CapacityPlanBusWeight[] = objective.weights.map(
    (entry, index) => {
      const row = object(entry, `objective.weights[${index}]`);
      exactKeys(row, ["bus_id", "weight"], `objective.weights[${index}]`);
      const busId = string(row.bus_id, `objective.weights[${index}].bus_id`);
      if (seenBuses.has(busId)) {
        throw new TellegenToolError(
          "INVALID_INPUT",
          `objective.weights repeats bus ${busId}`,
        );
      }
      seenBuses.add(busId);
      return {
        busId,
        weight: number(row.weight, `objective.weights[${index}].weight`),
      };
    },
  );
  if (
    !Array.isArray(value.candidates) ||
    value.candidates.length === 0 ||
    value.candidates.length > MAX_PLAN_CANDIDATES
  ) {
    throw new TellegenToolError(
      "INVALID_INPUT",
      `candidates must be a nonempty array with at most ${MAX_PLAN_CANDIDATES} branch IDs`,
    );
  }
  const seenBranches = new Set<string>();
  const candidates = value.candidates.map((entry, index) => {
    const id = string(entry, `candidates[${index}]`);
    if (seenBranches.has(id)) {
      throw new TellegenToolError(
        "INVALID_INPUT",
        `candidates repeats branch ${id}`,
      );
    }
    seenBranches.add(id);
    return id;
  });
  const maxIncreasePerBranchMw = positiveMw(
    value.max_increase_per_branch_mw,
    "max_increase_per_branch_mw",
  );
  const budgetMw = positiveMw(value.budget_mw, "budget_mw");
  const incrementMw = positiveMw(value.increment_mw, "increment_mw");
  if (incrementMw > maxIncreasePerBranchMw) {
    throw new TellegenToolError(
      "INVALID_INPUT",
      "increment_mw must not exceed max_increase_per_branch_mw",
    );
  }
  if (incrementMw > budgetMw) {
    throw new TellegenToolError(
      "INVALID_INPUT",
      "increment_mw must not exceed budget_mw",
    );
  }
  const maxChangedLines = integer(
    value.max_changed_lines,
    "max_changed_lines",
    1,
    MAX_PLAN_CANDIDATES,
  );
  if (maxChangedLines > candidates.length) {
    throw new TellegenToolError(
      "INVALID_INPUT",
      "max_changed_lines must not exceed the number of candidate branches",
    );
  }
  return {
    caseId: string(value.case_id, "case_id"),
    expectedRevision: string(value.expected_revision, "expected_revision"),
    objective: { kind: "weighted_lmp", weights },
    candidates,
    maxIncreasePerBranchMw,
    budgetMw,
    incrementMw,
    maxChangedLines,
    exactSolveBudget: integer(
      value.exact_solve_budget,
      "exact_solve_budget",
      2,
      32,
    ),
  };
}

export function validateApplyCapacityPlan(
  input: unknown,
): ApplyCapacityPlanInput {
  const value = object(input, "input");
  exactKeys(value, ["case_id", "expected_revision", "proposal_id"], "input");
  return {
    caseId: string(value.case_id, "case_id"),
    expectedRevision: string(value.expected_revision, "expected_revision"),
    proposalId: string(value.proposal_id, "proposal_id"),
  };
}
