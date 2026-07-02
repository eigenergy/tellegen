/** Lazy loader for the powerio wasm module on the calling thread, used by the
 * main-thread fallback host. The worker entry has its own loader (static
 * imports keep the worker a single chunk). Nothing downloads until the first
 * engine call; dropped files are parsed locally and never leave the machine. */

import { errorText, isPermanentWasmLoadFailure } from "./errors.js";

export type WasmModule = typeof import("./wasm-pkg/tellegen.js");
export type WasmStudy = InstanceType<WasmModule["Study"]>;

let engineReady: Promise<WasmModule> | null = null;
let engineUnsupported: string | null = null;

/** The one wasm module (Study, all formulations, sensitivities). Resolve the
 * wasm asset only when the engine is used: SvelteKit's dev mode SSR pass can
 * evaluate this module without touching the wasm loader. */
export function engineModule(): Promise<WasmModule> {
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
