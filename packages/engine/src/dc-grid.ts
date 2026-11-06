/** Explicit steady-state resistive DC model. Powers MW, voltages kV, currents kA. */
export interface DcGrid {
  nodes: { id: string; initial_kv: number; reference_kv: number | null }[];
  branches: {
    id: string;
    from: string;
    to: string;
    resistance_ohm: number;
    in_service: boolean;
    current_limit_ka: number | null;
  }[];
  converters: {
    id: string;
    positive: string;
    negative: string;
    ac_bus: number;
    in_service: boolean;
    control:
      | { kind: "voltage"; kv: number }
      | { kind: "active_power"; mw: number }
      | { kind: "current"; ka: number }
      | { kind: "droop"; mw: number; kv: number; mw_per_kv: number };
    current_direction?: "positive" | "negative";
    loss: [number, number, number];
    current_limit_ka: number | null;
    active_min_mw: number | null;
    active_max_mw: number | null;
    voltage_min_kv: number | null;
    voltage_max_kv: number | null;
  }[];
}
export interface DcGridSolution {
  iterations: number;
  residual: number;
  limits_checked: number;
  limits_unspecified: number;
  nodes: { id: string; voltage_kv: number }[];
  branches: { id: string; current_ka: number; loss_mw: number }[];
  converters: {
    id: string;
    ac_bus: number;
    voltage_kv: number;
    current_ka: number;
    ac_power_mw: number;
    loss_mw: number;
  }[];
  violations: string[];
}

export interface ControlResult {
  id: string;
  setting: number;
  units: string;
  measured: number;
  measured_units: string;
  target_low: number;
  target_high: number;
  status: "regulated" | "at_limit" | "discrete_deadband_unreachable";
}
export interface RegulatedSolution {
  solution: import("./generated/api-types.js").SolveResponse;
  controls: ControlResult[];
  passes: number;
  violations: string[];
}
export interface AcDcSolution {
  dc: DcGridSolution;
  ac: import("./generated/api-types.js").SolveResponse;
  violations: string[];
  controls: ControlResult[];
  converter_reactive: [string, number][];
  scope: string;
}
