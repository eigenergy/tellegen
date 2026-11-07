import { beforeEach, expect, it, vi } from "vitest";
const host = vi.hoisted(() => ({
  kind: "worker" as const,
  call: vi.fn(),
  cancel: vi.fn(),
}));
vi.mock("../src/host.js", async (importOriginal) => ({
  ...(await importOriginal<typeof import("../src/host.js")>()),
  isolatedEngineHost: () => host,
}));
import { createStudy } from "../src/index.js";
beforeEach(() => vi.resetAllMocks());
it("a mismatched settings calculation is rejected before creating a worker-backed Study", async () => {
  await expect(
    createStudy("input", "dcopf", {
      isolated: true,
      settings: { calculation: "acpf" },
    }),
  ).rejects.toThrow("must match");
  expect(host.call).not.toHaveBeenCalled();
});
it("a rejected isolated Study releases its host and preserves the construction error", async () => {
  host.call.mockImplementation(async (request) => {
    if (request.op === "study_new") throw new Error("did not converge");
    return null;
  });
  await expect(
    createStudy("input", "dcopf", {
      isolated: true,
      settings: { calculation: "dcopf" },
    }),
  ).rejects.toThrow("did not converge");
  expect(host.call).toHaveBeenCalledWith(
    expect.objectContaining({ op: "study_free" }),
  );
  expect(host.cancel).toHaveBeenCalledOnce();
});
