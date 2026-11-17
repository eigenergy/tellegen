/** Error helpers shared by the main-thread loader and the worker entry. Kept
 * separate from module.ts so the worker bundle never touches the dynamic
 * import there: the worker must stay a single chunk (Vite's default worker
 * format cannot code-split). */

import {
  ENGINE_ERROR_CODES,
  type EngineErrorCode,
} from "./generated/api-types.js";

export function errorText(e: unknown): string {
  return e instanceof Error ? e.message : String(e);
}

/** An error thrown by the engine. `code` names the class of failure when the
 * engine knows it; branch on it instead of matching the message text. */
export interface EngineError extends Error {
  code?: EngineErrorCode;
}

/** The engine's code on a thrown value, or undefined when it carries none
 * (an error raised outside the engine, or a failure the engine does not
 * classify). */
export function errorCode(e: unknown): EngineErrorCode | undefined {
  if (typeof e !== "object" || e === null) return undefined;
  const code = (e as { code?: unknown }).code;
  return (ENGINE_ERROR_CODES as readonly unknown[]).includes(code)
    ? (code as EngineErrorCode)
    : undefined;
}

/** An Error with `message` and, when given, the engine's `code`. Used to
 * rebuild an engine error on the far side of a worker message. */
export function engineError(
  message: string,
  code?: EngineErrorCode,
): EngineError {
  const error: EngineError = new Error(message);
  if (code !== undefined) error.code = code;
  return error;
}

export function isPermanentWasmLoadFailure(message: string): boolean {
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
