import { expect, it, vi } from "vitest";
import { parseApiResult } from "../src/result-validation.js";

const failures = vi.hoisted(() => ({ remaining: 1 }));

vi.mock("../src/generated/validator-index.js", async (importOriginal) => {
  const actual =
    await importOriginal<
      typeof import("../src/generated/validator-index.js")
    >();
  return {
    ...actual,
    loadFamily: {
      ...actual.loadFamily,
      mc: () =>
        failures.remaining-- > 0
          ? Promise.reject(new Error("validator import failed"))
          : actual.loadFamily.mc(),
    },
  };
});

it("retries a validator family whose import failed", async () => {
  await expect(parseApiResult("mc_strings", '["bus"]')).rejects.toThrow(
    "validator import failed",
  );
  await expect(parseApiResult("mc_strings", '["bus"]')).resolves.toEqual([
    "bus",
  ]);
});
