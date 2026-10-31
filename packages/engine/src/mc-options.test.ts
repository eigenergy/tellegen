import { describe, expect, it } from "vitest";
import {
  DEFAULT_MC_PF_OPTIONS,
  validateMcPfOptions,
  mcLoadModelLabel,
} from "./index.js";

describe("multiconductor settings", () => {
  it("uses the selected defaults and validates the whole threshold band even when the envelope is disabled", () => {
    expect(DEFAULT_MC_PF_OPTIONS).toEqual({
      tolerance: 1e-4,
      absolute_kcl_tolerance: 1e-4,
      relative_kcl_tolerance: 1e-5,
      damping: 1,
      zero_voltage_tolerance: 1e-9,
      voltage_envelope: true,
      v_low_pu: 0.5,
      v_min_pu: 0.85,
      v_max_pu: 1.15,
      max_iterations: 100,
    });
    expect(validateMcPfOptions({})).toBeNull();
    expect(
      validateMcPfOptions({ voltage_envelope: false, v_min_pu: 0.4 }),
    ).toContain("Vlow");
    expect(validateMcPfOptions({ max_iterations: 1.5 })).toContain("integer");
    expect(validateMcPfOptions({ damping: 0 })).toContain("Damping");
    expect(validateMcPfOptions({ v_min_pu: undefined })).toContain("finite");
    expect(validateMcPfOptions({ relative_kcl_tolerance: 0 })).toBeNull();
    expect(validateMcPfOptions({ tolerance: -1 })).toContain("Tolerances");
  });
  it("never presents missing qualification as original-model success", () => {
    expect(mcLoadModelLabel(undefined)).toContain("not verified");
    expect(mcLoadModelLabel(null)).toContain("not verified");
  });
});
