/** Maximum size accepted by one public byte-buffer API. A buffer crosses the
 * worker boundary and is then copied into WebAssembly memory, so reject it on
 * the calling thread before either copy begins. */
export const MAX_ENGINE_INPUT_BYTES = 128 * 1024 * 1024;

const INPUT_TOO_LARGE = "input exceeds 128 MiB limit";

/** Length-only form keeps the boundary test allocation-free. */
export function assertEngineInputLength(byteLength: number): void {
  if (byteLength > MAX_ENGINE_INPUT_BYTES) throw new Error(INPUT_TOO_LARGE);
}

export function assertEngineInputBytes(bytes: Uint8Array): void {
  assertEngineInputLength(bytes.byteLength);
}
