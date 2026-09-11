import { describe, expect, it } from "vitest";
import type {
  DistNetworkDetails,
  NetworkBranch,
  NetworkBus,
} from "@tellegen/engine";
import {
  distributionBusDetails,
  distributionEdgeDetails,
  transmissionBranchDetails,
  transmissionBusDetails,
} from "../src/lib/element-inspector.js";

const bus: NetworkBus = {
  id: 2,
  uid: "buses:2",
  lon: 0,
  lat: 0,
  demand_mw: 90,
  gen_mw: 0,
};

const branch: NetworkBranch = {
  id: 0,
  uid: "branches:1",
  from: 1,
  to: 2,
  rate_mw: 250,
  status: 1,
  path: [
    [0, 0],
    [1, 1],
  ],
};

const moduleJson = JSON.stringify({
  schema: "pio-ir",
  value: {
    data: {
      buses: [
        { id: 2, uid: "buses:2", kind: "PQ", vm: 1, vmin: 0.9, vmax: 1.1 },
      ],
      branches: [
        { uid: "branches:1", from: 1, to: 2, r: 0.01, x: 0.1, rate_a: 250 },
      ],
      loads: [{ uid: "loads:1", bus: 2, p: 90, q: 30 }],
      generators: [],
      shunts: [],
      static_var_compensators: [],
    },
  },
});

const distribution: DistNetworkDetails = {
  buses: [
    {
      id: "load_bus",
      terminals: ["1", "2", "3", "4"],
      grounded: ["4"],
      v_min: 6800,
      v_max: 7600,
    },
  ],
  linecodes: [
    { name: "lc1", n_conductors: 3, r_series: [[0.1]], x_series: [[0.2]] },
  ],
  lines: [
    {
      name: "l1",
      bus_from: "src",
      bus_to: "load_bus",
      linecode: "lc1",
      length: 100,
    },
  ],
  switches: [{ name: "sw1", bus_from: "load_bus", bus_to: "stub", open: true }],
  transformers: [
    { name: "tx1", phases: 3, windings: [{ bus: "load_bus", v_ref: 7200 }] },
  ],
  loads: [
    {
      name: "ld1",
      bus: "load_bus",
      p_nom: [50_000, 50_000, 50_000],
      q_nom: [10_000, 10_000, 10_000],
    },
  ],
  generators: [],
  shunts: [],
  sources: [],
};

describe("element inspector record resolution", () => {
  it("finds complete transmission bus records and attached equipment", () => {
    const details = transmissionBusDetails(moduleJson, bus);
    expect(details.record).toMatchObject({ kind: "PQ", vmin: 0.9, vmax: 1.1 });
    expect(details.related).toEqual([
      { label: "Loads", records: [{ uid: "loads:1", bus: 2, p: 90, q: 30 }] },
    ]);
  });

  it("finds transmission branches by stable uid", () => {
    expect(transmissionBranchDetails(moduleJson, branch).record).toMatchObject({
      uid: "branches:1",
      r: 0.01,
      x: 0.1,
      rate_a: 250,
    });
  });

  it("finds distribution bus properties and every attached element family", () => {
    const details = distributionBusDetails(distribution, "load_bus");
    expect(details.record).toMatchObject({ v_min: 6800, v_max: 7600 });
    expect(details.related.map((group) => group.label)).toEqual([
      "Loads",
      "Transformers",
    ]);
  });

  it("finds typed distribution edges and resolves linecodes", () => {
    const line = distributionEdgeDetails(distribution, "line", "l1");
    expect(line.record).toMatchObject({ length: 100, linecode: "lc1" });
    expect(line.related[0]).toMatchObject({ label: "Linecode" });
    expect(
      distributionEdgeDetails(distribution, "switch", "sw1").record,
    ).toMatchObject({ open: true });
    expect(
      distributionEdgeDetails(distribution, "transformer", "tx1").record,
    ).toMatchObject({ phases: 3 });
  });
});
