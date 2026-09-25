import type { SolveResponse } from "./generated/contracts.js";

export type AcOpfWorkerRequest =
  | { type: "probe"; wasmUrl: string }
  | {
      type: "solve";
      wasmUrl: string;
      moduleJson: string;
      requestJson: string;
    };

export type AcOpfWorkerResponse =
  | { type: "ready" }
  | { type: "result"; response: SolveResponse }
  | { type: "error"; message: string };
