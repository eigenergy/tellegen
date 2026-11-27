/** Experimental: one thread of the RMS engine's pools, in the build with
 * SIMD and threads. The RMS worker (rms-worker.ts) starts it with its
 * compiled module, its shared memory and the thread's task; the thread
 * instantiates the module on that memory, says it is ready and runs the
 * task (a pool's worker loop), which does not return. */

/** The parts of the threaded engine build (`wasm-pkg-mt`) a thread calls.
 * That build is optional and made apart from `npm run wasm`, so it is loaded
 * by URL at run time and not type-checked against. */
interface ThreadBuild {
  default(init: { module_or_path: WebAssembly.Module; memory: WebAssembly.Memory }): Promise<unknown>;
  rms_thread_entry(ptr: number): void;
}

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
  const mt = (await import(
    /* @vite-ignore */ new URL("./wasm-pkg-mt/tellegen.js", import.meta.url).href
  )) as ThreadBuild;
  await mt.default({ module_or_path: module, memory });
  scope.postMessage("ready");
  mt.rms_thread_entry(ptr);
};
