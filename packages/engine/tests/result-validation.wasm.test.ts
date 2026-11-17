import { readFileSync } from "node:fs";
import { expect, it } from "vitest";
import {
  initSync,
  ingest_case,
  solve_module,
  ingest_dist_case,
  solve_mc_module,
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
});
