import { readFileSync } from "node:fs";
import { expect, it } from "vitest";
import { parseApiResult, validateApiResult } from "../src/result-validation.js";

it("rejects malformed JSON and missing typed result fields at the shared boundary", () => {
  expect(() => parseApiResult("solve", "not json")).toThrow("not valid JSON");
  expect(() => parseApiResult("solve", "{}")).toThrow("/formulation");
  expect(() => parseApiResult("mc_result", '{"converged":true}')).toThrow(
    "Invalid engine mc_result",
  );
});

it("checks nested numeric data without accepting coerced or nonfinite values", () => {
  const valid = {
    formulation: "dcopf",
    status: "optimal",
    flows: [{ branch: 1, pf: 2, loading: 0.5 }],
  };
  expect(validateApiResult("solve", valid)).toBe(valid);
  expect(() =>
    validateApiResult("solve", {
      ...valid,
      flows: [{ branch: 1, pf: "2", loading: 0.5 }],
    }),
  ).toThrow("/flows/0/pf");
  expect(() =>
    validateApiResult("solve", {
      ...valid,
      flows: [{ branch: 1, pf: Infinity, loading: 0.5 }],
    }),
  ).toThrow("/flows/0/pf");
  expect(() =>
    validateApiResult("solve", { ...valid, status: "invented" }),
  ).toThrow("/status");
});

it("rejects malformed MC snapshots, synthetic reports and RMS inspections", () => {
  for (const name of [
    "mc_study",
    "synth_report",
    "rms_inspection",
    "settings_catalog",
  ] as const)
    expect(() => parseApiResult(name, "{}")).toThrow(`Invalid engine ${name}`);
  expect(() => parseApiResult("mc_terminal_ids", '[["bus", 3]]')).toThrow(
    "/0/1",
  );
  expect(() => parseApiResult("mc_strings", '["bus", 3]')).toThrow("/1");
  expect(() =>
    parseApiResult(
      "synth_step",
      JSON.stringify({
        stage: "context",
        progress: "0.5",
        done: false,
        warnings: [],
        preview_changed: false,
      }),
    ),
  ).toThrow("/progress");
});

it("checks JSON drop kind, format and payload together", () => {
  const fixture = (name: string) =>
    JSON.parse(
      readFileSync(
        new URL(
          `../../../crates/tellegen-wasm/tests/fixtures/adapter-${name}-before.json`,
          import.meta.url,
        ),
        "utf8",
      ),
    );
  expect(
    validateApiResult("json_drop", {
      kind: "unknown",
      format: null,
      payload: null,
    }),
  ).toEqual({ kind: "unknown", format: null, payload: null });
  for (const value of [
    { kind: "unknown", format: null, payload: {} },
    { kind: "ambiguous", format: "dss", payload: null },
    { kind: "transmission", format: "json", payload: fixture("distribution") },
    { kind: "distribution", format: "json", payload: fixture("balanced") },
  ])
    expect(() => validateApiResult("json_drop", value)).toThrow(
      "Invalid engine json_drop",
    );
  expect(() =>
    validateApiResult("classifier", { kind: "unknown", format: "dss" }),
  ).toThrow("Invalid engine classifier");
});
