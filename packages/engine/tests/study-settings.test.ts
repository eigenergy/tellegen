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
it("conflicting DC convention options are rejected before creating a Study", async () => {
  await expect(
    createStudy("input", "dcopf", {
      isolated: true,
      branch_susceptance_formula: "reactance_only",
      settings: {
        calculation: "dcopf",
        overrides: { branch_susceptance_formula: "series_susceptance" },
      },
    }),
  ).rejects.toThrow("Conflicting DC conventions");
  expect(host.call).not.toHaveBeenCalled();
});
it("DC convenience options enter the common settings request", async () => {
  host.call.mockImplementation(async (request) => {
    if (request.op === "study_new")
      throw new Error("stop after construction request");
    return null;
  });
  await expect(
    createStudy("input", "dcopf", {
      isolated: true,
      branch_susceptance_formula: "reactance_only",
      settings: {
        calculation: "dcopf",
        overrides: { angle_difference: false },
      },
    }),
  ).rejects.toThrow("stop after construction request");
  const request = host.call.mock.calls.find(
    ([request]) => request.op === "study_new",
  )![0];
  expect(JSON.parse(request.settings)).toMatchObject({
    calculation: "dcopf",
    overrides: {
      branch_susceptance_formula: "reactance_only",
      angle_difference: false,
    },
  });
});
