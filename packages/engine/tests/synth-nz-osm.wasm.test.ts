import { readFileSync } from "node:fs";
import { gunzipSync } from "node:zlib";
import { expect, it } from "vitest";
import { initSync, synth_generate, synth_load_places } from "../src/wasm-pkg/tellegen.js";
import type { GenerationReport, SynthRequest } from "../src/generated/synth-types.js";

it("serves New Zealand's full OSM demand with disclosed reinforcement in the browser engine", () => {
  initSync({ module: readFileSync(new URL("../src/wasm-pkg/tellegen_bg.wasm", import.meta.url)) });
  synth_load_places(readFileSync(new URL("../src/data/populated_places.json", import.meta.url)));
  const request = JSON.parse(readFileSync(new URL("./fixtures/new-zealand-request.json", import.meta.url), "utf8")) as SynthRequest;
  const osm = gunzipSync(readFileSync(new URL("./fixtures/new-zealand-overpass.json.gz", import.meta.url))).toString();
  const generate = (input: SynthRequest) => (JSON.parse(synth_generate(JSON.stringify(input), osm)) as { report: GenerationReport }).report;
  const original = generate({ ...request, osm: { reinforce: false } });
  const repaired = generate(request);
  expect(original.dc_opf?.status).toBe("infeasible");
  expect(repaired.dc_opf?.status).toBe("optimal");
  expect(repaired.ac_pf?.status).toBe("converged");
  expect(repaired.ac_pf?.buses_outside_planning_band).toBe(0);
  expect(repaired.stats?.total_load_mw).toBeCloseTo(original.stats!.total_load_mw, 6);
  expect(repaired.stats?.total_load_mw).toBeGreaterThan(6500);
  expect(repaired.stats?.total_generation_capacity_mw).toBeCloseTo(original.stats!.total_generation_capacity_mw, 6);
  expect(repaired.elements?.buses).toHaveLength(417);
  expect(repaired.elements?.branches.some((b) => b.source === "synthetic" && !b.is_transformer)).toBe(true);
  expect(repaired.warnings.some((w) => w.code === "osm_synthetic_reinforcement")).toBe(true);
  expect(repaired.context?.voltage_country).toBe("NZL");
  expect(repaired.warnings.some((w) => w.code === "voltage_fallback")).toBe(false);
}, 30_000);
