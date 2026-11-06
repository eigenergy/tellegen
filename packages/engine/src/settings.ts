import { engineHost, isolatedEngineHost } from "./host.js";
import { assertEngineInputLength } from "./input-limit.js";
import type {
  SettingsRequest,
  ResolvedSettings,
  CalculationRequest,
  CalculationResult,
} from "./generated/settings-types.js";
export type {
  SettingsRequest,
  ResolvedSettings,
  CalculationRequest,
  CalculationResult,
};
export { settingsCatalog } from "./generated/settings-types.js";

export interface SettingDescriptor {
  key: string;
  label: string;
  description: string;
  unit: string;
  group: string;
  type: string;
  default: unknown;
  choices?: readonly string[];
  minimum?: number;
  maximum?: number;
  exclusive_minimum?: number;
}
export interface CalculationDescriptor {
  calculation: string;
  available: boolean;
  reason?: string;
  defaults?: Record<string, unknown>;
  fields: readonly SettingDescriptor[];
}
export interface SettingsCatalog {
  version: number;
  calculations: readonly CalculationDescriptor[];
}

function decode<T>(value: unknown): T {
  if (typeof value !== "string")
    throw new Error("engine returned no settings payload");
  return JSON.parse(value) as T;
}
/** Discover capabilities of the running engine, rather than another build. */
export async function describeSettings(): Promise<SettingsCatalog> {
  return decode(await engineHost().call({ op: "settings_describe" }));
}
/** Rust validates the full candidate; no selected case state changes here. */
function assertJson(value: unknown): void {
  if (value === null || typeof value === "string" || typeof value === "boolean")
    return;
  if (typeof value === "number" && Number.isFinite(value)) return;
  if (Array.isArray(value)) {
    value.forEach(assertJson);
    return;
  }
  if (typeof value === "object" && value) {
    Object.values(value).forEach(assertJson);
    return;
  }
  throw new Error("Every setting must have a valid finite JSON value");
}
export async function resolveSettings(
  request: SettingsRequest,
): Promise<ResolvedSettings> {
  assertJson(request);
  return decode(
    await engineHost().call({
      op: "settings_resolve",
      request: JSON.stringify(request),
    }),
  );
}
/** Run in an isolated worker so cancellation cannot terminate another case. */
export async function calculateWithSettings(
  request: CalculationRequest,
  signal?: AbortSignal,
): Promise<CalculationResult> {
  assertJson(request);
  assertEngineInputLength(request.module_json.length);
  signal?.throwIfAborted();
  const host = isolatedEngineHost();
  const cancel = () =>
    host.cancel?.(new DOMException("Calculation cancelled", "AbortError"));
  signal?.addEventListener("abort", cancel, { once: true });
  try {
    const result = decode<CalculationResult>(
      await host.call({
        op: "settings_calculate",
        request: JSON.stringify(request),
      }),
    );
    signal?.throwIfAborted();
    return result;
  } catch (error) {
    signal?.throwIfAborted();
    throw error;
  } finally {
    signal?.removeEventListener("abort", cancel);
    host.cancel?.(new Error("Calculation completed"));
  }
}

/** Same stateless contract over HTTP; no browser engine is loaded by this client. */
export function createSettingsHttpClient(
  apiBase = "/api",
  fetcher: typeof fetch = globalThis.fetch,
) {
  const base = apiBase.replace(/\/+$/, "");
  async function call<T>(
    path: string,
    request?: unknown,
    signal?: AbortSignal,
  ): Promise<T> {
    if (request !== undefined) assertJson(request);
    const response = await fetcher(
      `${base}${path}`,
      request === undefined
        ? { signal }
        : {
            method: "POST",
            headers: { "content-type": "application/json" },
            body: JSON.stringify(request),
            signal,
          },
    );
    if (!response.ok) {
      const body = await response.text();
      let message = body;
      try {
        message = JSON.parse(body).error ?? body;
      } catch {
        /* Non-JSON proxy response. */
      }
      throw new Error(`HTTP ${response.status}: ${message}`);
    }
    return response.json() as Promise<T>;
  }
  return {
    describe: (signal?: AbortSignal) =>
      call<SettingsCatalog>("/settings", undefined, signal),
    resolve: (request: SettingsRequest, signal?: AbortSignal) =>
      call<ResolvedSettings>("/settings/resolve", request, signal),
    calculate: (request: CalculationRequest, signal?: AbortSignal) =>
      call<CalculationResult>("/calculate", request, signal),
  };
}
