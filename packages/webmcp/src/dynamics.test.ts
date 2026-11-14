import { describe, expect, it } from "vitest";
import { validateSimulateDynamics, validateImportCase } from "./validation.js";
import { responseSchema } from "./output.js";

const base = { case_id: "local-1", expected_revision: "r1" };

describe("simulate_dynamics input", () => {
  it("fills the defaults", () => {
    const v = validateSimulateDynamics({
      ...base,
      events: [{ kind: "branch_trip", branch: "1101-1401", at_s: 1 }],
    });
    expect(v).toMatchObject({
      tEndS: 20,
      dtS: 0.0025,
      channels: [],
      tracePoints: 0,
      showInApp: true,
    });
    expect(v.events[0]).toEqual({
      kind: "branch_trip",
      branch: "1101-1401",
      atS: 1,
    });
  });

  it("defaults a fault to 0.1 s clearing through j1e-4", () => {
    const v = validateSimulateDynamics({
      ...base,
      events: [{ kind: "bus_fault", bus: 1101, at_s: 1 }],
    });
    expect(v.events[0]).toMatchObject({ clearAfterS: 0.1, xPu: 1e-4, rPu: 0 });
  });

  it.each([
    [{ events: [] }, /1 to 16/],
    [{ events: [{ kind: "bus_fault", bus: 1, at_s: 25 }] }, /at_s/],
    [{ events: [{ kind: "bus_fault", bus: 1, at_s: 19.95 }] }, /clear before/],
    [
      { events: [{ kind: "branch_trip", branch: "1101/1401", at_s: 1 }] },
      /from-to/,
    ],
    [
      { events: [{ kind: "generator_trip", unit: "1431 N", at_s: 1 }] },
      /bus#id/,
    ],
    [
      { events: [{ kind: "load_change", bus: 1, at_s: 1, scale: -1 }] },
      /scale/,
    ],
    [{ events: [{ kind: "explode", at_s: 1 }] }, /kind/],
    [
      { events: [{ kind: "branch_trip", branch: "1-2", at_s: 1 }], dt_s: 0.01 },
      /dt_s/,
    ],
    [
      {
        events: [{ kind: "branch_trip", branch: "1-2", at_s: 1 }],
        channels: ["va:1"],
      },
      /channels\[0\]/,
    ],
    [
      {
        events: [{ kind: "branch_trip", branch: "1-2", at_s: 1 }],
        trace_points: 25,
      },
      /trace_points/,
    ],
    [
      {
        events: [{ kind: "branch_trip", branch: "1-2", at_s: 1 }],
        channels: ["coi:f", "vm:1", "f:1"],
        trace_points: 9,
      },
      /channels × trace_points/,
    ],
  ])("refuses %j", (patch, message) => {
    expect(() => validateSimulateDynamics({ ...base, ...patch })).toThrow(
      message,
    );
  });

  it("accepts the largest trace request", () => {
    for (const [channels, points] of [
      [["coi:f"], 24],
      [["coi:f", "vm:1", "f:1", "pl:1"], 6],
    ] as const)
      expect(
        validateSimulateDynamics({
          ...base,
          events: [{ kind: "branch_trip", branch: "1-2", at_s: 1 }],
          channels,
          trace_points: points,
        }),
      ).toMatchObject({ tracePoints: points });
  });

  it("has a response schema", () => {
    const s = responseSchema("simulate_dynamics") as {
      properties: { data: { properties: Record<string, unknown> } };
    };
    expect(JSON.stringify(s)).toContain("verdict");
  });

  it("import_case accepts .dyr text", () => {
    expect(
      validateImportCase({ file_name: "wecc.dyr", text: "1 'GENCLS' 1 3 0 /" })
        .fileName,
    ).toBe("wecc.dyr");
  });

  it("import_case replaces dynamic data only when asked, and only for .dyr", () => {
    const dyr = { file_name: "wecc.dyr", text: "1 'GENCLS' 1 3 0 /" };
    expect(validateImportCase(dyr).replaceDynamics).toBe(false);
    expect(
      validateImportCase({ ...dyr, replace_dynamics: true }).replaceDynamics,
    ).toBe(true);
    expect(() =>
      validateImportCase({ ...dyr, replace_dynamics: "yes" }),
    ).toThrow(/boolean/);
    expect(() =>
      validateImportCase({
        file_name: "case14.m",
        text: "mpc = struct;",
        replace_dynamics: true,
      }),
    ).toThrow(/\.dyr/);
  });
});
