import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { engineError, errorCode } from "../src/errors.js";
import {
  ENGINE_ERROR_CODES,
  OBJECTIVE_KINDS,
} from "../src/generated/api-types.js";
import {
  Study,
  calculate_with_settings,
  ingest_case,
  initSync,
  solve_module,
} from "../src/wasm-pkg/tellegen.js";

initSync({
  module: readFileSync(
    new URL("../src/wasm-pkg/tellegen_bg.wasm", import.meta.url),
  ),
});

const case3 = readFileSync(
  new URL("../../../validation/evidence/studies/case3.m", import.meta.url),
);
const moduleJson = (
  JSON.parse(ingest_case(case3, "matpower")) as { module_json: string }
).module_json;

function thrown(run: () => unknown): unknown {
  try {
    run();
  } catch (error) {
    return error;
  }
  throw new Error("expected the engine to throw");
}

describe("engine errors carry a code", () => {
  it("names the class of a one-shot solve failure and keeps the message", () => {
    const cases: [string, string, RegExp][] = [
      ["{not json", "invalid_request", /^bad request JSON: /],
      [
        '{"edits":{"deltas":{"99999":10}}}',
        "unknown_element",
        /^unknown demand delta bus 99999$/,
      ],
      [
        '{"edits":{"deltas":{"2":-1e9}}}',
        "out_of_range",
        /would make demand negative$/,
      ],
      [
        '{"formulation":"dcpf","edits":{"rates":{"1":5}}}',
        "unsupported",
        /^branch rating edits are not supported by dcpf$/,
      ],
    ];
    for (const [request, code, message] of cases) {
      const error = thrown(() => solve_module(moduleJson, request));
      expect(error).toBeInstanceOf(Error);
      expect((error as Error).message).toMatch(message);
      expect(errorCode(error)).toBe(code);
    }
    const input = thrown(() => solve_module("not a module", ""));
    expect(errorCode(input)).toBe("invalid_input");
  });

  it("refuses a sensitivity request on the DC power flow", () => {
    const request = JSON.stringify({
      formulation: "dcpf",
      sensitivities: [
        { operand: { Price: "Active" }, parameter: { Demand: "Active" } },
      ],
    });
    const error = thrown(() => solve_module(moduleJson, request));
    expect(errorCode(error)).toBe("unsupported");
    expect((error as Error).message).toContain(
      "sensitivities are not supported by dcpf",
    );
  });

  it("codes a failure from the settings calculation and from a Study", () => {
    const calculation = thrown(() =>
      calculate_with_settings(
        JSON.stringify({
          module_json: moduleJson,
          settings: { calculation: "dcopf" },
          edits: { deltas: { 2: -1e9 } },
        }),
      ),
    );
    expect(errorCode(calculation)).toBe("out_of_range");

    const study = new Study(moduleJson, "dcopf");
    try {
      const edit = thrown(() =>
        study.replace_edits(
          JSON.stringify([{ kind: "add_load", bus: 2, p_mw: -1e9 }]),
          "",
        ),
      );
      expect(errorCode(edit)).toBe("out_of_range");
      const edits = thrown(() => study.replace_edits("{not json", ""));
      expect(errorCode(edits)).toBe("invalid_request");
    } finally {
      study.free();
    }
  });

  it("rebuilds a coded error and ignores codes it does not know", () => {
    const rebuilt = engineError("DC OPF solve infeasible", "infeasible");
    expect(rebuilt.message).toBe("DC OPF solve infeasible");
    expect(errorCode(rebuilt)).toBe("infeasible");
    expect(errorCode(engineError("plain"))).toBeUndefined();
    expect(errorCode(new Error("plain"))).toBeUndefined();
    expect(errorCode({ code: "ENOENT" })).toBeUndefined();
    expect(errorCode("text")).toBeUndefined();
    expect(ENGINE_ERROR_CODES).toContain("internal");
  });
});

describe("objective_kind on OPF results", () => {
  it("is on the one-shot solve, the Study solution, and the Study commit", () => {
    expect(OBJECTIVE_KINDS).toEqual(["cost", "feasibility"]);
    const solved = JSON.parse(solve_module(moduleJson, "")) as {
      objective_kind?: string;
    };
    expect(solved.objective_kind).toBe("cost");

    const flow = JSON.parse(
      solve_module(moduleJson, '{"formulation":"acpf"}'),
    ) as object;
    expect(flow).not.toHaveProperty("objective_kind");

    const calculated = JSON.parse(
      calculate_with_settings(
        JSON.stringify({
          module_json: moduleJson,
          settings: { calculation: "dcopf" },
        }),
      ),
    ) as { result: { objective_kind?: string } };
    expect(calculated.result.objective_kind).toBe("cost");

    const study = new Study(moduleJson, "dcopf");
    try {
      expect(JSON.parse(study.solution()).objective_kind).toBe("cost");
      const commit = JSON.parse(
        study.replace_edits(
          JSON.stringify([{ kind: "add_load", bus: 2, p_mw: 5 }]),
          "",
        ),
      ) as { solution: { objective_kind?: string } };
      expect(commit.solution.objective_kind).toBe("cost");
    } finally {
      study.free();
    }
  });
});
