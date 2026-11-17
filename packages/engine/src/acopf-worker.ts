import { engineError, errorCode, errorText } from "./errors.js";
import type {
  AcOpfWorkerRequest,
  AcOpfWorkerResponse,
} from "./acopf-protocol.js";
import { createAcOpfWasi } from "./acopf-wasi.js";
import type { SolveResponse } from "./generated/api-types.js";

interface AcOpfExports extends WebAssembly.Exports {
  memory: WebAssembly.Memory;
  tellegen_acopf_alloc(len: number): number;
  tellegen_acopf_dealloc(ptr: number, len: number): void;
  tellegen_acopf_free_payload(ptr: number): void;
  tellegen_acopf_solve(
    modulePtr: number,
    moduleLen: number,
    requestPtr: number,
    requestLen: number,
  ): number;
}

const scope = globalThis as unknown as {
  onmessage: ((event: MessageEvent<AcOpfWorkerRequest>) => void) | null;
  postMessage(message: AcOpfWorkerResponse): void;
  close(): void;
};

const encoder = new TextEncoder();
const decoder = new TextDecoder();

/** Compiled modules by URL, kept for the life of the worker so that only the
 * first solve pays for the fetch and the compile. */
const modules = new Map<string, Promise<WebAssembly.Module>>();

async function fetchAndCompile(url: string): Promise<WebAssembly.Module> {
  const response = await fetch(url);
  if (!response.ok) {
    throw new Error(`AC OPF wasm fetch failed (HTTP ${response.status})`);
  }
  try {
    return await WebAssembly.compileStreaming(
      Promise.resolve(response.clone()),
    );
  } catch {
    return await WebAssembly.compile(await response.arrayBuffer());
  }
}

function compiled(url: string): Promise<WebAssembly.Module> {
  let module = modules.get(url);
  if (!module) {
    module = fetchAndCompile(url);
    modules.set(url, module);
    // A failed fetch or compile is not remembered: the next request retries.
    module.catch(() => modules.delete(url));
  }
  return module;
}

/** A fresh instance of the compiled module. Each solve gets its own memory, so
 * a solve that trapped or grew the heap leaves nothing behind for the next. */
async function instantiate(url: string): Promise<AcOpfExports> {
  const wasi = createAcOpfWasi((text) =>
    console.debug("[acopf]", text.trimEnd()),
  );
  const instance = await WebAssembly.instantiate(
    await compiled(url),
    wasi.imports,
  );
  wasi.bind(instance);
  const exports = instance.exports as AcOpfExports;
  for (const name of [
    "tellegen_acopf_alloc",
    "tellegen_acopf_dealloc",
    "tellegen_acopf_free_payload",
    "tellegen_acopf_solve",
  ] as const) {
    if (typeof exports[name] !== "function") {
      throw new Error(`AC OPF wasm is missing its ${name} export`);
    }
  }
  return exports;
}

function copyInput(exports: AcOpfExports, text: string): [number, number] {
  const bytes = encoder.encode(text);
  const ptr = exports.tellegen_acopf_alloc(bytes.length);
  if (!ptr) throw new Error("AC OPF wasm allocation failed");
  new Uint8Array(exports.memory.buffer, ptr, bytes.length).set(bytes);
  return [ptr, bytes.length];
}

function readPayload(exports: AcOpfExports, ptr: number): unknown {
  if (!ptr) throw new Error("AC OPF wasm returned no payload");
  const memory = new Uint8Array(exports.memory.buffer);
  if (ptr + 4 > memory.length)
    throw new Error("AC OPF wasm returned an invalid pointer");
  const len = new DataView(exports.memory.buffer).getUint32(ptr, true);
  if (ptr + 4 + len > memory.length) {
    throw new Error("AC OPF wasm returned an invalid payload length");
  }
  const text = decoder.decode(memory.subarray(ptr + 4, ptr + 4 + len));
  exports.tellegen_acopf_free_payload(ptr);
  return JSON.parse(text) as unknown;
}

scope.onmessage = async ({ data }: MessageEvent<AcOpfWorkerRequest>) => {
  try {
    const exports = await instantiate(data.wasmUrl);
    if (data.type === "probe") {
      scope.postMessage({ type: "ready" });
      return;
    }
    const moduleInput = copyInput(exports, data.moduleJson);
    const requestInput = copyInput(exports, data.requestJson);
    try {
      const value = readPayload(
        exports,
        exports.tellegen_acopf_solve(...moduleInput, ...requestInput),
      ) as { error?: unknown };
      if (typeof value?.error === "string") throw engineError(value.error, errorCode(value));
      if (JSON.parse(data.requestJson).ac_dc === true) {
        scope.postMessage({
          type: "joint_result",
          response: value as import("./dc-grid.js").AcDcSolution,
        });
      } else {
        scope.postMessage({ type: "result", response: value as SolveResponse });
      }
    } finally {
      exports.tellegen_acopf_dealloc(...moduleInput);
      exports.tellegen_acopf_dealloc(...requestInput);
    }
  } catch (error) {
    scope.postMessage({ type: "error", message: errorText(error), code: errorCode(error) });
  }
};
