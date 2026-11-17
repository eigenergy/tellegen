import { afterEach, describe, expect, it } from "vitest";
import {
  probeAcOpfWorker,
  releaseAcOpfWorker,
  solveAcOpfModule,
  solveJointAcDcModule,
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

const result: AcOpfWorkerResponse = {
  type: "result",
  response: { formulation: "acopf", status: "feasible" },
};

// The kept worker is module state; no test may leave one behind.
afterEach(() => {
  releaseAcOpfWorker();
});

describe("experimental AC OPF worker host", () => {
  it("sends sensitivity requests to the separate solver", async () => {
    const worker = new FakeWorker();
    const cells = [{ operand: { Price: "Active" }, parameter: { Demand: "Active" } }] as const;
    const pending = solveAcOpfModule("/acopf.wasm", "{}", undefined, () => worker, [...cells]);
    expect(worker.request?.type).toBe("solve");
    if (worker.request?.type === "solve") {
      expect(JSON.parse(worker.request.requestJson)).toEqual({ formulation: "acopf", sensitivities: cells });
    }
    worker.reply(result);
    await expect(pending).resolves.toEqual(result.response);
  });

  it("keeps the engine error code after a worker failure", async () => {
    const worker = new FakeWorker();
    const pending = solveAcOpfModule("/acopf.wasm", "{}", undefined, () => worker);
    worker.reply({ type: "error", message: "unsupported sensitivity", code: "unsupported" });
    await expect(pending).rejects.toMatchObject({ message: "unsupported sensitivity", code: "unsupported" });
  });

  it("enables the capability only after the WASI worker probes successfully", async () => {
    const worker = new FakeWorker();
    const factory = () => worker;
    const pending = probeAcOpfWorker("/acopf.wasm", factory);
    expect(worker.request).toEqual({ type: "probe", wasmUrl: "/acopf.wasm" });
    worker.reply({ type: "ready" });
    await expect(pending).resolves.toBe(true);
    // The probed worker holds the compiled module and serves the first solve.
    expect(worker.terminated).toBe(false);
    const solve = solveAcOpfModule("/acopf.wasm", "{}", undefined, factory);
    expect(worker.request).toMatchObject({ type: "solve" });
    worker.reply(result);
    await expect(solve).resolves.toMatchObject({ formulation: "acopf" });
    expect(releaseAcOpfWorker()).toBe(true);
    expect(worker.terminated).toBe(true);
    expect(releaseAcOpfWorker()).toBe(false);
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

  it("returns the typed solve response and keeps one worker across solves", async () => {
    const workers = [new FakeWorker(), new FakeWorker()];
    let index = 0;
    const factory = () => workers[index++];
    const first = solveAcOpfModule("/acopf.wasm", "{}", undefined, factory);
    workers[0].reply(result);
    await expect(first).resolves.toMatchObject({ formulation: "acopf" });
    const second = solveAcOpfModule("/acopf.wasm", "{}", undefined, factory);
    workers[0].reply(result);
    await expect(second).resolves.toMatchObject({ formulation: "acopf" });
    // A solve error is an answer; the worker stays.
    const failed = solveAcOpfModule("/acopf.wasm", "{}", undefined, factory);
    workers[0].reply({ type: "error", message: "did not converge" });
    await expect(failed).rejects.toThrow("did not converge");
    expect(index).toBe(1);
    expect(workers[0].terminated).toBe(false);
    expect(workers[0].onmessage).toBeNull();
  });

  it("starts a fresh worker after a cancelled solve", async () => {
    const workers = [new FakeWorker(), new FakeWorker()];
    let index = 0;
    const factory = () => workers[index++];
    const abort = new AbortController();
    const cancelled = solveAcOpfModule(
      "/acopf.wasm",
      "{}",
      abort.signal,
      factory,
    );
    abort.abort();
    await expect(cancelled).rejects.toMatchObject({ name: "AbortError" });
    expect(workers[0].terminated).toBe(true);
    const next = solveAcOpfModule("/acopf.wasm", "{}", undefined, factory);
    expect(workers[1].request).toMatchObject({ type: "solve" });
    workers[1].reply(result);
    await expect(next).resolves.toMatchObject({ formulation: "acopf" });
    expect(workers[1].terminated).toBe(false);
  });

  it("replaces a failed worker and gives an overlapping call its own", async () => {
    const workers = [new FakeWorker(), new FakeWorker(), new FakeWorker()];
    let index = 0;
    const factory = () => workers[index++];
    const crashed = solveAcOpfModule("/acopf.wasm", "{}", undefined, factory);
    workers[0].onerror?.({} as ErrorEvent);
    await expect(crashed).rejects.toThrow("AC OPF worker failed");
    expect(workers[0].terminated).toBe(true);

    const first = solveAcOpfModule("/acopf.wasm", "{}", undefined, factory);
    const overlapping = solveAcOpfModule(
      "/acopf.wasm",
      "{}",
      undefined,
      factory,
    );
    expect(index).toBe(3);
    workers[2].reply(result);
    await expect(overlapping).resolves.toMatchObject({ formulation: "acopf" });
    // The extra worker is not kept; the first one is.
    expect(workers[2].terminated).toBe(true);
    workers[1].reply(result);
    await expect(first).resolves.toMatchObject({ formulation: "acopf" });
    expect(workers[1].terminated).toBe(false);
  });
  it("terminates a pending probe when its owner is disposed", async () => {
    const worker = new FakeWorker();
    const abort = new AbortController();
    const pending = probeAcOpfWorker("/acopf.wasm", () => worker, abort.signal);
    abort.abort();
    await expect(pending).resolves.toBe(false);
    expect(worker.terminated).toBe(true);
    expect(worker.onmessage).toBeNull();
  });

  it("does not create a worker for an already cancelled probe", async () => {
    const abort = new AbortController();
    abort.abort();
    let created = false;
    await expect(
      probeAcOpfWorker(
        "/acopf.wasm",
        () => {
          created = true;
          return new FakeWorker();
        },
        abort.signal,
      ),
    ).resolves.toBe(false);
    expect(created).toBe(false);
  });

  it("disposes the worker if posting the request throws", async () => {
    const worker = new FakeWorker();
    worker.postMessage = () => {
      throw new Error("post failed");
    };
    await expect(
      solveAcOpfModule("/acopf.wasm", "{}", undefined, () => worker),
    ).rejects.toThrow("post failed");
    expect(worker.terminated).toBe(true);
  });
});

it("sends joint dispatch to the dedicated worker and rejects an ordinary result", async () => {
  const worker = new FakeWorker();
  const pending = solveJointAcDcModule(
    "/acopf.wasm",
    "{}",
    undefined,
    () => worker,
  );
  expect(worker.request).toMatchObject({
    type: "solve",
    requestJson: JSON.stringify({ formulation: "acopf", ac_dc: true }),
  });
  worker.reply({
    type: "result",
    response: { formulation: "acopf", status: "feasible" },
  });
  await expect(pending).rejects.toThrow("no joint result");
  // The worker answered, so it is kept until released.
  expect(worker.terminated).toBe(false);
  expect(releaseAcOpfWorker()).toBe(true);
  expect(worker.terminated).toBe(true);
});
it("cancels joint dispatch by terminating its worker", async () => {
  const worker = new FakeWorker();
  const abort = new AbortController();
  const pending = solveJointAcDcModule(
    "/acopf.wasm",
    "{}",
    abort.signal,
    () => worker,
  );
  abort.abort();
  await expect(pending).rejects.toMatchObject({ name: "AbortError" });
  expect(worker.terminated).toBe(true);
});
