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
