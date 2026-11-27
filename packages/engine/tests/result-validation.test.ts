import { readFileSync } from "node:fs";
import { expect, it } from "vitest";
import {
  parseApiResult,
  preloadValidators,
  validateApiResult,
  type ResultName,
} from "../src/result-validation.js";
import {
  parseApiResultSync,
  validateApiResultSync,
} from "../src/validators.js";

it("rejects malformed JSON and missing typed result fields at the shared boundary", async () => {
  await expect(parseApiResult("solve", "not json")).rejects.toThrow(
    "not valid JSON",
  );
  await expect(parseApiResult("solve", "{}")).rejects.toThrow("/formulation");
  await expect(
    parseApiResult("mc_result", '{"converged":true}'),
  ).rejects.toThrow("Invalid engine mc_result");
});

it("checks nested numeric data without accepting coerced or nonfinite values", async () => {
  const valid = {
    formulation: "dcopf",
    status: "optimal",
    flows: [{ branch: 1, pf: 2, loading: 0.5 }],
  };
  expect(await validateApiResult("solve", valid)).toBe(valid);
  await expect(
    validateApiResult("solve", {
      ...valid,
      flows: [{ branch: 1, pf: "2", loading: 0.5 }],
    }),
  ).rejects.toThrow("/flows/0/pf");
  await expect(
    validateApiResult("solve", {
      ...valid,
      flows: [{ branch: 1, pf: Infinity, loading: 0.5 }],
    }),
  ).rejects.toThrow("/flows/0/pf");
  await expect(
    validateApiResult("solve", { ...valid, status: "invented" }),
  ).rejects.toThrow("/status");
});

it("rejects malformed MC snapshots, synthetic reports and RMS inspections", async () => {
  for (const name of [
    "mc_study",
    "synth_report",
    "rms_inspection",
    "settings_catalog",
  ] as const)
    await expect(parseApiResult(name, "{}")).rejects.toThrow(
      `Invalid engine ${name}`,
    );
  await expect(
    parseApiResult("mc_terminal_ids", '[["bus", 3]]'),
  ).rejects.toThrow("/0/1");
  await expect(parseApiResult("mc_strings", '["bus", 3]')).rejects.toThrow(
    "/1",
  );
  await expect(
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
  ).rejects.toThrow("/progress");
});

it("checks JSON drop kind, format and payload together", async () => {
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
    await validateApiResult("json_drop", {
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
    await expect(validateApiResult("json_drop", value)).rejects.toThrow(
      "Invalid engine json_drop",
    );
  await expect(
    validateApiResult("classifier", { kind: "unknown", format: "dss" }),
  ).rejects.toThrow("Invalid engine classifier");
});

it("validates synchronously through the eager entry with the same messages", async () => {
  await preloadValidators(["solve", "http_cases", "synth_report"]);
  expect(() => parseApiResultSync("solve", "not json")).toThrow(
    "not valid JSON",
  );
  expect(() => parseApiResultSync("solve", "{}")).toThrow("/formulation");
  const valid = { formulation: "dcopf", status: "optimal" };
  expect(validateApiResultSync("solve", valid)).toBe(valid);
  expect(parseApiResultSync("capabilities", "[]")).toEqual([]);
});

it("rejects result names that only Object.prototype defines", async () => {
  const names = [
    "toString",
    "constructor",
    "missing",
  ] as unknown as ResultName[];
  for (const name of names) {
    expect(() => validateApiResultSync(name, {})).toThrow(
      `Unknown engine result ${name}`,
    );
    await expect(validateApiResult(name, {})).rejects.toThrow(
      `Unknown engine result ${name}`,
    );
    await expect(preloadValidators([name])).rejects.toThrow(
      `Unknown engine result ${name}`,
    );
  }
});
