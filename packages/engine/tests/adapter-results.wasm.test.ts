import { readFileSync } from "node:fs";
import { beforeAll, expect, it } from "vitest";
import * as wasm from "../src/wasm-pkg/tellegen.js";
import type { NetworkEquipment } from "../src/generated/api-types.js";
import { parseApiResult, validateApiResult } from "../src/result-validation.js";

const bytes = (text: string) => new TextEncoder().encode(text);
const source = (path: string) =>
  readFileSync(new URL(`../../../${path}`, import.meta.url), "utf8");
const balanced = () =>
  parseApiResult<{ module_json: string }>(
    "ingest_balanced",
    wasm.ingest_case(
      bytes(source("python/tellegen/mcp/examples/case3.m")),
      "matpower",
    ),
  );
const distribution = () =>
  parseApiResult<{ module_json: string; graph: unknown }>(
    "ingest_distribution",
    wasm.ingest_dist_case(
      "new circuit.validation basekv=.4\nnew load.l phases=3 bus1=sourcebus kv=.4 kw=3 kvar=1\n",
      "dss",
    ),
  );
beforeAll(() =>
  wasm.initSync({
    module: readFileSync(
      new URL("../src/wasm-pkg/tellegen_bg.wasm", import.meta.url),
    ),
  }),
);

it("validates actual import, geographic, layout and Study export responses", async () => {
  const input = await balanced();
  await parseApiResult("classifier", wasm.classify_json(bytes(input.module_json)));
  await parseApiResult("json_drop", wasm.ingest_json_drop(bytes(input.module_json)));
  await parseApiResult("json_drop", wasm.ingest_json_drop(bytes("{}")));
  const geo = await parseApiResult<{ layer: string }>(
    "parsed_geo",
    wasm.parse_geo(
      bytes("bus_i,lat,lon\n1,33,-84\n2,34,-84\n3,34,-83\n"),
      "coords.csv",
    ),
  );
  await parseApiResult(
    "ingest_balanced",
    wasm.apply_geo(input.module_json, geo.layer),
  );
  await parseApiResult(
    "ingest_balanced",
    wasm.apply_layout(
      input.module_json,
      JSON.stringify({ 1: [-84, 33], 2: [-84, 34], 3: [-83, 34] }),
      "manual",
    ),
  );
  await parseApiResult(
    "tx_layout",
    wasm.tx_schematic_layout(
      JSON.stringify({
        bus_substation: [0, 1, 2],
        kv: [230, 230, 230],
        from: [0, 0, 1],
        to: [1, 2, 2],
        in_service: [1, 1, 1],
        xy: [0, 0, 100, 0, 50, 100],
      }),
      "{}",
    ),
  );
  const study = new wasm.Study(input.module_json, "dcopf");
  try {
    await parseApiResult("exported_case", study.export("matpower"));
    await parseApiResult("geo_report", study.apply_geo(geo.layer));
  } finally {
    study.free();
  }
  await parseApiResult("regulated", wasm.solve_regulated(input.module_json));
});

it("validates actual multiconductor preparation and reports unavailable formulations faithfully", async () => {
  const input = await distribution();
  await parseApiResult("json_drop", wasm.ingest_json_drop(bytes(input.module_json)));
  await parseApiResult(
    "mc_layout",
    wasm.mc_schematic_layout(JSON.stringify(input.graph), "{}"),
  );
  await parseApiResult(
    "energized",
    wasm.prepare_mc_energized_network(input.module_json),
  );
  const retained = new wasm.McInput(input.module_json);
  try {
    const prepared = retained.prepare_energized();
    try {
      expect(
        (await parseApiResult<{ input_retained: boolean }>(
          "energized_retained",
          prepared.take_payload(),
        )).input_retained,
      ).toBe(true);
      prepared.take_input()?.free();
    } finally {
      prepared.free();
    }
  } finally {
    retained.free();
  }
  const capabilities = await parseApiResult<
    Array<{ formulation: string; available: boolean }>
  >("capabilities", wasm.capabilities_json());
  expect(
    capabilities.find((row) => row.formulation === "acopf")?.available,
  ).toBe(false);
  expect(
    capabilities.find((row) => row.formulation === "dcopf")?.available,
  ).toBe(true);
  await parseApiResult("memory_stats", wasm.memory_stats());
});

it("validates actual detailed DC and coupled AC/DC results", async () => {
  const converter = (id: string, positive: string, control: unknown) => ({
    id,
    positive,
    negative: "return",
    ac_bus: 1,
    in_service: true,
    control,
    loss: [0, 0, 0],
    current_limit_ka: null,
    active_min_mw: null,
    active_max_mw: null,
    voltage_min_kv: null,
    voltage_max_kv: null,
  });
  const grid = {
    nodes: [
      { id: "return", initial_kv: 0, reference_kv: 0 },
      { id: "a", initial_kv: 320, reference_kv: null },
      { id: "b", initial_kv: 320, reference_kv: null },
    ],
    branches: [
      {
        id: "ab",
        from: "a",
        to: "b",
        resistance_ohm: 5,
        in_service: true,
        current_limit_ka: null,
      },
    ],
    converters: [
      converter("source", "a", { kind: "voltage", kv: 320 }),
      converter("load", "b", { kind: "active_power", mw: -100 }),
    ],
  };
  await parseApiResult("dc_grid", wasm.solve_dc_grid(JSON.stringify(grid)));
  await parseApiResult(
    "ac_dc",
    wasm.solve_ac_dc(
      source("crates/tellegen/tests/data/transmission-mtdc.pio.json"),
      "dcpf",
      false,
    ),
  );
});

it("validates actual complete synthetic output and session ingestion", async () => {
  await parseApiResult(
    "synth_places",
    wasm.synth_load_places(
      bytes(
        JSON.stringify([
          { n: "London", lo: -0.12, la: 51.5, p: 9000000, c: "GB" },
        ]),
      ),
    ),
  );
  const request = JSON.stringify({
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
  });
  await parseApiResult("synth_queries", wasm.synth_overpass_queries(request));
  await parseApiResult("synth_generated", wasm.synth_generate(request));
  const session = new wasm.SynthSession(request);
  try {
    let done = false;
    for (let step = 0; step < 50 && !done; step++)
      done = (await parseApiResult<{ done: boolean }>(
        "synth_step",
        session.advance(),
      )).done;
    expect(done).toBe(true);
    await parseApiResult("ingest_balanced", session.ingest());
    await parseApiResult("ingest_balanced_view", session.ingest_view());
  } finally {
    session.free();
  }
});


it("validates detailed drawing output from the licensed CGMES fixture", async () => {
  const names = ["NodeBreaker_EQ.xml", "NodeBreaker_SSH.xml"];
  const files = names.map(name => bytes(source(`tests/data/node-breaker/${name}`)));
  const joined = new Uint8Array(files[0].length + files[1].length);
  joined.set(files[0]); joined.set(files[1], files[0].length);
  const collection = wasm.CaseCollection.from_files("node-breaker", JSON.stringify(names), new Uint32Array([0, files[0].length, joined.length]), joined);
  try {
    // Exercise the ZIP chooser's suggested selection, not a hand-assembled group.
    const zip = collection.source_zip(JSON.stringify({ members: names, format: "cgmes" }), "selected");
    const zipped = wasm.CaseCollection.from_zip("node-breaker.zip", zip);
    let imported: { payload: { topology: { equipment: NetworkEquipment } } };
    try {
      const inspection = await parseApiResult<{ candidates: { selection: { members: string[]; format: string } }[] }>("collection_inspection", zipped.inspect());
      expect(inspection.candidates).toHaveLength(1);
      expect(inspection.candidates[0].selection.members).toEqual(names);
      expect(inspection.candidates[0].selection.format).toBe("cgmes");
      imported = await parseApiResult("collection_import", zipped.ingest(JSON.stringify(inspection.candidates[0].selection)));
      expect(() => zipped.ingest(JSON.stringify({ primary: names[0], format: "cgmes" }))).toThrow("companion documents");
    } finally { zipped.free(); }
    const equipment = imported.payload.topology.equipment;
    const drawing = equipment.node_breaker;
    expect(drawing?.nodes).toHaveLength(5);
    expect(drawing?.switches).toHaveLength(3);
    expect(drawing?.switches.find(item => item.name === "DSC1")?.open).toBe(true);
    if (!drawing) throw new Error("Current engine omitted detailed drawing output");
    const network = { id: "station", name: "Station", base_mva: 100, synthetic_coords: true, buses: [], branches: [], equipment };
    await validateApiResult("http_network", network);
    const malformed = { ...equipment, node_breaker: { ...drawing, switches: drawing.switches.map((item, i) => i === 0 ? { ...item, open: "yes" } : item) } };
    await expect(validateApiResult("http_network", { ...network, equipment: malformed })).rejects.toThrow("/node_breaker/switches/0/open");
    await expect(validateApiResult("ingest_balanced", { ...imported.payload, topology: { ...imported.payload.topology, equipment: malformed } })).rejects.toThrow("/node_breaker/switches/0/open");
    const { node_breaker: _detail, ...legacy } = equipment;
    await validateApiResult("http_network", { ...network, equipment: legacy });
    await validateApiResult("ingest_balanced", { ...imported.payload, topology: { ...imported.payload.topology, equipment: legacy } });
  } finally { collection.free(); }
});
