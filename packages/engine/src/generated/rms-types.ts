// Generated from Rust by rms_schema and generate-rms-types.mjs.
// Source SHA256: c0179764f390b1383e37aa952d3a9b571048f5f11a52715636611feb6d4b0aad

/** Rotor angle figures, degrees. */
export type AngleScreen = { "first_swing_deg": number; "first_swing_unit"?: string | null; "initial_separation_deg"?: number; "max_coi_angle_deg": number; "max_coi_angle_t": number; "max_coi_angle_unit"?: string | null; "max_separation_deg": number; "pair": Array<string>; "pole_slips": Array<PoleSlip>; "separation_t": number; "settled": boolean; "trend"?: (SwingTrend) | (null); };

/** Frequency figures, Hz (the COI of the island with the most inertia). */
export type FrequencyScreen = { "coi_nadir_hz"?: number | null; "coi_peak_hz"?: number | null; "final_hz"?: number | null; "lowest_bus"?: number | null; "lowest_bus_hz"?: number | null; "lowest_bus_t": number; "lowest_unit"?: string | null; "lowest_unit_hz"?: number | null; "lowest_unit_t": number; "max_rocof_hz_s": number; "nadir_t": number; "nominal_hz": number; "peak_t": number; "rocof_t": number; "settled": boolean; };

/** A controller reference a [`RmsEvent::SetReference`] sets (and a frequency
 * response perturbs). */
export type InputSignal = ("v_ref") | ("p_ref") | ("vs");

/** One island with a unit in service at the end of the run. */
export type IslandScreen = { "buses": number; "final_hz"?: number | null; "inertia_mws": number; "machines": number; "runaway": boolean; };

/** How static loads respond to voltage during the simulation.
 * 
 * The default keeps every load's own law: the constant-power part, the ZIP
 * constant-current and constant-impedance parts and exponential loads, the
 * same decomposition the AC power flow solves (each part `P0 (V / V0)^γ`
 * with `P0` its power at the power flow voltage `V0`), so the loads draw
 * exactly the power flow demand at `t = 0`. The other variants are a global
 * override: the whole bus demand at `V0`, whatever its parts, follows one
 * law (PSS/E `CONL` with 100 % of P and Q in one class). */
export type LoadModel = ("constant_current" | "constant_power") | ("power_flow") | ("constant_impedance");

/** How the constant-power and constant-current load parts (and exponential
 * parts with `γ < 2`) behave at low voltage. */
export type LowVoltageLoad = ({ "kind": "break"; "v_break": number; }) | ({ "kind": "exact"; });

/** A machine that slipped a pole: the first time its angle relative to its
 * island's COI passed the criterion. */
export type PoleSlip = { "t": number; "unit": string; };

/** One reason for the verdict: a stable code and a plain sentence. */
export type Reason = { "code": string; "text": string; };

/** What a run records. Without one ([`RmsRequest::record`] `None`) every
 * channel is recorded at every output step. */
export type RecordSpec = { "channels"?: Array<string>; "checkpoint_step"?: number; "frame_step"?: number; "frames"?: Array<string>; };

/** A discrete event at `time` seconds. */
export type RmsEvent = ({ "circuit"?: string; "from": number; "kind": "branch_trip"; "time": number; "to": number; }) | ({ "circuit"?: string; "from": number; "kind": "branch_close"; "time": number; "to": number; }) | ({ "bus": number; "kind": "bus_fault"; "r"?: number; "time": number; "x"?: number; }) | ({ "bus": number; "kind": "clear_fault"; "time": number; }) | ({ "bus": number; "kind": "load_change"; "scale": number; "time": number; }) | ({ "bus": number; "kind": "set_reference"; "machine_id"?: string; "signal": InputSignal; "time": number; "value": number; }) | ({ "bus": number; "kind": "generator_trip"; "machine_id"?: string; "time": number; }) | ({ "circuit"?: string; "from": number; "kind": "line_fault"; "location": number; "permanent"?: boolean; "r"?: number; "time": number; "to": number; "x"?: number; }) | ({ "circuit"?: string; "from": number; "kind": "clear_line_fault"; "time": number; "to": number; }) | ({ "at": number; "circuit"?: string; "from": number; "kind": "breaker_open"; "time": number; "to": number; }) | ({ "at": number; "circuit"?: string; "from": number; "kind": "breaker_close"; "time": number; "to": number; });

/** One dynamic generator in the result. */
export type RmsGenerator = { "bus": number; "h"?: number; "initial_states": Array<number>; "machine_id": string; "mbase": number; "models": Array<string>; "p_ref": number; "states": Array<string>; "v_ref": number; };

/** What a case's dynamic data resolves to, for a UI to show before a run. */
export type RmsInspection = { "base_frequency": number; "base_mva": number; "branches": Array<[number, number, string]>; "buses": Array<number>; "dyr_records": number; "eigenvalues": Array<Array<number>>; "generators": Array<RmsGenerator>; "held_units"?: Array<string>; "initial_residual"?: number; "load_buses": Array<number>; "load_model"?: LoadModel; "notes": Array<string>; "options"?: RmsOptions; "plants"?: Array<string>; "reference_unit"?: string | null; "sources": Array<string>; "supported_models": Array<string>; };

/** Choices of one prepared simulation, fixed when it is built. The defaults
 * are tellegen's own; the others exist to compare with another tool or
 * method, and affect only the simulation they are given to. */
export type RmsOptions = { "factorize_by_blocks"?: boolean; "freeze_limits"?: boolean; "keep_factorization"?: boolean; "low_voltage_load"?: LowVoltageLoad; "psid_converter_ramp"?: boolean; "psid_lv_gain"?: boolean; "psid_saturation"?: boolean; "reactive_limits"?: boolean; "refactor_values"?: boolean; "renewable_plants"?: boolean; "threads"?: boolean; };

/** A simulation request. */
export type RmsRequest = { "dt"?: number; "events"?: Array<RmsEvent>; "load_model"?: (LoadModel) | (null); "output_step"?: number; "record"?: (RecordSpec) | (null); "small_signal"?: boolean; "t_end"?: number; };

/** The screening of a run. */
export type RmsScreening = { "angle": AngleScreen; "criteria": ScreeningCriteria; "criteria_name": string; "frequency": FrequencyScreen; "held_units": number; "islands": Array<IslandScreen>; "reasons": Array<Reason>; "reference_unit"?: string | null; "verdict": ScreeningVerdict; "voltage": VoltageScreen; };

/** The screening criteria. Every field has a default (`version` 1); a
 * request may override any of them. */
export type ScreeningCriteria = { "bus_frequency_filter_s"?: number; "bus_frequency_min_pu"?: number; "pole_slip_deg"?: number; "rocof_window_s"?: number; "runaway_pu"?: number; "settle_angle_deg"?: number; "settle_frequency_hz"?: number; "settle_window_s"?: number; "version"?: number; "voltage_recovery_pu"?: number; "voltage_recovery_s"?: number; };

/** The outcome of a screening. */
export type ScreeningVerdict = ("stable") | ("loss_of_synchronism") | ("island_runaway") | ("not_settled");

/** The trend of the rotor angle separation after the last event, read from
 * the peaks of its distance to its final value. Only peaks above
 * [`TREND_FLOOR_DEG`] and above [`TREND_FLOOR_FRACTION`] of the largest
 * distance count: smaller wiggles are numerical, not swings. */
export type SwingTrend = ("decaying") | ("growing") | ("damped_out") | ("sustained") | ("undetermined");

/** Bus voltage figures, pu. */
export type VoltageScreen = { "final_max_pu"?: number | null; "final_min_pu"?: number | null; "last_event_t": number; "min_bus"?: number | null; "min_pu"?: number | null; "min_t": number; "recovered": boolean; "recovered_s"?: number | null; };
