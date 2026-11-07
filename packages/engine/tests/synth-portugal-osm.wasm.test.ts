import { readFileSync } from "node:fs";
import { gunzipSync } from "node:zlib";
import { expect, it } from "vitest";
import { initSync, solve_module, synth_generate, synth_load_places } from "../src/wasm-pkg/tellegen.js";
import type { GenerationReport, SynthRequest } from "../src/generated/synth-types.js";

it("connects Portugal's rare voltage levels locally and supports the full AC load", () => {
  initSync({ module: readFileSync(new URL("../src/wasm-pkg/tellegen_bg.wasm", import.meta.url)) });
  synth_load_places(readFileSync(new URL("../src/data/populated_places.json", import.meta.url)));
  const request = JSON.parse(readFileSync(new URL("./fixtures/portugal-request.json", import.meta.url), "utf8")) as SynthRequest;
  const osm = gunzipSync(readFileSync(new URL("./fixtures/portugal-overpass.json.gz", import.meta.url))).toString();
  const result = JSON.parse(synth_generate(JSON.stringify(request), osm)) as { report: GenerationReport; module_json: string };
  const report = result.report;
  expect(report.dc_opf?.status).toBe("optimal");
  expect(report.ac_pf?.status).toBe("converged");
  expect(report.ac_pf?.buses_outside_planning_band).toBe(0);
  expect(report.ac_pf?.buses_outside_emergency_band).toBe(0);
  expect(report.stats?.total_load_mw).toBeCloseTo(8250.982518396087, 6);
  // The old voltage-first connector invented a 490 km, 132 kV straight line.
  // No mapped line is removed to satisfy this bound: only inferred ties count.
  const ties = report.elements!.branches.filter((b) => b.boundary_role === "interconnect");
  expect(ties.length).toBeGreaterThan(0);
  expect(Math.max(...ties.map((b) => b.length_km))).toBeLessThan(60);
  const network = JSON.parse(result.module_json).value.data;
  expect(network.shunts.every((s: { uid: string; extras: { tellegen_synth: { source: string } } }) =>
    s.uid.startsWith("synthetic-shunt:") && s.extras.tellegen_synth.source === "synthetic")).toBe(true);
  // The exported case must reproduce the result independently of the
  // generator's report and its private validation copy.
  const ac = JSON.parse(solve_module(result.module_json, JSON.stringify({ formulation: "acpf" }))) as {
    status: string; vm: { value: number }[];
  };
  expect(ac.status).toBe("feasible");
  expect(ac.vm.every((v) => v.value >= 0.95 && v.value <= 1.05)).toBe(true);
}, 30_000);
