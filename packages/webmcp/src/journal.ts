import type {
  JsonValue,
  TellegenToolActivityEvent,
  ToolPayload,
} from "./types.js";

type FinishedActivity = Extract<
  TellegenToolActivityEvent,
  { type: "finished" }
>;

export interface PredictionCheck {
  previewId: string;
  predictedDelta: number;
  exactDelta: number;
  absoluteError: number;
  /** Undefined relative error at an exactly zero observed change. */
  relativeError: number | null;
}

export interface ExperimentRecord extends FinishedActivity {
  predictionCheck?: PredictionCheck;
}

export interface ExperimentJournalDocument {
  schema: "tellegen.experiment-journal";
  version: 1;
  sessionId: string;
  exportedAt: string;
  droppedRecords: number;
  records: ExperimentRecord[];
}

function object(value: JsonValue | undefined): ToolPayload | null {
  return value && typeof value === "object" && !Array.isArray(value)
    ? value
    : null;
}

function finite(value: JsonValue | undefined): value is number {
  return typeof value === "number" && Number.isFinite(value);
}

// These are validated wire requests. Sort edits so request key order and
// branch order do not affect the match; keep the case and revision exact.
function editKey(input: ToolPayload | undefined): string | null {
  if (
    !input ||
    typeof input.case_id !== "string" ||
    typeof input.expected_revision !== "string"
  )
    return null;
  const edits = (value: JsonValue | undefined, id: string) =>
    (Array.isArray(value) ? value : [])
      .map((entry) => {
        const row = object(entry);
        return [row?.[id], row?.delta_mw];
      })
      .sort((a, b) => String(a[0]).localeCompare(String(b[0])));
  return JSON.stringify([
    input.case_id,
    input.expected_revision,
    input.mode ?? "set",
    edits(input.demand, "bus_id"),
    edits(input.ratings, "branch_id"),
  ]);
}

function comparePrediction(
  records: ExperimentRecord[],
  event: FinishedActivity,
): PredictionCheck | undefined {
  if (event.toolName !== "update_case" || !event.response.ok) return;
  const key = editKey(event.input);
  if (key === null) return;
  const data = event.response.data;
  const before = object(data.before),
    after = object(data.after);
  if (
    !before ||
    !after ||
    !finite(before.objective) ||
    !finite(after.objective) ||
    before.revision !== event.input?.expected_revision ||
    data.case_id !== event.input?.case_id ||
    before.formulation !== after.formulation
  )
    return;
  const preview = [...records].reverse().find((record) => {
    if (
      record.toolName !== "preview_case_update" ||
      !record.response.ok ||
      record.finishedAt > event.startedAt ||
      editKey(record.input) !== key
    )
      return false;
    return (
      record.response.data.case_id === data.case_id &&
      record.response.data.revision === before.revision &&
      record.response.data.formulation === before.formulation
    );
  });
  if (!preview?.response.ok) return;
  const predictedDelta = object(
    preview.response.data.prediction,
  )?.objective_delta;
  if (!finite(predictedDelta)) return;
  const exactDelta = after.objective - before.objective;
  const absoluteError = Math.abs(exactDelta - predictedDelta);
  if (!Number.isFinite(exactDelta) || !Number.isFinite(absoluteError)) return;
  const ratio = exactDelta === 0 ? null : absoluteError / Math.abs(exactDelta);
  return {
    previewId: preview.id,
    predictedDelta,
    exactDelta,
    absoluteError,
    relativeError: ratio !== null && Number.isFinite(ratio) ? ratio : null,
  };
}

/** A bounded journal of completed calls. It records data and never executes a replay. */
export class ExperimentJournal {
  #records: ExperimentRecord[] = [];
  #dropped = 0;

  constructor(
    readonly sessionId: string,
    readonly capacity = 100,
  ) {
    if (
      !sessionId ||
      !Number.isInteger(capacity) ||
      capacity < 1 ||
      capacity > 1000
    )
      throw new Error(
        "Journal requires a session ID and a capacity from 1 to 1000",
      );
  }

  record(event: TellegenToolActivityEvent): void {
    if (
      event.type !== "finished" ||
      this.#records.some((record) => record.id === event.id)
    )
      return;
    const snapshot: FinishedActivity = JSON.parse(JSON.stringify(event));
    const predictionCheck = comparePrediction(this.#records, snapshot);
    this.#records.push({
      ...snapshot,
      ...(predictionCheck ? { predictionCheck } : {}),
    });
    if (this.#records.length > this.capacity) {
      this.#records.shift();
      this.#dropped += 1;
    }
  }

  get records(): ExperimentRecord[] {
    return JSON.parse(JSON.stringify(this.#records));
  }

  export(): ExperimentJournalDocument {
    return {
      schema: "tellegen.experiment-journal",
      version: 1,
      sessionId: this.sessionId,
      exportedAt: new Date().toISOString(),
      droppedRecords: this.#dropped,
      records: this.records,
    };
  }
}
