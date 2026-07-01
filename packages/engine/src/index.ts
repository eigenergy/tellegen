/** Lazy loader for the powerio wasm module. Nothing downloads until the
 * first file is dropped; the dropped file is parsed in the browser and never
 * leaves the machine. */
import type {
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
} from "./generated/contracts.js";
export {
  CONTRACT_SOURCE_SHA256,
  CONTRACT_VERSION,
  FORMULATION_IDS,
  SOLVE_STATUSES,
} from "./generated/contracts.js";

export type {
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
} from "./generated/contracts.js";

export interface CaseFileSummary {
  name: string;
  base_mva: number;
  n_bus: number;
  n_branch: number;
  n_gen: number;
  load_mw: number;
  gen_mw: number;
  has_coords: boolean;
  coords_kind: "file" | "synthetic_pending";
  warnings: string[];
}

export interface TopologyBus {
  id: number;
  demand_mw: number;
  gen_mw: number;
}

export interface TopologyBranch {
  id: number;
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
  network_json: string;
  topology: Topology;
  view: { buses: NetworkBus[]; branches: NetworkBranch[] } | null;
}

let ready: Promise<typeof import("./wasm-pkg/tellegen.js")> | null = null;
let sensitivityReady: Promise<
  typeof import("./wasm-sens-pkg/tellegen_sens.js")
> | null = null;
let sensitivityUnsupported: string | null = null;

function powerio() {
  // The wasm asset is imported here, not at module top level, so that
  // evaluating this module (e.g. during SvelteKit's dev-mode SSR pass, which
  // never calls this function) never touches the wasm loader.
  ready ??= Promise.all([
    import("./wasm-pkg/tellegen.js"),
    import("./wasm-pkg/tellegen_bg.wasm?url"),
  ])
    .then(async ([mod, wasmMod]) => {
      await mod.default({ module_or_path: wasmMod.default });
      return mod;
    })
    .catch((e) => {
      // Don't cache a rejected load: a transient failure (chunk fetch or
      // instantiate) must not disable the feature for the whole session.
      ready = null;
      throw e;
    });
  return ready;
}

function powerioSensitivity() {
  if (sensitivityUnsupported)
    return Promise.reject(new Error(sensitivityUnsupported));
  sensitivityReady ??= Promise.all([
    import("./wasm-sens-pkg/tellegen_sens.js"),
    import("./wasm-sens-pkg/tellegen_sens_bg.wasm?url"),
  ])
    .then(async ([mod, wasmMod]) => {
      await mod.default({ module_or_path: wasmMod.default });
      return mod;
    })
    .catch((e) => {
      const message = errorText(e);
      sensitivityReady = null;
      if (isPermanentWasmLoadFailure(message)) sensitivityUnsupported = message;
      throw e;
    });
  return sensitivityReady;
}

export async function preloadCore(): Promise<void> {
  await powerio();
}

export async function preloadSensitivity(): Promise<void> {
  await powerioSensitivity();
}

function isPermanentWasmLoadFailure(message: string): boolean {
  // Latch only genuine browser-capability failures the sensitivity module can
  // never recover from in this browser: no WebAssembly, or an opcode the
  // engine rejects. Transient fetch or
  // instantiate failures (offline, 503, aborted navigation) routinely carry
  // the .wasm URL or "Failed to fetch" in their message, so keying on the bare
  // word "wasm"/"compile" wrongly disables the feature for the whole session.
  // Those stay retryable, matching the powerio() loader.
  if (/Failed to fetch|NetworkError|load failed|aborted|ERR_/i.test(message))
    return false;
  return /CompileError|LinkError|invalid opcode|unsupported|relaxed|WebAssembly is not defined/i.test(
    message,
  );
}

/** powerio format token from a file name; null for non-case files. */
export function formatOf(name: string): string | null {
  const ext = name.split(".").pop()?.toLowerCase();
  return ext === "m" || ext === "raw" || ext === "aux" ? ext : null;
}

export async function ingestCase(
  text: string,
  format: string,
): Promise<IngestedCase> {
  return JSON.parse((await powerio()).ingest_case(text, format));
}

/** Substations from a PowerWorld .pwd display file. x/y are diagram
 * coordinates as stored (not lat/lon); the caller projects them. */
export interface DisplayPreview {
  substations: { number: number; name: string; x: number; y: number }[];
  canvas_width: number;
  canvas_height: number;
}

/** True for binary display files (PowerWorld .pwd), read via parseDisplay.
 * Kept separate from formatOf: a .pwd is display data, not a case format. */
export function isDisplayFile(name: string): boolean {
  return name.split(".").pop()?.toLowerCase() === "pwd";
}

export async function parseDisplay(bytes: Uint8Array): Promise<DisplayPreview> {
  return JSON.parse((await powerio()).parse_display(bytes, "pwd"));
}

export async function capabilities(): Promise<ProblemCaps[]> {
  return JSON.parse((await powerioSensitivity()).capabilities_json());
}

export async function solveJson(
  networkJson: string,
  request: SolveRequest = {},
): Promise<SolveResponse> {
  return JSON.parse(
    (await powerioSensitivity()).solve_json(
      networkJson,
      JSON.stringify(request),
    ),
  );
}

/** The browser DC solve: the exact solution plus, when `sensBus` is given, its
 * dLMP/dd column — the same shapes the server serves. */
export interface BrowserSolution {
  solution: Solution;
  sensitivity: SensitivityColumn | null;
  sensitivityError?: string;
  iterations: SolveIteration[];
}

/** Solve the DC OPF in the browser at demand = base + `deltas`. `networkJson` is
 * the raw powerio Network (from the `/case` endpoint or a browser parse). When
 * `sensBus` is set, the dLMP/dd column is loaded from the sensitivity wasm
 * package when the browser supports that module. */
export async function solveDc(
  caseId: string,
  networkJson: string,
  deltas: DemandDeltas,
  sensBus: number | null,
): Promise<BrowserSolution> {
  const request = JSON.stringify({ deltas, sens_bus: sensBus });
  if (sensBus !== null) {
    try {
      return parseSolveOutput(
        caseId,
        (await powerioSensitivity()).solve_dc(networkJson, request),
      );
    } catch (e) {
      const baseRequest = JSON.stringify({ deltas, sens_bus: null });
      const message = errorText(e);
      return {
        ...parseSolveOutput(
          caseId,
          (await powerio()).solve_dc(networkJson, baseRequest),
        ),
        sensitivityError: isPermanentWasmLoadFailure(message)
          ? "sensitivity wasm is not supported by this browser"
          : message,
      };
    }
  }
  return parseSolveOutput(
    caseId,
    (await powerio()).solve_dc(networkJson, request),
  );
}

function parseSolveOutput(caseId: string, json: string): BrowserSolution {
  const out = JSON.parse(json);
  const solution: Solution = {
    objective: out.objective,
    lmp: out.lmp,
    va: out.va ?? [],
    w: out.w ?? [],
    flows: out.flows,
    dispatch: out.dispatch,
  };
  const d = out.dlmp_dd;
  const sensitivity: SensitivityColumn | null = d
    ? {
        case: caseId,
        operand: d.operand,
        parameter: d.parameter,
        bus: d.bus,
        units: d.units,
        values: d.values,
      }
    : null;
  return { solution, sensitivity, iterations: out.iterations ?? [] };
}

export function errorText(e: unknown): string {
  return e instanceof Error ? e.message : String(e);
}

/** True when the sensitivity wasm module has failed to load in a way it can
 * never recover from in this browser. The Study path uses the sens module, so
 * the caller must fall back to `solveDc`/the server when this is true. */
export function isPermanentSensFailure(message: string): boolean {
  return isPermanentWasmLoadFailure(message);
}

/** One in-place network mutation for the Study handle. `bus` is the original
 * bus id; `p_mw` is the demand delta in MW. */
interface NetworkEdit {
  kind: "add_load";
  bus: number;
  p_mw: number;
}

/** A `NetworkEdit[]` for the wasm Study, dropping zero deltas so an unchanged
 * bus is never sent. */
function deltasToEdits(deltas: DemandDeltas): NetworkEdit[] {
  return Object.entries(deltas)
    .filter(([, mw]) => mw !== 0)
    .map(([bus, p_mw]) => ({ kind: "add_load", bus: Number(bus), p_mw }));
}

/** The (operand, parameter) cell the UI drives: a study whose sensitivity column is
 * ∂(price, active) / ∂(demand, active). The *formulation* is no longer fixed here — it is
 * a parameter threaded from the UI's selector through `createStudy` (every formulation the
 * full wasm build carries returns LMP, so this same column applies to all of them). The
 * operand/parameter stay centralized so `createStudy` and the Study's `commit`/`preview`
 * requests read one source. */
const STUDY_CAPABILITY = {
  /** The watched operand: locational marginal price (active power). */
  operand: { Price: "Active" },
  /** The varied parameter: bus demand (active power). */
  parameter: { Demand: "Active" },
} as const;

/** The formulations the full wasm build solves entirely in the browser. Each returns LMP,
 * so the price map, legend, and the ∂LMP/∂d overlay apply unchanged to all of them. Tags
 * are the engine's serde-lowercase `Problem` variants accepted by `new Study(json, tag)`.
 * `dcopf` is the default (zero regression from the prior fixed behavior). */
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

/** The default formulation: DC OPF, preserving the prior fixed behavior byte-for-byte. */
export const DEFAULT_FORMULATION: Formulation = "dcopf";

/** The `Operand[]` JSON `Study.preview` watches (the LMP column). */
const PREVIEW_OPERANDS_JSON = JSON.stringify([STUDY_CAPABILITY.operand]);

/** The `SensRequest[]` JSON for `Study.commit`: the ∂LMP/∂demand column at the dense bus
 * `index`, or `[]` when there is none. NOTE: `SensRequest.indices` are **dense positional**
 * indices into the bus axis (0-based), *not* external bus ids — `BrowserStudy` translates
 * the selected external bus id to its dense index before calling this. */
function sensitivitiesJson(index: number | null): string {
  if (index === null) return "[]";
  return JSON.stringify([
    {
      operand: STUDY_CAPABILITY.operand,
      parameter: STUDY_CAPABILITY.parameter,
      indices: [index],
    },
  ]);
}

/** The SolveResponse JSON the Study returns from commit/solution. A superset of
 * formulations; DC fills objective/lmp/flows/dispatch/iterations. */
interface StudySolveResponse {
  formulation: string;
  status: string;
  objective: number;
  iterations?: SolveIteration[];
  lmp?: { bus: number; value: number }[];
  va?: { bus: number; value: number }[];
  w?: { bus: number; value: number }[];
  flows?: { branch: number; pf: number; loading: number }[];
  dispatch?: { gen: number; pg: number }[];
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
 * tagged `ElementId`, e.g. `{ Bus: id }`). */
interface SensitivityMatrixJson {
  values: number[][];
  rows: { element: { Bus?: number }; index: number }[];
  cols: { element: { Bus?: number }; index: number }[];
  units: string;
}

/** The `{ solution, iterations, sensitivities }` JSON the Study's `commit` returns:
 * the committed `SolveResponse`, its convergence trace, and the watched ∂operand/∂param
 * columns — so the ∂LMP/∂d column comes back in the same solve, no second round-trip. */
interface StudyCommitOutput {
  solution: StudySolveResponse;
  iterations: SolveIteration[] | null;
  sensitivities: SensitivityMatrixJson[];
}

/** Extract the ∂LMP/∂demand column from the first requested `SensitivityMatrix` into the
 * legacy `SensitivityColumn` the map and legend consume — the same shape `solve_dc`'s
 * `dlmp_dd` and the server serve. Rows are Price operands per bus; the single column is
 * the demand-at-`sensBus` parameter, so the column is `values[r][0]` keyed by each row's
 * source bus id (`rows[r].element.Bus`), and the column bus is `cols[0].element.Bus`.
 * Returns null when no matrix was requested or it lacks a bus-keyed column. */
function sensitivityColumn(
  caseId: string,
  matrices: SensitivityMatrixJson[],
): SensitivityColumn | null {
  const m = matrices[0];
  const colBus = m?.cols[0]?.element.Bus;
  if (!m || colBus === undefined) return null;
  const values = m.rows
    .map((row, r) => ({ bus: row.element.Bus, value: m.values[r]?.[0] ?? 0 }))
    .filter((v): v is { bus: number; value: number } => v.bus !== undefined);
  return {
    case: caseId,
    operand: "lmp",
    parameter: "d",
    bus: colBus,
    units: m.units,
    values,
  };
}

function solveResponseToSolution(out: StudySolveResponse): Solution {
  return {
    objective: out.objective ?? 0,
    lmp: (out.lmp ?? []).map((e) => ({ bus: e.bus, usd_per_mwh: e.value })),
    va: out.va ?? [],
    w: out.w ?? [],
    flows: (out.flows ?? []).map((f) => ({
      branch: f.branch,
      mw: f.pf,
      loading: f.loading,
    })),
    dispatch: (out.dispatch ?? []).map((d) => ({ gen: d.gen, mw: d.pg })),
  };
}

/** Build-once browser transport for the reactive demand drag. The network is
 * parsed and the model built when the Study is created; `commit` solves exactly
 * at the UI's absolute demand delta state and `preview` returns a first-order
 * linearization toward an absolute demand delta state, neither re-parsing the
 * network (unlike `solveDc`, which rebuilds the DcNetwork on every call). */
export class BrowserStudy {
  #study: import("./wasm-sens-pkg/tellegen_sens.js").Study;
  /** External bus id -> dense positional bus index, built once from the committed solution's
   * LMP ordering (each `lmp[i].bus` sits at dense index `i`). The engine keys `SensRequest`
   * by this dense index, not the external bus id, so the selected bus must be translated
   * before a sensitivity request. Null until first needed; the bus set is solve-invariant. */
  #busToIndex: Map<number, number> | null = null;

  constructor(study: import("./wasm-sens-pkg/tellegen_sens.js").Study) {
    this.#study = study;
  }

  /** The dense bus index the engine expects for `sensBus` (an external bus id), or null when
   * no bus is selected or the id is unknown. Memoizes the id->index map from the committed
   * solution's LMP order — the same axis order the engine's dense sensitivity columns use. */
  #senseIndex(sensBus: number | null): number | null {
    if (sensBus === null) return null;
    if (!this.#busToIndex) {
      const sol: StudySolveResponse = JSON.parse(this.#study.solution());
      this.#busToIndex = new Map((sol.lmp ?? []).map((e, i) => [e.bus, i]));
    }
    return this.#busToIndex.get(sensBus) ?? null;
  }

  /** Exact solve at demand = base + `deltas`, replacing the committed point. When
   * `sensBus` is set, the ∂LMP/∂demand column at that bus is computed in the *same* solve
   * and returned (no second round-trip); otherwise `sensitivity` is null. `caseId` labels
   * the returned column. Returns the UI Solution, the solver iterates, and the column. */
  commit(
    caseId: string,
    deltas: DemandDeltas,
    sensBus: number | null,
  ): {
    solution: Solution;
    iterations: SolveIteration[];
    sensitivity: SensitivityColumn | null;
  } {
    const out: StudyCommitOutput = JSON.parse(
      this.#study.replace_edits(
        JSON.stringify(deltasToEdits(deltas)),
        sensitivitiesJson(this.#senseIndex(sensBus)),
      ),
    );
    return {
      solution: solveResponseToSolution(out.solution),
      iterations: out.iterations ?? [],
      sensitivity:
        sensBus === null ? null : sensitivityColumn(caseId, out.sensitivities),
    };
  }

  /** The ∂LMP/∂demand column at `sensBus` for this study's formulation, computed by an
   * exact re-solve at demand = base + `deltas` (the same one-call path `commit` uses,
   * but returning only the column). Used when a bus is selected so the overlay matches the
   * active formulation — DC OPF, AC OPF, or SOCWR — rather than always the DC sensitivity.
   * `caseId` labels the column; returns null when no bus-keyed column comes back. */
  sensitivity(
    caseId: string,
    deltas: DemandDeltas,
    sensBus: number,
  ): SensitivityColumn | null {
    const out: StudyCommitOutput = JSON.parse(
      this.#study.replace_edits(
        JSON.stringify(deltasToEdits(deltas)),
        sensitivitiesJson(this.#senseIndex(sensBus)),
      ),
    );
    return sensitivityColumn(caseId, out.sensitivities);
  }

  /** First-order LMP preview for replacing the committed point with
   * demand = base + `deltas`, with no re-solve: predicted per-bus ΔLMP and the
   * predicted Δobjective. */
  preview(deltas: DemandDeltas): {
    lmp: { bus: number; usd_per_mwh: number }[];
    objectiveDelta: number | null;
  } {
    const out: StudyPreview = JSON.parse(
      this.#study.preview_replacement(
        JSON.stringify(deltasToEdits(deltas)),
        PREVIEW_OPERANDS_JSON,
      ),
    );
    const lmp = (out.operands[0]?.values ?? [])
      .filter((v) => v.element.Bus !== undefined)
      .map((v) => ({ bus: v.element.Bus as number, usd_per_mwh: v.value }));
    return { lmp, objectiveDelta: out.objective_delta };
  }

  /** The Study's current exact solution. Called immediately after Study creation
   * to cache the base point for formulation comparisons. */
  currentSolution(): Solution {
    return solveResponseToSolution(
      JSON.parse(this.#study.solution()) as StudySolveResponse,
    );
  }

  /** Release the wasm Study; call when discarding it (e.g. the case's
   * networkJson changed, or the case was removed). */
  free() {
    this.#study.free();
  }
}

/** Construct a build-once Study over `networkJson` for `formulation`, parsing the network
 * and solving the base case once. `formulation` is a `Problem` tag (`dcopf`/`acopf`/`socwr`,
 * defaulting to DC OPF); the full wasm build solves every one entirely in the browser.
 * Throws if the sens module can't load (or the formulation is unknown/not built); the
 * caller must catch and fall back (see `isPermanentSensFailure`). */
export async function createStudy(
  networkJson: string,
  formulation: Formulation = DEFAULT_FORMULATION,
): Promise<BrowserStudy> {
  const mod = await powerioSensitivity();
  return new BrowserStudy(new mod.Study(networkJson, formulation));
}

export interface EngineTransport {
  preloadCore(): Promise<void>;
  preloadSensitivity(): Promise<void>;
  ingestCase(text: string, format: string): Promise<IngestedCase>;
  parseDisplay(bytes: Uint8Array): Promise<DisplayPreview>;
  capabilities(): Promise<ProblemCaps[]>;
  solveJson(networkJson: string, request?: SolveRequest): Promise<SolveResponse>;
  solveDc(
    caseId: string,
    networkJson: string,
    deltas: DemandDeltas,
    sensBus: number | null,
  ): Promise<BrowserSolution>;
  createStudy(
    networkJson: string,
    formulation?: Formulation,
  ): Promise<BrowserStudy>;
}

export const browserWasmTransport: EngineTransport = {
  preloadCore,
  preloadSensitivity,
  ingestCase,
  parseDisplay,
  capabilities,
  solveJson,
  solveDc,
  createStudy,
};

export function createBrowserWasmTransport(): EngineTransport {
  return browserWasmTransport;
}

export { BrowserStudy as Study };
