import { describe, expect, it } from "vitest";

import { FORMULATIONS, calculationAvailable } from "../src/index.js";

const offered = (context: Parameters<typeof calculationAvailable>[1]) =>
  FORMULATIONS.map((f) => f.id).filter((id) =>
    calculationAvailable(id, context),
  );

describe("calculationAvailable", () => {
  it("offers every calculation but AC OPF until its solver can run", () => {
    expect(offered({})).toEqual(["dcopf", "acpf", "socwr"]);
    expect(offered({ acOpf: true })).toEqual([
      "dcopf",
      "acpf",
      "socwr",
      "acopf",
    ]);
  });

  it("offers only the calculation a case file declares", () => {
    expect(offered({ declared: "acpf", acOpf: true })).toEqual(["acpf"]);
  });

  it("drops AC OPF for an edited case and AC power flow for edited ratings", () => {
    expect(offered({ acOpf: true, edited: true })).toEqual([
      "dcopf",
      "acpf",
      "socwr",
    ]);
    expect(offered({ edited: true, ratingsEdited: true })).toEqual([
      "dcopf",
      "socwr",
    ]);
  });
});
