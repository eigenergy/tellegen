import type {
  AnalyzeSensitivityInput,
  ExportCaseInput,
  ImportCaseInput,
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
  ListCasesInput,
  SelectCaseInput,
  SimulateDynamicsEvent,
  SimulateDynamicsInput,
  SolveMulticonductorInput,
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
  "voltage_pu",
  "voltage_v",
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

export function validateListCases(input: unknown): ListCasesInput {
  const value = object(input ?? {}, "input");
  exactKeys(value, ["offset", "limit"], "input");
  return {
    offset:
      value.offset === undefined
        ? 0
        : integer(value.offset, "offset", 0, 100_000),
    limit:
      value.limit === undefined ? 10 : integer(value.limit, "limit", 1, 20),
  };
}

export function validateSolveMulticonductor(
  input: unknown,
): SolveMulticonductorInput {
  const value = object(input, "input");
  exactKeys(value, ["case_id", "expected_revision", "max_iterations"], "input");
  return {
    caseId: string(value.case_id, "case_id"),
    expectedRevision: string(value.expected_revision, "expected_revision"),
    maxIterations:
      value.max_iterations === undefined
        ? undefined
        : integer(value.max_iterations, "max_iterations", 1, 10000),
  };
}

export function validateSelectCase(input: unknown): SelectCaseInput {
  const value = object(input, "input");
  exactKeys(value, ["case_id", "expected_revision"], "input");
  return {
    caseId: string(value.case_id, "case_id"),
    ...(value.expected_revision === undefined
      ? {}
      : {
          expectedRevision: string(
            value.expected_revision,
            "expected_revision",
          ),
        }),
  };
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

export function validateSettings(
  input: unknown,
  action: import("./types.js").SettingsInput["action"],
): import("./types.js").SettingsInput {
  const value = object(input, "input");
  exactKeys(
    value,
    [
      "scope",
      "case_id",
      "expected_revision",
      "settings_revision",
      "calculation",
      "field",
      "patch",
    ],
    "input",
  );
  if (action !== "inspect" && value.field !== undefined)
    throw new TellegenToolError(
      "INVALID_INPUT",
      "field is only supported by inspection",
    );
  if (action !== "update" && value.patch !== undefined)
    throw new TellegenToolError(
      "INVALID_INPUT",
      "patch is only supported by update_settings",
    );
  if (
    (action === "inspect" || action === "run") &&
    value.calculation !== undefined
  )
    throw new TellegenToolError(
      "INVALID_INPUT",
      "calculation can be selected by update_settings or reset_settings",
    );
  const scope = value.scope ?? "calculation";
  if (scope !== "calculation" && scope !== "display")
    throw new TellegenToolError(
      "INVALID_INPUT",
      "scope must be calculation or display",
    );
  if (
    scope === "display" &&
    (value.calculation !== undefined || value.field !== undefined)
  )
    throw new TellegenToolError(
      "INVALID_INPUT",
      "display scope has no calculation fields",
    );
  if (
    action !== "inspect" &&
    scope === "calculation" &&
    (value.case_id === undefined || value.expected_revision === undefined)
  )
    throw new TellegenToolError(
      "INVALID_INPUT",
      "case_id and expected_revision are required",
    );
  if (action !== "inspect" && value.settings_revision === undefined)
    throw new TellegenToolError(
      "INVALID_INPUT",
      "settings_revision is required",
    );
  const patch =
    value.patch === undefined ? undefined : object(value.patch, "patch");
  if (
    patch &&
    (Object.keys(patch).length > 24 || JSON.stringify(patch).length > 4096)
  )
    throw new TellegenToolError("INVALID_INPUT", "patch is too large");
  if (action === "update" && !patch)
    throw new TellegenToolError("INVALID_INPUT", "patch is required");
  if (action === "run" && scope === "display")
    throw new TellegenToolError(
      "INVALID_INPUT",
      "display preferences do not run calculations",
    );
  return {
    action,
    scope,
    caseId:
      value.case_id === undefined
        ? undefined
        : string(value.case_id, "case_id"),
    expectedRevision:
      value.expected_revision === undefined
        ? undefined
        : string(value.expected_revision, "expected_revision"),
    settingsRevision:
      value.settings_revision === undefined
        ? undefined
        : integer(
            value.settings_revision,
            "settings_revision",
            0,
            Number.MAX_SAFE_INTEGER,
          ),
    calculation:
      value.calculation === undefined
        ? undefined
        : string(value.calculation, "calculation"),
    field: value.field === undefined ? undefined : string(value.field, "field"),
    patch,
  };
}

export function validateExportCase(
  input: unknown,
  formats: readonly string[],
): ExportCaseInput {
  const value = object(input, "input");
  exactKeys(value, ["case_id", "expected_revision", "format"], "input");
  const format = string(value.format, "format");
  if (!formats.includes(format))
    throw new TellegenToolError(
      "INVALID_INPUT",
      `format must be one of ${formats.join(", ")}`,
    );
  return {
    caseId: string(value.case_id, "case_id"),
    expectedRevision: string(value.expected_revision, "expected_revision"),
    format,
  };
}

/** Text case formats `import_case` opens, by file extension. */
export const IMPORT_EXTENSIONS: readonly string[] = [
  "m",
  "raw",
  "rawx",
  "aux",
  "epc",
  "dss",
  "json",
  "xml",
  "xiidm",
  "uct",
  "ucte",
  "cdf",
  "dyr",
];
const IMPORT_EXTENSION_SET = new Set(IMPORT_EXTENSIONS);
/** Largest case text `import_case` accepts, in UTF-16 code units. */
export const MAX_IMPORT_TEXT = 4_000_000;

export function validateImportCase(input: unknown): ImportCaseInput {
  const value = object(input, "input");
  exactKeys(value, ["file_name", "text"], "input");
  const fileName = string(value.file_name, "file_name");
  const extension = /\.([A-Za-z0-9]+)$/.exec(fileName)?.[1]?.toLowerCase();
  if (/[/\\\u0000-\u001f]/.test(fileName) || !extension)
    throw new TellegenToolError(
      "INVALID_INPUT",
      "file_name must be a plain file name with an extension, such as case14.m",
    );
  if (!IMPORT_EXTENSION_SET.has(extension))
    throw new TellegenToolError(
      "INVALID_INPUT",
      `import_case opens ${[...IMPORT_EXTENSION_SET].map((ext) => `.${ext}`).join(", ")} text; use the file picker for .${extension}`,
    );
  if (typeof value.text !== "string" || value.text.trim().length === 0)
    throw new TellegenToolError("INVALID_INPUT", "text must be nonempty");
  if (value.text.length > MAX_IMPORT_TEXT)
    throw new TellegenToolError(
      "INVALID_INPUT",
      `text exceeds ${MAX_IMPORT_TEXT} characters; use the file picker`,
    );
  return { fileName, text: value.text };
}

/** Traces `simulate_dynamics` may return. */
export const DYNAMICS_CHANNEL =
  /^(coi:f|f:\d+|vm:\d+|omega:\d+(#\w+)?|dcoi:\d+(#\w+)?|pl:\d+)$/;
const DYN_DT = [0.001, 0.0025, 0.005] as const;
const LOAD_MODELS = [
  "power_flow",
  "constant_impedance",
  "constant_current",
  "constant_power",
] as const;

function busNumber(value: unknown, label: string): number {
  return integer(value, label, 0, 2 ** 31 - 1);
}

function seconds(
  value: unknown,
  label: string,
  min: number,
  max: number,
): number {
  const v = number(value, label);
  if (v < min || v > max)
    throw new TellegenToolError(
      "INVALID_INPUT",
      `${label} must lie in [${min}, ${max}] s`,
    );
  return v;
}

const BRANCH_ID = /^\d+-\d+(#\w+)?$/;
const UNIT_ID = /^\d+(#\w+)?$/;

export function validateSimulateDynamics(
  input: unknown,
): SimulateDynamicsInput {
  const value = object(input, "input");
  exactKeys(
    value,
    [
      "case_id",
      "expected_revision",
      "events",
      "t_end_s",
      "dt_s",
      "load_model",
      "channels",
      "trace_points",
      "show_in_app",
    ],
    "input",
  );
  const tEndS =
    value.t_end_s === undefined
      ? 20
      : seconds(value.t_end_s, "t_end_s", 0.1, 60);
  if (
    !Array.isArray(value.events) ||
    value.events.length < 1 ||
    value.events.length > 16
  )
    throw new TellegenToolError(
      "INVALID_INPUT",
      "events must hold 1 to 16 events",
    );
  const events = value.events.map((raw, k): SimulateDynamicsEvent => {
    const label = `events[${k}]`;
    const e = object(raw, label);
    const atS = seconds(e.at_s, `${label}.at_s`, 0, tEndS);
    if (atS >= tEndS)
      throw new TellegenToolError(
        "INVALID_INPUT",
        `${label}.at_s must be before t_end_s`,
      );
    const clear = () => {
      const c =
        e.clear_after_s === undefined
          ? 0.1
          : seconds(e.clear_after_s, `${label}.clear_after_s`, 0.001, 10);
      if (atS + c >= tEndS)
        throw new TellegenToolError(
          "INVALID_INPUT",
          `${label} must clear before t_end_s`,
        );
      return c;
    };
    const branch = () => {
      const b = string(e.branch, `${label}.branch`);
      if (!BRANCH_ID.test(b))
        throw new TellegenToolError(
          "INVALID_INPUT",
          `${label}.branch must read from-to or from-to#circuit, such as 1101-1401#1`,
        );
      return b;
    };
    switch (e.kind) {
      case "bus_fault": {
        exactKeys(
          e,
          ["kind", "bus", "at_s", "clear_after_s", "x_pu", "r_pu"],
          label,
        );
        const xPu =
          e.x_pu === undefined ? 1e-4 : number(e.x_pu, `${label}.x_pu`);
        const rPu = e.r_pu === undefined ? 0 : number(e.r_pu, `${label}.r_pu`);
        if (xPu < 0 || rPu < 0 || xPu + rPu <= 0)
          throw new TellegenToolError(
            "INVALID_INPUT",
            `${label} needs r_pu, x_pu >= 0 and a nonzero impedance`,
          );
        return {
          kind: "bus_fault",
          bus: busNumber(e.bus, `${label}.bus`),
          atS,
          clearAfterS: clear(),
          xPu,
          rPu,
        };
      }
      case "line_fault":
        exactKeys(
          e,
          ["kind", "branch", "near_bus", "at_s", "clear_after_s"],
          label,
        );
        return {
          kind: "line_fault",
          branch: branch(),
          ...(e.near_bus === undefined
            ? {}
            : { nearBus: busNumber(e.near_bus, `${label}.near_bus`) }),
          atS,
          clearAfterS: clear(),
        };
      case "branch_trip":
        exactKeys(e, ["kind", "branch", "at_s"], label);
        return { kind: "branch_trip", branch: branch(), atS };
      case "generator_trip": {
        exactKeys(e, ["kind", "unit", "at_s"], label);
        const unit = string(e.unit, `${label}.unit`);
        if (!UNIT_ID.test(unit))
          throw new TellegenToolError(
            "INVALID_INPUT",
            `${label}.unit must read bus or bus#id, such as 1431#N`,
          );
        return { kind: "generator_trip", unit, atS };
      }
      case "load_change": {
        exactKeys(e, ["kind", "bus", "at_s", "scale"], label);
        const scale = number(e.scale, `${label}.scale`);
        if (scale < 0 || scale > 10)
          throw new TellegenToolError(
            "INVALID_INPUT",
            `${label}.scale must lie in [0, 10]`,
          );
        return {
          kind: "load_change",
          bus: busNumber(e.bus, `${label}.bus`),
          atS,
          scale,
        };
      }
      default:
        throw new TellegenToolError(
          "INVALID_INPUT",
          `${label}.kind must be bus_fault, line_fault, branch_trip, generator_trip or load_change`,
        );
    }
  });
  const dtS = value.dt_s === undefined ? 0.0025 : number(value.dt_s, "dt_s");
  if (!(DYN_DT as readonly number[]).includes(dtS))
    throw new TellegenToolError(
      "INVALID_INPUT",
      "dt_s must be 0.001, 0.0025 or 0.005",
    );
  let loadModel: SimulateDynamicsInput["loadModel"];
  if (value.load_model !== undefined) {
    if (!(LOAD_MODELS as readonly unknown[]).includes(value.load_model))
      throw new TellegenToolError(
        "INVALID_INPUT",
        `load_model must be one of ${LOAD_MODELS.join(", ")}`,
      );
    loadModel = value.load_model as SimulateDynamicsInput["loadModel"];
  }
  const channels =
    value.channels === undefined
      ? []
      : Array.isArray(value.channels) && value.channels.length <= 4
        ? value.channels.map((c, k) => {
            const ch = string(c, `channels[${k}]`);
            if (!DYNAMICS_CHANNEL.test(ch))
              throw new TellegenToolError(
                "INVALID_INPUT",
                `channels[${k}] must be coi:f, f:<bus>, vm:<bus>, omega:<unit>, dcoi:<unit> or pl:<bus>`,
              );
            return ch;
          })
        : (() => {
            throw new TellegenToolError(
              "INVALID_INPUT",
              "channels must hold at most 4 channels",
            );
          })();
  const tracePoints =
    value.trace_points === undefined
      ? 0
      : integer(value.trace_points, "trace_points", 0, 40);
  if (value.show_in_app !== undefined && typeof value.show_in_app !== "boolean")
    throw new TellegenToolError(
      "INVALID_INPUT",
      "show_in_app must be a boolean",
    );
  return {
    caseId: string(value.case_id, "case_id"),
    expectedRevision: string(value.expected_revision, "expected_revision"),
    events,
    tEndS,
    dtS: dtS as SimulateDynamicsInput["dtS"],
    ...(loadModel ? { loadModel } : {}),
    channels,
    tracePoints,
    showInApp: value.show_in_app !== false,
  };
}
