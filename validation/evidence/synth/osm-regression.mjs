#!/usr/bin/env node
// Offline country evidence using the same public fixtures as the WASM regression suite.
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { readFileSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { gunzipSync } from "node:zlib";

const root = fileURLToPath(new URL("../../../", import.meta.url));
const engineRoot = resolve(process.argv[2] ?? root);
// Supply the revision used to build reused WASM, rather than falsely attributing it to this checkout.
const engineRevision =
  process.argv[3] ??
  execFileSync("git", ["rev-parse", "HEAD"], {
    cwd: engineRoot,
    encoding: "utf8",
  }).trim();
assert.match(engineRevision, /^[0-9a-f]{40}$/);
const engine = await import(
  pathToFileURL(resolve(engineRoot, "packages/engine/src/wasm-pkg/tellegen.js"))
);
const wasm = readFileSync(
  resolve(engineRoot, "packages/engine/src/wasm-pkg/tellegen_bg.wasm"),
);
engine.initSync({ module: wasm });
engine.synth_load_places(
  readFileSync(
    resolve(engineRoot, "packages/engine/src/data/populated_places.json"),
  ),
);
const sha256 = (bytes) => createHash("sha256").update(bytes).digest("hex");
const runs = [];
const fixtures = [];
const mappedOnly = [];
for (const country of ["new-zealand", "portugal"]) {
  const stem = resolve(root, "packages/engine/tests/fixtures", country);
  const requestBytes = readFileSync(`${stem}-request.json`);
  const compressed = readFileSync(`${stem}-overpass.json.gz`);
  const osm = gunzipSync(compressed).toString();
  const baseRequest = JSON.parse(requestBytes);
  const original = JSON.parse(
    engine.synth_generate(
      JSON.stringify({
        ...baseRequest,
        seed: 2026,
        osm: { ...baseRequest.osm, reinforce: false },
      }),
      osm,
    ),
  ).report;
  mappedOnly.push({
    country,
    seed: 2026,
    demand_mw: original.stats.total_load_mw,
    dc_opf: original.dc_opf.status,
    ac_pf: original.ac_pf.status,
  });
  fixtures.push({
    country,
    request_sha256: sha256(requestBytes),
    compressed_osm_sha256: sha256(compressed),
    osm_base: JSON.parse(osm).osm3s.timestamp_osm_base,
  });
  for (const seed of [1, 7, 2026]) {
    const request = { ...baseRequest, seed };
    const started = performance.now();
    const { report, module_json } = JSON.parse(
      engine.synth_generate(JSON.stringify(request), osm),
    );
    const generationMs = performance.now() - started;
    assert(
      Math.abs(report.stats.total_load_mw - original.stats.total_load_mw) <
        1e-6,
      `${country}/${seed} full demand retained`,
    );
    assert.equal(report.dc_opf.status, "optimal", `${country}/${seed} DC OPF`);
    assert.equal(report.ac_pf.status, "converged", `${country}/${seed} AC PF`);
    assert.equal(
      report.ac_pf.buses_outside_planning_band,
      0,
      `${country}/${seed} planning band`,
    );
    assert.equal(
      report.ac_pf.buses_outside_emergency_band,
      0,
      `${country}/${seed} emergency band`,
    );
    const ac = JSON.parse(
      engine.solve_module(module_json, JSON.stringify({ formulation: "acpf" })),
    );
    assert.equal(ac.status, "feasible", `${country}/${seed} exported AC case`);
    const voltages = ac.vm.map((bus) => bus.value);
    const [min, max] = request.validation?.voltage_band ?? [0.95, 1.05];
    assert(
      voltages.every((value) => value >= min && value <= max),
      `${country}/${seed} exported voltage band`,
    );
    const ties = report.elements.branches.filter(
      (branch) => branch.boundary_role === "interconnect",
    );
    runs.push({
      country,
      seed,
      demand_mw: report.stats.total_load_mw,
      buses: report.stats.n_buses,
      branches: report.stats.n_branches,
      dc_opf: report.dc_opf.status,
      ac_pf: report.ac_pf.status,
      exported_ac_pf: ac.status,
      vm_min: Math.min(...voltages),
      vm_max: Math.max(...voltages),
      longest_inferred_connection_km: Math.max(
        0,
        ...ties.map((tie) => tie.length_km),
      ),
      generation_ms: Math.round(generationMs),
      synthetic_reinforcement: report.warnings.some(
        (warning) => warning.code === "osm_synthetic_reinforcement",
      ),
    });
    console.log(
      `${country} seed ${seed}: DC optimal, exported AC feasible, ${voltages.length} buses`,
    );
  }
}
const result = {
  engine_revision: engineRevision,
  wasm_sha256: sha256(wasm),
  node: process.version,
  platform: `${process.platform}/${process.arch}`,
  measured_at: new Date().toISOString(),
  note: "Single measurements under concurrent workstation load; timings are provenance, not a speed comparison. Mapped routes/voltages are retained; reinforcement is explicitly synthetic.",
  fixtures,
  mapped_only: mappedOnly,
  runs,
};
writeFileSync(
  new URL("./osm-regression.json", import.meta.url),
  JSON.stringify(result, null, 2) + "\n",
);
