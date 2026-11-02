import { expect, it, vi } from "vitest";
import {
  BrowserMcInput,
  BrowserMcPfSession,
  createMcPfSession,
  summarizeMcPfResult,
  type McPfResult,
} from "../src/index.js";
import type { EngineHost } from "../src/host.js";
import type { WasmModule } from "../src/module.js";
import { emptyHandles, runRequest } from "../src/protocol.js";

const summary = summarizeMcPfResult(
  {
    converged: true,
    voltage_valid: true,
    min_voltage_pu: 1,
    max_voltage_pu: 1,
    voltage_violations: [],
    iterations: 1,
    factorization_count: 1,
    matrix_dimension: 1,
    matrix_nonzeros: 1,
    voltage_change: 0,
    physical_kcl_residual: 0,
    scaled_kcl_residual: 0,
    terminals: [],
    element_ports: [],
    source_reactions: [],
  } satisfies McPfResult,
  1,
);

it("solves a kept module on its own host without sending IR text", async () => {
  const call = vi.fn<EngineHost["call"]>(async (request) => {
    expect(request).toMatchObject({ op: "mc_pf_session_from_input", input: 4 });
    expect(request).not.toHaveProperty("module_json");
    return JSON.stringify(summary);
  });
  const input = new BrowserMcInput({ kind: "worker", call }, 4);
  const session = await createMcPfSession(input, { tolerance: 1e-9 });
  expect(session.initialSummary).toEqual(summary);
  expect(JSON.parse((call.mock.calls[0][0] as { options: string }).options)).toEqual({
    tolerance: 1e-9,
  });
});

it("writes IR text only on request and frees a kept module once", async () => {
  const call = vi.fn<EngineHost["call"]>(async (request) =>
    request.op === "mc_input_module_json" ? '{"schema":"pio-ir"}' : null,
  );
  const input = new BrowserMcInput({ kind: "worker", call }, 9);
  expect(call).not.toHaveBeenCalled();
  await expect(input.moduleJson()).resolves.toBe('{"schema":"pio-ir"}');
  input.free();
  input.free();
  expect(call.mock.calls.filter(([r]) => r.op === "mc_input_free")).toEqual([
    [{ op: "mc_input_free", input: 9 }],
  ]);
  await expect(input.moduleJson()).rejects.toThrow("freed");
});

it("keeps a session's edited input in the engine under a new handle", async () => {
  const call = vi.fn<EngineHost["call"]>(async () => null);
  const host: EngineHost = { kind: "worker", call };
  const session = new BrowserMcPfSession(host, 5, summary);
  const input = await session.input();
  expect(call).toHaveBeenCalledWith({
    op: "mc_pf_session_input",
    session: 5,
    input: input.handle,
  });
  expect(input.host).toBe(host);
});

it("routes retained ingests, sessions, and frees through one handle table", () => {
  const kept = { module_json: vi.fn(() => "ir"), free: vi.fn() };
  const ingest = {
    take_input: vi.fn(() => kept),
    take_payload: vi.fn(() => '{"input_retained":true}'),
    free: vi.fn(),
  };
  const created = { summary: vi.fn(() => "summary") };
  const from_input = vi.fn(() => created);
  const mod = {
    ingest_json_drop: vi.fn(() => "legacy"),
    ingest_json_drop_retained: vi.fn(() => ingest),
    McPfSession: { from_input },
  } as unknown as WasmModule;
  const handles = emptyHandles();

  const bytes = new Uint8Array([123, 125]);
  expect(runRequest(mod, handles, { op: "ingest_json_drop", bytes })).toBe("legacy");
  expect(handles.mcInputs.size).toBe(0);
  expect(runRequest(mod, handles, { op: "ingest_json_drop", bytes, input: 2 })).toBe(
    '{"input_retained":true}',
  );
  expect(handles.mcInputs.get(2)).toBe(kept);
  expect(ingest.free).toHaveBeenCalled();

  expect(
    runRequest(mod, handles, { op: "mc_pf_session_from_input", session: 8, input: 2, options: "{}" }),
  ).toBe("summary");
  expect(from_input).toHaveBeenCalledWith(kept, "{}");
  expect(handles.mcPfSessions.get(8)).toBe(created);
  expect(runRequest(mod, handles, { op: "mc_input_module_json", input: 2 })).toBe("ir");

  runRequest(mod, handles, { op: "mc_input_free", input: 2 });
  expect(kept.free).toHaveBeenCalled();
  expect(handles.mcInputs.size).toBe(0);
  expect(() =>
    runRequest(mod, handles, { op: "mc_pf_session_from_input", session: 9, input: 2, options: "{}" }),
  ).toThrow("unknown multiconductor input handle 2");
});
