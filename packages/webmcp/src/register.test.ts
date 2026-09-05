import { describe, expect, it, vi } from "vitest";
import {
  documentModelContext,
  registerDocumentTellegenWebMcp,
  registerTellegenWebMcp,
} from "./register.js";
import type {
  ModelContextLike,
  TellegenToolDefinition,
  TellegenWebMcpAdapter,
} from "./types.js";

const adapter: TellegenWebMcpAdapter = {
  inspectCase: () => ({}),
  queryNetwork: () => ({}),
  analyzeSensitivity: () => ({}),
  focusNetwork: () => ({}),
  previewCaseUpdate: () => ({}),
  updateCase: () => ({}),
  resetCase: () => ({}),
};

describe("registerTellegenWebMcp", () => {
  it("registers every tool under one abortable lifecycle", async () => {
    const registered: Array<{
      tool: TellegenToolDefinition;
      signal?: AbortSignal;
    }> = [];
    const context: ModelContextLike = {
      async registerTool(tool, options) {
        registered.push({ tool, signal: options?.signal });
      },
    };
    const handle = await registerTellegenWebMcp(context, adapter);
    expect(handle.toolNames).toHaveLength(7);
    expect(handle.registrationError).toBeNull();
    expect(new Set(registered.map((entry) => entry.signal))).toHaveLength(1);
    expect(registered[0].signal?.aborted).toBe(false);
    handle.dispose();
    expect(registered[0].signal?.aborted).toBe(true);
  });

  it("rolls back registrations if a later registration fails", async () => {
    const signals: AbortSignal[] = [];
    let count = 0;
    const context: ModelContextLike = {
      async registerTool(_tool, options) {
        if (options?.signal) signals.push(options.signal);
        count += 1;
        if (count === 3) throw new Error("duplicate tool");
      },
    };
    await expect(registerTellegenWebMcp(context, adapter)).rejects.toThrow(
      "duplicate tool",
    );
    expect(signals.every((entry) => entry.aborted)).toBe(true);
  });
});

describe("document registration", () => {
  it("uses progressive feature detection without a navigator fallback", async () => {
    const plain = {} as Document;
    expect(documentModelContext(plain)).toBeNull();
    const handle = await registerDocumentTellegenWebMcp(plain, adapter);
    expect(handle).toMatchObject({ supported: false, toolNames: [] });

    const registerTool = vi.fn(async () => {});
    const supported = { modelContext: { registerTool } } as unknown as Document;
    expect(documentModelContext(supported)).not.toBeNull();
    const live = await registerDocumentTellegenWebMcp(supported, adapter);
    expect(live.supported).toBe(true);
    expect(registerTool).toHaveBeenCalledTimes(7);
    live.dispose();
  });
});

describe("dynamic planning registration", () => {
  type FakePlanning = TellegenWebMcpAdapter["planning"] & object;

  function planningCapability(initial: {
    planning: boolean;
    proposal: boolean;
  }): {
    capability: FakePlanning;
    set: (next: { planning?: boolean; proposal?: boolean }) => void;
  } {
    const state = { ...initial };
    const listeners = new Set<() => void>();
    const capability = {
      proposeCapacityPlan: () => ({}),
      applyCapacityPlan: () => ({}),
      planningAvailable: () => state.planning,
      proposalAvailable: () => state.proposal,
      onAvailabilityChange(listener: () => void) {
        listeners.add(listener);
        return () => listeners.delete(listener);
      },
    };
    return {
      capability,
      set(next) {
        Object.assign(state, next);
        for (const listener of listeners) listener();
      },
    };
  }

  function trackingContext(): {
    context: ModelContextLike;
    live: () => string[];
  } {
    const registrations = new Map<string, AbortSignal>();
    const context: ModelContextLike = {
      async registerTool(tool, options) {
        registrations.set(tool.name, options?.signal as AbortSignal);
      },
    };
    return {
      context,
      live: () =>
        [...registrations.entries()]
          .filter(([, signal]) => !signal.aborted)
          .map(([name]) => name)
          .sort(),
    };
  }

  const settle = () => new Promise((resolve) => setTimeout(resolve, 0));

  it("registers plan tools only while the capability supports them", async () => {
    const { capability, set } = planningCapability({
      planning: false,
      proposal: false,
    });
    const { context, live } = trackingContext();
    const handle = await registerTellegenWebMcp(context, {
      ...adapter,
      planning: capability,
    });
    expect(live()).not.toContain("propose_capacity_plan");
    expect(handle.toolNames).toHaveLength(7);

    set({ planning: true });
    await settle();
    expect(live()).toContain("propose_capacity_plan");
    expect(live()).not.toContain("apply_capacity_plan");
    expect(handle.toolNames).toHaveLength(8);

    set({ proposal: true });
    await settle();
    expect(live()).toContain("apply_capacity_plan");
    expect(handle.toolNames).toHaveLength(9);

    // A revision change invalidates the proposal: apply drops alone.
    set({ proposal: false });
    await settle();
    expect(live()).not.toContain("apply_capacity_plan");
    expect(live()).toContain("propose_capacity_plan");

    // Case teardown drops the whole planning group.
    set({ planning: false });
    await settle();
    expect(live()).not.toContain("propose_capacity_plan");

    handle.dispose();
  });

  it("tears down dynamic tools with the lifecycle", async () => {
    const { capability } = planningCapability({
      planning: true,
      proposal: true,
    });
    const { context, live } = trackingContext();
    const handle = await registerTellegenWebMcp(context, {
      ...adapter,
      planning: capability,
    });
    await settle();
    expect(live()).toContain("apply_capacity_plan");
    handle.dispose();
    expect(live()).toHaveLength(0);
  });

  it("surfaces a dynamic registration failure and keeps base tools live", async () => {
    const { capability, set } = planningCapability({
      planning: false,
      proposal: false,
    });
    const live = new Map<string, AbortSignal>();
    const errors: Array<Error | null> = [];
    const handle = await registerTellegenWebMcp(
      {
        async registerTool(tool, options) {
          if (tool.name === "propose_capacity_plan") {
            throw new Error("planning registration refused");
          }
          live.set(tool.name, options?.signal as AbortSignal);
        },
      },
      { ...adapter, planning: capability },
      { onRegistrationError: (error) => errors.push(error) },
    );

    set({ planning: true });
    await settle();
    expect(errors).toHaveLength(1);
    expect(handle.registrationError?.message).toBe(
      "planning registration refused",
    );
    expect(
      [...live.entries()].filter(([, signal]) => !signal.aborted),
    ).toHaveLength(7);
    handle.dispose();
  });

  it("keeps base tools when initial planning registration fails and retries", async () => {
    const { capability, set } = planningCapability({
      planning: true,
      proposal: false,
    });
    const live = new Map<string, AbortSignal>();
    const errors: Array<Error | null> = [];
    let rejectPlanning = true;
    const handle = await registerTellegenWebMcp(
      {
        async registerTool(tool, options) {
          if (tool.name === "propose_capacity_plan" && rejectPlanning) {
            throw new Error("initial planning registration refused");
          }
          live.set(tool.name, options?.signal as AbortSignal);
        },
      },
      { ...adapter, planning: capability },
      { onRegistrationError: (error) => errors.push(error) },
    );

    expect(handle.supported).toBe(true);
    expect(handle.registrationError?.message).toBe(
      "initial planning registration refused",
    );
    expect(
      [...live.entries()].filter(([, signal]) => !signal.aborted),
    ).toHaveLength(7);

    rejectPlanning = false;
    set({ planning: true });
    await settle();
    expect(handle.registrationError).toBeNull();
    expect(handle.toolNames).toContain("propose_capacity_plan");
    expect(errors).toEqual([expect.any(Error), null]);
    handle.dispose();
  });
});
