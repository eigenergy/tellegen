import { describe, expect, it } from "vitest";

import {
  MAX_ENGINE_INPUT_BYTES,
  applyDisplayGeo,
  classifyJson,
  ingestCase,
  ingestDistCaseBytes,
  ingestJsonDrop,
  parseDisplay,
  parseGeo,
} from "../src/index.js";
import { assertEngineInputLength } from "../src/input-limit.js";

describe("engine byte input limit", () => {
  it("accepts the boundary and rejects one byte over without allocating it", () => {
    expect(() => assertEngineInputLength(MAX_ENGINE_INPUT_BYTES)).not.toThrow();
    expect(() => assertEngineInputLength(MAX_ENGINE_INPUT_BYTES + 1)).toThrow(
      "input exceeds 128 MiB limit",
    );
  });

  const oversized = {
    byteLength: MAX_ENGINE_INPUT_BYTES + 1,
  } as unknown as Uint8Array;

  it.each([
    ["classifyJson", () => classifyJson(oversized)],
    ["ingestJsonDrop", () => ingestJsonDrop(oversized)],
    ["ingestCase", () => ingestCase(oversized, "m")],
    ["ingestDistCaseBytes", () => ingestDistCaseBytes(oversized, "dss")],
    ["parseDisplay", () => parseDisplay(oversized)],
    ["parseGeo", () => parseGeo(oversized, "coords.csv")],
    ["applyDisplayGeo", () => applyDisplayGeo("{}", oversized)],
  ])("rejects %s before engine dispatch", async (_name, invoke) => {
    await expect(invoke()).rejects.toThrow("input exceeds 128 MiB limit");
  });
});
