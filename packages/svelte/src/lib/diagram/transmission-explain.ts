/** Plain-language readings of the transmission diagram's result views: one lead sentence that
 * says what the view shows for this result, and one that says why, for readers who are learning
 * what the quantities mean. The panels keep the exact counts and notation below these. Pure, so
 * the wording is tested without a renderer. */

import type { operatingExceptions } from './transmission-inspection.js';
import { economicAmount } from './transmission-economics.js';

export type LeadTone = 'ok' | 'warning' | 'violation' | 'neutral';

export interface Lead {
	/** What this result shows, in one or two short sentences. */
	text: string;
	/** Why it looks that way, or what to do next. */
	why: string;
	tone: LeadTone;
}

/** Sections of the transmission diagrams page each view links to ("How to read this view"). */
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

/** Short explanations for the terms the panels use, shown as tooltips. */
export const TERMS = {
	rating:
		'The most power a line or transformer may carry, set by heating (its thermal rating). Loading is its flow as a percentage of this.',
	warning: 'Lines at or above the warning threshold but not over their rating.',
	atRating: 'Lines carrying exactly their rating, within a small tolerance.',
	loadingUnknown:
		'Lines whose utilization cannot be assessed because flow values or reliable rating metadata are missing.',
	outOfService: 'Lines switched out in the case; they carry no power and are drawn dashed.',
	lmp: 'Locational marginal price: what one more MW of demand at this bus would add to the total cost, per hour.',
	objectiveUnits:
		"The units of the case's generator costs per hour, per MW. MATPOWER files give costs in $/h, so for them this is $/MWh.",
	binding:
		'An enforced limit reached by the solution, within tolerance. A binding limit can have zero marginal value.',
	relief:
		'The local rate of cost reduction per additional MW of rating. Increasing the rating changes the permitted flow, not the actual flow directly. A binding limit can have negligible marginal relief.',
	activePower:
		'MW measures active power transferred to loads and losses. MVAr measures reactive power exchanged with electric and magnetic fields; both contribute to apparent power in MVA and equipment loading.',
	lossless:
		'DC power flow ignores losses, so both ends of a line carry the same MW in opposite directions.'
} as const;

const plural = (n: number, one: string, many = `${one}s`) =>
	`${n.toLocaleString()} ${n === 1 ? one : many}`;

export interface OperatingLeadInput {
	hasResult: boolean;
	/** The calculation solves voltage magnitudes (AC); DC does not. */
	voltageSolved: boolean;
	counts: ReturnType<typeof operatingExceptions>['counts'];
	/** Warning threshold in percent. */
	warning: number;
}

export function operatingLead(input: OperatingLeadInput): Lead {
	const { counts, warning } = input;
	if (!input.hasResult)
		return {
			text: 'No result yet.',
			why: 'Solve the case to see how close each line runs to its rating and, for AC, whether bus voltages stay within their limits.',
			tone: 'neutral'
		};
	const near = counts.warning + counts.atLimit;
	const voltageOut = counts.low + counts.high;
	let text: string;
	let tone: LeadTone;
	if (counts.overload > 0) {
		text = `${plural(counts.overload, 'line carries', 'lines carry')} more than ${counts.overload === 1 ? 'its' : 'their'} rating.`;
		tone = 'violation';
	} else if (near > 0) {
		text = `No checked line is over its rating, but ${near.toLocaleString()} ${near === 1 ? 'runs' : 'run'} at ${warning}% or more of it.`;
		tone = 'warning';
	} else {
		text = counts.loadingChecked
			? `${plural(counts.loadingChecked, 'checked line runs', 'checked lines run')} below ${warning}% of ${counts.loadingChecked === 1 ? 'its' : 'their'} rating.`
			: 'No line loading could be assessed.';
		tone = counts.loadingChecked ? 'ok' : 'neutral';
	}
	if (input.voltageSolved) {
		if (voltageOut > 0) {
			text += ` ${plural(voltageOut, 'bus is', 'buses are')} outside ${voltageOut === 1 ? 'its' : 'their'} voltage limits.`;
			tone = 'violation';
		} else
			text += counts.voltageChecked
				? ` No voltage violations among ${plural(counts.voltageChecked, 'checked bus', 'checked buses')}.`
				: ' No bus voltage could be assessed.';
		if (counts.voltageUnknown)
			text += ` ${plural(counts.voltageUnknown, 'bus could', 'buses could')} not be assessed.`;
	}
	if (counts.flowMissing)
		text += ` ${plural(counts.flowMissing, 'line has', 'lines have')} no usable flow value.`;
	if (counts.ratingUnknown)
		text += ` ${plural(counts.ratingUnknown, 'line has', 'lines have')} no reliable rating metadata.`;
	if (
		tone === 'ok' &&
		(counts.loadingUnknown ||
			(input.voltageSolved && (!counts.voltageChecked || counts.voltageUnknown)))
	)
		tone = 'neutral';
	const why = !input.voltageSolved
		? 'DC power flow treats every voltage as 1 pu, so it checks line loading only. Choose AC power flow or AC OPF to check voltages too.'
		: tone === 'ok'
			? `Lines turn amber from ${warning}% of their rating and red above 100%, so problems stand out; the rest stay grey. The busiest lines get a gauge.`
			: `Amber lines run at ${warning}% or more of their rating, red ones above 100%. Select a line or open Exceptions to see by how much.`;
	return { text, why, tone };
}

export interface EconomicsLeadInput {
	/** The calculation is an optimal power flow (DC OPF, AC OPF or SOCWR). */
	opf: boolean;
	/** Whether the result solves AC physics (losses and voltage), which also separate prices. */
	ac: boolean;
	/** Lowest and highest price; null when the result has none. */
	range: { lo: number; hi: number } | null;
	uniform: number | null;
	binding: number;
	thermalAvailable: boolean;
}

export function economicsLead(input: EconomicsLeadInput): Lead {
	if (!input.opf)
		return {
			text: 'Prices come from an optimal power flow (OPF).',
			why: 'Choose DC OPF, AC OPF or SOCWR as the calculation in the Network panel. An OPF finds the cheapest dispatch that respects the network limits; the price at a bus is what one more MW of demand there would cost.',
			tone: 'neutral'
		};
	if (!input.range)
		return {
			text: 'This result has no prices.',
			why: 'Solve the case again, or choose another OPF calculation, to see locational marginal prices.',
			tone: 'neutral'
		};
	const text =
		input.uniform !== null
			? `Reported bus prices are uniform at ${economicAmount(input.uniform)}.`
			: `Prices range from ${economicAmount(input.range.lo)} to ${economicAmount(input.range.hi)}.`;
	let why: string;
	if (!input.thermalAvailable) {
		why =
			'Thermal binding information is unavailable for this result. Price differences can reflect network constraints' +
			(input.ac
				? ' and AC losses; this view cannot attribute them to a particular cause.'
				: '; this view cannot attribute them to a particular cause.');
	} else if (input.binding > 0) {
		why = `${plural(input.binding, 'reported line limit binds', 'reported line limits bind')}. Binding means an enforced limit is reached, not necessarily that relaxing it reduces cost. Wide purple lines have positive marginal rating relief; inspect a line to see its value.`;
	} else {
		why =
			'No reported thermal limit binds. ' +
			(input.uniform !== null
				? 'The marginal cost of added demand is the same at the buses with reported prices; equal prices alone do not identify a single marginal generator.'
				: input.ac
					? 'AC losses, voltage limits and other constraints can also make prices differ.'
					: 'Other constraints or separate electrical islands can also make prices differ. Inspect the constraint list and network connectivity.');
	}
	return { text, why, tone: 'neutral' };
}

export interface TransferLeadInput {
	hasResult: boolean;
	awaiting: boolean;
	missing: number;
	zero: number;
	/** The busiest branch, when any carries power. */
	top: { id: number; mw: string; direction: string } | null;
	/** Width cap in MW. */
	scale: number;
}

export function transferLead(input: TransferLeadInput): Lead {
	if (!input.hasResult)
		return {
			text: input.awaiting ? 'Awaiting a result for this calculation.' : 'No result yet.',
			why: 'Solve the case to see how power moves from generators to loads.',
			tone: 'neutral'
		};
	if (!input.top)
		return {
			text: input.missing
				? `Flow values are unavailable for ${plural(input.missing, 'in-service line')}.`
				: input.zero
					? `${plural(input.zero, 'checked line carries', 'checked lines carry')} zero active power.`
					: 'No energized in-service lines to assess.',
			why: input.missing
				? 'Missing flow values do not mean zero power flow.'
				: 'This describes active power (MW); it does not establish that reactive power is zero.',
			tone: 'neutral'
		};
	return {
		text: `${input.missing ? 'Among reported flows, the busiest line' : 'The busiest line'}, branch ${input.top.id}, carries ${input.top.mw} (${input.top.direction}).${input.missing ? ` ${plural(input.missing, 'line has', 'lines have')} no usable flow value.` : ''}`,
		why: `Wider lines carry more power, up to ${input.scale.toLocaleString()} MW; zoom in to see chevrons pointing the way it flows. Power runs from generators towards loads; Operating limits shows which lines run close to their rating.`,
		tone: 'neutral'
	};
}
