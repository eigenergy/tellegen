import { expect, it, vi } from "vitest";
import { CollectionImport } from "../src/lib/collection-import.svelte.js";
import { captureDrop, namedFiles } from "../src/lib/collection-files.js";
import type {
  CollectionInspection,
  ImportedCollectionCase,
} from "@tellegen/engine";

const result = {
  kind: "balanced",
  payload: { module_json: "independent" },
  dependencies: [],
  incomplete: false,
  origin: {},
} as unknown as ImportedCollectionCase;
function session(paths = ["a.m", "b.dss"], bytes = 10) {
  return {
    inspection: {
      name: "same",
      entries: paths.map((path) => ({ path, bytes })),
      candidates: paths.map((primary) => ({
        selection: { primary },
        status: "selectable",
      })),
      original_zip: true,
      limits: { files: 4096, expanded_bytes: 64 << 20 },
    } as CollectionInspection,
    ingest: vi.fn(async () => result),
    originalZip: vi.fn(async () => new Uint8Array([1])),
    sourceZip: vi.fn(async () => new Uint8Array([2])),
    free: vi.fn(async () => {}),
    cancel: vi.fn(),
  };
}
const zip = (name = "study.zip") => ({
  path: name,
  file: new File(["zip"], name),
});

it("turns a late export failure into cancellation after a new chooser starts", async () => {
  const s = session();
  let fail!: (error: Error) => void;
  s.sourceZip.mockImplementation(
    () =>
      new Promise((_resolve, reject) => {
        fail = reject;
      }),
  );
  const model = new CollectionImport(vi.fn(), {
    zip: vi.fn(async () => s),
    files: vi.fn(),
  });
  await model.start(async () => [zip()]);
  const pending = model.download(model.collections[0].id, model.rows[0]);
  model.close();
  await model.start(async () => [zip("next.zip")]);
  fail(new Error("old worker failed"));
  await expect(pending).rejects.toMatchObject({ name: "AbortError" });
  expect(model.errors).toEqual([]);
  expect(model.busy).toBe(false);
  model.close();
});

it("isolates identically named ZIP uploads and keeps successful cases after a failure", async () => {
  const a = session(),
    b = session();
  a.ingest.mockRejectedValueOnce(new Error("missing include"));
  const accept = vi.fn();
  const acquire = {
    zip: vi.fn().mockResolvedValueOnce(a).mockResolvedValueOnce(b),
    files: vi.fn(),
  };
  const model = new CollectionImport(accept, acquire);
  await model.start(async () => [zip(), zip()]);
  expect(model.rows).toHaveLength(4);
  expect(new Set(model.rows.map((r) => r.session)).size).toBe(2);
  expect(model.rows[0].label).not.toBe(model.rows[2].label);
  expect(accept).not.toHaveBeenCalled();
  model.rows.forEach((r) => {
    r.checked = true;
  });
  await model.importSelected();
  expect(accept).toHaveBeenCalledTimes(3);
  expect(model.rows.map((r) => r.outcome)).toEqual([
    "failed",
    "imported",
    "imported",
    "imported",
  ]);
  model.close();
  expect(a.cancel).toHaveBeenCalledOnce();
  expect(b.cancel).toHaveBeenCalledOnce();
  expect(accept.mock.calls[0][1]).toEqual(result);
});

it("discards a late import after cancellation and permits a fresh acquisition", async () => {
  const kept = session(["one.m"]);
  let finish!: (value: ImportedCollectionCase) => void;
  kept.ingest.mockImplementation(
    () =>
      new Promise((resolve) => {
        finish = resolve;
      }),
  );
  const accept = vi.fn();
  const model = new CollectionImport(accept, {
    zip: vi.fn(async () => kept),
    files: vi.fn(),
  });
  await model.start(async () => [zip()]);
  model.rows[0].checked = true;
  const pending = model.importSelected();
  model.close();
  finish(result);
  await pending;
  expect(accept).not.toHaveBeenCalled();
  expect(model.open).toBe(false);
  await model.start(async () => [zip()]);
  expect(model.rows).toHaveLength(1);
  model.close();
});

it("bounds aggregate expanded residency and releases a rejected archive", async () => {
  const a = session(["a.m"], 40 << 20),
    b = session(["b.m"], 40 << 20);
  const model = new CollectionImport(vi.fn(), {
    zip: vi.fn().mockResolvedValueOnce(a).mockResolvedValueOnce(b),
    files: vi.fn(),
  });
  await model.start(async () => [zip("a.zip"), zip("b.zip")]);
  expect(model.rows).toHaveLength(1);
  expect(b.free).toHaveBeenCalledOnce();
  expect(model.errors.join()).toContain("64 MiB expanded");
  model.close();
});

it("shows incomplete and solver-restricted imports without discarding them", async () => {
  const s = session();
  s.ingest
    .mockResolvedValueOnce({ ...result, incomplete: true })
    .mockResolvedValueOnce({
      ...result,
      kind: "multiconductor",
      payload: {
        mc_pf_unavailable_reason: "Unsupported three-winding transformer",
      },
    } as ImportedCollectionCase);
  const accept = vi.fn(),
    model = new CollectionImport(accept, {
      zip: vi.fn(async () => s),
      files: vi.fn(),
    });
  await model.start(async () => [zip()]);
  model.rows.forEach((r) => {
    r.checked = true;
  });
  await model.importSelected();
  expect(accept).toHaveBeenCalledTimes(2);
  expect(model.rows.map((r) => r.outcome)).toEqual(["incomplete", "view-only"]);
  expect(model.rows[1].message).toContain("three-winding");
  model.close();
});

it("requires explicit formats/groups and never joins documents from separate collections", async () => {
  const s = session(["one.xml", "two.xml"]);
  const model = new CollectionImport(vi.fn(), {
    zip: vi.fn(async () => s),
    files: vi.fn(),
  });
  await model.start(async () => [zip()]);
  model.rows.forEach((r) => {
    r.checked = true;
  });
  model.groupSelected("cgmes");
  expect(model.rows[2].selection).toEqual({
    members: ["one.xml", "two.xml"],
    format: "cgmes",
  });
  expect(model.rows[0].checked).toBe(false);
  model.addExplicit(model.collections[0].id, "nonstandard.txt", "dss");
  expect(model.rows[3].format).toBe("dss");
  model.close();
});

it("reads every directory batch and preserves relative paths without reading file bytes", async () => {
  const file = (name: string) => ({
    name,
    isFile: true,
    isDirectory: false,
    file: (done: (f: File) => void) => done(new File(["data"], name)),
  });
  const batches = [[file("a.dss")], [file("b.inc")], []];
  const directory = {
    name: "folder",
    isDirectory: true,
    isFile: false,
    createReader: () => ({
      readEntries: (done: (v: unknown[]) => void) => done(batches.shift()!),
    }),
  };
  const data = {
    items: [{ kind: "file", webkitGetAsEntry: () => directory }],
    files: [],
  } as unknown as DataTransfer;
  const read = captureDrop(data);
  const files = await read(new AbortController().signal);
  expect(files.map((f) => f.path)).toEqual(["folder/a.dss", "folder/b.inc"]);
  expect(batches).toHaveLength(0);
});

it("admits folder member counts separately from upload counts and rejects overages before reads", () => {
  const files = Array.from(
    { length: 40 },
    (_, i) => new File(["x"], `${i}.inc`),
  );
  expect(namedFiles(files)).toHaveLength(40);
  const read = vi.fn();
  expect(() =>
    namedFiles([
      {
        name: "huge",
        size: (64 << 20) + 1,
        arrayBuffer: read,
      } as unknown as File,
    ]),
  ).toThrow("64 MiB");
  expect(read).not.toHaveBeenCalled();
});
