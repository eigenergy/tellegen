import {
  createTellegenDebugTool,
  createTellegenPlanningTools,
  createTellegenStudyTools,
  createTellegenTools,
  DEFAULT_OUTPUT_BUDGET,
  DEFAULT_TOOL_TIMEOUT_MS,
  type CreateTellegenToolsOptions,
} from "./tools.js";
import {
  TellegenRegistrationError,
  type ModelContextLike,
  type RegisterToolOptions,
  type RegistrationHandle,
  type TellegenToolDefinition,
  type TellegenWebMcpAdapter,
} from "./types.js";

export interface RegisterOptions extends CreateTellegenToolsOptions {
  signal?: AbortSignal;
  /** Reports a dynamic registration error, then `null` after a later retry succeeds. */
  onRegistrationError?: (error: Error | null) => void;
  /**
   * Origins, other than this document's, allowed to discover and call the
   * tools: for example the page that embeds tellegen in an iframe. The
   * embedding page must also opt in from its side. Omit to keep the tools
   * same-origin.
   */
  exposedTo?: string[];
  /**
   * Register `inspect_webmcp_registration`, a read-only tool annotated
   * `debugging` that reports the registered tools, the last registration
   * error and the WebMCP features the browser offers. Defaults to false.
   */
  debugTool?: boolean;
}

function asError(error: unknown): Error {
  return error instanceof Error ? error : new Error(String(error));
}

async function registerOne(
  modelContext: ModelContextLike,
  tool: TellegenToolDefinition,
  options: RegisterToolOptions,
): Promise<void> {
  try {
    await modelContext.registerTool(tool, options);
  } catch (error) {
    throw new TellegenRegistrationError(tool.name, error);
  }
}

/** The optional WebMCP features a model context offers. */
export function modelContextFeatures(
  modelContext: ModelContextLike,
): Record<string, boolean> {
  const has = (name: string) =>
    typeof (modelContext as unknown as Record<string, unknown>)[name] ===
    "function";
  return {
    register_tool: has("registerTool"),
    get_tools: has("getTools"),
    execute_tool: has("executeTool"),
    events: has("addEventListener"),
  };
}

/**
 * One dynamically registered tool group: registered while `available()` is
 * true, dropped (its per-group signal aborted) while it is false. The
 * lifecycle signal ends the group either way.
 */
class DynamicGroup {
  private controller: AbortController | null = null;
  private abortLifecycle: (() => void) | null = null;
  private registered = false;

  constructor(
    private readonly modelContext: ModelContextLike,
    private readonly tools: TellegenToolDefinition[],
    private readonly lifecycle: AbortSignal,
    private readonly exposedTo: string[] | undefined,
  ) {}

  async sync(available: boolean): Promise<void> {
    if (this.lifecycle.aborted) return;
    if (available && !this.registered) {
      const controller = new AbortController();
      const abort = () => controller.abort();
      this.lifecycle.addEventListener("abort", abort, { once: true });
      this.controller = controller;
      this.abortLifecycle = abort;
      this.registered = true;
      try {
        for (const tool of this.tools) {
          await registerOne(this.modelContext, tool, {
            signal: controller.signal,
            ...(this.exposedTo ? { exposedTo: this.exposedTo } : {}),
          });
        }
      } catch (error) {
        // The group signal removes any tools registered before the failure.
        controller.abort();
        this.lifecycle.removeEventListener("abort", abort);
        this.registered = false;
        this.controller = null;
        this.abortLifecycle = null;
        throw asError(error);
      }
    } else if (!available && this.registered) {
      this.controller?.abort();
      if (this.abortLifecycle) {
        this.lifecycle.removeEventListener("abort", this.abortLifecycle);
      }
      this.controller = null;
      this.abortLifecycle = null;
      this.registered = false;
    }
  }

  names(): string[] {
    return this.registered ? this.tools.map((tool) => tool.name) : [];
  }
}

/**
 * Register the tellegen tools with one lifecycle signal. The general
 * inspection and edit tools register unconditionally. When the adapter carries
 * the planning capability, `propose_capacity_plan` registers while planning is
 * available and `apply_capacity_plan` only while a current proposal is staged.
 */
export async function registerTellegenWebMcp(
  modelContext: ModelContextLike,
  adapter: TellegenWebMcpAdapter,
  options: RegisterOptions = {},
): Promise<RegistrationHandle> {
  const lifecycle = new AbortController();
  const abort = () => lifecycle.abort(options.signal?.reason);
  if (options.signal?.aborted) abort();
  else options.signal?.addEventListener("abort", abort, { once: true });
  const toolOptions = {
    outputBudget: options.outputBudget,
    timeoutMs: options.timeoutMs,
    onActivity: options.onActivity,
    recordValidatedInput: options.recordValidatedInput,
  };
  const exposedTo = options.exposedTo ? [...options.exposedTo] : undefined;
  const registration: RegisterToolOptions = {
    signal: lifecycle.signal,
    ...(exposedTo ? { exposedTo } : {}),
  };
  const tools = [
    ...createTellegenTools(adapter, toolOptions),
    ...(adapter.studies
      ? createTellegenStudyTools(adapter.studies, toolOptions)
      : []),
  ];
  let dynamicNames: () => string[] = () => [];
  let unsubscribe: () => void = () => {};
  let registrationError: Error | null = null;
  const reportRegistrationError = (error: unknown) => {
    registrationError = asError(error);
    options.onRegistrationError?.(registrationError);
  };
  const allNames = () => [...tools.map((tool) => tool.name), ...dynamicNames()];
  if (options.debugTool) {
    tools.push(
      createTellegenDebugTool(
        () => ({
          tools: allNames(),
          registration_error: registrationError?.message ?? null,
          exposed_to: exposedTo ?? [],
          output_budget: toolOptions.outputBudget ?? DEFAULT_OUTPUT_BUDGET,
          timeout_ms: toolOptions.timeoutMs ?? DEFAULT_TOOL_TIMEOUT_MS,
          model_context: modelContextFeatures(modelContext),
        }),
        toolOptions,
      ),
    );
  }
  try {
    for (const tool of tools) {
      await registerOne(modelContext, tool, registration);
    }
    const planning = adapter.planning;
    if (planning) {
      const groups = createTellegenPlanningTools(planning, toolOptions);
      const planningGroup = new DynamicGroup(
        modelContext,
        groups.planning,
        lifecycle.signal,
        exposedTo,
      );
      const proposalGroup = new DynamicGroup(
        modelContext,
        groups.proposal,
        lifecycle.signal,
        exposedTo,
      );
      // Serialize syncs: availability can change while a registration is in
      // flight, and interleaved register/abort calls on one group would race.
      let tail: Promise<void> = Promise.resolve();
      const sync = () => {
        tail = tail
          .catch(() => undefined)
          .then(async () => {
            await planningGroup.sync(planning.planningAvailable());
            await proposalGroup.sync(
              planning.planningAvailable() && planning.proposalAvailable(),
            );
            if (registrationError) {
              registrationError = null;
              options.onRegistrationError?.(null);
            }
          });
        return tail;
      };
      // A host can reject a conditional tool even though the general tools
      // registered. Keep those tools alive and retry on the next availability
      // pulse instead of failing the whole WebMCP lifecycle.
      await sync().catch(reportRegistrationError);
      unsubscribe = planning.onAvailabilityChange(() => {
        void sync().catch(reportRegistrationError);
      });
      lifecycle.signal.addEventListener("abort", () => unsubscribe(), {
        once: true,
      });
      dynamicNames = () => [...planningGroup.names(), ...proposalGroup.names()];
    }
  } catch (error) {
    lifecycle.abort(error);
    options.signal?.removeEventListener("abort", abort);
    unsubscribe();
    throw error;
  }
  return {
    supported: true,
    get toolNames() {
      return allNames();
    },
    get registrationError() {
      return registrationError;
    },
    dispose() {
      lifecycle.abort();
      options.signal?.removeEventListener("abort", abort);
    },
  };
}

/** Feature-detect the current document without adding a browser-global fallback. */
export function documentModelContext(
  document: Document,
): ModelContextLike | null {
  const candidate = (document as Document & { modelContext?: ModelContextLike })
    .modelContext;
  return candidate && typeof candidate.registerTool === "function"
    ? candidate
    : null;
}

/** Progressive enhancement helper. Unsupported browsers receive a no-op handle. */
export async function registerDocumentTellegenWebMcp(
  document: Document,
  adapter: TellegenWebMcpAdapter,
  options: RegisterOptions = {},
): Promise<RegistrationHandle> {
  const modelContext = documentModelContext(document);
  if (!modelContext) {
    return {
      supported: false,
      toolNames: [],
      registrationError: null,
      dispose() {},
    };
  }
  return registerTellegenWebMcp(modelContext, adapter, options);
}
