import { describe, expect, it } from "vitest";
import { responseSchema, schemaErrors } from "./output.js";
import { registerTellegenWebMcp } from "./register.js";
import {
  createTellegenPlanningTools,
  createTellegenStudyTools,
  createTellegenTools,
} from "./tools.js";
import type {
  TellegenToolDefinition,
  TellegenWebMcpAdapter,
  ToolPayload,
} from "./types.js";

const base: TellegenWebMcpAdapter = {
  inspectCase: () => ({ case_id: "c", revision: "r1", view: "live_case" }),
  queryNetwork: () => ({
    case_id: "c",
    revision: "r1",
    element_kind: "bus",
    total: 3,
    matched: 3,
    returned: 1,
    truncated: true,
    elements: [{ element_id: "bus:1", demand_mw: 10 }],
  }),
  analyzeSensitivity: () => ({}),
  focusNetwork: () => ({}),
  previewCaseUpdate: () => ({}),
  updateCase: () => ({ revision: "r2", formulation: "dcopf" }),
  resetCase: () => ({}),
};

const full: TellegenWebMcpAdapter = {
  ...base,
  settings: () => ({ scope: "display", settings_revision: 2 }),
  cases: {
    listCases: () => ({
      revision: "r1",
      total: 1,
      next_offset: null,
      cases: [
        {
          case_id: "c",
          name: "case14",
          kind: "local",
          availability: "ready",
          calculation: "balanced",
          selected: true,
        },
      ],
    }),
    selectCase: () => ({ case_id: "c", selected: true, network_ready: true }),
  },
  solveMulticonductorPowerFlow: () => ({ converged: true, iterations: 4 }),
  files: {
    exportFormats: [{ token: "matpower", label: "MATPOWER (.m)" }],
    exportCase: () => ({
      format: "matpower",
      file_name: "case.m",
      bytes: 120,
      downloaded: true,
      diagnostic_count: 0,
      diagnostics: [],
    }),
    importCase: () => ({
      case_id: "local-1",
      opened: [{ case_id: "local-1", name: "case14", kind: "local" }],
      diagnostic_count: 0,
      diagnostics: [],
    }),
  },
  studies: {
    inputSchema: () => ({ type: "object" }),
    execute: () => ({ id: "s", revision: 3 }),
  },
};

function everyTool(): TellegenToolDefinition[] {
  const planning = createTellegenPlanningTools({
    proposeCapacityPlan: () => ({ proposal_id: "p", exact_solves: 2 }),
    applyCapacityPlan: () => ({ proposal_id: "p", applied: [] }),
    planningAvailable: () => true,
    proposalAvailable: () => true,
    onAvailabilityChange: () => () => {},
  });
  return [
    ...createTellegenTools(full),
    ...createTellegenStudyTools(full.studies!),
    ...planning.planning,
    ...planning.proposal,
  ];
}

describe("output schemas", () => {
  it("gives every tool a serializable response envelope schema", () => {
    for (const tool of everyTool()) {
      expect(tool.outputSchema, tool.name).toEqual(responseSchema(tool.name));
      expect(JSON.parse(JSON.stringify(tool.outputSchema))).toEqual(
        tool.outputSchema,
      );
      const branches = tool.outputSchema.oneOf as unknown[];
      expect(branches, tool.name).toHaveLength(2);
    }
  });

  it("accepts the success, failure and truncated responses tools return", () => {
    const success = {
      ok: true,
      data: { completed: true, output_truncated: true },
    };
    const failure = {
      ok: false,
      error: { code: "STALE_REVISION", message: "inspect_case first" },
    };
    for (const tool of everyTool()) {
      expect(schemaErrors(tool.outputSchema, success), tool.name).toEqual([]);
      expect(schemaErrors(tool.outputSchema, failure), tool.name).toEqual([]);
    }
  });

  it("validates each tool's live response against its own schema", async () => {
    const inputs: Record<string, Record<string, unknown>> = {
      list_cases: {},
      select_case: { case_id: "c" },
      solve_multiconductor_pf: { case_id: "c", expected_revision: "r1" },
      inspect_case: {},
      query_network: { case_id: "c", element_kind: "bus" },
      update_case: {
        case_id: "c",
        expected_revision: "r1",
        demand: [{ bus_id: "bus:1", delta_mw: 5 }],
      },
      export_case: {
        case_id: "c",
        expected_revision: "r1",
        format: "matpower",
      },
      import_case: { file_name: "case14.m", text: "function mpc = case14" },
      inspect_settings: { scope: "display" },
      propose_capacity_plan: {},
      create_study: {},
    };
    for (const tool of everyTool()) {
      const input = inputs[tool.name];
      if (!input) continue;
      const response = await tool.execute(input);
      if (tool.name !== "propose_capacity_plan")
        expect(response.ok, `${tool.name}: ${JSON.stringify(response)}`).toBe(
          true,
        );
      expect(schemaErrors(tool.outputSchema, response), tool.name).toEqual([]);
    }
  });

  it("rejects responses outside the envelope", () => {
    const schema = responseSchema("query_network");
    expect(schemaErrors(schema, { ok: true })).not.toEqual([]);
    expect(
      schemaErrors(schema, { ok: false, error: { code: "X" } }),
    ).not.toEqual([]);
    expect(
      schemaErrors(schema, { ok: true, data: { total: "three" } }),
    ).not.toEqual([]);
    expect(schemaErrors(schema, { ok: true, data: {}, extra: 1 })).not.toEqual(
      [],
    );
  });

  it("keeps the declared input schemas satisfiable by valid requests", () => {
    const tools = createTellegenTools(full);
    const importCase = tools.find((tool) => tool.name === "import_case")!;
    expect(
      schemaErrors(importCase.inputSchema, {
        file_name: "case14.m",
        text: "x",
      }),
    ).toEqual([]);
    expect(
      schemaErrors(importCase.inputSchema, {
        file_name: "../case.m",
        text: "x",
      }),
    ).not.toEqual([]);
  });
});

describe("debug tool", () => {
  it("registers inspect_webmcp_registration only on request", async () => {
    const names: string[] = [];
    const context = {
      async registerTool(tool: TellegenToolDefinition) {
        names.push(tool.name);
      },
    };
    const plain = await registerTellegenWebMcp(context, base);
    expect(plain.toolNames).not.toContain("inspect_webmcp_registration");
    plain.dispose();

    const tools: TellegenToolDefinition[] = [];
    const debug = await registerTellegenWebMcp(
      {
        async registerTool(tool) {
          tools.push(tool);
        },
      },
      base,
      { debugTool: true, exposedTo: ["https://host.example"] },
    );
    const tool = tools.find(
      (entry) => entry.name === "inspect_webmcp_registration",
    )!;
    expect(tool.annotations).toEqual({ readOnlyHint: true, debugging: true });
    const response = await tool.execute({});
    expect(response.ok).toBe(true);
    const data = (response as { ok: true; data: ToolPayload }).data;
    expect(data.tools).toEqual(debug.toolNames);
    expect(data.exposed_to).toEqual(["https://host.example"]);
    expect(data.model_context).toEqual({
      register_tool: true,
      get_tools: false,
      execute_tool: false,
      events: false,
    });
    expect(schemaErrors(tool.outputSchema, response)).toEqual([]);
    debug.dispose();
  });
});
