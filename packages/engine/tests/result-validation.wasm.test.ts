import { readFileSync } from "node:fs";
import { expect, it } from "vitest";
import {
  initSync,
  ingest_case,
  solve_module,
  ingest_dist_case,
  solve_mc_module,
  SynthSession,
  McPfSession,
  synth_load_places,
  describe_settings,
} from "../src/wasm-pkg/tellegen.js";
import { parseApiResult } from "../src/result-validation.js";

it("accepts actual balanced and multiconductor results from the browser engine", () => {
  initSync({
    module: readFileSync(
      new URL("../src/wasm-pkg/tellegen_bg.wasm", import.meta.url),
    ),
  });
  const source = readFileSync(
    new URL("../../../python/tellegen/mcp/examples/case3.m", import.meta.url),
    "utf8",
  );
  const input = JSON.parse(
    ingest_case(new TextEncoder().encode(source), "matpower"),
  );
  const balanced = parseApiResult<{ status: string }>(
    "solve",
    solve_module(input.module_json, "{}"),
  );
  expect(balanced.status).toBe("optimal");
  // An actual infeasible solve must retain the engine error rather than turn
  // into a complaint about JSON or missing result fields.
  expect(() =>
    parseApiResult(
      "solve",
      solve_module(
        input.module_json,
        JSON.stringify({ edits: { deltas: { "2": 1_000_000 } } }),
      ),
    ),
  ).toThrow("DC OPF solve infeasible");
  const dist = JSON.parse(
    ingest_dist_case(
      "new circuit.test basekv=.4\nnew load.l phases=3 bus1=sourcebus kv=.4 kw=3 kvar=1\n",
      "dss",
    ),
  );
  const solved = parseApiResult<{ converged: boolean }>(
    "mc_result",
    solve_mc_module(dist.module_json, "{}"),
  );
  expect(solved.converged).toBe(true);
  const session = new McPfSession(dist.module_json, "{}");
  try {
    parseApiResult("mc_study", session.snapshot("validation", "Validation"));
    parseApiResult("mc_terminal_ids", session.terminal_ids());
    const buffers = session.result_tables();
    try {
      parseApiResult("mc_metadata", buffers.metadata());
      parseApiResult("mc_strings", buffers.take_strings());
    } finally {
      buffers.free();
    }
  } finally {
    session.free();
  }
});

it("accepts actual synthetic stages, reports and settings from the browser engine", () => {
  initSync({
    module: readFileSync(
      new URL("../src/wasm-pkg/tellegen_bg.wasm", import.meta.url),
    ),
  });
  parseApiResult("settings_catalog", describe_settings());
  synth_load_places(
    new TextEncoder().encode(
      JSON.stringify([
        { n: "London", lo: -0.12, la: 51.5, p: 9000000, c: "GB" },
      ]),
    ),
  );
  const session = new SynthSession(
    JSON.stringify({
      region: {
        geometry: {
          type: "Polygon",
          coordinates: [
            [
              [-1, 51],
              [1, 51],
              [1, 52],
              [-1, 52],
              [-1, 51],
            ],
          ],
        },
      },
      target_bus_count: 10,
      seed: 7,
    }),
  );
  try {
    parseApiResult("synth_report", session.report());
    let done = false;
    for (let step = 0; step < 50 && !done; step++) {
      const result = parseApiResult<{ done: boolean }>(
        "synth_step",
        session.advance(),
      );
      done = result.done;
      parseApiResult("synth_report", session.report());
    }
    expect(done).toBe(true);
  } finally {
    session.free();
  }
});
