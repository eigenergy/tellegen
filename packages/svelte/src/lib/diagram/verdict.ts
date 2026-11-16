/** The result's key numbers and the solver's one-word status. Pure, so the wording is tested
 * without a renderer. */

import { formulationLabel } from '../format.js';
import type { Formulation } from '@tellegen/engine';

export type VerdictTone = 'ok' | 'warning' | 'failure' | 'pending';

export interface Verdict {
	text: string;
	tone: VerdictTone;
}

/** Raw solver status strings mapped to plain words. Unknown strings fall back to their own
 * words, spaced and sentence-cased, so a new solver status still reads. */
const STATUS_WORDS: Record<string, string> = {
	optimal: 'Optimal',
	feasible: 'Feasible',
	converged: 'Converged',
	not_converged: 'Did not converge',
	infeasible: 'Infeasible',
	unbounded: 'Unbounded',
	solve_succeeded: 'Optimal',
	solved_to_acceptable_level: 'Acceptable',
	restoration_failed: 'Stalled',
	maximum_iterations_exceeded: 'Iteration limit',
	infeasible_problem_detected: 'Infeasible',
	rank_deficient: 'Rank deficient'
};

export function statusWords(status: string | null | undefined): string {
	if (!status) return 'Not solved';
	const key = status
		.trim()
		.toLowerCase()
		.replace(/[\s-]+/g, '_');
	const known = STATUS_WORDS[key];
	if (known) return known;
	const words = key.replace(/_/g, ' ');
	return words.charAt(0).toUpperCase() + words.slice(1);
}

/** Statuses that mean the calculation found what it set out to. */
export function cleanStatus(status: string | null | undefined): boolean {
	return CLEAN.has(statusWords(status));
}
const CLEAN = new Set(['Optimal', 'Feasible', 'Converged']);

/** The solver's iteration count from either convergence record the engine returns: the
 * interior-point trace (an array) or the Newton summary (`{ count }`). */
export function iterationCount(record: unknown): number | null {
	if (Array.isArray(record)) return record.length > 0 ? record.length : null;
	if (record && typeof record === 'object' && 'count' in record) {
		const count = (record as { count: unknown }).count;
		return typeof count === 'number' && Number.isFinite(count) ? count : null;
	}
	return null;
}

/** Elapsed solve time: milliseconds below a second, seconds above. */
export function duration(ms: number): string {
	if (ms < 1000) return `${Math.round(ms)} ms`;
	return `${(ms / 1000).toFixed(ms < 10_000 ? 1 : 0)} s`;
}

const OPF = new Set<string>(['dcopf', 'socwr', 'acopf']);

export interface SolveVerdictInput {
	formulation: Formulation;
	solving: boolean;
	/** The latest solve ended without a result. */
	failed: boolean;
	/** The solver's own status for the result, when it reports one. */
	status?: string | null;
	/** The engine's failure text, when the solve failed. */
	failure?: string | null;
}

/** One short line on the solve: "Solving…", what failed, or a status that is not clean. Null
 * when the solve succeeded cleanly: the result's numbers say the rest. */
export function solveVerdict(input: SolveVerdictInput): Verdict | null {
	if (input.solving) return { text: 'Solving…', tone: 'pending' };
	if (input.failed) {
		const infeasible = /\binfeasib/i.test(`${input.failure ?? ''} ${input.status ?? ''}`);
		const what = infeasible
			? 'infeasible'
			: input.formulation === 'acpf'
				? 'did not converge'
				: 'failed';
		return { text: `${formulationLabel(input.formulation)} ${what}`, tone: 'failure' };
	}
	return input.status != null && !cleanStatus(input.status)
		? { text: statusWords(input.status), tone: 'warning' }
		: null;
}

/** A saved study's result status. */
export function savedVerdict(input: { status?: string | null }): Verdict {
	if (!input.status) return { text: 'No result', tone: 'pending' };
	return {
		text: `Saved · ${statusWords(input.status)}`,
		tone: cleanStatus(input.status) ? 'ok' : 'warning'
	};
}

/** Dollars per hour, the unit of MATPOWER generator costs: whole dollars from $100 up. */
export function costPerHour(value: number): string {
	const digits = Math.abs(value) >= 100 ? 0 : 2;
	const amount = Math.abs(value).toLocaleString('en-US', {
		minimumFractionDigits: digits,
		maximumFractionDigits: digits
	});
	return `${value < 0 ? '−' : ''}$${amount}/h`;
}

/** A labelled number: the unit of every result summary. */
export interface Stat {
	label: string;
	value: string;
	/** A small value under the main one, such as the change from the base case. */
	sub?: string;
	tone?: VerdictTone;
}

export interface ResultStatsInput {
	formulation: Formulation;
	/** A solve is running, so missing values are on their way. */
	solving?: boolean;
	objective: number | null;
	/** Lines at their limit; null when the result does not say. */
	binding: number | null;
	/** Objective change from the base case, when the case is edited. */
	deltaObjective?: number | null;
	/** Lowest and highest bus voltage (pu), for an AC power flow. */
	voltage?: { min: number; max: number } | null;
	/** Highest line loading as a fraction of rating, when known. */
	maxLoading?: number | null;
}

/** One decimal, as on the diagram's gauges and the list of lines at limit. */
const percent = (fraction: number) => `${(fraction * 100).toFixed(1)}%`;

/** The key numbers of a result, chosen per calculation: an OPF reports its operational cost and
 * the lines at their limit; AC power flow its voltage range and heaviest line; DC power flow its
 * heaviest line and the lines at their limit. */
export function resultStats(input: ResultStatsInput): Stat[] {
	const missing = input.solving ? '…' : '—';
	const stats: Stat[] = [];
	if (OPF.has(input.formulation)) {
		const delta = input.objective !== null ? input.deltaObjective : null;
		// A relaxation's cost is a lower bound on the true operating cost: the label says so.
		stats.push({
			label: input.formulation === 'socwr' ? 'Cost lower bound' : 'Operational cost',
			value: input.objective === null ? missing : costPerHour(input.objective),
			sub: delta ? `${delta > 0 ? '+' : ''}${costPerHour(delta)}` : undefined
		});
	} else if (input.formulation === 'acpf') {
		const v = input.voltage;
		stats.push({
			label: 'Voltage range',
			value: v ? `${v.min.toFixed(3)}–${v.max.toFixed(3)} pu` : missing
		});
	}
	if (!OPF.has(input.formulation)) {
		const m = input.maxLoading;
		stats.push({
			label: 'Max loading',
			value: m == null ? missing : percent(m),
			tone: m != null && m >= 1 ? 'failure' : 'ok'
		});
	}
	if (input.formulation !== 'acpf') {
		const n = input.binding;
		stats.push({
			label: 'Lines at limit',
			value: n === null ? missing : n.toLocaleString('en-US'),
			// A line at its limit is at 100% of its rating: red, as on the map.
			tone: n ? 'failure' : 'ok'
		});
	}
	return stats;
}
