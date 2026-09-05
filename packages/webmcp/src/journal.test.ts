import { describe, expect, it } from "vitest";
import { ExperimentJournal } from "./journal.js";
import type { TellegenToolActivityEvent, ToolPayload } from "./types.js";

const input: ToolPayload = {
  case_id: "case",
  expected_revision: "session:4",
  demand: [{ bus_id: "2", delta_mw: 5 }],
};
function event(
  id: string,
  toolName: string,
  data: ToolPayload,
  request = input,
): TellegenToolActivityEvent {
  return {
    type: "finished",
    id,
    toolName,
    title: toolName,
    startedAt: toolName === "preview_case_update" ? 1 : 3,
    finishedAt: toolName === "preview_case_update" ? 2 : 4,
    input: request,
    response: { ok: true, data },
  };
}
const preview = () =>
  event("preview", "preview_case_update", {
    case_id: "case",
    revision: "session:4",
    formulation: "dcopf",
    prediction: { objective_delta: 20 },
  });
const update = (after = 119) =>
  event("update", "update_case", {
    case_id: "case",
    before: { revision: "session:4", formulation: "dcopf", objective: 100 },
    after: { formulation: "dcopf", objective: after },
  });

describe("experiment journal", () => {
  it("compares the exact committed change with the matching preview", () => {
    const journal = new ExperimentJournal("session");
    journal.record(preview());
    journal.record(update());
    expect(journal.records[1].predictionCheck).toEqual({
      previewId: "preview",
      predictedDelta: 20,
      exactDelta: 19,
      absoluteError: 1,
      relativeError: 1 / 19,
    });
  });

  it.each(["case", "revision", "edit", "formulation", "failure"])(
    "does not pair a mismatched %s",
    (mismatch) => {
      const journal = new ExperimentJournal("session");
      journal.record(preview());
      const changed = structuredClone(update());
      if (changed.type !== "finished" || !changed.input || !changed.response.ok)
        throw new Error("fixture");
      if (mismatch === "case") changed.input.case_id = "other";
      if (mismatch === "revision")
        changed.input.expected_revision = "session:5";
      if (mismatch === "edit")
        changed.input.demand = [{ bus_id: "2", delta_mw: 6 }];
      if (mismatch === "formulation")
        changed.response.data.after = { objective: 119, formulation: "socwr" };
      if (mismatch === "failure")
        changed.response = {
          ok: false,
          error: { code: "CANCELLED", message: "cancelled" },
        };
      journal.record(changed);
      expect(journal.records[1].predictionCheck).toBeUndefined();
    },
  );

  it("keeps zero exact changes finite and reports the undefined ratio", () => {
    const journal = new ExperimentJournal("session");
    journal.record(preview());
    journal.record(update(100));
    expect(journal.records[1].predictionCheck).toMatchObject({
      exactDelta: 0,
      absoluteError: 20,
      relativeError: null,
    });
  });

  it("exports detached data and records how much history was evicted", () => {
    const journal = new ExperimentJournal("session", 1);
    const first = preview();
    journal.record(first);
    if (first.type === "finished") first.input = { case_id: "mutated" };
    expect(journal.records[0].input?.case_id).toBe("case");
    journal.record(update());
    journal.record(update());
    const exported = journal.export();
    expect(exported).toMatchObject({
      schema: "tellegen.experiment-journal",
      sessionId: "session",
      droppedRecords: 1,
    });
    exported.records[0].title = "mutated";
    expect(journal.records[0].title).toBe("update_case");
  });
});
