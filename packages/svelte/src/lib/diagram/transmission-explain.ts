/** The key numbers at the top of each transmission diagram view, as labelled values. Pure, so
 * the choice of numbers is tested without a renderer. The docs page explains the quantities. */

import type { operatingExceptions } from './transmission-inspection.js';
import type { Stat } from './verdict.js';

/** A view's summary: labelled numbers, or one short line when there is nothing to count. */
export interface Lead {
	stats: Stat[];
	/** Shown in place of the numbers: "No result", "No prices". */
	empty?: string;
}

/** Sections of the transmission diagrams page each view links to. */
export const DOC_SECTIONS = {
	voltage: 'voltage-levels',
	operating: 'results',
	economics: 'opf-economics',
	transfer: 'power-transfers'
} as const;

/** The docs page section for a view, from the docs root (`config.docsHref`). */
export function docsLink(root: string, view: keyof typeof DOC_SECTIONS): string {
	const base = root.endsWith('/') ? root : `${root}/`;
	return `${base}transmission-diagrams.html#${DOC_SECTIONS[view]}`;
}

const n = (value: number) => value.toLocaleString('en-US');

export interface OperatingLeadInput {
	hasResult: boolean;
	/** The calculation solves voltage magnitudes (AC); DC does not. */
	voltageSolved: boolean;
	counts: ReturnType<typeof operatingExceptions>['counts'];
	/** Warning threshold in percent; the Key shows it. */
	warning: number;
}

/** Operating limits: a few counts, each shown only when it is not zero: lines at or over their
 * rating, lines from the warning level up to their rating, buses outside their voltage limits (AC), and what
 * could not be checked. With nothing to count, one short line says so; it reads as an all-clear
 * only when every line and (AC) every bus was checked. */
export function operatingLead(input: OperatingLeadInput): Lead {
	const { counts } = input;
	if (!input.hasResult) return { stats: [], empty: 'No result' };
	const stats: Stat[] = [];
	// One name for a line at or over its rating, as in the Network panel.
	const atLimit = counts.overload + counts.atLimit;
	if (atLimit) stats.push({ label: 'Lines at limit', value: n(atLimit), tone: 'failure' });
	if (counts.warning)
		stats.push({
			label: `${input.warning}–100%`,
			value: n(counts.warning),
			tone: 'warning'
		});
	if (input.voltageSolved) {
		if (counts.low)
			stats.push({
				label: 'Low voltage',
				value: n(counts.low),
				tone: 'failure'
			});
		if (counts.high)
			stats.push({
				label: 'High voltage',
				value: n(counts.high),
				tone: 'failure'
			});
	}
	const voltageUnchecked = input.voltageSolved ? counts.voltageUnknown : 0;
	if (voltageUnchecked) stats.push({ label: 'Voltage unchecked', value: n(voltageUnchecked) });
	if (counts.flowMissing) stats.push({ label: 'No flow', value: n(counts.flowMissing) });
	if (counts.ratingUnknown) stats.push({ label: 'Unrated', value: n(counts.ratingUnknown) });
	if (counts.inactive) stats.push({ label: 'Out of service', value: n(counts.inactive) });
	if (stats.length) return { stats };
	if (!counts.loadingChecked) return { stats: [], empty: 'No line loading' };
	if (input.voltageSolved && counts.voltageChecked === 0)
		return { stats: [], empty: 'No voltages checked' };
	return { stats: [], empty: `All lines below ${input.warning}%` };
}

export interface EconomicsLeadInput {
	/** The calculation is an optimal power flow (DC OPF, AC OPF or relaxed AC OPF). */
	opf: boolean;
	/** Lowest and highest price; null when the result has none. */
	range: { lo: number; hi: number } | null;
	uniform: number | null;
}

/** A price in $/MWh with a fixed number of decimals, so the ends of a range read alike. */
export function priceNumber(value: number, digits: number): string {
	return (Math.abs(value) < 5e-7 ? 0 : value).toLocaleString('en-US', {
		minimumFractionDigits: digits,
		maximumFractionDigits: digits
	});
}

/** OPF prices: the lowest and highest price, to the same precision. The lines at their limit are
 * in the Network panel and the list below, so they are not repeated here. */
export function economicsLead(input: EconomicsLeadInput): Lead {
	if (!input.opf) return { stats: [], empty: 'No prices in power flow' };
	if (!input.range) return { stats: [], empty: 'No prices' };
	const { lo, hi } = input.range;
	const digits = Math.max(Math.abs(lo), Math.abs(hi)) >= 100 ? 0 : 2;
	if (input.uniform !== null)
		return {
			stats: [
				{
					label: 'Price',
					value: priceNumber(input.uniform, digits),
					sub: '$/MWh'
				}
			]
		};
	return {
		stats: [
			{ label: 'Lowest', value: priceNumber(lo, digits), sub: '$/MWh' },
			{ label: 'Highest', value: priceNumber(hi, digits), sub: '$/MWh' }
		]
	};
}

export interface TransferLeadInput {
	hasResult: boolean;
	missing: number;
	zero: number;
	/** The busiest branch, when any carries power. */
	top: { id: number; mw: string } | null;
}

/** Power flows: the largest flow, and the lines with no flow or no value. */
export function transferLead(input: TransferLeadInput): Lead {
	if (!input.hasResult) return { stats: [], empty: 'No result' };
	const stats: Stat[] = [];
	if (input.top)
		stats.push({
			label: 'Largest flow',
			value: input.top.mw,
			sub: `Line ${input.top.id}`
		});
	if (input.zero) stats.push({ label: 'Zero flow', value: n(input.zero) });
	if (input.missing) stats.push({ label: 'No flow', value: n(input.missing) });
	return stats.length ? { stats } : { stats: [], empty: 'No flows' };
}
