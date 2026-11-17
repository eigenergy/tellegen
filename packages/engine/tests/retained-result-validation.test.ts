import { readFileSync } from "node:fs";
import { beforeEach, expect, it, vi } from "vitest";
import type { EngineRequest } from "../src/protocol.js";

const host = vi.hoisted(() => ({ kind: "worker" as const, call: vi.fn() }));
vi.mock("../src/host.js", async (original) => ({
  ...(await original<typeof import("../src/host.js")>()),
  engineHost: () => host,
}));
import {
  BrowserMcInput,
  ingestDistCaseBytes,
  ingestJsonDrop,
} from "../src/index.js";

beforeEach(() => {
  host.call.mockReset();
});

function releases(): EngineRequest[] {
  return host.call.mock.calls
    .map(([request]) => request as EngineRequest)
    .filter((request) => request.op === "mc_input_free");
}

for (const kind of ["distribution", "json"] as const) {
  it(`frees the new retained ${kind} input when response validation fails`, async () => {
    host.call.mockImplementation(async (request: EngineRequest) =>
      request.op === "mc_input_free" ? null : "{}",
    );
    const action =
      kind === "distribution"
        ? ingestDistCaseBytes(new Uint8Array([1]), "dss", { retain: true })
        : ingestJsonDrop(new Uint8Array([1]), { retain: true });
    await expect(action).rejects.toThrow("Invalid engine");
    const request = host.call.mock.calls[0][0];
    await vi.waitFor(() =>
      expect(releases()).toEqual([
        { op: "mc_input_free", input: request.input },
      ]),
    );
  });
}

it("frees only the new energized input after a malformed response", async () => {
  host.call.mockImplementation(async (request: EngineRequest) => {
    if (request.op === "mc_input_free") return null;
    if (request.op === "mc_input_module_json") return "original module";
    return "{}";
  });
  const original = new BrowserMcInput(host, 50_000);
  await expect(original.prepareEnergized()).rejects.toThrow("Invalid engine");
  const request = host.call.mock.calls[0][0];
  await vi.waitFor(() =>
    expect(releases()).toEqual([
      { op: "mc_input_free", input: request.output },
    ]),
  );
  expect(request.output).not.toBe(original.handle);
  await expect(original.moduleJson()).resolves.toBe("original module");
  original.free();
});

it("releases an unused retained handle after a successful balanced JSON import", async () => {
  const payload = JSON.parse(
    readFileSync(
      new URL(
        "../../../crates/tellegen-wasm/tests/fixtures/adapter-balanced-before.json",
        import.meta.url,
      ),
      "utf8",
    ),
  );
  host.call.mockImplementation(async (request: EngineRequest) =>
    request.op === "mc_input_free"
      ? null
      : JSON.stringify({ kind: "module", format: null, payload }),
  );
  const result = await ingestJsonDrop(new Uint8Array([1]), { retain: true });
  expect(result.kind).toBe("module");
  const request = host.call.mock.calls[0][0];
  await vi.waitFor(() =>
    expect(releases()).toEqual([{ op: "mc_input_free", input: request.input }]),
  );
});
