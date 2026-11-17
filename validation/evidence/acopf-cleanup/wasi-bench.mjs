// Node 22.18+; use the same WASI host as the browser worker.
// node wasi-bench.mjs SOLVER.wasm CASE.pio.json ...
import { readFileSync } from "node:fs";
import { createHash } from "node:crypto";
import { createAcOpfWasi } from "../../../packages/engine/src/acopf-wasi.ts";
const [path, ...cases] = process.argv.slice(2);
const bytes = readFileSync(path);
const hash = (bytes) => createHash("sha256").update(bytes).digest("hex");
const started = performance.now();
const module = await WebAssembly.compile(bytes);
const compileMs = performance.now() - started;
for (const file of cases) {
  const input = readFileSync(file, "utf8");
  for (let run = 0; run < Number(process.env.RUNS ?? 3); run++) {
    const host = createAcOpfWasi(() => {});
    const start = performance.now();
    const instance = await WebAssembly.instantiate(module, host.imports);
    host.bind(instance);
    const e = instance.exports;
    const instantiateMs = performance.now() - start;
    const encode = (text) => {
      const b = new TextEncoder().encode(text);
      const ptr = e.tellegen_acopf_alloc(b.length);
      new Uint8Array(e.memory.buffer, ptr, b.length).set(b);
      return [ptr, b.length];
    };
    const m = encode(input);
    const r = encode(process.env.REQUEST ?? '{"formulation":"acopf"}');
    const solveStart = performance.now();
    const ptr = e.tellegen_acopf_solve(...m, ...r);
    const solveMs = performance.now() - solveStart;
    const len = new DataView(e.memory.buffer).getUint32(ptr, true);
    const response = JSON.parse(new TextDecoder().decode(new Uint8Array(e.memory.buffer, ptr + 4, len)));
    const memoryBytes = e.memory.buffer.byteLength;
    e.tellegen_acopf_free_payload(ptr);
    e.tellegen_acopf_dealloc(...m);
    e.tellegen_acopf_dealloc(...r);
    console.log(JSON.stringify({case: file.split("/").pop(), run, artifact_sha256: hash(bytes), input_sha256: hash(input), artifact_bytes: bytes.length, compile_ms: compileMs, instantiate_ms: instantiateMs, solve_ms: solveMs, memory_bytes: memoryBytes, status: response.status, objective: response.objective, iterations: response.iterations, error: response.error, code: response.code, sensitivities: response.sensitivities}));
  }
}
