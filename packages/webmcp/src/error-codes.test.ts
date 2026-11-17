import { describe, expect, it } from "vitest";
import { TELLEGEN_ERROR_CODES } from "./error-codes.js";
import { ERROR_CODES } from "./output.js";

// apps/web/scripts/test-error-codes.mjs checks the list against the codes the
// tools raise across the repository.
describe("TELLEGEN_ERROR_CODES", () => {
  it("covers the codes named in the output schemas", () => {
    const registered = new Set<string>(Object.keys(TELLEGEN_ERROR_CODES));
    expect(ERROR_CODES.filter((code) => !registered.has(code))).toEqual([]);
  });

  it("says when each code happens and what to do next", () => {
    for (const [code, entry] of Object.entries(TELLEGEN_ERROR_CODES)) {
      expect(entry.when.length, code).toBeGreaterThan(10);
      expect(entry.next.length, code).toBeGreaterThan(10);
    }
  });
});
