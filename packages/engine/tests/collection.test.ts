import { beforeEach, expect, it, vi } from "vitest";
import { BrowserCaseCollection } from "../src/collection.js";
import type { EngineHost } from "../src/host.js";
import type { EngineRequest, EngineValue } from "../src/protocol.js";
import { emptyHandles, runRequest, transferables } from "../src/protocol.js";
import type { WasmModule } from "../src/module.js";

const mock = vi.hoisted(() => ({ host: null as EngineHost | null }));
vi.mock("../src/host.js", () => ({ isolatedEngineHost: () => mock.host }));
const inspection = { name: "study", entries: [], candidates: [], original_zip: true };
const imported = { kind: "balanced", payload: { module_json: "independent IR" }, dependencies: [], incomplete: false };
let call: ReturnType<typeof vi.fn<EngineHost["call"]>>;
beforeEach(() => {
  call = vi.fn<EngineHost["call"]>(async req => req.op === "collection_ingest" ? JSON.stringify(imported) : req.op === "collection_free" ? null : JSON.stringify(inspection));
  mock.host = { kind: "worker", call, cancel: vi.fn(() => true) };
});

it("acquires a binary tree once and sends only selections for multiple imports", async () => {
  const collection = await BrowserCaseCollection.fromFiles("study", [
    { path: "a.dss", bytes: new Uint8Array([0, 255]) },
    { path: "shared.inc", bytes: new Uint8Array([1]) },
  ]);
  expect(call.mock.calls[0][0]).toMatchObject({ names: '["a.dss","shared.inc"]', offsets: new Uint32Array([0, 2, 3]), bytes: new Uint8Array([0, 255, 1]) });
  const first = await collection.ingest({ primary: "a.dss" });
  await collection.ingest({ primary: "b.dss" });
  expect(call.mock.calls.slice(1).every(([r]) => r.op === "collection_ingest" && !("bytes" in r))).toBe(true);
  await collection.free();
  await collection.free();
  expect(call.mock.calls.filter(([r]) => r.op === "collection_free")).toHaveLength(1);
  expect(first).toEqual(imported);
  await expect(collection.ingest({ primary: "a.dss" })).rejects.toThrow("closed");
});

it("cancels acquisition and rejects late results from the direct fallback", async () => {
  let finish!: (value: EngineValue) => void;
  mock.host = { kind: "direct", call: vi.fn<EngineHost["call"]>(async (req: EngineRequest) => req.op === "collection_free" ? null : new Promise<EngineValue>(resolve => { finish = resolve; })), cancel: vi.fn(() => false) };
  const abort = new AbortController();
  const pending = BrowserCaseCollection.fromZip("study.zip", new Uint8Array([1]), abort.signal);
  abort.abort();
  finish(JSON.stringify(inspection));
  await expect(pending).rejects.toMatchObject({ name: "AbortError" });
  expect(mock.host!.call).toHaveBeenCalledWith(expect.objectContaining({ op: "collection_free" }));
});

it("keeps completed imports after one selection fails and rejects concurrent work", async () => {
  const collection = await BrowserCaseCollection.fromZip("study.zip", new Uint8Array([1]));
  const first = await collection.ingest({ primary: "first.m" });
  let finish!: () => void;
  call.mockImplementationOnce(() => new Promise((_resolve, reject) => { finish = () => reject(new Error("missing include")); }));
  const pending = collection.ingest({ primary: "bad.dss" });
  await expect(collection.ingest({ primary: "other.m" })).rejects.toThrow("already running");
  finish();
  await expect(pending).rejects.toThrow("missing include");
  await expect(collection.ingest({ primary: "other.m" })).resolves.toEqual(imported);
  collection.cancel();
  expect(first).toEqual(imported);
});

it("checks limits before creating a host or copying a tree", async () => {
  await expect(BrowserCaseCollection.fromFiles("study", Array(4097).fill({ path: "x", bytes: new Uint8Array() }))).rejects.toThrow("4096");
  await expect(BrowserCaseCollection.fromFiles("study", [{ path: "x".repeat(1024 * 1024), bytes: new Uint8Array() }])).rejects.toThrow("metadata");
  const abort = new AbortController(); abort.abort();
  await expect(BrowserCaseCollection.fromZip("study.zip", new Uint8Array(), abort.signal)).rejects.toMatchObject({ name: "AbortError" });
  expect(call).not.toHaveBeenCalled();
});

it("frees a WASM collection when inspection fails and transfers binary exports", () => {
  const failed = { inspect: () => { throw new Error("budget"); }, free: vi.fn() };
  const mod = { CaseCollection: { from_zip: () => failed } } as unknown as WasmModule;
  const handles = emptyHandles();
  expect(() => runRequest(mod, handles, { op: "collection_zip", collection: 1, name: "study", bytes: new Uint8Array() })).toThrow("budget");
  expect(failed.free).toHaveBeenCalledOnce();
  expect(handles.collections.size).toBe(0);
  const bytes = new Uint8Array([1, 2]);
  expect(transferables(bytes)).toEqual([bytes.buffer]);
});
