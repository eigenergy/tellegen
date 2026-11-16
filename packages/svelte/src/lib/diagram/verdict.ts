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

/** The names for lines at or over their rating: an OPF holds a line at its limit, while a
 * power flow can load one past it. */
export function limitWords(formulation?: string | null): {
	lines: string;
	key: string;
} {
	return formulation === 'acpf' || formulation === 'dcpf'
		? { lines: 'Lines over limit', key: 'Over limit' }
		: { lines: 'Lines at limit', key: 'At limit' };
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
}

/** One short line on the solve: "Solving…", what failed, or a status that is not clean. Null
 * when the solve succeeded cleanly: the result's numbers say the rest. */
export function solveVerdict(input: SolveVerdictInput): Verdict | null {
	if (input.solving) return { text: 'Solving…', tone: 'pending' };
	if (input.failed) {
		const infeasible = /\binfeasib/i.test(`${input.failure ?? ''} ${input.status ?? ''}`);
		const what = infeasible
			? 'infeasible'
			: /converg/i.test(input.failure ?? '')
				? 'did not converge'
				: 'failed';
		return {
			text: `${formulationLabel(input.formulation)} ${what}`,
			tone: 'failure'
		};
	}
	return input.status != null && !cleanStatus(input.status)
		? { text: statusWords(input.status), tone: 'warning' }
		: null;
}

/** A saved state's result status, as for a live result: null when the result is clean (its
 * numbers say the rest), else one or two words. */
export function savedVerdict(input: { status?: string | null }): Verdict | null {
	if (!input.status) return { text: 'Not solved', tone: 'pending' };
	return cleanStatus(input.status) ? null : { text: statusWords(input.status), tone: 'warning' };
}

/** The voltage range, heaviest line and branch losses of a result, for {@link resultStats}.
 * A flow's `from` and `to` are the active power entering at each end (MW); `to` is absent in
 * results without it, which leave losses unknown. */
export function flowSummary(
	vm: readonly { value: number }[] | null | undefined,
	flows: readonly { loading: number; from: number; to?: number | null }[] | null | undefined
): Pick<ResultStatsInput, 'voltage' | 'maxLoading' | 'losses'> {
	let voltage: { min: number; max: number } | null = null;
	if (vm?.length) {
		let min = Infinity;
		let max = -Infinity;
		for (const { value } of vm) {
			min = Math.min(min, value);
			max = Math.max(max, value);
		}
		voltage = { min, max };
	}
	let maxLoading: number | null = null;
	// Each branch loses what enters at one end and does not leave at the other.
	let losses: number | null = null;
	for (const flow of flows ?? []) {
		if (Number.isFinite(flow.loading)) maxLoading = Math.max(maxLoading ?? 0, flow.loading);
		if (flow.to != null && Number.isFinite(flow.from + flow.to))
			losses = (losses ?? 0) + flow.from + flow.to;
	}
	return { voltage, maxLoading, losses };
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
	/** A unit or short name after the value, such as $/MWh. */
	sub?: string;
	/** A short tooltip on the box. */
	title?: string;
	tone?: VerdictTone;
}

export interface ResultStatsInput {
	formulation: Formulation;
	/** A solve is running, so missing values are on their way. */
	solving?: boolean;
	objective: number | null;
	/** Lines at their limit; null when the result does not say. */
	binding: number | null;
	/** Lowest and highest bus voltage (pu), for an AC power flow. */
	voltage?: { min: number; max: number } | null;
	/** Highest line loading as a fraction of rating, when known. */
	maxLoading?: number | null;
	/** Real power lost in the branches (MW), for an AC power flow. */
	losses?: number | null;
}

/** A line's loading as a percentage of its rating, to one decimal, as on the diagram's gauges
 * and every list of lines. A loading over its rating that would round to 100.0% shows two
 * decimals, so an overload never reads as exactly at the limit. */
export function percent(fraction: number): string {
	const value = fraction * 100;
	const text = value.toFixed(1);
	return `${value - 100 > 1e-6 && text === '100.0' ? value.toFixed(2) : text}%`;
}

/** The key numbers of a result, chosen per calculation, always in pairs: an OPF reports its
 * operational cost and the lines at their limit; AC power flow its losses, heaviest line, and
 * lowest and highest voltage (the list under the boxes counts and names lines over their limit);
 * DC power flow its heaviest line and the lines over their limit. An OPF whose cost is zero (a
 * case without generator costs) leaves out a meaningless $0 and shows its heaviest line. */
export function resultStats(input: ResultStatsInput): Stat[] {
	const missing = input.solving ? '…' : '—';
	const stats: Stat[] = [];
	const maxLoading = () => {
		const m = input.maxLoading;
		stats.push({
			label: 'Max loading',
			value: m == null ? missing : percent(m),
			tone: m != null && m >= 1 ? 'failure' : 'ok'
		});
	};
	const linesAtLimit = () => {
		const n = input.binding;
		stats.push({
			label: limitWords(input.formulation).lines,
			value: n === null ? missing : n.toLocaleString('en-US'),
			// A line at its limit is at 100% of its rating: red, as on the map.
			tone: n ? 'failure' : 'ok'
		});
	};
	if (OPF.has(input.formulation)) {
		// A case without generator costs solves to $0: no cost to show, so the box is left out and
		// the heaviest line takes its place.
		if (input.objective === 0) maxLoading();
		else {
			stats.push({
				label: 'Operational cost',
				value: input.objective === null ? missing : costPerHour(input.objective),
				// A relaxation's cost is a lower bound on the true operating cost.
				title: input.formulation === 'socwr' ? 'Lower bound on the AC OPF cost' : undefined
			});
		}
	} else if (input.formulation === 'acpf') {
		const l = input.losses;
		stats.push({
			label: 'Losses',
			value:
				l == null
					? missing
					: `${l.toLocaleString('en-US', { minimumFractionDigits: 1, maximumFractionDigits: 1 })} MW`
		});
		maxLoading();
		const v = input.voltage;
		stats.push(
			{
				label: 'Lowest voltage',
				value: v ? `${v.min.toFixed(3)} pu` : missing
			},
			{
				label: 'Highest voltage',
				value: v ? `${v.max.toFixed(3)} pu` : missing
			}
		);
		return stats;
	} else {
		maxLoading();
	}
	linesAtLimit();
	return stats;
}
