/** Protection relays (IEC 60255 measuring-relay functions) acting on RMS
 * runs: a scheme set by rule with its audit, the standard test cases, the
 * relays at the operating point and a static study of one fault. A run
 * takes a scheme in `RmsRunRequest.protection`; a live session through
 * `BrowserRmsLive.setProtection`.
 *
 * The setting calls run on the engine worker; the test suite runs on an
 * isolated worker of its own, so it never holds up the RMS worker or the
 * interactive studies, and aborting it terminates only that worker. The
 * calls need an engine build with `protection` ({@link protectionSupported}). */
import { engineHost, isolatedEngineHost } from "./host.js";
import type { EngineValue } from "./protocol.js";
import { assertEngineInputLength } from "./input-limit.js";
import type { RmsCaseOptions } from "./rms.js";
import type { RmsEvent } from "./generated/rms-types.js";

export type Direction = "forward" | "reverse" | "non_directional";
export type Curve =
  | "definite"
  | "iec_standard_inverse"
  | "iec_very_inverse"
  | "iec_extremely_inverse"
  | "iec_long_time_inverse"
  | "ieee_moderately_inverse"
  | "ieee_very_inverse"
  | "ieee_extremely_inverse";

export type RelayLocation =
  | { kind: "line"; from: number; to: number; circuit?: string; at: number }
  | { kind: "transformer"; from: number; to: number; circuit?: string }
  | { kind: "busbar"; bus: number }
  | { kind: "generator"; bus: number; machine_id?: string }
  | { kind: "inverter"; bus: number; machine_id?: string };

export interface DistanceZone {
  name: string;
  /** Primary ohms along `angle` (default the line angle). */
  reach: number;
  angle?: number | null;
  resistive_reach?: number | null;
  delay: number;
  direction?: Direction;
  autoreclose?: boolean;
}

export interface Overcurrent {
  label?: string | null;
  /** Multiples of the rated current. */
  pickup: number;
  curve?: Curve;
  tms?: number;
  delay?: number;
  direction?: Direction;
}

export interface VoltageStage {
  label?: string | null;
  pickup: number;
  delay: number;
}

export interface FrequencyStage {
  label?: string | null;
  pickup: number;
  delay: number;
  min_voltage?: number;
}

export type ProtectionFunction =
  | ({ type: "overcurrent" } & Overcurrent)
  | ({ type: "earth_fault" } & Overcurrent)
  | {
      type: "distance";
      characteristic?: "mho" | "quadrilateral";
      zones: DistanceZone[];
      line_angle: number;
      k0?: [number, number];
      min_current?: number;
      power_swing_blocking?: { outer?: number; delta_t?: number; max_block?: number } | null;
      teleprotection?: { scheme: "pott" | "putt"; zone?: string; channel_time?: number } | null;
      load_encroachment?: { r: number; angle: number } | null;
    }
  | {
      type: "differential";
      label?: string | null;
      pickup?: number;
      slope1?: number;
      slope2?: number;
      knee?: number;
      unrestrained?: number;
      harmonic2?: number | null;
      channel_time?: number;
      delay?: number;
    }
  | ({ type: "undervoltage" } & VoltageStage)
  | ({ type: "overvoltage" } & VoltageStage)
  | ({ type: "neutral_overvoltage" } & VoltageStage)
  | ({ type: "overexcitation" } & VoltageStage)
  | ({ type: "underfrequency" } & FrequencyStage)
  | ({ type: "overfrequency" } & FrequencyStage)
  | {
      type: "rocof";
      label?: string | null;
      pickup: number;
      window?: number;
      delay?: number;
      min_voltage?: number;
    }
  | { type: "reverse_power"; label?: string | null; pickup: number; delay: number }
  | {
      type: "loss_of_field";
      label?: string | null;
      offset: number;
      diameter1: number;
      delay1: number;
      diameter2: number;
      delay2: number;
    }
  | {
      type: "out_of_step";
      label?: string | null;
      blinder: number;
      forward_reach: number;
      reverse_reach: number;
      min_traverse?: number;
      slips?: number;
    }
  | { type: "negative_sequence"; label?: string | null; continuous: number; k: number }
  | { type: "thermal"; label?: string | null; k?: number; tau: number }
  | {
      type: "fault_ride_through";
      label?: string | null;
      points: [number, number][];
      start?: number;
      reset?: number;
    };

export interface AutoReclose {
  dead_time?: number;
  reclaim?: number;
  shots?: number;
  /** Largest voltage angle across the open breaker a reclose waits for, °. */
  sync_angle?: number;
}

export interface Relay {
  id: string;
  location: RelayLocation;
  functions: ProtectionFunction[];
  breaker_time?: number;
  breaker_failure?: { delay?: number; current?: number } | null;
  autoreclose?: AutoReclose | null;
  enabled?: boolean;
  i_nom?: number | null;
}

export interface BreakerRef {
  from?: number;
  to?: number;
  circuit?: string;
  at: number;
  machine_id?: string | null;
}

/** A series capacitor bank's own protection: its varistor bypasses the
 * bank while the bank current is above the protective level. */
export interface SeriesCapacitor {
  from: number;
  to: number;
  circuit?: string;
  /** Rated current, system pu. */
  rated: number;
  /** Multiples of the rated current. */
  protective_level?: number;
  /** Time below the rating before the bank is reinserted, s. */
  reinsert?: number;
}

export interface ProtectionScheme {
  relays: Relay[];
  stuck_breakers?: BreakerRef[];
  /** RMS measuring time of the relays (default 20 ms). */
  measuring_time?: number;
  series_capacitors?: SeriesCapacitor[];
}

export type OperationKind =
  | "pickup"
  | "reset"
  | "trip"
  | "breaker_open"
  | "breaker_stuck"
  | "breaker_failure"
  | "reclose"
  | "lockout"
  | "blocked"
  | "unblocked"
  | "bypassed"
  | "reinserted"
  | "sync_held";

export interface RelayOperation {
  time: number;
  relay: string;
  function: string;
  kind: OperationKind;
  detail: string;
}

export interface TripRecord {
  relay: string;
  function: string;
  time: number;
}

export interface ProtectionReport {
  operations: RelayOperation[];
  trips: TripRecord[];
  notes: string[];
}

export interface SettingOptions {
  characteristic?: "mho" | "quadrilateral";
  lines?: boolean;
  line_differential?: boolean;
  teleprotection?: boolean;
  autoreclose?: boolean;
  transformers?: boolean;
  busbars?: boolean;
  generators?: boolean;
  inverters?: boolean;
  breaker_time?: number;
}

export type Severity = "ok" | "info" | "warning" | "error";

export interface Finding {
  severity: Severity;
  relay: string;
  check: string;
  message: string;
}

/** A stretch of a branch inside a distance zone (fractions from `from`). */
export interface ZoneSpan {
  relay: string;
  zone: string;
  from: number;
  to: number;
  circuit: string;
  start: number;
  end: number;
  delay: number;
}

export interface AutoProtection {
  scheme: ProtectionScheme;
  notes: string[];
  audit: Finding[];
  zones: ZoneSpan[];
}

export type Expectation =
  | { kind: "trip"; relay: string; function?: string | null; min: number; max: number }
  | { kind: "no_trip"; relay: string; function?: string | null }
  | { kind: "only_trips"; relays: string[]; until?: number | null }
  | { kind: "reclose"; relay: string }
  | { kind: "lockout"; relay: string }
  | { kind: "breaker_failure"; relay: string };

/** A test expectation in words, as the engine words it in a test's results. */
export function describeExpectation(x: Expectation): string {
  const by = (f?: string | null) => (f ? ` by ${f}` : "");
  switch (x.kind) {
    case "trip":
      return `${x.relay} trips${by(x.function)} within ${Math.round(x.min * 1e3)}–${Math.round(x.max * 1e3)} ms`;
    case "no_trip":
      return `${x.relay} does not trip${by(x.function)}`;
    case "only_trips":
      return x.until != null
        ? `only ${x.relays.join(", ")} trip within ${Math.round(x.until * 1e3)} ms`
        : `only ${x.relays.join(", ")} trip`;
    case "reclose":
      return `${x.relay} recloses successfully`;
    case "lockout":
      return `${x.relay} locks out`;
    case "breaker_failure":
      return `${x.relay} breaker failure trips the adjacent breakers`;
  }
}

export interface ProtectionTest {
  name: string;
  category: string;
  description?: string;
  fault_time: number;
  t_end: number;
  events: RmsEvent[];
  disable?: string[];
  disable_functions?: string[];
  stuck_breakers?: BreakerRef[];
  expect: Expectation[];
}

export interface TestOutcome {
  name: string;
  category: string;
  passed: boolean;
  checks: { expectation: string; passed: boolean; observed: string }[];
  trips: TripRecord[];
  error?: string | null;
  seconds: number;
}

export interface SuiteReport {
  audit: Finding[];
  tests: TestOutcome[];
  passed: number;
  failed: number;
}

export interface RelaySeen {
  relay: string;
  current_ka: number;
  voltage_pu: number;
  impedance: [number, number] | null;
}

export type StaticFault =
  | { kind: "bus"; bus: number; r?: number; x?: number }
  | {
      kind: "line";
      from: number;
      to: number;
      circuit?: string;
      location: number;
      r?: number;
      x?: number;
    };

export interface StaticStudy {
  fault_current_ka: number;
  seen: RelaySeen[];
  operations: RelayOperation[];
  trips: TripRecord[];
  steps: { time: number; opened: string[]; fault_current_ka: number }[];
  cleared_at: number | null;
  notes: string[];
}

function text(value: EngineValue): string {
  if (typeof value !== "string") throw new Error("engine returned no text payload");
  return value;
}

function check(moduleJson: string, dyr: string) {
  assertEngineInputLength(moduleJson.length);
  assertEngineInputLength(dyr.length);
}

/** Whether this engine build carries the protection relays. */
export async function protectionSupported(): Promise<boolean> {
  try {
    return (await engineHost().call({ op: "protection_supported" })) === "true";
  } catch {
    return false;
  }
}

/** A scheme by the setting rules, with its audit and zone spans. */
export async function autoProtection(
  moduleJson: string,
  dyr: string,
  options: SettingOptions = {},
  caseOptions: RmsCaseOptions = {},
): Promise<AutoProtection> {
  check(moduleJson, dyr);
  const value = await engineHost().call({
    op: "protection_auto",
    module_json: moduleJson,
    dyr,
    options: JSON.stringify(options),
    case: JSON.stringify(caseOptions),
  });
  return JSON.parse(text(value));
}

/** The audit and zone spans of a scheme. */
export async function auditProtection(
  moduleJson: string,
  dyr: string,
  scheme: ProtectionScheme,
  caseOptions: RmsCaseOptions = {},
): Promise<{ audit: Finding[]; zones: ZoneSpan[] }> {
  check(moduleJson, dyr);
  const value = await engineHost().call({
    op: "protection_audit",
    module_json: moduleJson,
    dyr,
    scheme: JSON.stringify(scheme),
    case: JSON.stringify(caseOptions),
  });
  return JSON.parse(text(value));
}

/** The standard test cases of a scheme. */
export async function protectionTests(
  moduleJson: string,
  dyr: string,
  scheme: ProtectionScheme,
  options: { max_lines?: number; fault_time?: number } = {},
  caseOptions: RmsCaseOptions = {},
): Promise<ProtectionTest[]> {
  check(moduleJson, dyr);
  const value = await engineHost().call({
    op: "protection_tests",
    module_json: moduleJson,
    dyr,
    scheme: JSON.stringify(scheme),
    options: JSON.stringify(options),
    case: JSON.stringify(caseOptions),
  });
  return JSON.parse(text(value));
}

/** Run test cases in RMS on an isolated worker (all standard ones when
 * `tests` is omitted); aborting `signal` stops it. */
export async function runProtectionTests(
  moduleJson: string,
  dyr: string,
  scheme: ProtectionScheme,
  tests?: ProtectionTest[],
  signal?: AbortSignal,
  caseOptions: RmsCaseOptions = {},
): Promise<SuiteReport> {
  check(moduleJson, dyr);
  signal?.throwIfAborted();
  const host = isolatedEngineHost();
  const abort = () => host.cancel?.(new Error("Protection tests cancelled"));
  signal?.addEventListener("abort", abort, { once: true });
  try {
    const value = await host.call({
      op: "protection_suite",
      module_json: moduleJson,
      dyr,
      scheme: JSON.stringify(scheme),
      tests: tests ? JSON.stringify(tests) : "",
      options: "",
      case: JSON.stringify(caseOptions),
    });
    signal?.throwIfAborted();
    return JSON.parse(text(value));
  } finally {
    signal?.removeEventListener("abort", abort);
    host.cancel?.(new Error("Protection tests finished"));
  }
}

/** The relays at the load flow operating point. */
export async function protectionAtOperatingPoint(
  moduleJson: string,
  dyr: string,
  scheme: ProtectionScheme,
  caseOptions: RmsCaseOptions = {},
): Promise<{ pickups: RelayOperation[]; seen: RelaySeen[] }> {
  check(moduleJson, dyr);
  return JSON.parse(
    text(
      await engineHost().call({
        op: "protection_operating",
        module_json: moduleJson,
        dyr,
        scheme: JSON.stringify(scheme),
        case: JSON.stringify(caseOptions),
      }),
    ),
  );
}

/** A static protection study of one fault: the relays on the short
 * circuit, breakers opening in sequence, until the fault is isolated. */
export async function staticProtectionStudy(
  moduleJson: string,
  dyr: string,
  scheme: ProtectionScheme,
  fault: StaticFault,
  tMax = 3,
  caseOptions: RmsCaseOptions = {},
): Promise<StaticStudy> {
  check(moduleJson, dyr);
  return JSON.parse(
    text(
      await engineHost().call({
        op: "protection_static",
        module_json: moduleJson,
        dyr,
        scheme: JSON.stringify(scheme),
        fault: JSON.stringify(fault),
        t_max: tMax,
        case: JSON.stringify(caseOptions),
      }),
    ),
  );
}

/** The scheme as a protection test runs it (relays and functions switched
 * off, stuck breakers), as the engine prepares it for the test. */
export function testScheme(scheme: ProtectionScheme, test: ProtectionTest): ProtectionScheme {
  const label = (f: ProtectionFunction): string => {
    if ("label" in f && f.label) return f.label;
    switch (f.type) {
      case "distance":
        return "21";
      case "differential":
        return "87";
      default:
        return f.type;
    }
  };
  const off = test.disable_functions ?? [];
  const tele = off.some((t) => t === "POTT" || t === "PUTT");
  return {
    ...scheme,
    stuck_breakers: [...(scheme.stuck_breakers ?? []), ...(test.stuck_breakers ?? [])],
    relays: scheme.relays.map((r) => ({
      ...r,
      enabled: (r.enabled ?? true) && !(test.disable ?? []).includes(r.id),
      functions: r.functions
        .filter((f) => !off.some((t) => t !== "POTT" && t !== "PUTT" && label(f).startsWith(t)))
        .map((f) => (f.type === "distance" && tele ? { ...f, teleprotection: null } : f)),
    })),
  };
}
