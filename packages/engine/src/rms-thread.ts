/** Experimental: one thread of the RMS engine's pools, in the build with
 * SIMD and threads. The RMS worker (rms-worker.ts) starts it with its
 * compiled module, its shared memory and the thread's task; the thread
 * instantiates the module on that memory, says it is ready and runs the
 * task (a pool's worker loop), which does not return. */

import * as mt from "./wasm-pkg-mt/tellegen.js";

interface Start {
  module: WebAssembly.Module;
  memory: WebAssembly.Memory;
  ptr: number;
}

const scope = globalThis as unknown as {
  onmessage: ((ev: MessageEvent<Start>) => void) | null;
  postMessage(msg: unknown): void;
};

scope.onmessage = async (ev: MessageEvent<Start>) => {
  const { module, memory, ptr } = ev.data;
  await mt.default({ module_or_path: module, memory });
  scope.postMessage("ready");
  mt.rms_thread_entry(ptr);
};
