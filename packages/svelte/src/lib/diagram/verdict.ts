/** The sentence at the top of each result: whether the solve worked, how long it took, and
 * (for an OPF) what it found. The panels keep the exact rows below it. Pure, so the wording is
 * tested without a renderer. */

import { formulationLabel } from "../format.js";
import type { Formulation } from "@tellegen/engine";

export type VerdictTone = "ok" | "warning" | "failure" | "pending";

export interface Verdict {
  text: string;
  tone: VerdictTone;
}

/** Raw solver status strings mapped to plain words. Unknown strings fall back to their own
 * words, spaced and sentence-cased, so a new solver status still reads. */
const STATUS_WORDS: Record<string, string> = {
  optimal: "Optimal",
  feasible: "Feasible",
  converged: "Converged",
  not_converged: "Did not converge",
  infeasible: "Infeasible",
  unbounded: "Unbounded",
  solve_succeeded: "Optimal",
  solved_to_acceptable_level: "Acceptable (looser tolerance)",
  restoration_failed: "Stalled during restoration",
  maximum_iterations_exceeded: "Stopped at the iteration limit",
  infeasible_problem_detected: "Infeasible",
  rank_deficient: "Rank deficient",
};

export function statusWords(status: string | null | undefined): string {
  if (!status) return "Not solved";
  const key = status
    .trim()
    .toLowerCase()
    .replace(/[\s-]+/g, "_");
  const known = STATUS_WORDS[key];
  if (known) return known;
  const words = key.replace(/_/g, " ");
  return words.charAt(0).toUpperCase() + words.slice(1);
}

/** The solver's iteration count from either convergence record the engine returns: the
 * interior-point trace (an array) or the Newton summary (`{ count }`). */
export function iterationCount(record: unknown): number | null {
  if (Array.isArray(record)) return record.length > 0 ? record.length : null;
  if (record && typeof record === "object" && "count" in record) {
    const count = (record as { count: unknown }).count;
    return typeof count === "number" && Number.isFinite(count) ? count : null;
  }
  return null;
}

/** Elapsed solve time: milliseconds below a second, seconds above. */
export function duration(ms: number): string {
  if (ms < 1000) return `${Math.round(ms)} ms`;
  return `${(ms / 1000).toFixed(ms < 10_000 ? 1 : 0)} s`;
}

const plural = (n: number, one: string, many = `${one}s`) =>
  `${n.toLocaleString("en-US")} ${n === 1 ? one : many}`;

const OPF = new Set<string>(["dcopf", "socwr", "acopf"]);

/** What the calculation finds when it succeeds, as the subject of the sentence. */
function finding(formulation: Formulation): string {
  switch (formulation) {
    case "acpf":
      return "AC power flow converged";
    case "socwr":
      return "SOCWR found a lower bound on generation cost";
    case "acopf":
      return "AC OPF found a locally optimal dispatch";
    default:
      return `${formulationLabel(formulation)} found the cheapest dispatch`;
  }
}

/** What to try when a calculation fails, by calculation and cause. */
function advice(formulation: Formulation, infeasible: boolean): string {
  if (infeasible)
    return "Reduce demand or raise line ratings, then solve again.";
  if (formulation === "acpf")
    return "Try DC OPF for a starting point, or relax the tolerance in Settings.";
  return "Check the case data, or relax the tolerance in Settings.";
}

export interface SolveVerdictInput {
  formulation: Formulation;
  solving: boolean;
  /** The latest solve ended without a result. */
  failed: boolean;
  /** The solver's own status for the result, when it reports one. */
  status?: string | null;
  /** The engine's failure text, when the solve failed. */
  failure?: string | null;
  iterations?: number | null;
  ms?: number | null;
}

/** The lead sentence for the solver card. */
export function solveVerdict(input: SolveVerdictInput): Verdict {
  const label = formulationLabel(input.formulation);
  if (input.solving) return { text: `Solving ${label}…`, tone: "pending" };
  const iterations = input.iterations ?? null;
  if (input.failed) {
    const infeasible = /\binfeasib/i.test(
      `${input.failure ?? ""} ${input.status ?? ""}`,
    );
    const what = infeasible
      ? OPF.has(input.formulation)
        ? `${label} found no feasible dispatch: demand cannot be served within the line and generator limits.`
        : `${label} found no feasible operating point.`
      : input.formulation === "acpf"
        ? `AC power flow did not converge${iterations ? ` after ${plural(iterations, "iteration")}` : ""}.`
        : `${label} did not finish${iterations ? ` after ${plural(iterations, "iteration")}` : ""}.`;
    return {
      text: `${what} ${advice(input.formulation, infeasible)}`,
      tone: "failure",
    };
  }
  const parts: string[] = [];
  if (iterations) parts.push(`in ${plural(iterations, "iteration")}`);
  const time = input.ms != null ? ` (${duration(input.ms)})` : "";
  const sentence = `${finding(input.formulation)}${parts.length ? ` ${parts.join(" ")}` : ""}${time}.`;
  const words = statusWords(input.status);
  const loose =
    input.status != null &&
    !["Optimal", "Feasible", "Converged"].includes(words);
  return loose
    ? {
        text: `${sentence} Solver status: ${words.toLowerCase()}.`,
        tone: "warning",
      }
    : { text: sentence, tone: "ok" };
}

/** The lead sentence for a saved study's result. */
export function savedVerdict(input: {
  formulation: Formulation;
  status?: string | null;
  label: string;
}): Verdict {
  const label = formulationLabel(input.formulation);
  if (!input.status)
    return {
      text: `Saved state “${input.label}” has no ${label} result.`,
      tone: "pending",
    };
  const words = statusWords(input.status);
  const ok = ["Optimal", "Feasible", "Converged"].includes(words);
  return {
    text: `Saved ${label} result: ${words.toLowerCase()}.`,
    tone: ok ? "ok" : "warning",
  };
}

export interface OutcomeVerdictInput {
  formulation: Formulation;
  objective: number | null;
  /** Lines at their limit; null when the result does not say. */
  binding: number | null;
  /** Whether `binding` counts the solver's binding flags (true) or lines loaded to rating. */
  flagged: boolean;
  /** Objective change from the base case, when the case is edited. */
  deltaObjective?: number | null;
}

const WORDS = [
  "No",
  "One",
  "Two",
  "Three",
  "Four",
  "Five",
  "Six",
  "Seven",
  "Eight",
  "Nine",
];

/** A count that starts a sentence: a word below ten, digits above. */
const count = (n: number) => WORDS[n] ?? n.toLocaleString("en-US");

/** Dollars per hour, the unit of MATPOWER generator costs: whole dollars from $100 up. */
export function costPerHour(value: number): string {
  const digits = Math.abs(value) >= 100 ? 0 : 2;
  return `$${value.toLocaleString("en-US", { minimumFractionDigits: digits, maximumFractionDigits: digits })}/h`;
}

/** The sentences at the top of the network summary: what the result means for the network. Null
 * when there is nothing yet to say (no result, or a power flow, whose verdict is the solver's). */
export function outcomeVerdict(input: OutcomeVerdictInput): Verdict | null {
  if (!OPF.has(input.formulation) || input.objective === null) return null;
  const bound = input.formulation === "socwr" ? "at least " : "";
  const sentences = [
    `Generation cost is ${bound}${costPerHour(input.objective)}.`,
  ];
  if (input.deltaObjective != null && input.deltaObjective !== 0) {
    const change = Math.abs(input.deltaObjective);
    sentences.push(
      `That is ${costPerHour(change)} ${input.deltaObjective > 0 ? "more" : "less"} than the base case.`,
    );
  }
  let tone: VerdictTone = "ok";
  if (input.binding !== null) {
    const n = input.binding;
    if (n > 0) tone = "warning";
    const line = n === 1 ? "line is" : "lines are";
    sentences.push(
      input.flagged
        ? `${count(n)} ${line} at ${n === 1 ? "its" : "their"} limit.`
        : `${count(n)} ${line} at or above ${n === 1 ? "its" : "their"} rating.`,
    );
  }
  return { text: sentences.join(" "), tone };
}
