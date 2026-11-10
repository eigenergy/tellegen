import { expect, test } from "vitest";
import { solveResponseToSolution } from "../src/index.js";

test("compact results retain fallback provenance and accept legacy responses", () => {
  const initialization = {
    requested: "case" as const,
    used: "perturbed" as const,
    attempts: 4,
    restart: 2,
  };
  expect(solveResponseToSolution({ initialization }).initialization).toEqual(initialization);
  expect(solveResponseToSolution({}).initialization).toBeUndefined();
});
