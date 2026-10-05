import { expect, it, vi } from "vitest";
import {
  BrowserMcPfSession,
  mcPfResultDetail,
  summarizeMcPfResult,
  type McPfResult,
  type McPfSummary,
} from "../src/index.js";
import type { EngineHost } from "../src/host.js";

const c = (re: number, im: number) => ({ re, im });

const result: McPfResult = {
  converged: true,
  voltage_valid: false,
  min_voltage_pu: 0.84,
  max_voltage_pu: 1.01,
  voltage_violations: [
    {
      load: "ld",
      branch: 0,
      bus: "b2",
      voltage: 201,
      nominal_voltage: 240,
      voltage_pu: 0.84,
      bound: "minimum",
    },
  ],
  iterations: 9,
  factorization_count: 1,
  matrix_dimension: 3,
  matrix_nonzeros: 7,
  voltage_change: 1e-9,
  physical_kcl_residual: 1e-7,
  scaled_kcl_residual: 0.1,
  terminals: [
    { bus: "b1", terminal: "a", voltage: c(240, 0), current_into_network: c(5, -1), power_into_network: c(1200, 240) },
    { bus: "b2", terminal: "a", voltage: c(201, -3), current_into_network: c(-5, 1), power_into_network: c(-1008, 186) },
    { bus: "b2", terminal: "n", voltage: c(0.4, 0.1), current_into_network: c(0, 0), power_into_network: c(0, 0) },
  ],
  element_ports: [
    { element: "ld", kind: "load", branch: 0, bus: "b2", terminal: "a", current_into_element: c(5, -1), power_into_element: c(1008, -186) },
    { element: "l1", kind: "line", branch: 0, bus: "b1", terminal: "a", current_into_element: c(5, -1), power_into_element: c(1200, 240) },
    { element: "l1", kind: "line", branch: 0, bus: "b2", terminal: "a", current_into_element: c(-5, 1), power_into_element: c(-1008, 186) },
  ],
  source_reactions: [
    { source: "vs", terminal: "a", current_into_network: c(5, -1), power_into_network: c(1200, 240) },
    { source: "vs", terminal: "n", current_into_network: c(0, 0), power_into_network: c(0.5, 0.25) },
  ],
};

it("summarizes a stored result exactly as the engine does", () => {
  expect(summarizeMcPfResult(result, 3)).toEqual({
    converged: true,
    voltage_valid: false,
    min_voltage_pu: 0.84,
    max_voltage_pu: 1.01,
    voltage_violation_count: 1,
    iterations: 9,
    factorization_count: 1,
    matrix_dimension: 3,
    matrix_nonzeros: 7,
    voltage_change: 1e-9,
    physical_kcl_residual: 1e-7,
    scaled_kcl_residual: 0.1,
    terminal_count: 3,
    element_port_count: 3,
    source_power_into_network: c(1200.5, 240.25),
    passive_loss: c(192, 426),
    solve_count: 3,
  } satisfies McPfSummary);
});

it("pages a stored result like a live session detail query", () => {
  const page = mcPfResultDetail(result, { bus: "b2", port_offset: 1, port_limit: 1 });
  expect(page.terminals.map((t) => t.terminal)).toEqual(["a", "n"]);
  expect(page.element_ports).toEqual([result.element_ports[1]]);
  expect(page.element_port_total).toBe(3);
  const line = mcPfResultDetail(result, { element: "l1" });
  expect(line.terminals).toEqual([]);
  expect(line.element_port_total).toBe(2);
});

it("returns the summary from an edit and records its latency breakdown", async () => {
  const summary = summarizeMcPfResult(result, 2);
  const call = vi.fn<EngineHost["call"]>();
  const callTimed = vi.fn<NonNullable<EngineHost["callTimed"]>>(async (request) => {
    expect(request).toEqual({
      op: "mc_pf_session_replace_load_powers",
      session: 7,
      edits: JSON.stringify([{ load: "ld", branch: 0, p_w: 900, q_var: 100 }]),
    });
    return { value: JSON.stringify(summary), engineMs: 4.5 };
  });
  const session = new BrowserMcPfSession({ call, callTimed }, 7, summarizeMcPfResult(result, 1));
  await expect(
    session.replaceLoadPowers([{ load: "ld", branch: 0, p_w: 900, q_var: 100 }]),
  ).resolves.toEqual(summary);
  expect(call).not.toHaveBeenCalled();
  expect(session.lastTiming).toMatchObject({
    engine_ms: 4.5,
    payload_chars: JSON.stringify(summary).length,
  });
  expect(session.lastTiming!.round_trip_ms).toBeGreaterThanOrEqual(0);
  expect(session.initialSummary.solve_count).toBe(1);
});

it("fetches terminal identities once and terminal arrays on demand", async () => {
  const voltages = new Float64Array([240, 0, 201, -3, 0.4, 0.1]);
  const call = vi.fn<EngineHost["call"]>(async (request) => {
    switch (request.op) {
      case "mc_pf_session_terminal_ids":
        return JSON.stringify([["b1", "a"], ["b2", "a"], ["b2", "n"]]);
      case "mc_pf_session_terminal_voltages":
        return voltages;
      case "mc_pf_session_terminal_currents":
        return "not an array";
      case "mc_pf_session_detail":
        expect(JSON.parse(request.query)).toEqual({ bus: "b2", port_limit: 5 });
        return JSON.stringify(mcPfResultDetail(result, { bus: "b2", port_limit: 5 }, 2));
      default:
        throw new Error(`unexpected operation ${request.op}`);
    }
  });
  const session = new BrowserMcPfSession({ call }, 3, summarizeMcPfResult(result, 1));
  const [first, second] = await Promise.all([session.terminalIds(), session.terminalIds()]);
  expect(first).toBe(second);
  expect(call.mock.calls.filter(([request]) => request.op === "mc_pf_session_terminal_ids")).toHaveLength(1);
  await expect(session.terminalVoltages()).resolves.toBe(voltages);
  await expect(session.terminalCurrents()).rejects.toThrow("numeric array");
  await expect(session.detail({ bus: "b2", port_limit: 5 })).resolves.toMatchObject({
    solve_count: 2,
    element_port_total: 3,
  });
});
