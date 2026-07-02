/** Lazy loader for the powerio wasm module. Nothing downloads until the
 * first file is dropped; the dropped file is parsed in the browser and never
 * leaves the machine. */
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
} from "./generated/contracts.js";
export {
  CONTRACT_SOURCE_SHA256,
  CONTRACT_VERSION,
  FORMULATION_IDS,
  SOLVE_STATUSES,
} from "./generated/contracts.js";

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

let engineReady: Promise<typeof import("./wasm-pkg/tellegen.js")> | null =
  null;
let engineUnsupported: string | null = null;

/** The one wasm module (Study, all formulations, sensitivities). Resolve the
 * wasm asset only when the engine is used: SvelteKit's dev mode SSR pass can
 * evaluate this module without touching the wasm loader. */
function engineModule() {
  if (engineUnsupported) return Promise.reject(new Error(engineUnsupported));
  const wasmUrl = new URL("./wasm-pkg/tellegen_bg.wasm", import.meta.url).href;
  engineReady ??= import("./wasm-pkg/tellegen.js")
    .then(async (mod) => {
      await mod.default({ module_or_path: wasmUrl });
      return mod;
    })
    .catch((e) => {
      const message = errorText(e);
      // Don't cache a rejected load: a transient failure (chunk fetch or
      // instantiate) must not disable the engine for the whole session.
      engineReady = null;
      if (isPermanentWasmLoadFailure(message)) engineUnsupported = message;
      throw e;
    });
  return engineReady;
}

export async function preloadEngine(): Promise<void> {
  await engineModule();
}

function isPermanentWasmLoadFailure(message: string): boolean {
  // Latch only genuine browser-capability failures the engine module can
  // never recover from in this browser: no WebAssembly, or an opcode the
  // engine rejects. Transient fetch or
  // instantiate failures (offline, 503, aborted navigation) routinely carry
  // the .wasm URL or "Failed to fetch" in their message, so keying on the bare
  // word "wasm"/"compile" wrongly disables the engine for the whole session.
  // Those stay retryable.
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
  return JSON.parse((await engineModule()).ingest_case(text, format));
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
  return JSON.parse((await engineModule()).parse_display(bytes, "pwd"));
}

export async function capabilities(): Promise<ProblemCaps[]> {
  return JSON.parse((await engineModule()).capabilities_json());
}

export async function solveJson(
  networkJson: string,
  request: SolveRequest = {},
): Promise<SolveResponse> {
  return JSON.parse(
    (await engineModule()).solve_json(networkJson, JSON.stringify(request)),
  );
}

export function errorText(e: unknown): string {
  return e instanceof Error ? e.message : String(e);
}

/** True when the engine wasm module has failed to load in a way it can never
 * recover from in this browser. Nothing solves client side when this is true;
 * the caller decides whether a server fallback exists. */
export function isPermanentEngineFailure(message: string): boolean {
  return isPermanentWasmLoadFailure(message);
}

/** One in-place network mutation for the Study handle: a demand delta in MW at
 * an original bus id, or a thermal rating delta in MW at an original branch id. */
type NetworkEdit =
  | { kind: "add_load"; bus: number; p_mw: number }
  | { kind: "adjust_branch_rating"; branch: number; delta_mw: number };

/** A `NetworkEdit[]` for the wasm Study, dropping zero deltas so an unchanged
 * element is never sent. */
function toEdits(deltas: DemandDeltas, rates: BranchRatingDeltas): NetworkEdit[] {
  const edits: NetworkEdit[] = Object.entries(deltas)
    .filter(([, mw]) => mw !== 0)
    .map(([bus, p_mw]) => ({ kind: "add_load", bus: Number(bus), p_mw }));
  for (const [branch, delta_mw] of Object.entries(rates)) {
    if (delta_mw !== 0) {
      edits.push({ kind: "adjust_branch_rating", branch: Number(branch), delta_mw });
    }
  }
  return edits;
}

/** The sensitivity selection target: a bus (the ∂LMP/∂d column at that bus) or a
 * branch (the ∂LMP/∂rating column at that branch). Ids are the external element
 * ids; `BrowserStudy` translates them to the engine's dense indices. */
export type SensTarget = { bus: number } | { branch: number };

/** The (operand, parameter) cells the UI drives: a study whose sensitivity column is
 * ∂(price, active) / ∂(demand, active) for a bus target, or ∂(price, active) /
 * ∂(line limit) for a branch target. The *formulation* is no longer fixed here — it is
 * a parameter threaded from the UI's selector through `createStudy` (every formulation the
 * full wasm build carries returns LMP, so these columns apply to all of them). The
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

/** The `SensRequest[]` JSON for `Study.commit`: the ∂LMP/∂demand column at a dense bus
 * index, or the ∂LMP/∂rating column at a dense branch index, or `[]` when there is no
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
 * tagged `ElementId`, e.g. `{ Bus: id }` / `{ Branch: id }`). */
interface SensitivityMatrixJson {
  values: number[][];
  rows: { element: { Bus?: number; Branch?: number }; index: number }[];
  cols: { element: { Bus?: number; Branch?: number }; index: number }[];
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

/** Extract the ∂LMP/∂parameter column from the first requested `SensitivityMatrix` into
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
  const shared = { case: caseId, operand: "lmp", units: m.units, values };
  if (el.Bus !== undefined) {
    return { ...shared, parameter: "d", bus: el.Bus };
  }
  if (el.Branch !== undefined) {
    return { ...shared, parameter: "fmax", branch: el.Branch };
  }
  return null;
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
 * network. */
export class BrowserStudy {
  #study: import("./wasm-pkg/tellegen.js").Study;
  /** External bus id -> dense positional bus index, built once from the committed solution's
   * LMP ordering (each `lmp[i].bus` sits at dense index `i`). The engine keys `SensRequest`
   * by this dense index, not the external bus id, so the selected bus must be translated
   * before a sensitivity request. Null until first needed; the bus set is solve-invariant. */
  #busToIndex: Map<number, number> | null = null;
  /** External branch id -> dense positional branch index, from the committed solution's
   * flows ordering — the branch-axis counterpart of `#busToIndex`. */
  #branchToIndex: Map<number, number> | null = null;

  constructor(study: import("./wasm-pkg/tellegen.js").Study) {
    this.#study = study;
  }

  /** The dense-axis sensitivity target the engine expects for a selection (external
   * ids), or null when nothing is selected or the id is unknown. Memoizes the
   * id->index maps from the committed solution's LMP / flows order — the same axis
   * orders the engine's dense sensitivity columns use. */
  #senseTarget(
    target: SensTarget | null,
  ): { kind: "bus" | "branch"; index: number } | null {
    if (target === null) return null;
    const sol = () => JSON.parse(this.#study.solution()) as StudySolveResponse;
    if ("bus" in target) {
      if (!this.#busToIndex) {
        this.#busToIndex = new Map((sol().lmp ?? []).map((e, i) => [e.bus, i]));
      }
      const index = this.#busToIndex.get(target.bus);
      return index === undefined ? null : { kind: "bus", index };
    }
    if (!this.#branchToIndex) {
      this.#branchToIndex = new Map(
        (sol().flows ?? []).map((f, i) => [f.branch, i]),
      );
    }
    const index = this.#branchToIndex.get(target.branch);
    return index === undefined ? null : { kind: "branch", index };
  }

  /** Exact solve at demand = base + `deltas` and ratings = base + `rates`, replacing
   * the committed point. When `target` names a bus or branch, its ∂LMP/∂parameter
   * column is computed in the *same* solve and returned (no second round-trip);
   * otherwise `sensitivity` is null. `caseId` labels the returned column. Returns the
   * UI Solution, the solver iterates, and the column. */
  commit(
    caseId: string,
    deltas: DemandDeltas,
    rates: BranchRatingDeltas,
    target: SensTarget | null,
  ): {
    solution: Solution;
    iterations: SolveIteration[];
    sensitivity: SensitivityColumn | null;
  } {
    const out: StudyCommitOutput = JSON.parse(
      this.#study.replace_edits(
        JSON.stringify(toEdits(deltas, rates)),
        sensitivitiesJson(this.#senseTarget(target)),
      ),
    );
    return {
      solution: solveResponseToSolution(out.solution),
      iterations: out.iterations ?? [],
      sensitivity:
        target === null ? null : sensitivityColumn(caseId, out.sensitivities),
    };
  }

  /** The ∂LMP/∂parameter column at `target` for this study's formulation, computed by
   * an exact re-solve at the edited point (the same one-call path `commit` uses, but
   * returning only the column). Used when a bus or branch is selected so the overlay
   * matches the active formulation rather than always the DC sensitivity. `caseId`
   * labels the column; returns null when no recognized column comes back. */
  sensitivity(
    caseId: string,
    deltas: DemandDeltas,
    rates: BranchRatingDeltas,
    target: SensTarget,
  ): SensitivityColumn | null {
    const out: StudyCommitOutput = JSON.parse(
      this.#study.replace_edits(
        JSON.stringify(toEdits(deltas, rates)),
        sensitivitiesJson(this.#senseTarget(target)),
      ),
    );
    return sensitivityColumn(caseId, out.sensitivities);
  }

  /** First-order LMP preview for replacing the committed point with
   * demand = base + `deltas` and ratings = base + `rates`, with no re-solve:
   * predicted per-bus ΔLMP and the predicted Δobjective. */
  preview(
    deltas: DemandDeltas,
    rates: BranchRatingDeltas = {},
  ): {
    lmp: { bus: number; usd_per_mwh: number }[];
    objectiveDelta: number | null;
  } {
    const out: StudyPreview = JSON.parse(
      this.#study.preview_replacement(
        JSON.stringify(toEdits(deltas, rates)),
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
 * Throws if the engine module can't load (or the formulation is unknown/not built); the
 * caller must catch and fall back (see `isPermanentEngineFailure`). */
export async function createStudy(
  networkJson: string,
  formulation: Formulation = DEFAULT_FORMULATION,
): Promise<BrowserStudy> {
  const mod = await engineModule();
  return new BrowserStudy(new mod.Study(networkJson, formulation));
}

export interface EngineTransport {
  preloadEngine(): Promise<void>;
  ingestCase(text: string, format: string): Promise<IngestedCase>;
  parseDisplay(bytes: Uint8Array): Promise<DisplayPreview>;
  capabilities(): Promise<ProblemCaps[]>;
  solveJson(networkJson: string, request?: SolveRequest): Promise<SolveResponse>;
  createStudy(
    networkJson: string,
    formulation?: Formulation,
  ): Promise<BrowserStudy>;
}

export const browserWasmTransport: EngineTransport = {
  preloadEngine,
  ingestCase,
  parseDisplay,
  capabilities,
  solveJson,
  createStudy,
};

export function createBrowserWasmTransport(): EngineTransport {
  return browserWasmTransport;
}

export { BrowserStudy as Study };
