#!/usr/bin/env node
// Generated-grid -> viewer hand-off benchmark (#261).
//
// Replays, in one Node process against the built wasm package, every engine
// step between "the synthetic grid is generated" and "the viewer has its
// first DC OPF result", timing each one:
//
//   generate       SynthSession advance() to the end (synthetic worker)
//   ingest         synth_session_ingest: the drop-panel payload as JSON text
//   copy 1         synthetic worker -> page postMessage (structured clone)
//   page parse     JSON.parse of the payload on the page (BrowserSynthSession.ingest)
//   copy 2         page -> study worker postMessage of module_json (structured clone)
//   study build    study_new: wasm Study construction from module_json,
//                  which parses the module and solves the base case
//   first solve    study_solution: the retained base-case answer as JSON
//   solution copy  study worker -> page clone + parse of that answer
//
// The two copies use structuredClone, the algorithm postMessage uses; the
// browser's cross-thread hop adds scheduling on top (see the opt-in browser
// spec `apps/web/tests/synth-handoff-bench.spec.ts`). Layout is not timed: a
// generated module carries bus coordinates, so the viewer skips the
// connectivity layout. First paint needs a browser.
//
// Usage (after `npm run wasm`):
//   node packages/engine/scripts/bench-synth-handoff.mjs \
//     [--buses 3000] [--runs 5] [--json] [--write-module path.pio.json]
import { readFileSync, writeFileSync } from "node:fs";
import { performance } from "node:perf_hooks";

const args = process.argv.slice(2);
const option = (name) => {
  const at = args.indexOf(`--${name}`);
  return at >= 0 ? args[at + 1] : undefined;
};
const buses = Number(option("buses") ?? 3000);
const runs = Number(option("runs") ?? 5);
const writeModule = option("write-module");
const asJson = args.includes("--json");
if (!Number.isInteger(buses) || buses < 2)
  throw new Error("--buses must be an integer >= 2");
if (!Number.isInteger(runs) || runs < 1)
  throw new Error("--runs must be a positive integer");

const pkg = new URL("../src/wasm-pkg/", import.meta.url);
const mod = await import(new URL("tellegen.js", pkg).href);
mod.initSync({ module: readFileSync(new URL("tellegen_bg.wasm", pkg)) });
mod.synth_load_places(
  readFileSync(new URL("../src/data/populated_places.json", import.meta.url)),
);

const request = JSON.stringify({
  ...JSON.parse(
    readFileSync(
      new URL("../tests/fixtures/handoff-request.json", import.meta.url),
      "utf8",
    ),
  ),
  target_bus_count: buses,
});

function time(stages, name, fn) {
  const start = performance.now();
  const value = fn();
  stages[name] = performance.now() - start;
  return value;
}

function once() {
  const stages = {};
  const session = time(stages, "generate", () => {
    const s = new mod.SynthSession(request, undefined);
    try {
      while (!JSON.parse(s.advance()).done);
    } catch (error) {
      s.free();
      throw error;
    }
    return s;
  });
  let payloadText;
  try {
    payloadText = time(stages, "ingest", () => session.ingest());
  } finally {
    session.free();
  }
  const received = time(stages, "copy 1", () => structuredClone(payloadText));
  const payload = time(stages, "page parse", () => JSON.parse(received));
  const message = time(stages, "copy 2", () =>
    structuredClone({
      op: "study_new",
      study: 1,
      module_json: payload.module_json,
      formulation: "dcopf",
    }),
  );
  const study = time(
    stages,
    "study build",
    () => new mod.Study(message.module_json, message.formulation),
  );
  try {
    const solution = time(stages, "first solve", () => study.solution());
    time(stages, "solution copy", () => JSON.parse(structuredClone(solution)));
  } finally {
    study.free();
  }
  return {
    stages,
    moduleJson: payload.module_json,
    sizes: {
      buses: payload.n_bus,
      branches: payload.n_branch,
      payload_chars: payloadText.length,
      module_json_chars: payload.module_json.length,
    },
  };
}

const warm = once(); // warm the wasm instance and the JIT
if (writeModule) writeFileSync(writeModule, warm.moduleJson);
const samples = Array.from({ length: runs }, once);
const median = (xs) => {
  const s = [...xs].sort((a, b) => a - b);
  const m = Math.floor(s.length / 2);
  return s.length % 2 ? s[m] : (s[m - 1] + s[m]) / 2;
};
const names = Object.keys(samples[0].stages);
const handoff = names.filter((n) => n !== "generate");
const result = {
  target_bus_count: buses,
  runs,
  node: process.version,
  sizes: samples[0].sizes,
  median_ms: Object.fromEntries(
    names.map((n) => [n, +median(samples.map((s) => s.stages[n])).toFixed(1)]),
  ),
  handoff_ms: +median(
    samples.map((s) => handoff.reduce((sum, n) => sum + s.stages[n], 0)),
  ).toFixed(1),
};

if (asJson) {
  console.log(JSON.stringify(result, null, 2));
} else {
  const { sizes } = result;
  console.log(
    `generated-grid hand-off: ${sizes.buses} buses, ${sizes.branches} branches; ` +
      `median of ${runs} runs, Node ${process.version}`,
  );
  console.log(
    `payload ${(sizes.payload_chars / 1e6).toFixed(2)} M chars, ` +
      `module_json ${(sizes.module_json_chars / 1e6).toFixed(2)} M chars`,
  );
  console.log("| stage | ms |\n|---|---:|");
  for (const n of names) console.log(`| ${n} | ${result.median_ms[n]} |`);
  console.log(`| **hand-off (all but generate)** | **${result.handoff_ms}** |`);
}
