// Time synthetic generation through the real wasm build: one SynthSession
// per spec, every stage, with the engine's own stage clock (Date.now).
// usage: [SEED=2026] node docs/evidence/synth/wasm_time.mjs WASM_PKG_DIR PLACES_JSON SPEC.json...
// SEED defaults to 2026, the MapToGrid app's default seed.
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { pathToFileURL } from "node:url";

const [pkgDir, placesPath, ...specs] = process.argv.slice(2);
const mod = await import(pathToFileURL(join(pkgDir, "tellegen.js")).href);
mod.initSync({ module: readFileSync(join(pkgDir, "tellegen_bg.wasm")) });
mod.synth_load_places(readFileSync(placesPath));

for (const path of specs) {
  const spec = JSON.parse(readFileSync(path, "utf8"));
  const seed = Number(process.env.SEED ?? 2026);
  const request = JSON.stringify({ ...spec.request, seed });
  const t0 = performance.now();
  const session = new mod.SynthSession(request, undefined);
  let done = false;
  while (!done) {
    done = JSON.parse(session.advance()).done;
  }
  const wall = performance.now() - t0;
  const report = JSON.parse(session.report());
  session.free();
  const stages = {};
  for (const s of report.stages) {
    if (s.elapsed_ms != null) stages[s.stage] = (stages[s.stage] ?? 0) + s.elapsed_ms;
  }
  const engine = Object.values(stages).reduce((a, b) => a + b, 0);
  console.log(
    JSON.stringify({
      region: spec.id,
      seed,
      buses: report.stats?.n_buses,
      dc: report.dc_opf?.status,
      ac: report.ac_pf?.status,
      engine_ms: Math.round(engine),
      wall_ms: Math.round(wall),
      stages: Object.fromEntries(Object.entries(stages).map(([k, v]) => [k, Math.round(v)])),
    }),
  );
}
