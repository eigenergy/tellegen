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

export interface QueryNetworkInput {
  caseId: string;
  elementKind: ElementKind;
  elementIds?: string[];
  sortBy?:
    | "id"
    | "demand_mw"
    | "generation_mw"
    | "price"
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
export interface TellegenWebMcpAdapter {
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

export interface ToolAnnotations {
  readOnlyHint?: boolean;
  untrustedContentHint?: boolean;
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
    };

export interface TellegenToolDefinition {
  name: string;
  title?: string;
  description: string;
  inputSchema: Record<string, unknown>;
  execute(
    input: Record<string, unknown>,
    options?: ToolExecuteOptions,
  ): Promise<ToolResponse>;
  annotations: ToolAnnotations;
}

export type ToolResponse =
  | { ok: true; data: ToolPayload }
  | { ok: false; error: { code: string; message: string } };

export interface ModelContextLike {
  registerTool(
    tool: TellegenToolDefinition,
    options?: { signal?: AbortSignal },
  ): Promise<void>;
}

export interface RegistrationHandle {
  supported: boolean;
  toolNames: string[];
  /** Most recent dynamic registration failure, if one occurred after setup. */
  readonly registrationError: Error | null;
  dispose(): void;
}

export class TellegenToolError extends Error {
  readonly code: string;

  constructor(code: string, message: string) {
    super(message);
    this.name = "TellegenToolError";
    this.code = code;
  }
}
