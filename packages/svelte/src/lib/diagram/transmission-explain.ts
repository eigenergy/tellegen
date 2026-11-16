/** The key numbers at the top of each transmission diagram view, as labelled values. Pure, so
 * the choice of numbers is tested without a renderer. The docs page explains the quantities. */

import type { operatingExceptions } from './transmission-inspection.js';
import { economicAmount } from './transmission-economics.js';
import type { Stat } from './verdict.js';

/** A view's summary: labelled numbers, or one short line when there is nothing to count. */
export interface Lead {
	stats: Stat[];
	/** Shown in place of the numbers: "No result", "No prices". */
	empty?: string;
}

/** Sections of the transmission diagrams page each view links to. */
export const DOC_SECTIONS = {
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
	/** Warning threshold in percent. */
	warning: number;
}

/** Operating limits: the lines over, at and near their rating, and (AC) the buses outside their
 * voltage limits. Zero counts are left out; with none left, the view is clear. */
export function operatingLead(input: OperatingLeadInput): Lead {
	const { counts, warning } = input;
	if (!input.hasResult) return { stats: [], empty: 'No result' };
	const stats: Stat[] = [];
	if (counts.overload)
		stats.push({ label: 'Overloaded', value: n(counts.overload), tone: 'failure' });
	if (counts.atLimit) stats.push({ label: 'At rating', value: n(counts.atLimit), tone: 'failure' });
	if (counts.warning)
		stats.push({ label: `≥${warning}%`, value: n(counts.warning), tone: 'warning' });
	if (input.voltageSolved) {
		if (counts.low) stats.push({ label: 'Low voltage', value: n(counts.low), tone: 'failure' });
		if (counts.high) stats.push({ label: 'High voltage', value: n(counts.high), tone: 'failure' });
	}
	// Nothing over a limit reads as 0 only when some line was actually checked.
	if (!stats.length)
		stats.push(
			counts.loadingChecked
				? { label: 'Over limit', value: '0', tone: 'ok' }
				: { label: 'Over limit', value: '—' }
		);
	if (counts.flowMissing) stats.push({ label: 'No data', value: n(counts.flowMissing) });
	if (counts.ratingUnknown) stats.push({ label: 'Unrated', value: n(counts.ratingUnknown) });
	return { stats };
}

export interface EconomicsLeadInput {
	/** The calculation is an optimal power flow (DC OPF, AC OPF or SOCWR). */
	opf: boolean;
	/** Lowest and highest price; null when the result has none. */
	range: { lo: number; hi: number } | null;
	uniform: number | null;
	binding: number;
	thermalAvailable: boolean;
}

/** OPF prices: the price range and the lines at their limit. */
export function economicsLead(input: EconomicsLeadInput): Lead {
	if (!input.opf) return { stats: [], empty: 'Needs an OPF' };
	if (!input.range) return { stats: [], empty: 'No prices' };
	const stats: Stat[] = [
		{
			label: 'Price',
			value:
				input.uniform !== null
					? economicAmount(input.uniform)
					: `${economicAmount(input.range.lo)}–${economicAmount(input.range.hi)}`
		}
	];
	if (input.thermalAvailable) stats.push({ label: 'Lines at limit', value: n(input.binding) });
	return { stats };
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
		stats.push({ label: 'Largest flow', value: input.top.mw, sub: `Line ${input.top.id}` });
	if (input.zero) stats.push({ label: 'Zero flow', value: n(input.zero) });
	if (input.missing) stats.push({ label: 'No data', value: n(input.missing) });
	return stats.length ? { stats } : { stats: [], empty: 'No flows' };
}
