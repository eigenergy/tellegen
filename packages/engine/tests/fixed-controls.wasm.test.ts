import { readFileSync } from "node:fs";
import { expect, it } from "vitest";
import { parseApiResult, validateApiResult } from "../src/result-validation.js";
import type { IngestedDistCase } from "../src/index.js";
import {
  initSync,
  ingest_dist_case,
  prepare_mc_fixed_controls,
  ingest_json_drop,
} from "../src/wasm-pkg/tellegen.js";

it("requires an explicit fixed-settings choice and preserves it through browser-engine reload", async () => {
  initSync({
    module: readFileSync(
      new URL("../src/wasm-pkg/tellegen_bg.wasm", import.meta.url),
    ),
  });
  const source =
    "new circuit.test basekv=11\nnew transformer.t phases=3 windings=2 buses=[sourcebus load] conns=[wye wye] kvs=[11 .4] kvas=[100 100] taps=[1 1.025] xhl=4\nnew load.l bus1=load phases=3 kv=.4 kw=10 kvar=2\nnew regcontrol.r transformer=t winding=2 vreg=120 band=2 ptratio=1\n";
  const original = JSON.parse(ingest_dist_case(source, "dss"));
  expect(original.mc_pf_supported).toBe(false);
  expect(original.mc_fixed_controls_available).toBe(true);
  const fixed = await parseApiResult<IngestedDistCase>(
    "ingest_distribution",
    prepare_mc_fixed_controls(original.module_json),
  );
  expect(fixed.mc_pf_supported).toBe(true);
  expect(fixed.mc_fixed_controls).toBe(true);
  expect(fixed.mc_fixed_controls_available).toBe(false);
  await expect(
    validateApiResult("ingest_distribution", {
      ...fixed,
      mc_fixed_controls: "true",
    }),
  ).rejects.toThrow("/mc_fixed_controls");
  await expect(
    validateApiResult("ingest_distribution", {
      ...fixed,
      mc_fixed_controls_available: 1,
    }),
  ).rejects.toThrow("/mc_fixed_controls_available");
  expect(fixed.module_json).not.toBe(original.module_json);
  const restored = JSON.parse(
    ingest_json_drop(new TextEncoder().encode(fixed.module_json)),
  );
  expect(restored.payload.mc_fixed_controls).toBe(true);
  expect(
    JSON.parse(ingest_json_drop(new TextEncoder().encode(original.module_json)))
      .payload.mc_pf_supported,
  ).toBe(false);
});
