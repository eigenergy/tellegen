import { readFileSync } from "node:fs";
import { expect, it } from "vitest";
import { CaseCollection, collection_supported, initSync } from "../src/wasm-pkg/tellegen.js";

it("matches collection availability to the compiled backend and keeps its API actionable", () => {
  initSync({ module: readFileSync(new URL("../src/wasm-pkg/tellegen_bg.wasm", import.meta.url)) });
  const enabled = (process.env.TELLEGEN_POWERIO_FEATURES ?? "").split(",").includes("powerio-collections");
  expect(collection_supported()).toBe(enabled);
  if (!enabled) {
    expect(() => CaseCollection.from_zip("study.zip", new Uint8Array())).toThrow("Open a single case file instead");
    return;
  }
  const bytes = new TextEncoder().encode("mpc.version = '2';\nmpc.baseMVA = 100;\nmpc.bus = [1 3 0 0 0 0 1 1 0 110 1 1.1 0.9];\nmpc.gen = [1 0 0 100 -100 1 100 1 100 0];\nmpc.branch = [];\n");
  const collection = CaseCollection.from_files("study", '["case.m"]', new Uint32Array([0, bytes.length]), bytes);
  try {
    expect(JSON.parse(collection.inspect()).candidates).toHaveLength(1);
    expect(JSON.parse(collection.ingest('{"primary":"case.m"}')).kind).toBe("balanced");
  } finally {
    collection.free();
  }
});
