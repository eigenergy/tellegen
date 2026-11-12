export type MaybePromise<T> = T | Promise<T>;

export type JsonPrimitive = string | number | boolean | null;
export type JsonValue =
  JsonPrimitive | JsonValue[] | { [key: string]: JsonValue };
export type ToolPayload = { [key: string]: JsonValue };

export type ElementKind = "bus" | "branch";
export type SortDirection = "asc" | "desc";
export type EditMode = "set" | "increment";

export interface ElementTarget {
  kind: ElementKind;
  elementId: string;
}

export interface ListCasesInput {
  offset: number;
  limit: number;
}

export interface SelectCaseInput {
  caseId: string;
  expectedRevision?: string;
}

export interface SolveMulticonductorInput {
  caseId: string;
  expectedRevision: string;
  maxIterations?: number;
}

export interface TellegenCaseAdapter {
  listCases(
    input: ListCasesInput,
    signal: AbortSignal,
  ): MaybePromise<ToolPayload>;
  selectCase(
    input: SelectCaseInput,
    signal: AbortSignal,
  ): MaybePromise<ToolPayload>;
}

export interface QueryNetworkInput {
  caseId: string;
  elementKind: ElementKind;
  elementIds?: string[];
  sortBy?:
    | "id"
    | "demand_mw"
    | "generation_mw"
    | "price"
    | "voltage_pu"
    | "voltage_v"
    | "loading"
    | "flow_mw"
    | "rating_mw";
  direction: SortDirection;
  limit: number;
}

export interface AnalyzeSensitivityInput {
  caseId: string;
  target: ElementTarget;
  limit: number;
}

export interface FocusNetworkInput {
  caseId: string;
  target: ElementTarget;
}

export interface DemandEdit {
  busId: string;
  deltaMw: number;
}

export interface RatingEdit {
  branchId: string;
  deltaMw: number;
}

export interface PreviewCaseUpdateInput {
  caseId: string;
  expectedRevision: string;
  mode: EditMode;
  demand: DemandEdit[];
  ratings: RatingEdit[];
  limit: number;
}

export interface UpdateCaseInput {
  caseId: string;
  expectedRevision: string;
  mode: EditMode;
  demand: DemandEdit[];
  ratings: RatingEdit[];
  formulation?: string;
}

export interface ResetCaseInput {
  caseId: string;
  expectedRevision: string;
}

export interface CapacityPlanBusWeight {
  busId: string;
  weight: number;
}

export interface ProposeCapacityPlanInput {
  caseId: string;
  expectedRevision: string;
  objective: {
    kind: "weighted_lmp";
    weights: CapacityPlanBusWeight[];
  };
  candidates: string[];
  maxIncreasePerBranchMw: number;
  budgetMw: number;
  incrementMw: number;
  maxChangedLines: number;
  exactSolveBudget: number;
}

export interface ApplyCapacityPlanInput {
  caseId: string;
  expectedRevision: string;
  proposalId: string;
}

/**
 * The optional differentiable planning capability of a tellegen host.
 *
 * Kept separate from the always available adapter verbs so registration can
 * be dynamic: the planning tools exist only while the active case and
 * formulation support implicit differentiation, and the apply tool only
 * while an unapplied proposal matches the current revision.
 */
export interface TellegenPlanningAdapter {
  proposeCapacityPlan(
    input: ProposeCapacityPlanInput,
    signal: AbortSignal,
  ): MaybePromise<ToolPayload>;
  applyCapacityPlan(
    input: ApplyCapacityPlanInput,
    signal: AbortSignal,
  ): MaybePromise<ToolPayload>;
  /** Whether the active case and formulation support planning right now. */
  planningAvailable(): boolean;
  /** Whether an unapplied proposal exists for the current revision. */
  proposalAvailable(): boolean;
  /**
   * Subscribe to availability changes so registration can follow the case.
   * Returns the unsubscribe function.
   */
  onAvailabilityChange(listener: () => void): () => void;
}

/**
 * The stable boundary between WebMCP and an interactive tellegen host.
 *
 * It has no dependency on the engine `Study` class or on Svelte. Host adapters
 * keep PowerIO modules as their portable input and persistence boundary.
 */
export type StudyToolName =
  | "create_study"
  | "inspect_study"
  | "revise_study_goal"
  | "branch_study"
  | "compare_study_states"
  | "propose_study"
  | "record_study_evidence"
  | "edit_demand"
  | "restore_base_case";

export interface TellegenStudyAdapter {
  inputSchema(name: StudyToolName): Record<string, unknown>;
  execute(
    name: StudyToolName,
    input: Record<string, unknown>,
    signal: AbortSignal,
  ): MaybePromise<ToolPayload>;
}

export interface ExportCaseInput {
  caseId: string;
  expectedRevision: string;
  format: string;
}

export interface ImportCaseInput {
  fileName: string;
  text: string;
}

/** One target `export_case` can write. `token` is the PowerIO format token. */
export interface ExportFormat {
  token: string;
  label: string;
}

/**
 * Case files: write the displayed case through PowerIO for the user, and open
 * case text an agent supplies the way a dropped file opens.
 */
export interface TellegenFileAdapter {
  /** The targets `export_case` offers, fixed when the tools are created. */
  exportFormats: readonly ExportFormat[];
  exportCase(
    input: ExportCaseInput,
    signal: AbortSignal,
  ): MaybePromise<ToolPayload>;
  importCase(
    input: ImportCaseInput,
    signal: AbortSignal,
  ): MaybePromise<ToolPayload>;
}

export interface TellegenWebMcpAdapter {
  settings?(
    input: SettingsInput,
    signal: AbortSignal,
  ): MaybePromise<ToolPayload>;
  /** Case discovery and display selection, when supported by the host. */
  cases?: TellegenCaseAdapter;
  solveMulticonductorPowerFlow?(
    input: SolveMulticonductorInput,
    signal: AbortSignal,
  ): MaybePromise<ToolPayload>;
  studies?: TellegenStudyAdapter;
  /** Case export and import, when the host provides them. */
  files?: TellegenFileAdapter;
  /**
   * The differentiable planning capability, when the host provides one.
   * Absent, the general inspection and edit tools register alone.
   */
  planning?: TellegenPlanningAdapter;
  inspectCase(signal: AbortSignal): MaybePromise<ToolPayload>;
  queryNetwork(
    input: QueryNetworkInput,
    signal: AbortSignal,
  ): MaybePromise<ToolPayload>;
  analyzeSensitivity(
    input: AnalyzeSensitivityInput,
    signal: AbortSignal,
  ): MaybePromise<ToolPayload>;
  focusNetwork(
    input: FocusNetworkInput,
    signal: AbortSignal,
  ): MaybePromise<ToolPayload>;
  previewCaseUpdate(
    input: PreviewCaseUpdateInput,
    signal: AbortSignal,
  ): MaybePromise<ToolPayload>;
  updateCase(
    input: UpdateCaseInput,
    signal: AbortSignal,
  ): MaybePromise<ToolPayload>;
  resetCase(
    input: ResetCaseInput,
    signal: AbortSignal,
  ): MaybePromise<ToolPayload>;
}

/** The WebMCP `ToolAnnotations` dictionary. Every member defaults to false. */
export interface ToolAnnotations {
  /** The tool reads state and changes nothing. */
  readOnlyHint?: boolean;
  /** The result can carry content from an untrusted source, such as a case file. */
  untrustedContentHint?: boolean;
  /** The tool takes a significant, real-world or irreversible action. */
  consequentialHint?: boolean;
  /** The tool serves debugging and developer tooling, not end users. */
  debugging?: boolean;
}

export interface ToolExecuteOptions {
  signal?: AbortSignal;
}

export type TellegenToolActivityEvent =
  | {
      type: "started";
      id: string;
      toolName: string;
      title: string;
      startedAt: number;
    }
  | {
      type: "finished";
      id: string;
      toolName: string;
      title: string;
      startedAt: number;
      finishedAt: number;
      /** Validated wire input, copied before the adapter runs. Invalid input is omitted. */
      input?: ToolPayload;
      response: ToolResponse;
      /**
       * The caller cancelled the call before it finished. The agent receives no
       * result then, so a committed change it never saw is visible here.
       */
      callerCancelled?: boolean;
    };

export interface TellegenToolDefinition {
  name: string;
  title?: string;
  description: string;
  inputSchema: Record<string, unknown>;
  /** JSON Schema of the `ToolResponse` envelope the tool resolves with. */
  outputSchema: Record<string, unknown>;
  execute(
    input: Record<string, unknown>,
    options?: ToolExecuteOptions,
  ): Promise<ToolResponse>;
  annotations: ToolAnnotations;
}

export type ToolResponse =
  | { ok: true; data: ToolPayload }
  | { ok: false; error: { code: string; message: string } };

/** Options of the WebMCP `registerTool()` call. */
export interface RegisterToolOptions {
  /** Unregisters the tool when aborted. */
  signal?: AbortSignal;
  /** Other origins allowed to discover and call the tool, such as an embedding page. */
  exposedTo?: string[];
}

/**
 * The part of the WebMCP `ModelContext` interface tellegen uses. `registerTool`
 * is required; the rest are feature-detected, because early implementations
 * ship `registerTool` alone.
 */
export interface ModelContextLike {
  registerTool(
    tool: TellegenToolDefinition,
    options?: RegisterToolOptions,
  ): Promise<void>;
  getTools?(options?: { fromOrigins?: string[] }): Promise<unknown[]>;
  executeTool?(
    tool: unknown,
    inputObject?: object,
    options?: { signal?: AbortSignal },
  ): Promise<string>;
  addEventListener?(
    type: "toolchange" | "toolactivated" | "toolcancel",
    listener: (event: Event & { toolName?: string }) => void,
    options?: AddEventListenerOptions,
  ): void;
  removeEventListener?(
    type: "toolchange" | "toolactivated" | "toolcancel",
    listener: (event: Event & { toolName?: string }) => void,
  ): void;
}

export interface RegistrationHandle {
  supported: boolean;
  toolNames: string[];
  /** Most recent dynamic registration failure, if one occurred after setup. */
  readonly registrationError: Error | null;
  dispose(): void;
}

/**
 * A `registerTool()` rejection, named by the WebMCP DOMException it carried:
 * `InvalidStateError` for a duplicate name or detached document,
 * `NotAllowedError` when a permissions policy disallows WebMCP, `SecurityError`
 * for an `exposedTo` origin the browser refuses, `TypeError` for a tool
 * definition it cannot serialize.
 */
export class TellegenRegistrationError extends Error {
  readonly toolName: string;
  readonly reason: string;

  constructor(toolName: string, cause: unknown) {
    const reason =
      cause instanceof Error && cause.name !== "Error" ? cause.name : "Error";
    const detail = cause instanceof Error ? cause.message : String(cause);
    const hint =
      reason === "InvalidStateError"
        ? "a tool of this name is already registered on this document, or the document is no longer active"
        : reason === "NotAllowedError"
          ? "WebMCP is not allowed on this document; check its permissions policy"
          : reason === "SecurityError"
            ? "the browser refused an exposedTo origin"
            : reason === "TypeError"
              ? "the browser could not serialize the tool definition"
              : "the browser rejected the registration";
    super(`${toolName}: ${hint}${detail ? ` (${detail})` : ""}`, { cause });
    this.name = "TellegenRegistrationError";
    this.toolName = toolName;
    this.reason = reason;
  }
}

export class TellegenToolError extends Error {
  readonly code: string;

  constructor(code: string, message: string) {
    super(message);
    this.name = "TellegenToolError";
    this.code = code;
  }
}

export interface SettingsInput {
  action: "inspect" | "update" | "reset" | "run";
  scope: "calculation" | "display";
  caseId?: string;
  expectedRevision?: string;
  settingsRevision?: number;
  calculation?: string;
  field?: string;
  patch?: Record<string, unknown>;
}
