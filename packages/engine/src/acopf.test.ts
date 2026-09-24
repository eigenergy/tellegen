import { describe, expect, it } from "vitest";
import {
  probeAcOpfWorker,
  solveAcOpfModule,
  type AcOpfWorkerFactory,
} from "./acopf.js";
import type {
  AcOpfWorkerRequest,
  AcOpfWorkerResponse,
} from "./acopf-protocol.js";

class FakeWorker {
  onmessage: ((event: MessageEvent<AcOpfWorkerResponse>) => void) | null = null;
  onerror: ((event: ErrorEvent) => void) | null = null;
  onmessageerror: ((event: MessageEvent) => void) | null = null;
  terminated = false;
  request: AcOpfWorkerRequest | null = null;

  postMessage(request: AcOpfWorkerRequest) {
    this.request = request;
  }

  terminate() {
    this.terminated = true;
  }

  reply(response: AcOpfWorkerResponse) {
    this.onmessage?.({ data: response } as MessageEvent<AcOpfWorkerResponse>);
  }
}

describe("experimental AC OPF worker host", () => {
  it("enables the capability only after the WASI worker probes successfully", async () => {
    const worker = new FakeWorker();
    const pending = probeAcOpfWorker("/acopf.wasm", () => worker);
    expect(worker.request).toEqual({ type: "probe", wasmUrl: "/acopf.wasm" });
    worker.reply({ type: "ready" });
    await expect(pending).resolves.toBe(true);
    expect(worker.terminated).toBe(true);
  });

  it("keeps the capability disabled when the asset or ABI probe fails", async () => {
    const worker = new FakeWorker();
    const pending = probeAcOpfWorker("/missing.wasm", () => worker);
    worker.reply({ type: "error", message: "HTTP 404" });
    await expect(pending).resolves.toBe(false);
    expect(worker.terminated).toBe(true);
  });

  it("terminates the isolated worker on cancellation and never falls back", async () => {
    const worker = new FakeWorker();
    const factory: AcOpfWorkerFactory = () => worker;
    const abort = new AbortController();
    const pending = solveAcOpfModule(
      "/acopf.wasm",
      "{}",
      abort.signal,
      factory,
    );
    abort.abort();
    await expect(pending).rejects.toMatchObject({ name: "AbortError" });
    expect(worker.terminated).toBe(true);
  });

  it("returns the typed solve response and disposes the one-shot worker", async () => {
    const workers = [new FakeWorker(), new FakeWorker()];
    let index = 0;
    const factory = () => workers[index++];
    const first = solveAcOpfModule("/acopf.wasm", "{}", undefined, factory);
    workers[0].reply({
      type: "result",
      response: { formulation: "acopf", status: "feasible" },
    });
    await expect(first).resolves.toMatchObject({ formulation: "acopf" });
    const second = solveAcOpfModule("/acopf.wasm", "{}", undefined, factory);
    workers[1].reply({
      type: "result",
      response: { formulation: "acopf", status: "feasible" },
    });
    await expect(second).resolves.toMatchObject({ formulation: "acopf" });
    expect(workers.every((worker) => worker.terminated)).toBe(true);
    expect(index).toBe(2);
  });
});
