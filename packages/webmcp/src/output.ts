/**
 * Output schemas for the WebMCP `outputSchema` member.
 *
 * Every tellegen tool resolves with the same `ToolResponse` envelope: either
 * `{ ok: true, data }` or `{ ok: false, error: { code, message } }`. The
 * envelope is exact. A tool's `data` schema names the fields the tool reports
 * and their types, but requires none of them and allows others: a bounded
 * response can drop a detailed field (a committed mutation whose details
 * exceed the output budget reports `completed` and `output_truncated`
 * instead), and hosts may add fields. An agent that validates against the
 * schema therefore never rejects a response tellegen can produce.
 */

type Schema = Record<string, unknown>;

const nullable = (type: string | string[]): Schema => ({
  type: [...(Array.isArray(type) ? type : [type]), "null"],
});

const described = (schema: Schema, description: string): Schema => ({
  ...schema,
  description,
});

/** Fields every tool's `data` can carry. */
const COMMON: Record<string, Schema> = {
  completed: described(
    { type: "boolean" },
    "True when a committed change's details exceeded the output budget.",
  ),
  output_truncated: described(
    { type: "boolean" },
    "True when detailed fields were dropped to fit the output budget.",
  ),
};

/** The displayed-case context most tools repeat. */
const CONTEXT: Record<string, Schema> = {
  case_id: described(nullable("string"), "Displayed case ID."),
  revision: described(
    nullable("string"),
    "Displayed revision; pass it back as expected_revision.",
  ),
  study_id: nullable("string"),
  state_id: described(
    nullable("string"),
    "Saved Study state shown, or null for the live case.",
  ),
  formulation: nullable("string"),
  view: { type: "string", enum: ["live_case", "saved_state"] },
  units: { type: "object" },
};

const SETTINGS: Record<string, Schema> = {
  ...CONTEXT,
  scope: { type: "string", enum: ["calculation", "display"] },
  settings_revision: described(
    nullable("integer"),
    "Pass it back as settings_revision.",
  ),
  calculation: nullable("string"),
  editable: { type: "boolean" },
  selected: {},
  accepted: {},
  pending: {},
  field: { type: "object" },
};

const DIAGNOSTICS: Schema = {
  type: "array",
  items: {
    type: "object",
    properties: {
      code: { type: "string" },
      severity: { type: "string" },
      message: { type: "string" },
    },
  },
};

/** The `data` fields of each tool. Tools absent here report an open object. */
export const DATA_FIELDS: Record<string, Record<string, Schema>> = {
  inspect_settings: SETTINGS,
  update_settings: SETTINGS,
  reset_settings: SETTINGS,
  run_calculation: SETTINGS,
  list_cases: {
    revision: nullable("string"),
    total: { type: "integer", minimum: 0 },
    next_offset: described(
      nullable("integer"),
      "Offset of the next page, or null on the last page.",
    ),
    cases: {
      type: "array",
      items: {
        type: "object",
        properties: {
          case_id: { type: "string" },
          name: { type: "string" },
          kind: { type: "string" },
          availability: { type: "string" },
          reason: { type: "string" },
          calculation: { type: "string" },
          selected: { type: "boolean" },
        },
      },
    },
  },
  select_case: {
    ...CONTEXT,
    selected: { type: "boolean" },
    network_ready: { type: "boolean" },
  },
  solve_multiconductor_pf: {
    ...CONTEXT,
    converged: { type: "boolean" },
    iterations: nullable("integer"),
    terminal_count: nullable("integer"),
    kcl_residual_a: nullable("number"),
    scaled_kcl_residual: nullable("number"),
    solve_ms: nullable("number"),
  },
  inspect_case: CONTEXT,
  query_network: {
    ...CONTEXT,
    element_kind: { type: "string", enum: ["bus", "branch"] },
    total: { type: "integer", minimum: 0 },
    matched: { type: "integer", minimum: 0 },
    returned: { type: "integer", minimum: 0 },
    truncated: { type: "boolean" },
    elements: {
      type: "array",
      items: {
        type: "object",
        properties: { element_id: { type: "string" } },
      },
    },
  },
  analyze_sensitivity: CONTEXT,
  focus_network: CONTEXT,
  preview_case_update: CONTEXT,
  update_case: CONTEXT,
  reset_case: CONTEXT,
  propose_capacity_plan: {
    proposal_id: nullable("string"),
    revision: nullable("string"),
    exact_solves: nullable("integer"),
  },
  apply_capacity_plan: {
    proposal_id: nullable("string"),
    revision: nullable("string"),
    exact_phi: nullable("number"),
    applied: { type: "array" },
    applied_count: { type: "integer", minimum: 0 },
  },
  export_case: {
    ...CONTEXT,
    format: { type: "string" },
    file_name: { type: "string" },
    bytes: { type: "integer", minimum: 0 },
    downloaded: { type: "boolean" },
    diagnostic_count: { type: "integer", minimum: 0 },
    diagnostics_truncated: { type: "boolean" },
    diagnostics: DIAGNOSTICS,
  },
  import_case: {
    ...CONTEXT,
    opened: {
      type: "array",
      items: {
        type: "object",
        properties: {
          case_id: { type: "string" },
          name: { type: "string" },
          kind: { type: "string" },
          calculation: { type: "string" },
          availability: { type: "string" },
        },
      },
    },
    diagnostic_count: { type: "integer", minimum: 0 },
    diagnostics_truncated: { type: "boolean" },
    diagnostics: DIAGNOSTICS,
  },
  inspect_webmcp_registration: {
    tools: { type: "array", items: { type: "string" } },
    registration_error: nullable("string"),
    exposed_to: { type: "array", items: { type: "string" } },
    output_budget: { type: "integer" },
    timeout_ms: { type: "integer" },
    model_context: { type: "object" },
  },
};

const STUDY_FIELDS: Record<string, Schema> = {
  id: nullable("string"),
  revision: nullable(["string", "integer"]),
  study_revision: nullable(["string", "integer"]),
  experiment: {},
  active_goal: {},
  inspected_state: {},
  recommended_state: {},
  applied_state: {},
};

/** Error codes a tool can report, beyond host-specific ones. */
export const ERROR_CODES = [
  "INVALID_INPUT",
  "INVALID_OUTPUT",
  "OUTPUT_TOO_LARGE",
  "CANCELLED",
  "TIMEOUT",
  "TOOL_FAILED",
  "STALE_CASE",
  "STALE_REVISION",
  "CASE_NOT_FOUND",
  "CASE_UNAVAILABLE",
  "CASE_NOT_READY",
  "NO_ACTIVE_CASE",
  "SAVED_STATE_READ_ONLY",
  "EXPORT_UNAVAILABLE",
  "IMPORT_FAILED",
] as const;

/** The `ToolResponse` envelope schema for one tool. */
export function responseSchema(toolName: string): Schema {
  const fields =
    DATA_FIELDS[toolName] ??
    (toolName.includes("study") ||
    toolName === "edit_demand" ||
    toolName === "restore_base_case"
      ? STUDY_FIELDS
      : {});
  return {
    oneOf: [
      {
        type: "object",
        properties: {
          ok: { const: true },
          data: {
            type: "object",
            properties: { ...COMMON, ...fields },
          },
        },
        required: ["ok", "data"],
        additionalProperties: false,
      },
      {
        type: "object",
        properties: {
          ok: { const: false },
          error: {
            type: "object",
            properties: {
              code: {
                type: "string",
                description: `A stable code such as ${ERROR_CODES.join(", ")}.`,
              },
              message: { type: "string", maxLength: 320 },
            },
            required: ["code", "message"],
            additionalProperties: false,
          },
        },
        required: ["ok", "error"],
        additionalProperties: false,
      },
    ],
  };
}

function typeMatches(type: string, value: unknown): boolean {
  switch (type) {
    case "null":
      return value === null;
    case "boolean":
      return typeof value === "boolean";
    case "string":
      return typeof value === "string";
    case "number":
      return typeof value === "number" && Number.isFinite(value);
    case "integer":
      return typeof value === "number" && Number.isInteger(value);
    case "array":
      return Array.isArray(value);
    case "object":
      return !!value && typeof value === "object" && !Array.isArray(value);
    default:
      return false;
  }
}

/**
 * Check a value against the JSON Schema subset tellegen's input and output
 * schemas use (`type`, `const`, `enum`, `properties`, `required`,
 * `additionalProperties: false`, `items`, `oneOf`, `minimum`, `maximum`,
 * `minLength`, `maxLength`, `maxItems`, `pattern`). Returns one message per
 * violation, empty when the value conforms. Hosts can use it in tests to check
 * that their adapter's results match each tool's `outputSchema`.
 */
export function schemaErrors(
  schema: Record<string, unknown>,
  value: unknown,
  path = "$",
): string[] {
  const errors: string[] = [];
  if (Array.isArray(schema.oneOf)) {
    const matching = (schema.oneOf as Record<string, unknown>[]).filter(
      (branch) => schemaErrors(branch, value, path).length === 0,
    );
    if (matching.length !== 1)
      errors.push(
        `${path}: matches ${matching.length} of the oneOf branches, not exactly one`,
      );
  }
  if ("const" in schema && value !== schema.const)
    errors.push(`${path}: must be ${JSON.stringify(schema.const)}`);
  if (Array.isArray(schema.enum) && !schema.enum.includes(value as never))
    errors.push(`${path}: must be one of ${schema.enum.join(", ")}`);
  if (schema.type !== undefined) {
    const types = Array.isArray(schema.type) ? schema.type : [schema.type];
    if (!types.some((type) => typeMatches(String(type), value))) {
      errors.push(`${path}: must be ${types.join(" or ")}`);
      return errors;
    }
  }
  if (typeof value === "number") {
    if (typeof schema.minimum === "number" && value < schema.minimum)
      errors.push(`${path}: must be at least ${schema.minimum}`);
    if (typeof schema.maximum === "number" && value > schema.maximum)
      errors.push(`${path}: must be at most ${schema.maximum}`);
  }
  if (typeof value === "string") {
    if (typeof schema.minLength === "number" && value.length < schema.minLength)
      errors.push(`${path}: must have at least ${schema.minLength} characters`);
    if (typeof schema.maxLength === "number" && value.length > schema.maxLength)
      errors.push(`${path}: must have at most ${schema.maxLength} characters`);
    if (
      typeof schema.pattern === "string" &&
      !new RegExp(schema.pattern, "u").test(value)
    )
      errors.push(`${path}: must match ${schema.pattern}`);
  }
  if (Array.isArray(value)) {
    if (typeof schema.maxItems === "number" && value.length > schema.maxItems)
      errors.push(`${path}: must have at most ${schema.maxItems} items`);
    if (schema.items && typeof schema.items === "object")
      value.forEach((item, index) =>
        errors.push(
          ...schemaErrors(
            schema.items as Record<string, unknown>,
            item,
            `${path}[${index}]`,
          ),
        ),
      );
  }
  if (value && typeof value === "object" && !Array.isArray(value)) {
    const record = value as Record<string, unknown>;
    const properties = (schema.properties ?? {}) as Record<
      string,
      Record<string, unknown>
    >;
    for (const key of (schema.required ?? []) as string[])
      if (!(key in record)) errors.push(`${path}: missing ${key}`);
    for (const [key, item] of Object.entries(record)) {
      if (key in properties)
        errors.push(...schemaErrors(properties[key], item, `${path}.${key}`));
      else if (schema.additionalProperties === false)
        errors.push(`${path}: unknown field ${key}`);
    }
  }
  return errors;
}
