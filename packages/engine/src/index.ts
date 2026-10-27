/** The browser engine API. Requests run in a dedicated Web Worker when the
 * browser allows one — a solve never blocks the page — and on the calling
 * thread otherwise (see host.ts). Nothing downloads until the first engine
 * call; dropped files are parsed locally and never leave the machine. */
import {
  callTimed,
  engineHost,
  isolatedEngineHost,
  synthEngineHost,
  type EngineHost,
} from "./host.js";
import type { EngineRequest, EngineValue } from "./protocol.js";
export {
  probeAcOpfWorker,
  solveAcOpfModule,
  type AcOpfWorkerFactory,
} from "./acopf.js";
import { isPermanentWasmLoadFailure } from "./errors.js";
import {
  assertEngineInputBytes,
  assertEngineInputLength,
} from "./input-limit.js";
import type {
  BranchRatingDeltas,
  BrowserFormulation,
  DemandDeltas,
  NetworkBranch,
  NetworkBus,
  ProblemCaps,
  SensitivityColumn,
  Solution,
  SolveRequest,
  SolveResponse,
  SolveIteration,
} from "./generated/api-types.js";
import type {
  ErrorPayload as SynthErrorPayload,
  GenerationReport,
  Stage as SynthStage,
  StepOutcome,
  SynthRequest,
} from "./generated/synth-types.js";
export {
  API_SOURCE_SHA256,
  ENGINE_VERSION,
  FORMULATION_IDS,
  SOLVE_STATUSES,
} from "./generated/api-types.js";
export { MAX_ENGINE_INPUT_BYTES } from "./input-limit.js";

export type {
  BranchRatingDeltas,
  BrowserFormulation,
  CaseSummary,
  DemandDeltas,
  Network,
  NetworkBranch,
  NetworkBus,
  ProblemCaps,
  SensRequest,
  SensitivityColumn,
  SensitivityMatrix,
  Solution,
  SolveIteration,
  SolveRequest,
  SolveResponse,
} from "./generated/api-types.js";

/** One structured PowerIO finding. */
export interface PowerIoDiagnostic {
  code: string;
  severity: "note" | "remark" | "warning" | "error";
  message: string;
  id?: string;
  target?: string;
  spans?: { source: string; byte_start: number; byte_end: number }[];
  related?: string[];
  details?: Record<string, unknown>;
  suggested_action?: string;
}

/** Compact display spelling for a structured PowerIO finding. */
export function formatPowerIoDiagnostic(diagnostic: PowerIoDiagnostic): string {
  return `${diagnostic.code}: ${diagnostic.message}`;
}

export interface CaseFileSummary {
  name: string;
  base_mva: number;
  /** Canonical typed PowerIO bus rows. */
  n_bus: number;
  /** Canonical typed PowerIO branch rows. */
  n_branch: number;
  /** Rendered buses after analysis lowering; absent on older engine builds. */
  n_analysis_bus?: number;
  /** Rendered branches after analysis lowering; absent on older engine builds. */
  n_analysis_branch?: number;
  n_gen: number;
  load_mw: number;
  gen_mw: number;
  has_coords: boolean;
  /** Coordinate provenance: `file` for coordinates the case (or a geographic
   * sidecar) carried, `synthetic`/`manual` for a stamped layout (a restored
   * package echoes the kind it was saved with), `synthetic_pending` when the
   * case awaits placement. */
  coords_kind: "file" | "synthetic_pending" | "synthetic" | "manual";
  /** How `synthetic` coordinates were made, when the module records it:
   * `generated` for a grid a generator sited on real geography (the module's
   * history holds `tellegen-synth.generate`). Absent for a stamped topology
   * layout. */
  coords_method?: "generated";
  /** Findings produced while PowerIO parsed the retained module. */
  diagnostics: PowerIoDiagnostic[];
  /** Tellegen display notes, such as omitted unlocated analysis rows. */
  warnings: string[];
}

export interface TopologyBus {
  id: number;
  /** PowerIO row uid when the source carries one. */
  uid: string | null;
  /** False for a display-only row synthesized by analysis lowering. */
  editable?: boolean;
  demand_mw: number;
  gen_mw: number;
}

export interface TopologyBranch {
  id: number;
  /** PowerIO row uid when the source carries one. */
  uid: string | null;
  /** False for a display-only row synthesized by analysis lowering. */
  editable?: boolean;
  from: number;
  to: number;
  rate_mw: number;
  status: number;
}

export interface Topology {
  buses: TopologyBus[];
  branches: TopologyBranch[];
}

/** One parse per dropped file: summary stats, plus map geometry when the
 * file carries coordinates and topology for synthetic placement otherwise. */
export interface IngestedCase extends CaseFileSummary {
  /** Calculation declared by a typed problem instance. */
  formulation?: Formulation;
  /** Generation 2 PowerIO IR used for display edits and every solver Study. */
  module_json: string;
  topology: Topology;
  view: {
    coordinate_space?: "geographic" | "diagram";
    buses: NetworkBus[];
    branches: NetworkBranch[];
  } | null;
}

/** Edge family in the collapsed distribution graph. */
export type DistEdgeKind = "line" | "switch" | "transformer";

/** Terminal attachment family. */
export type DistAttachmentKind =
  "load" | "generator" | "ibr" | "shunt" | "source";

/** One element connected to a bus terminal. */
export interface DistGraphAttachment {
  kind: DistAttachmentKind;
  id: string;
}

/** One bus node in the collapsed distribution graph. `xy` is present when the
 * source carried a position (`[x, y]`, longitude/latitude when the case is
 * geographic); `terminal_attachments` maps each terminal name to the elements
 * on it. */
export interface DistGraphBus {
  id: string;
  terminals: string[];
  grounded: string[];
  /** Declared or conservative conventional neutral terminal, when present. */
  neutral_terminal?: string | null;
  xy?: [number, number];
  load_kw: number;
  gen_kw: number;
  has_source: boolean;
  terminal_attachments?: Record<string, DistGraphAttachment[]>;
}

/** One edge in the collapsed distribution graph. `conductors` pairs the
 * `[from_terminal, to_terminal]` names in conductor order; `closed` is false
 * for an open switch. */
export interface DistGraphEdge {
  kind: DistEdgeKind;
  id: string;
  from: string;
  to: string;
  conductors: [string, string][];
  closed: boolean;
  n_phases: number;
}

/** The render-ready bus/terminal projection of a distribution network. */
export interface DistGraph {
  buses: DistGraphBus[];
  edges: DistGraphEdge[];
}

/** Multiconductor case data, terminal graph, and retained PowerIO IR. */
export interface IngestedDistCase {
  module_json?: string;
  geo_layer?: string;
  mc_pf_enabled?: boolean;
  mc_pf_unavailable_reason?: string | null;
  name: string | null;
  model: "multiconductor";
  n_bus: number;
  n_edge: number;
  n_line: number;
  n_switch: number;
  n_transformer: number;
  n_load: number;
  n_generator: number;
  n_ibr: number;
  n_source: number;
  n_shunt: number;
  /** Capacitor banks read as their own type, which only the BMOPF reader does.
   * A `.dss` or PMD capacitor counts in `n_shunt` instead. Optional because an
   * older compatible engine payload can omit it. */
  n_capacitor?: number;
  load_kw: number;
  gen_kw: number;
  base_frequency: number;
  has_coords: boolean;
  placed_buses: number;
  coords_space: "geographic" | "projected" | "diagram" | "unknown" | "none";
  coords_kind: "geographic" | "planar" | "synthetic";
  diagnostics: PowerIoDiagnostic[];
  graph: DistGraph;
  /** Explicit capability from the reader; absent means inspection only. */
  mc_pf_supported?: boolean;
  /** Reader explanation for an inspection-only distribution payload. */
  mc_pf_reason?: string;
}

/** A materialized case written to a target format: the serialized case text,
 * this output operation's diagnostics, and the format token and file extension. */
export interface ExportedCase {
  text: string;
  diagnostics: PowerIoDiagnostic[];
  format: string;
  extension: string;
}

/** One weighted LMP objective term, addressed by its PowerIO bus id. */
export interface CapacityPlanBusWeightJson {
  bus: number;
  weight: number;
}

export type ImplicitObjectiveJson = {
  kind: "weighted_lmp";
  weights: CapacityPlanBusWeightJson[];
};

/** Capacity increases considered by the bounded planning search. */
export interface CapacityPlanSpecJson {
  objective: ImplicitObjectiveJson;
  candidates: string[];
  max_increase_per_branch_mw: number;
  budget_mw: number;
  increment_mw: number;
  max_changed_lines: number;
  exact_solve_budget: number;
}

export interface CapacityPlanRatingChangeJson {
  branch: string;
  delta_mw: number;
}

/** One recorded search step: the gradient direction, the trial, the first
 * order prediction, the exact outcome, and whether the step was kept. */
export interface CapacityPlanIterationJson {
  gradient: Array<{ branch: string; value: number }>;
  delta_mw: CapacityPlanRatingChangeJson[];
  predicted_phi_delta: number;
  exact_phi_delta: number | null;
  first_order_error: number | null;
  first_order_error_rel: number | null;
  accepted: boolean;
  reason: string;
}

export interface CapacityPlanResultSummaryJson {
  phi: number;
  declared_objective: number;
  /** One-based position in this planning operation's exact solve budget. */
  exact_solve: number;
}

/** The outcome of one bounded planning search and its unapplied
 * proposal. */
export interface CapacityPlanOutcomeJson {
  baseline: CapacityPlanResultSummaryJson;
  exact_verified_result: CapacityPlanResultSummaryJson;
  baseline_phi: number;
  final_phi: number;
  proposal: CapacityPlanRatingChangeJson[];
  spent_budget_mw: number;
  exact_solves: number;
  iterations: CapacityPlanIterationJson[];
}

export async function preloadEngine(): Promise<void> {
  await engineHost().call({ op: "preload" });
}

export type JsonDropKind =
  "module" | "transmission" | "distribution" | "ambiguous" | "unknown";

export interface JsonDropClassification {
  kind: JsonDropKind;
  format: string | null;
}

/** A JSON drop classified and ingested in one engine request. Unknown and
 * ambiguous documents carry no payload; every recognized family carries the
 * same typed payload returned by its dedicated ingest API. */
export type IngestedJsonDrop =
  | {
      /** A generation 2 `pio-ir` document; the payload family follows the
       * network family carried by the module value. */
      kind: "module";
      format: null;
      payload: IngestedCase | IngestedDistCase;
    }
  | { kind: "transmission"; format: string; payload: IngestedCase }
  | { kind: "distribution"; format: string; payload: IngestedDistCase }
  | { kind: "ambiguous" | "unknown"; format: null; payload: null };

/** Classify JSON bytes through powerio's Rust routing table. A stored module
 * passes the strict module reader so the payload family is authoritative. */
export async function classifyJson(
  bytes: Uint8Array,
): Promise<JsonDropClassification> {
  assertEngineInputBytes(bytes);
  return JSON.parse(
    expectText(await engineHost().call({ op: "classify_json", bytes })),
  );
}

/** Classify and ingest one dropped JSON document in the same engine request.
 * This avoids parsing a recognized package once to classify it and again to
 * consume it. */
export async function ingestJsonDrop(
  bytes: Uint8Array,
): Promise<IngestedJsonDrop> {
  assertEngineInputBytes(bytes);
  return JSON.parse(
    expectText(await engineHost().call({ op: "ingest_json_drop", bytes })),
  );
}

/** Asserts a request that must carry a text payload actually did. */
function expectText(value: EngineValue): string {
  if (typeof value !== "string") throw new Error("engine returned no text payload");
  return value;
}

/** Asserts a request that must carry a numeric array actually did. */
function expectFloat64(value: EngineValue): Float64Array {
  if (!(value instanceof Float64Array))
    throw new Error("engine returned no numeric array");
  return value;
}

/** powerio format token from a file name; null for non-case files. */
export function formatOf(name: string): string | null {
  const ext = name.split(".").pop()?.toLowerCase();
  switch (ext) {
    case "m":
    case "raw":
    case "aux":
    case "pwb":
      return ext;
    case "epc":
      return "pslf";
    default:
      return null;
  }
}

/** Parse a balanced case file for viewing. Takes the upload's bytes rather than
 * decoded text: powerio refuses a text format whose bytes are not UTF-8, where
 * `File.text()` would have replaced each offending byte with U+FFFD. */
export async function ingestCase(
  bytes: Uint8Array,
  format: string,
): Promise<IngestedCase> {
  assertEngineInputBytes(bytes);
  return JSON.parse(
    expectText(await engineHost().call({ op: "ingest_case", bytes, format })),
  );
}

/** Parse a multiconductor distribution case for viewing. `format` is a
 * distribution reader token (`dss`, `bmopf`, `pmd`) or `pio` for a `.pio.json`
 * package carrying a multiconductor payload. Rejects on malformed input or a
 * balanced package (which the study-restore path handles). */
export async function ingestDistCase(
  text: string,
  format: string,
): Promise<IngestedDistCase> {
  assertEngineInputLength(text.length);
  return JSON.parse(
    expectText(
      await engineHost().call({ op: "ingest_dist_case", text, format }),
    ),
  );
}

/** Byte entry point for a dropped distribution case or package. */
export async function ingestDistCaseBytes(
  bytes: Uint8Array,
  format: string,
): Promise<IngestedDistCase> {
  assertEngineInputBytes(bytes);
  return JSON.parse(
    expectText(
      await engineHost().call({ op: "ingest_dist_case_bytes", bytes, format }),
    ),
  );
}

/** A PowerWorld drawing decoded in its declared diagram coordinate system. */
export type DisplayPreview = ParsedGeoLayer;

/** True for binary display files (PowerWorld .pwd), read via parseDisplay.
 * Kept separate from formatOf: a .pwd is display data, not a case format. */
export function isDisplayFile(name: string): boolean {
  return name.split(".").pop()?.toLowerCase() === "pwd";
}

export async function parseDisplay(bytes: Uint8Array): Promise<DisplayPreview> {
  assertEngineInputBytes(bytes);
  return JSON.parse(
    expectText(
      await engineHost().call({ op: "parse_display", bytes, format: "pwd" }),
    ),
  );
}

/** A parsed geographic sidecar: the layer as its canonical `.geo.json`
 * document (hand it to {@link applyGeo} or `BrowserStudy.applyGeoLayer`),
 * the reader's structured diagnostics, and the feature counts. */
export interface ParsedGeoLayer {
  layer: string;
  diagnostics: PowerIoDiagnostic[];
  n_points: number;
  n_routes: number;
}

/** Result of applying a geographic layer onto a case network. */
export interface GeoApplyReport {
  matched_buses: number;
  matched_branches: number;
  unmatched_features: number;
  notes: string[];
}

/** The refreshed ingest payload after a geo apply: `module_json`, the view,
 * and the summary all reflect the applied coordinates, and `report` carries
 * the matched/unmatched counts for the warnings panel. */
export interface AppliedMcGeoCase extends IngestedDistCase {
  report: GeoApplyReport;
}

export interface AppliedGeoCase extends IngestedCase {
  report: GeoApplyReport;
}

/** The refreshed ingest payload after stamping a layout, plus `layer`, the
 * stamped layout as a canonical `.geo.json` document (ready to download or
 * to sync onto a live study). */
export interface StampedLayoutCase extends IngestedCase {
  layer: string;
}

/** Parse a geographic sidecar (buscoords CSV, aliased CSV/JSON records,
 * GeoJSON) from raw dropped bytes through powerio's tolerant reader. `hint`
 * is the file name (picks CSV against JSON; "" sniffs). Rejects input with
 * no usable coordinates. */
export async function parseGeo(
  bytes: Uint8Array,
  hint: string,
): Promise<ParsedGeoLayer> {
  assertEngineInputBytes(bytes);
  return JSON.parse(
    expectText(await engineHost().call({ op: "parse_geo", bytes, hint })),
  );
}

/** Apply a parsed layer onto a case network (matching by uid, external id,
 * name, and the unordered branch endpoint pair). Rejects when nothing
 * matched. */
export async function applyGeo(
  moduleJson: string,
  layer: string,
): Promise<AppliedGeoCase> {
  assertEngineInputLength(moduleJson.length);
  return JSON.parse(
    expectText(
      await engineHost().call({
        op: "apply_geo",
        module_json: moduleJson,
        layer,
      }),
    ),
  );
}

/** Apply geographic or drawing coordinates to a multiconductor case. */
export async function applyMcGeo(
  moduleJson: string,
  layer: string,
): Promise<AppliedMcGeoCase> {
  assertEngineInputLength(moduleJson.length);
  const result = JSON.parse(
    expectText(
      await engineHost().call({
        op: "apply_geo",
        module_json: moduleJson,
        layer,
      }),
    ),
  );
  if (result.model !== "multiconductor")
    throw new Error("Expected a multiconductor case");
  return result;
}

/** Stamp a computed layout (bus id => [lon, lat]) onto a case network with
 * its provenance, so saves and exports carry it. */
export async function applyLayout(
  moduleJson: string,
  coords: Record<number, [number, number]>,
  kind: "synthetic" | "manual",
): Promise<StampedLayoutCase> {
  assertEngineInputLength(moduleJson.length);
  return JSON.parse(
    expectText(
      await engineHost().call({
        op: "apply_layout",
        module_json: moduleJson,
        coords: JSON.stringify(coords),
        kind,
      }),
    ),
  );
}

/** A case's coordinates as a canonical `.geo.json` document. Rejects when the
 * case carries none. */
export async function extractGeo(moduleJson: string): Promise<string> {
  assertEngineInputLength(moduleJson.length);
  return expectText(
    await engineHost().call({ op: "extract_geo", module_json: moduleJson }),
  );
}

/** Fill case coordinates from a PowerWorld `.pwd` sibling: substation symbols
 * project to approximate longitude/latitude and join onto buses through the
 * `SubNum` extras key. Rejects when no bus joined. */
export async function applyDisplayGeo(
  moduleJson: string,
  bytes: Uint8Array,
): Promise<AppliedGeoCase> {
  assertEngineInputLength(moduleJson.length);
  assertEngineInputBytes(bytes);
  return JSON.parse(
    expectText(
      await engineHost().call({
        op: "apply_display_geo",
        module_json: moduleJson,
        bytes,
      }),
    ),
  );
}

export async function capabilities(): Promise<ProblemCaps[]> {
  return JSON.parse(
    expectText(await engineHost().call({ op: "capabilities" })),
  );
}

export async function solveModule(
  moduleJson: string,
  request: SolveRequest = {},
): Promise<SolveResponse> {
  return JSON.parse(
    expectText(
      await engineHost().call({
        op: "solve_module",
        module_json: moduleJson,
        request: JSON.stringify(request),
      }),
    ),
  );
}

/** Portable terminal-aware result returned by the multiconductor fixed-point
 * BMOPF entry point. Complex quantities use `{re, im}` so the JSON remains
 * lossless across the wasm boundary. */
export interface McPfOptions {
  tolerance?: number;
  max_iterations?: number;
  damping?: number;
  zero_voltage_tolerance?: number;
  /** Absolute physical KCL floor in amperes; default 1e-6. */
  absolute_kcl_tolerance?: number;
  /** Relative KCL tolerance against prepared incident current. */
  relative_kcl_tolerance?: number;
  /** Apply the OpenDSS-style bounded-voltage load envelope; default true. */
  voltage_envelope?: boolean;
  /** Nominal-impedance breakpoint in per unit; default 0.5. */
  v_low_pu?: number;
  /** Lower edge of the normal load-model range; default 0.85. */
  v_min_pu?: number;
  /** Upper edge of the normal load-model range; default 1.15. */
  v_max_pu?: number;
}

export interface McComplex {
  re: number;
  im: number;
}

export interface McElementPort {
  element: string;
  kind: string;
  branch: number;
  bus: string;
  terminal: string;
  current_into_element: McComplex;
  power_into_element: McComplex;
}

export interface McSourceReaction {
  source: string;
  terminal: string;
  current_into_network: McComplex;
  power_into_network: McComplex;
}

export interface McPfResult {
  converged: boolean;
  voltage_valid: boolean;
  min_voltage_pu: number | null;
  max_voltage_pu: number | null;
  voltage_violations: Array<{
    load: string;
    branch: number;
    bus: string;
    voltage: number;
    nominal_voltage: number;
    voltage_pu: number;
    bound: "minimum" | "maximum";
  }>;
  iterations: number;
  factorization_count: number;
  matrix_dimension: number;
  matrix_nonzeros: number;
  voltage_change: number;
  physical_kcl_residual: number;
  scaled_kcl_residual: number;
  terminals: McTerminalResult[];
  element_ports: McElementPort[];
  source_reactions: McSourceReaction[];
}

export interface McTerminalResult {
  bus: string;
  terminal: string;
  voltage: McComplex;
  /** Current injected into the network at this terminal, amperes. */
  current_into_network: McComplex;
  power_into_network: McComplex;
}

/** The constant-size view of one converged operating point. Scalar fields
 * match {@link McPfResult}; the aggregates are what a results summary shows.
 * Interactive edits return this instead of the full result. */
export interface McPfSummary {
  converged: boolean;
  voltage_valid: boolean;
  min_voltage_pu: number | null;
  max_voltage_pu: number | null;
  voltage_violation_count: number;
  iterations: number;
  factorization_count: number;
  matrix_dimension: number;
  matrix_nonzeros: number;
  voltage_change: number;
  physical_kcl_residual: number;
  scaled_kcl_residual: number;
  terminal_count: number;
  element_port_count: number;
  /** Sum of the source reactions' `V * conj(I)`, VA. */
  source_power_into_network: McComplex;
  /** Sum of `V * conj(I)` over passive equipment ports, VA. */
  passive_loss: McComplex;
  /** Solves performed by the producing session (0 for a stored result). */
  solve_count: number;
}

/** Where one solve spent its time, in milliseconds. */
export interface McPfProfile {
  timed: boolean;
  parse_ms: number;
  prepare_ms: number;
  factor_ms: number;
  load_evaluation_ms: number;
  kcl_and_matvec_ms: number;
  linear_solve_ms: number;
  summary_ms: number;
  total_ms: number;
  iterations: number;
  linear_solves: number;
}

/** A bounded request for one bus's terminals and a page of equipment ports. */
export interface McPfDetailQuery {
  bus?: string | null;
  element?: string | null;
  port_offset?: number;
  /** At most 500; 0 or omitted selects 20. */
  port_limit?: number;
}

export interface McPfDetail {
  /** The session solve this page describes; compare with the summary. */
  solve_count: number;
  terminals: McTerminalResult[];
  element_ports: McElementPort[];
  element_port_total: number;
}

/** Engine heap accounting: live and peak allocator bytes (peak since the last
 * reset) and the WASM linear memory size, which only grows. */
export interface EngineMemoryStats {
  heap_live_bytes: number;
  heap_peak_bytes: number;
  linear_memory_bytes: number;
}

/** Latency breakdown of the most recent session call, milliseconds. */
export interface McPfCallTiming {
  /** Time inside the engine thread: solve plus its own serialization. */
  engine_ms: number | null;
  /** Request to response on the calling thread, including transfer. */
  round_trip_ms: number;
  /** JSON parsing on the calling thread. */
  parse_ms: number;
  /** Characters of JSON received. */
  payload_chars: number;
}

/** Reduce a stored full result to the summary an interactive edit returns.
 * Sums run in result order, matching the engine's own aggregation. */
export function summarizeMcPfResult(
  result: McPfResult,
  solveCount = 0,
): McPfSummary {
  const sourcePower = { re: 0, im: 0 };
  for (const reaction of result.source_reactions) {
    sourcePower.re += reaction.power_into_network.re;
    sourcePower.im += reaction.power_into_network.im;
  }
  const passiveLoss = { re: 0, im: 0 };
  for (const port of result.element_ports) {
    if (port.kind === "load" || port.kind === "generator" || port.kind === "ibr")
      continue;
    passiveLoss.re += port.power_into_element.re;
    passiveLoss.im += port.power_into_element.im;
  }
  return {
    converged: result.converged,
    voltage_valid: result.voltage_valid,
    min_voltage_pu: result.min_voltage_pu,
    max_voltage_pu: result.max_voltage_pu,
    voltage_violation_count: result.voltage_violations.length,
    iterations: result.iterations,
    factorization_count: result.factorization_count,
    matrix_dimension: result.matrix_dimension,
    matrix_nonzeros: result.matrix_nonzeros,
    voltage_change: result.voltage_change,
    physical_kcl_residual: result.physical_kcl_residual,
    scaled_kcl_residual: result.scaled_kcl_residual,
    terminal_count: result.terminals.length,
    element_port_count: result.element_ports.length,
    source_power_into_network: sourcePower,
    passive_loss: passiveLoss,
    solve_count: solveCount,
  };
}

/** The detail page {@link BrowserMcPfSession.detail} would return, read from
 * a stored full result (a saved Study, or a solve without a session). */
export function mcPfResultDetail(
  result: McPfResult,
  query: McPfDetailQuery,
  solveCount = 0,
): McPfDetail {
  const limit = query.port_limit || 20;
  const offset = query.port_offset ?? 0;
  const ports = query.element
    ? result.element_ports.filter((port) => port.element === query.element)
    : result.element_ports;
  return {
    solve_count: solveCount,
    terminals: query.bus
      ? result.terminals.filter((terminal) => terminal.bus === query.bus)
      : [],
    element_ports: ports.slice(offset, offset + limit),
    element_port_total: ports.length,
  };
}

/** Self-contained saved Study result for a supported distribution AC PF.
 * The input and solution modules make this replayable; `result` is the rich
 * terminal/element view rendered by the Study panel. */
export interface McStudySnapshot {
  schema: "tellegen-mc-pf-study";
  version: 1;
  id: string;
  title: string;
  formulation: "mc_ac_pf";
  input_module: string;
  solution_module: string;
  options: McPfOptions;
  result: McPfResult;
}

export interface McLoadPowerEdit {
  load: string;
  branch: number;
  p_w: number;
  q_var: number;
}

export interface McLoadBranchState extends McLoadPowerEdit {
  bus: string;
  base_p_w: number;
  base_q_var: number;
}

/** A live fixed-point current-injection solver retained in the shared engine
 * worker. Load edits reuse its sparse LU and previous converged phasors and
 * answer with the constant-size {@link McPfSummary}; terminal and equipment
 * detail is fetched on demand, as bounded pages or numeric arrays. */
export class BrowserMcPfSession {
  #terminalIds: Promise<Array<[string, string]>> | null = null;
  /** Breakdown of the most recent {@link replaceLoadPowers} call. */
  lastTiming: McPfCallTiming | null = null;

  constructor(
    private readonly host: EngineHost,
    private readonly handle: number,
    /** Summary of the operating point solved when the session was created. */
    readonly initialSummary: McPfSummary,
  ) {}

  async summary(): Promise<McPfSummary> {
    return JSON.parse(
      expectText(
        await this.host.call({ op: "mc_pf_session_summary", session: this.handle }),
      ),
    );
  }

  /** The complete terminal and equipment result. Its size grows with the
   * network, so interactive views use {@link detail} instead. */
  async result(): Promise<McPfResult> {
    return JSON.parse(
      expectText(
        await this.host.call({
          op: "mc_pf_session_result",
          session: this.handle,
        }),
      ),
    );
  }

  async loadBranches(): Promise<McLoadBranchState[]> {
    return JSON.parse(
      expectText(
        await this.host.call({
          op: "mc_pf_session_load_branches",
          session: this.handle,
        }),
      ),
    );
  }

  /** Replace the absolute edit set and re-solve warm. Returns the summary
   * and records its latency breakdown in {@link lastTiming}. */
  async replaceLoadPowers(edits: McLoadPowerEdit[]): Promise<McPfSummary> {
    const started = performance.now();
    const answer = await callTimed(this.host, {
      op: "mc_pf_session_replace_load_powers",
      session: this.handle,
      edits: JSON.stringify(edits),
    });
    const received = performance.now();
    const text = expectText(answer.value);
    const summary = JSON.parse(text) as McPfSummary;
    this.lastTiming = {
      engine_ms: answer.engineMs,
      round_trip_ms: received - started,
      parse_ms: performance.now() - received,
      payload_chars: text.length,
    };
    return summary;
  }

  /** One bus's terminals and a page of equipment ports. */
  async detail(query: McPfDetailQuery): Promise<McPfDetail> {
    return JSON.parse(
      expectText(
        await this.host.call({
          op: "mc_pf_session_detail",
          session: this.handle,
          query: JSON.stringify(query),
        }),
      ),
    );
  }

  /** `[bus, terminal]` identities in the order of the terminal arrays. They
   * are fixed for the session, so they are fetched once. */
  terminalIds(): Promise<Array<[string, string]>> {
    this.#terminalIds ??= this.host
      .call({ op: "mc_pf_session_terminal_ids", session: this.handle })
      .then((value) => JSON.parse(expectText(value)));
    return this.#terminalIds;
  }

  /** Terminal voltages as interleaved `[re, im]` volts. */
  async terminalVoltages(): Promise<Float64Array> {
    return expectFloat64(
      await this.host.call({
        op: "mc_pf_session_terminal_voltages",
        session: this.handle,
      }),
    );
  }

  /** Currents injected into the network as interleaved `[re, im]` amperes. */
  async terminalCurrents(): Promise<Float64Array> {
    return expectFloat64(
      await this.host.call({
        op: "mc_pf_session_terminal_currents",
        session: this.handle,
      }),
    );
  }

  /** Engine phase timings of the latest solve, or of the cold start. */
  async profile(cold = false): Promise<McPfProfile> {
    return JSON.parse(
      expectText(
        await this.host.call({
          op: "mc_pf_session_profile",
          session: this.handle,
          cold,
        }),
      ),
    );
  }

  /** Edited networks built for portable output; ordinary edits build none. */
  async materializationCount(): Promise<number> {
    return Number(
      expectText(
        await this.host.call({
          op: "mc_pf_session_materialization_count",
          session: this.handle,
        }),
      ),
    );
  }

  async inputModule(): Promise<string> {
    return expectText(
      await this.host.call({
        op: "mc_pf_session_input_module",
        session: this.handle,
      }),
    );
  }

  async snapshot(
    studyId: string,
    studyTitle: string,
  ): Promise<McStudySnapshot> {
    return JSON.parse(
      expectText(
        await this.host.call({
          op: "mc_pf_session_snapshot",
          session: this.handle,
          study_id: studyId,
          study_title: studyTitle,
        }),
      ),
    );
  }

  free(): void {
    void this.host
      .call({ op: "mc_pf_session_free", session: this.handle })
      .catch(() => {});
  }
}

let mcPfSessionSeq = 0;

/** Parse, prepare, factor, and solve a persistent multiconductor PF session. */
export async function createMcPfSession(
  moduleJson: string,
  options: McPfOptions = {},
  signal?: AbortSignal,
): Promise<BrowserMcPfSession> {
  assertEngineInputLength(moduleJson.length);
  signal?.throwIfAborted();
  const host = engineHost();
  const handle = ++mcPfSessionSeq;
  const summary = JSON.parse(
    expectText(
      await host.call({
        op: "mc_pf_session_new",
        session: handle,
        module_json: moduleJson,
        options: JSON.stringify(options),
      }),
    ),
  ) as McPfSummary;
  const session = new BrowserMcPfSession(host, handle, summary);
  if (signal?.aborted) {
    session.free();
    signal.throwIfAborted();
  }
  return session;
}

/** Live and peak heap of the shared engine, with its linear memory size. */
export async function engineMemoryStats(): Promise<EngineMemoryStats> {
  return JSON.parse(expectText(await engineHost().call({ op: "memory_stats" })));
}

/** Start a new peak-heap window in the shared engine. */
export async function resetEngineMemoryPeak(): Promise<void> {
  await engineHost().call({ op: "reset_peak_memory" });
}

/** Parse and solve a raw BMOPF multiconductor case in the wasm module. */
export async function solveMcBmopf(
  text: string,
  options: McPfOptions = {},
): Promise<McPfResult> {
  assertEngineInputLength(text.length);
  return JSON.parse(
    expectText(
      await engineHost().call({
        op: "solve_mc_bmopf",
        text,
        options: JSON.stringify(options),
      }),
    ),
  );
}

/** Solve a stored PowerIO multiconductor module through the same PF engine. */
export async function solveMcModule(
  moduleJson: string,
  options: McPfOptions = {},
  signal?: AbortSignal,
): Promise<McPfResult> {
  assertEngineInputLength(moduleJson.length);
  signal?.throwIfAborted();
  const host = isolatedEngineHost();
  const abort = () =>
    host.cancel?.(new Error("Multiconductor calculation cancelled"));
  signal?.addEventListener("abort", abort, { once: true });
  try {
    const result = await host.call({
      op: "solve_mc_module",
      module_json: moduleJson,
      options: JSON.stringify(options),
    });
    signal?.throwIfAborted();
    return JSON.parse(expectText(result));
  } finally {
    signal?.removeEventListener("abort", abort);
    host.cancel?.(new Error("Multiconductor calculation finished"));
  }
}

/** Solve a supported multiconductor Study input and return a replayable
 * snapshot containing the typed input, PowerIO solution, and rich result. */
export async function solveMcStudy(
  moduleJson: string,
  studyId: string,
  studyTitle: string,
  options: McPfOptions = {},
  signal?: AbortSignal,
): Promise<McStudySnapshot> {
  assertEngineInputLength(moduleJson.length);
  signal?.throwIfAborted();
  const host = isolatedEngineHost();
  const abort = () =>
    host.cancel?.(new Error("Multiconductor calculation cancelled"));
  signal?.addEventListener("abort", abort, { once: true });
  try {
    const result = await host.call({
      op: "solve_mc_study",
      module_json: moduleJson,
      study_id: studyId,
      study_title: studyTitle,
      options: JSON.stringify(options),
    });
    signal?.throwIfAborted();
    return JSON.parse(expectText(result));
  } finally {
    signal?.removeEventListener("abort", abort);
    host.cancel?.(new Error("Multiconductor calculation finished"));
  }
}

/** Validate and canonicalize a saved multiconductor Study snapshot without
 * running the solver again. */
export async function replayMcStudy(
  snapshotJson: string,
): Promise<McStudySnapshot> {
  assertEngineInputLength(snapshotJson.length);
  return JSON.parse(
    expectText(
      await engineHost().call({
        op: "replay_mc_study",
        snapshot: snapshotJson,
      }),
    ),
  );
}

/** Update both saved modules with matching positions while retaining the solved values. */
export async function applyMcStudyGeo(
  snapshot: McStudySnapshot,
  layer: string,
): Promise<McStudySnapshot> {
  const text = JSON.stringify(snapshot);
  assertEngineInputLength(text.length);
  assertEngineInputLength(layer.length);
  return JSON.parse(
    expectText(
      await engineHost().call({
        op: "apply_mc_study_geo",
        snapshot: text,
        layer,
      }),
    ),
  );
}

export { errorText } from "./errors.js";

/** True when the engine wasm module has failed to load in a way it can never
 * recover from in this browser. Nothing solves client side when this is true;
 * the caller decides whether a server fallback exists. */
export function isPermanentEngineFailure(message: string): boolean {
  return isPermanentWasmLoadFailure(message);
}

/** One in-place network mutation for the Study handle: a demand delta in MW, or a
 * thermal rating delta in MW. The element key is the original numeric id or the
 * powerio row uid string (e.g. "buses:1"). */
type NetworkEdit =
  | { kind: "add_load"; bus: number | string; p_mw: number }
  | { kind: "adjust_branch_rating"; branch: number | string; delta_mw: number };

/** A safely representable all-digit key uses the numeric wire form. Larger IDs
 * remain strings so serde can parse the exact integer without JS rounding. */
function toElementKey(key: string): number | string {
  if (!/^\d+$/.test(key)) return key;
  const numeric = Number(key);
  return Number.isSafeInteger(numeric) ? numeric : key;
}

/** A `NetworkEdit[]` for the wasm Study, dropping zero deltas so an unchanged
 * element is never sent. */
function toEdits(
  deltas: DemandDeltas,
  rates: BranchRatingDeltas,
): NetworkEdit[] {
  const edits: NetworkEdit[] = Object.entries(deltas)
    .filter(([, mw]) => mw !== 0)
    .map(([bus, p_mw]) => ({ kind: "add_load", bus: toElementKey(bus), p_mw }));
  for (const [branch, delta_mw] of Object.entries(rates)) {
    if (delta_mw !== 0) {
      edits.push({
        kind: "adjust_branch_rating",
        branch: toElementKey(branch),
        delta_mw,
      });
    }
  }
  return edits;
}

/** The sensitivity selection target: a bus (the nodal-value/demand column at that bus) or a
 * branch (the nodal-value/rating column at that branch). Ids are the external element
 * ids; `BrowserStudy` translates them to the engine's dense indices. */
export type SensTarget = { bus: number } | { branch: number };

/** The (operand, parameter) cells the UI drives: a study whose sensitivity column is
 * ∂(price, active) / ∂(demand, active) for a bus target, or ∂(price, active) /
 * ∂(line limit) for a branch target. The *formulation* is no longer fixed here — it is
 * a parameter threaded from the UI's selector through `createStudy` (every formulation the
 * default wasm Study carries returns nodal marginal values, so these columns apply to all of them).
 * Experimental AC OPF uses the separate one-shot WASI path and requests no sensitivities. The
 * operand/parameters stay centralized so `createStudy` and the Study's `commit`/`preview`
 * requests read one source. */
const STUDY_CAPABILITY = {
  /** The watched operand: locational marginal price (active power). */
  operand: { Price: "Active" },
  /** The varied bus parameter: bus demand (active power). */
  parameter: { Demand: "Active" },
  /** The varied branch parameter: the thermal rating (serde unit variant). */
  ratingParameter: "LineLimit",
} as const;

/** Calculation names accepted by the browser engine. Power flow returns voltages
 * and flows; OPF calculations also return an objective and LMPs. */
export type Formulation = BrowserFormulation;

/** UI-facing formulation menu: tag, a short label, and a one-line description. The order
 * is the menu order; `dcopf` is first and is the default. */
export const FORMULATIONS: ReadonlyArray<{
  id: Formulation;
  label: string;
  hint: string;
  disabled?: boolean;
}> = [
  { id: "dcopf", label: "DC OPF", hint: "DC optimal power flow (the default)" },
  { id: "acpf", label: "AC power flow", hint: "Balanced AC power flow" },
  {
    id: "socwr",
    label: "SOCWR",
    hint: "Jabr second-order-cone relaxation of AC OPF",
  },
  {
    id: "acopf",
    label: "AC OPF",
    hint: "full nonlinear AC optimal power flow",
    disabled: true,
  },
];

/** DC OPF is the default calculation. */
export const DEFAULT_FORMULATION: Formulation = "dcopf";

/** The `Operand[]` JSON `Study.preview` watches (the active nodal value column). */
const PREVIEW_OPERANDS_JSON = JSON.stringify([STUDY_CAPABILITY.operand]);

/** The `SensRequest[]` JSON for `Study.commit`: the nodal-value/demand column at a dense bus
 * index, or the nodal-value/rating column at a dense branch index, or `[]` when there is no
 * target. NOTE: `SensRequest.indices` are **dense positional** indices into the target's
 * axis (0-based), *not* external ids — `BrowserStudy` translates the selected external id
 * to its dense index before calling this. */
function sensitivitiesJson(
  target: { kind: "bus" | "branch"; index: number } | null,
): string {
  if (target === null) return "[]";
  return JSON.stringify([
    {
      operand: STUDY_CAPABILITY.operand,
      parameter:
        target.kind === "bus"
          ? STUDY_CAPABILITY.parameter
          : STUDY_CAPABILITY.ratingParameter,
      indices: [target.index],
    },
  ]);
}

/** The SolveResponse JSON the Study returns from commit/solution. A superset of
 * formulations; DC fills objective/lmp/flows/dispatch/iterations. */
interface StudySolveResponse {
  formulation: string;
  status: string;
  objective: number | null;
  iterations?: SolveIteration[];
  lmp?: { bus: number; value: number }[];
  vm?: { bus: number; value: number }[];
  va?: { bus: number; value: number }[];
  w?: { bus: number; value: number }[];
  flows?: { branch: number; pf: number; loading: number }[];
  dispatch?: { gen: number; bus?: number; pg: number }[];
}

/** The Preview JSON the Study returns: first-order operand changes plus the
 * predicted objective change, with no re-solve. */
interface StudyPreview {
  operands: {
    operand: unknown;
    values: { element: { Bus?: number }; index: number; value: number }[];
    units: string;
  }[];
  objective_delta: number | null;
  local_only: boolean;
}

/** One `SensitivityMatrix` from the engine: `values[r][c] = d(rows[r])/d(cols[c])`,
 * with row/column metadata naming the source element (`element` is an externally
 * tagged `ElementId`, e.g. `{ Bus: id }` / `{ Branch: id }`). */
interface SensitivityMatrixJson {
  values: number[][];
  rows: { element: { Bus?: number; Branch?: number }; index: number }[];
  cols: { element: { Bus?: number; Branch?: number }; index: number }[];
  units: string;
}

/** The `{ solution, iterations, sensitivities }` JSON the Study's `commit` returns:
 * the committed `SolveResponse`, its convergence trace, and the watched ∂operand/∂param
 * columns — so the selected nodal value column comes back in the same solve. */
interface StudyCommitOutput {
  solution: StudySolveResponse;
  iterations: SolveIteration[] | null;
  sensitivities: SensitivityMatrixJson[];
}

/** Extract the nodal-value/parameter column from the first requested `SensitivityMatrix` into
 * the `SensitivityColumn` the map and legend consume, the same shape the server serves.
 * Rows are Price operands per bus; the single column is the selected parameter, so the
 * column is `values[r][0]` keyed by each row's source bus id, and the source element is
 * `cols[0].element`: a bus for the demand parameter (`parameter: "d"`) or a branch for
 * the rating parameter (`parameter: "fmax"`). Returns null when no matrix was requested
 * or its column has no recognized source element. */
function sensitivityColumn(
  caseId: string,
  matrices: SensitivityMatrixJson[],
): SensitivityColumn | null {
  const m = matrices[0];
  const el = m?.cols[0]?.element;
  if (!m || !el) return null;
  const values = m.rows
    .map((row, r) => ({ bus: row.element.Bus, value: m.values[r]?.[0] ?? 0 }))
    .filter((v): v is { bus: number; value: number } => v.bus !== undefined);
  const shared = { case: caseId, operand: "price", units: m.units, values };
  if (el.Bus !== undefined) {
    return { ...shared, parameter: "d", bus: el.Bus };
  }
  if (el.Branch !== undefined) {
    return { ...shared, parameter: "fmax", branch: el.Branch };
  }
  return null;
}

export interface SolutionResponseProjection {
  objective?: number | null;
  lmp?: { bus: number; value: number }[] | null;
  vm?: { bus: number; value: number }[] | null;
  va?: { bus: number; value: number }[] | null;
  w?: { bus: number; value: number }[] | null;
  flows?: { branch: number; pf: number; loading: number }[] | null;
  dispatch?: { gen: number; bus?: number | null; pg: number }[] | null;
}

/** Project the formulation-agnostic wire response onto the compact map result. */
export function solveResponseToSolution(
  out: SolutionResponseProjection,
): Solution {
  return {
    objective: out.objective ?? null,
    prices: (out.lmp ?? []).map((e) => ({ bus: e.bus, value: e.value })),
    vm: out.vm ?? undefined,
    va: out.va ?? [],
    w: out.w ?? [],
    flows: (out.flows ?? []).map((f) => ({
      branch: f.branch,
      mw: f.pf,
      loading: f.loading,
    })),
    dispatch: (out.dispatch ?? []).map((d) => ({
      gen: d.gen,
      bus: d.bus ?? undefined,
      mw: d.pg,
    })),
  };
}

/** Browser transport for the reactive demand drag. The retained PowerIO module
 * is parsed and the model built when the Study is created; `commit` solves exactly
 * at the UI's absolute demand delta state and `preview` returns a first-order
 * linearization toward an absolute demand delta state, neither re-parsing the
 * network. The wasm Study lives behind the engine host (a dedicated worker
 * when the browser allows one), so every method is async. */
export class BrowserStudy {
  #host: EngineHost;
  /** The caller-allocated handle naming the wasm Study on the host. */
  #handle: number;
  #formulation: Formulation;
  #planningHost: () => EngineHost;
  /** External bus id -> dense positional bus index, built once from the committed solution's
   * nodal value ordering (each wire `lmp[i].bus` sits at dense index `i`). The engine keys `SensRequest`
   * by this dense index, not the external bus id, so the selected bus must be translated
   * before a sensitivity request. Null until first needed; the bus set is solve-invariant. */
  #busToIndex: Map<number, number> | null = null;
  /** External branch id -> dense positional branch index, from the committed solution's
   * flows ordering — the branch-axis counterpart of `#busToIndex`. */
  #branchToIndex: Map<number, number> | null = null;

  constructor(
    host: EngineHost,
    handle: number,
    formulation: Formulation = DEFAULT_FORMULATION,
    planningHost: () => EngineHost = isolatedEngineHost,
  ) {
    this.#host = host;
    this.#handle = handle;
    this.#formulation = formulation;
    this.#planningHost = planningHost;
  }

  async #solution(): Promise<StudySolveResponse> {
    return JSON.parse(
      expectText(
        await this.#host.call({ op: "study_solution", study: this.#handle }),
      ),
    );
  }

  async #replaceEdits(
    deltas: DemandDeltas,
    rates: BranchRatingDeltas,
    target: SensTarget | null,
    signal?: AbortSignal,
  ): Promise<StudyCommitOutput> {
    const sensitivities = sensitivitiesJson(await this.#senseTarget(target));
    signal?.throwIfAborted();
    const cancel = () => {
      this.#host.cancel?.(new DOMException("solve cancelled", "AbortError"));
    };
    signal?.addEventListener("abort", cancel, { once: true });
    try {
      const result = await this.#host.call({
        op: "study_replace_edits",
        study: this.#handle,
        edits: JSON.stringify(toEdits(deltas, rates)),
        sensitivities,
      });
      signal?.throwIfAborted();
      return JSON.parse(expectText(result));
    } catch (error) {
      signal?.throwIfAborted();
      throw error;
    } finally {
      signal?.removeEventListener("abort", cancel);
    }
  }

  /** The dense-axis sensitivity target the engine expects for a selection (external
   * ids), or null when nothing is selected or the id is unknown. Memoizes the
   * id->index maps from the committed solution's nodal value and flow order — the same axis
   * orders the engine's dense sensitivity columns use. */
  async #senseTarget(
    target: SensTarget | null,
  ): Promise<{ kind: "bus" | "branch"; index: number } | null> {
    if (target === null || this.#formulation === "acpf") return null;
    if ("bus" in target) {
      if (!this.#busToIndex) {
        const sol = await this.#solution();
        this.#busToIndex = new Map((sol.lmp ?? []).map((e, i) => [e.bus, i]));
      }
      const index = this.#busToIndex.get(target.bus);
      return index === undefined ? null : { kind: "bus", index };
    }
    if (!this.#branchToIndex) {
      const sol = await this.#solution();
      this.#branchToIndex = new Map(
        (sol.flows ?? []).map((f, i) => [f.branch, i]),
      );
    }
    const index = this.#branchToIndex.get(target.branch);
    return index === undefined ? null : { kind: "branch", index };
  }

  /** Exact solve at demand = base + `deltas` and ratings = base + `rates`, replacing
   * the committed point. When `target` names a bus or branch, its nodal-value/parameter
   * column is computed in the *same* solve and returned (no second round-trip);
   * otherwise `sensitivity` is null. `caseId` labels the returned column. Returns the
   * UI Solution, the solver iterates, and the column. */
  async commit(
    caseId: string,
    deltas: DemandDeltas,
    rates: BranchRatingDeltas,
    target: SensTarget | null,
    signal?: AbortSignal,
  ): Promise<{
    solution: Solution;
    iterations: SolveIteration[];
    sensitivity: SensitivityColumn | null;
  }> {
    const out = await this.#replaceEdits(deltas, rates, target, signal);
    return {
      solution: solveResponseToSolution(out.solution),
      iterations: out.iterations ?? [],
      sensitivity:
        target === null ? null : sensitivityColumn(caseId, out.sensitivities),
    };
  }

  /** The nodal-value/parameter column at `target` for this study's formulation, computed by
   * an exact re-solve at the edited point (the same one-call path `commit` uses, but
   * returning only the column). Used when a bus or branch is selected so the overlay
   * matches the active formulation rather than always the DC sensitivity. `caseId`
   * labels the column; returns null when no recognized column comes back. */
  async sensitivity(
    caseId: string,
    deltas: DemandDeltas,
    rates: BranchRatingDeltas,
    target: SensTarget,
  ): Promise<SensitivityColumn | null> {
    if (this.#formulation === "acpf") return null;
    const out = await this.#replaceEdits(deltas, rates, target);
    return sensitivityColumn(caseId, out.sensitivities);
  }

  /** First order nodal value preview for replacing the committed point with
   * demand = base + `deltas` and ratings = base + `rates`, with no re-solve:
   * predicted per-bus nodal value changes and the predicted objective change. */
  async preview(
    deltas: DemandDeltas,
    rates: BranchRatingDeltas = {},
  ): Promise<{
    prices: { bus: number; value: number }[];
    objectiveDelta: number | null;
    units: string | null;
  }> {
    if (this.#formulation === "acpf") {
      return { prices: [], objectiveDelta: null, units: null };
    }
    const out: StudyPreview = JSON.parse(
      expectText(
        await this.#host.call({
          op: "study_preview",
          study: this.#handle,
          edits: JSON.stringify(toEdits(deltas, rates)),
          operands: PREVIEW_OPERANDS_JSON,
        }),
      ),
    );
    const prices = (out.operands[0]?.values ?? [])
      .filter((v) => v.element.Bus !== undefined)
      .map((v) => ({ bus: v.element.Bus as number, value: v.value }));
    return {
      prices,
      objectiveDelta: out.objective_delta,
      units: out.operands[0]?.units ?? null,
    };
  }

  /** The Study's current exact solution. Called immediately after Study creation
   * to cache the base point for formulation comparisons. */
  async currentView(): Promise<
    import("./generated/study-types.js").SolveResponse
  > {
    return JSON.parse(
      expectText(
        await this.#host.call({ op: "study_solution", study: this.#handle }),
      ),
    );
  }

  async currentSolution(): Promise<Solution> {
    return solveResponseToSolution(await this.#solution());
  }

  /** Materialize the current case as generation 2 PowerIO IR. */
  async saveModule(): Promise<string> {
    return expectText(
      await this.#host.call({ op: "study_save_module", study: this.#handle }),
    );
  }

  /** Preserve the materialized instance's inner objective and constraints in PowerIO IR. */
  async saveInstanceModule(): Promise<string> {
    return expectText(
      await this.#host.call({
        op: "study_save_instance_module",
        study: this.#handle,
      }),
    );
  }

  /** Serialize the current exact DC OPF result as a PowerIO solution module.
   * Its embedded problem instance contains the materialized case that was
   * solved. */
  async saveSolutionModule(): Promise<string> {
    return expectText(
      await this.#host.call({
        op: "study_save_solution_module",
        study: this.#handle,
      }),
    );
  }

  /** Write the current materialized case to one PowerIO target format. */
  async export(format: string): Promise<ExportedCase> {
    return JSON.parse(
      expectText(
        await this.#host.call({
          op: "study_export",
          study: this.#handle,
          format,
        }),
      ),
    );
  }

  /** Run a bounded differentiable planning search over the committed
   * operating point. Read only: the committed case, edits, and revision are
   * untouched; the returned outcome carries the unapplied proposal and the
   * search trace (gradient, per step prediction, exact result, first
   * order error, accept or reject reason). Rejects on a non-DC formulation
   * or an unknown element key. Worker hosts stop the solve on cancellation;
   * the synchronous fallback finishes the wasm call but rejects its result. */
  async plan(
    spec: CapacityPlanSpecJson,
    signal?: AbortSignal,
  ): Promise<CapacityPlanOutcomeJson> {
    signal?.throwIfAborted();
    const moduleJson = await this.saveModule();
    signal?.throwIfAborted();
    const planningStudy = await createStudyOnHost(
      moduleJson,
      this.#formulation,
      this.#planningHost(),
      signal,
    );
    try {
      return await planningStudy.#planInPlace(spec, signal);
    } finally {
      planningStudy.free();
    }
  }

  /** Run planning on this disposable Study. Public callers go through
   * `plan`, which first clones the committed PowerIO module onto an isolated
   * host so cancellation cannot invalidate the interactive Study. */
  async #planInPlace(
    spec: CapacityPlanSpecJson,
    signal?: AbortSignal,
  ): Promise<CapacityPlanOutcomeJson> {
    signal?.throwIfAborted();
    const cancel = () => {
      this.#host.cancel?.(new DOMException("planning cancelled", "AbortError"));
    };
    signal?.addEventListener("abort", cancel, { once: true });
    try {
      const result = await this.#host.call({
        op: "study_plan",
        study: this.#handle,
        spec: JSON.stringify(spec),
      });
      signal?.throwIfAborted();
      return JSON.parse(expectText(result));
    } catch (error) {
      signal?.throwIfAborted();
      throw error;
    } finally {
      signal?.removeEventListener("abort", cancel);
    }
  }

  /** Apply a parsed geographic layer (the canonical `.geo.json` from
   * `parseGeo` or `applyLayout`) onto this study's base network, so the next
   * `saveModule` or export carries the coordinates on screen. Locations are
   * metadata the model never reads; nothing re-solves. */
  async applyGeoLayer(layer: string): Promise<GeoApplyReport> {
    return JSON.parse(
      expectText(
        await this.#host.call({
          op: "study_apply_geo",
          study: this.#handle,
          layer,
        }),
      ),
    );
  }

  /** Release the wasm Study; call when discarding it (e.g. the case's
   * module input changed, or the case was removed). Fire and forget: the
   * handle is invalid from this call on. */
  free() {
    void this.#host
      .call({ op: "study_free", study: this.#handle })
      .catch(() => {});
  }
}

/** Construct a Study from a retained PowerIO module for `formulation`, parsing its
 * problem instance and solving the base case once. `formulation` is a browser Study tag
 * (`dcopf`/`acpf`/`socwr`, defaulting to DC OPF). Experimental `acopf` is deliberately
 * served by `solveAcOpfModule` in a separate WASI worker, not this retained Study.
 * Throws if the engine module can't load (or the formulation is unknown/not built); the
 * caller must catch and fall back (see `isPermanentEngineFailure`). */
let studySeq = 0;

export async function createStudy(
  moduleJson: string,
  formulation: Formulation = DEFAULT_FORMULATION,
  options: { isolated?: boolean; signal?: AbortSignal } = {},
): Promise<BrowserStudy> {
  const host = options.isolated ? isolatedEngineHost() : engineHost();
  return createStudyOnHost(moduleJson, formulation, host, options.signal);
}

async function createStudyOnHost(
  moduleJson: string,
  formulation: Formulation,
  host: EngineHost,
  signal?: AbortSignal,
): Promise<BrowserStudy> {
  // Handles are allocated here, not by the host, so a pending build replayed
  // onto the fallback host keeps naming the same study.
  const handle = ++studySeq;
  signal?.throwIfAborted();
  const cancel = () => {
    host.cancel?.(new DOMException("study creation cancelled", "AbortError"));
  };
  signal?.addEventListener("abort", cancel, { once: true });
  try {
    await host.call({
      op: "study_new",
      study: handle,
      module_json: moduleJson,
      formulation,
    });
    signal?.throwIfAborted();
  } catch (error) {
    signal?.throwIfAborted();
    throw error;
  } finally {
    signal?.removeEventListener("abort", cancel);
  }
  return new BrowserStudy(host, handle, formulation);
}

// ---- Synthetic grid generation --------------------------------------------

export type {
  GenerationReport,
  StepOutcome,
  SynthRequest,
  SynthStage,
  SynthErrorPayload,
};

/** One `advance()`: the stage that ran, the next one, progress, warnings,
 * and whether the preview changed. */
export type SynthAdvance = StepOutcome;

/** A synthetic generation preview, ready for binary map attributes: bus
 * `[lon, lat]` pairs, kinds (0 load, 1 substation, 2 generator, 3 converter)
 * and kV in bus order (bus id = index + 1); concatenated branch route points,
 * the point index where each route starts (one entry more than there are
 * branches, the path layer's binary layout), and branch kV. */
export interface SynthPreview {
  busPositions: Float64Array;
  busKinds: Uint8Array;
  busKv: Float32Array;
  branchPositions: Float64Array;
  branchStartIndices: Uint32Array;
  branchKv: Float32Array;
}

/** Summary of a loaded places table. */
export interface SynthPlacesInfo {
  count: number;
  sha256: string;
}

/** A refusal from the generator, with the code a caller switches on:
 * `invalid_request`, `region`, `places`, `osm`, `osm_insufficient` (an
 * Overpass document too thin to build from, with its bus and branch counts),
 * `build`, `engine`, `state`, `cancelled`. */
export class SynthEngineError extends Error {
  readonly code: string;
  readonly buses?: number;
  readonly branches?: number;

  constructor(payload: SynthErrorPayload) {
    super(payload.message);
    this.name = "SynthEngineError";
    this.code = payload.code;
    if (payload.buses != null) this.buses = payload.buses;
    if (payload.branches != null) this.branches = payload.branches;
  }
}

/** The engine reports generator refusals as their JSON payload. */
function synthError(error: unknown): unknown {
  const message = error instanceof Error ? error.message : String(error);
  try {
    const payload = JSON.parse(message) as Partial<SynthErrorPayload>;
    if (typeof payload?.code === "string" && typeof payload.message === "string") {
      return new SynthEngineError(payload as SynthErrorPayload);
    }
  } catch {
    // Not a generator payload: a host or transport failure.
  }
  return error;
}

async function callSynth(host: EngineHost, req: EngineRequest): Promise<EngineValue> {
  try {
    return await host.call(req);
  } catch (error) {
    throw synthError(error);
  }
}

/** URL of the places table shipped beside this module (Natural Earth
 * populated places, public domain). Resolved on use, like the wasm asset. */
export function syntheticPlacesUrl(): string {
  return new URL("./data/populated_places.json", import.meta.url).href;
}

let placesBytes: Promise<Uint8Array> | null = null;

/** Fetch the shipped places table, once per page. */
export function fetchSyntheticPlaces(): Promise<Uint8Array> {
  placesBytes ??= fetch(syntheticPlacesUrl())
    .then(async (response) => {
      if (!response.ok) throw new Error(`places table: HTTP ${response.status}`);
      return new Uint8Array(await response.arrayBuffer());
    })
    .catch((error) => {
      // A failed fetch must not disable generation for the whole session.
      placesBytes = null;
      throw error;
    });
  return placesBytes;
}

/** Hosts holding a places table, with the thread kind they loaded it on (a
 * worker that fell back to the page needs it loaded again there). */
const placesLoaded = new WeakMap<EngineHost, EngineHost["kind"]>();

async function loadPlacesOn(host: EngineHost, bytes: Uint8Array): Promise<SynthPlacesInfo> {
  assertEngineInputBytes(bytes);
  const info = JSON.parse(
    expectText(await callSynth(host, { op: "synth_load_places", bytes })),
  ) as SynthPlacesInfo;
  placesLoaded.set(host, host.kind);
  return info;
}

/** Load a places table onto the synthetic generation host. Sessions load the
 * shipped table on their own; call this to warm the host up or to use
 * another table of `{n, lo, la, p, c}` rows. */
export async function loadSyntheticPlaces(bytes: Uint8Array): Promise<SynthPlacesInfo> {
  return loadPlacesOn(synthEngineHost(), bytes);
}

/** The Overpass QL queries for a request's region, one per bounding box (two
 * when it crosses the antimeridian). The caller fetches them and passes the
 * documents as the `osm` option. */
export async function syntheticOverpassQueries(request: SynthRequest): Promise<string[]> {
  const text = JSON.stringify(request);
  assertEngineInputLength(text.length);
  return JSON.parse(
    expectText(await callSynth(synthEngineHost(), { op: "synth_overpass_queries", request: text })),
  ) as string[];
}

function expectSynthPreview(value: EngineValue): SynthPreview {
  if (value === null || typeof value !== "object" || value instanceof Float64Array || value.kind !== "synth_preview") {
    throw new Error("engine returned no synthetic preview");
  }
  return {
    busPositions: value.busPositions,
    busKinds: value.busKinds,
    busKv: value.busKv,
    branchPositions: value.branchPositions,
    branchStartIndices: value.branchStartIndices,
    branchKv: value.branchKv,
  };
}

/** A staged synthetic grid generation on the dedicated synthetic host. Each
 * `advance()` runs one bounded stage (`context`, `site_buses`, `topology`,
 * `electrify`, `dc_opf`, `calibrate` once per iteration, `ac_pf`, `finalize`),
 * so a caller can show progress and a map preview between stages. */
export class BrowserSynthSession {
  #done = false;
  #freed = false;
  #stopped: unknown = null;

  constructor(
    private readonly host: EngineHost,
    private readonly handle: number,
  ) {}

  /** Where the session runs: `"direct"` means the page's own thread, which
   * can pause while a stage runs. */
  get hostKind(): "worker" | "direct" {
    return this.host.kind;
  }

  /** True once `finalize` has run. */
  get done(): boolean {
    return this.#done;
  }

  #check(): void {
    if (this.#stopped !== null) throw this.#stopped;
    if (this.#freed) throw new Error("synthetic generation session is freed");
  }

  /** Run one stage. With `withPreview`, a stage that changed the preview
   * also returns it. */
  async advance(
    options: { withPreview?: boolean } = {},
  ): Promise<SynthAdvance & { preview?: SynthPreview }> {
    this.#check();
    const outcome = JSON.parse(
      expectText(await callSynth(this.host, { op: "synth_session_advance", session: this.handle })),
    ) as SynthAdvance;
    this.#done = outcome.done;
    if (options.withPreview && outcome.preview_changed) {
      return { ...outcome, preview: await this.preview() };
    }
    return outcome;
  }

  /** Advance to the end, calling `onStep` after each stage (with the preview
   * when it changed). An abort cancels the session; the promise rejects with
   * the abort reason. */
  async run(
    onStep?: (outcome: SynthAdvance, preview?: SynthPreview) => void,
    signal?: AbortSignal,
  ): Promise<GenerationReport> {
    signal?.throwIfAborted();
    const abort = () => this.cancel(signal?.reason);
    signal?.addEventListener("abort", abort, { once: true });
    try {
      while (!this.#done) {
        const { preview, ...outcome } = await this.advance({ withPreview: onStep !== undefined });
        signal?.throwIfAborted();
        onStep?.(outcome, preview);
      }
      return await this.report();
    } catch (error) {
      signal?.throwIfAborted();
      throw error;
    } finally {
      signal?.removeEventListener("abort", abort);
    }
  }

  async preview(): Promise<SynthPreview> {
    this.#check();
    return expectSynthPreview(
      await callSynth(this.host, { op: "synth_session_preview", session: this.handle }),
    );
  }

  /** The generation report, partial until the session is done. */
  async report(): Promise<GenerationReport> {
    this.#check();
    return JSON.parse(
      expectText(await callSynth(this.host, { op: "synth_session_report", session: this.handle })),
    ) as GenerationReport;
  }

  /** The module as generation 2 PowerIO IR text, with bus locations and
   * branch routes. Available from `electrify` on; does not free the session. */
  async takeModule(): Promise<string> {
    this.#check();
    return expectText(
      await callSynth(this.host, { op: "synth_session_module", session: this.handle }),
    );
  }

  /** The module's drop-panel payload, the same {@link IngestedCase} a
   * dropped `.pio.json` produces, so it opens without a second parse. */
  async ingest(): Promise<IngestedCase> {
    this.#check();
    return JSON.parse(
      expectText(await callSynth(this.host, { op: "synth_session_ingest", session: this.handle })),
    ) as IngestedCase;
  }

  /** The network as GridFM graphkit JSON: per-unit bus, branch and
   * generator feature matrices on a 0-based bus index, with the edge index
   * in both directions. Available from `electrify` on. */
  async exportGridfm(): Promise<string> {
    this.#check();
    return expectText(
      await callSynth(this.host, { op: "synth_session_export_gridfm", session: this.handle }),
    );
  }

  /** Stop the generation. On a worker host this terminates the synthetic
   * generation worker: interactive studies live on another worker and are
   * untouched, and the next session starts a fresh one. On the page's own
   * thread it takes effect before the next stage. Later calls reject with
   * `reason`. */
  cancel(reason: unknown = new DOMException("synthetic generation cancelled", "AbortError")): void {
    if (this.#stopped !== null || this.#freed) return;
    this.#stopped = reason;
    const error = reason instanceof Error ? reason : new Error(String(reason));
    if (this.host.cancel?.(error)) {
      this.#freed = true; // the worker and its sessions are gone
    } else {
      this.free();
    }
  }

  /** Release the session's engine state. */
  free(): void {
    if (this.#freed) return;
    this.#freed = true;
    void this.host
      .call({ op: "synth_session_free", session: this.handle })
      .catch(() => {});
  }
}

let synthSessionSeq = 0;

export interface SynthSessionOptions {
  /** Overpass document(s) for `source: "osm"`: JSON text, or the parsed
   * document or array of documents. Kept out of the request, which is hashed
   * for provenance. */
  osm?: string | object;
  /** A places table to use instead of the shipped one. */
  places?: Uint8Array;
  signal?: AbortSignal;
}

function osmText(osm: SynthSessionOptions["osm"]): string | null {
  if (osm === undefined) return null;
  const text = typeof osm === "string" ? osm : JSON.stringify(osm);
  assertEngineInputLength(text.length);
  return text;
}

/** Open a staged generation on the synthetic host, loading the places table
 * there first when the host has none. */
export async function createSynthSession(
  request: SynthRequest,
  options: SynthSessionOptions = {},
): Promise<BrowserSynthSession> {
  const { signal } = options;
  signal?.throwIfAborted();
  const requestText = JSON.stringify(request);
  assertEngineInputLength(requestText.length);
  const osm = osmText(options.osm);
  const host = synthEngineHost();
  if (options.places) await loadPlacesOn(host, options.places);
  else if (placesLoaded.get(host) !== host.kind) await loadPlacesOn(host, await fetchSyntheticPlaces());
  signal?.throwIfAborted();
  // Handles are allocated here, so a pending request replayed onto the
  // fallback host keeps naming the same session.
  const handle = ++synthSessionSeq;
  await callSynth(host, { op: "synth_session_new", session: handle, request: requestText, osm });
  const session = new BrowserSynthSession(host, handle);
  if (signal?.aborted) {
    session.free();
    signal.throwIfAborted();
  }
  return session;
}

/** Run every stage in one engine request and return the module, the report
 * and the drop-panel payload. An abort cancels the run. */
export async function generateSynthetic(
  request: SynthRequest,
  options: SynthSessionOptions = {},
): Promise<{ moduleJson: string; report: GenerationReport; ingest: IngestedCase }> {
  const { signal } = options;
  signal?.throwIfAborted();
  const requestText = JSON.stringify(request);
  assertEngineInputLength(requestText.length);
  const osm = osmText(options.osm);
  const host = synthEngineHost();
  if (options.places) await loadPlacesOn(host, options.places);
  else if (placesLoaded.get(host) !== host.kind) await loadPlacesOn(host, await fetchSyntheticPlaces());
  signal?.throwIfAborted();
  const cancel = () => {
    host.cancel?.(new DOMException("synthetic generation cancelled", "AbortError"));
  };
  signal?.addEventListener("abort", cancel, { once: true });
  try {
    const out = JSON.parse(
      expectText(await callSynth(host, { op: "synth_generate", request: requestText, osm })),
    ) as { module_json: string; report: GenerationReport; ingest: IngestedCase };
    signal?.throwIfAborted();
    return { moduleJson: out.module_json, report: out.report, ingest: out.ingest };
  } catch (error) {
    signal?.throwIfAborted();
    throw error;
  } finally {
    signal?.removeEventListener("abort", cancel);
  }
}

export interface EngineTransport {
  preloadEngine(): Promise<void>;
  classifyJson(bytes: Uint8Array): Promise<JsonDropClassification>;
  ingestJsonDrop(bytes: Uint8Array): Promise<IngestedJsonDrop>;
  ingestCase(bytes: Uint8Array, format: string): Promise<IngestedCase>;
  ingestDistCase(text: string, format: string): Promise<IngestedDistCase>;
  ingestDistCaseBytes(
    bytes: Uint8Array,
    format: string,
  ): Promise<IngestedDistCase>;
  parseDisplay(bytes: Uint8Array): Promise<DisplayPreview>;
  parseGeo(bytes: Uint8Array, hint: string): Promise<ParsedGeoLayer>;
  applyGeo(moduleJson: string, layer: string): Promise<AppliedGeoCase>;
  applyMcGeo?(moduleJson: string, layer: string): Promise<AppliedMcGeoCase>;
  applyLayout(
    moduleJson: string,
    coords: Record<number, [number, number]>,
    kind: "synthetic" | "manual",
  ): Promise<StampedLayoutCase>;
  extractGeo(moduleJson: string): Promise<string>;
  applyDisplayGeo(
    moduleJson: string,
    bytes: Uint8Array,
  ): Promise<AppliedGeoCase>;
  capabilities(): Promise<ProblemCaps[]>;
  solveModule(
    moduleJson: string,
    request?: SolveRequest,
  ): Promise<SolveResponse>;
  solveMcBmopf(text: string, options?: McPfOptions): Promise<McPfResult>;
  solveMcModule(
    moduleJson: string,
    options?: McPfOptions,
    signal?: AbortSignal,
  ): Promise<McPfResult>;
  solveMcStudy(
    moduleJson: string,
    studyId: string,
    studyTitle: string,
    options?: McPfOptions,
    signal?: AbortSignal,
  ): Promise<McStudySnapshot>;
  createMcPfSession(
    moduleJson: string,
    options?: McPfOptions,
    signal?: AbortSignal,
  ): Promise<BrowserMcPfSession>;
  replayMcStudy(snapshotJson: string): Promise<McStudySnapshot>;
  applyMcStudyGeo?(
    snapshot: McStudySnapshot,
    layer: string,
  ): Promise<McStudySnapshot>;
  createStudy(
    moduleJson: string,
    formulation?: Formulation,
    options?: { isolated?: boolean; signal?: AbortSignal },
  ): Promise<BrowserStudy>;
  loadSyntheticPlaces?(bytes: Uint8Array): Promise<SynthPlacesInfo>;
  syntheticOverpassQueries?(request: SynthRequest): Promise<string[]>;
  createSynthSession?(
    request: SynthRequest,
    options?: SynthSessionOptions,
  ): Promise<BrowserSynthSession>;
  generateSynthetic?(
    request: SynthRequest,
    options?: SynthSessionOptions,
  ): Promise<{ moduleJson: string; report: GenerationReport; ingest: IngestedCase }>;
}

export const browserWasmTransport: EngineTransport = {
  preloadEngine,
  classifyJson,
  ingestJsonDrop,
  ingestCase,
  ingestDistCase,
  ingestDistCaseBytes,
  parseDisplay,
  parseGeo,
  applyGeo,
  applyMcGeo,
  applyLayout,
  extractGeo,
  applyDisplayGeo,
  capabilities,
  solveModule,
  solveMcBmopf,
  solveMcModule,
  solveMcStudy,
  createMcPfSession,
  replayMcStudy,
  applyMcStudyGeo,
  createStudy,
  loadSyntheticPlaces,
  syntheticOverpassQueries,
  createSynthSession,
  generateSynthetic,
};

export function createBrowserWasmTransport(): EngineTransport {
  return browserWasmTransport;
}

export { BrowserStudy as Study };

export {
  IndexedDbStudyStore,
  StudyDocumentController,
  studyBackend,
} from "./study-document.js";
export type { StudyBackend, StudyStore } from "./study-document.js";
export type {
  CreateStudy,
  StudyBundle,
  StudyDocument,
  StudyRequest,
  StudyOperation,
  StudyOperationResult,
  GoalRevision,
  DecisionSpace,
  StudyObjective,
  StateNode,
  Comparison,
} from "./generated/study-types.js";

export type {
  ModelDetails,
  SolveResponse as StudyView,
  SearchOptions,
  StudySummary,
} from "./generated/study-types.js";
export { studySchema } from "./generated/study-schema.js";
export { synthSchema } from "./generated/synth-schema.js";
