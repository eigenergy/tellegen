/** Error helpers shared by the main-thread loader and the worker entry. Kept
 * separate from module.ts so the worker bundle never touches the dynamic
 * import there: the worker must stay a single chunk (Vite's default worker
 * format cannot code-split). */

export function errorText(e: unknown): string {
  return e instanceof Error ? e.message : String(e);
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
