import { parseApiResult } from "../src/result-validation.js";
import { readFileSync } from "node:fs";
import { expect, it } from "vitest";
import {
  CaseCollection,
  collection_supported,
  initSync,
} from "../src/wasm-pkg/tellegen.js";

it("matches collection availability to the compiled backend and keeps its API actionable", async () => {
  initSync({
    module: readFileSync(
      new URL("../src/wasm-pkg/tellegen_bg.wasm", import.meta.url),
    ),
  });
  expect(collection_supported()).toBe(true);
  const bytes = new TextEncoder().encode(
    "mpc.version = '2';\nmpc.baseMVA = 100;\nmpc.bus = [1 3 0 0 0 0 1 1 0 110 1 1.1 0.9];\nmpc.gen = [1 0 0 100 -100 1 100 1 100 0];\nmpc.branch = [];\n",
  );
  const collection = CaseCollection.from_files(
    "study",
    '["case.m"]',
    new Uint32Array([0, bytes.length]),
    bytes,
  );
  try {
    expect(
      (
        await parseApiResult<{ candidates: unknown[] }>(
          "collection_inspection",
          collection.inspect(),
        )
      ).candidates,
    ).toHaveLength(1);
    expect(
      (
        await parseApiResult<{ kind: string }>(
          "collection_import",
          collection.ingest('{"primary":"case.m"}'),
        )
      ).kind,
    ).toBe("balanced");
  } finally {
    collection.free();
  }
});
