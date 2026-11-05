/** Plain-language readings of the transmission diagram's result views: one lead sentence that
 * says what the view shows for this result, and one that says why, for readers who are learning
 * what the quantities mean. The panels keep the exact counts and notation below these. Pure, so
 * the wording is tested without a renderer. */

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
	loadingUnknown: 'Lines with no rating in the case, so their loading cannot be checked.',
	outOfService: 'Lines switched out in the case; they carry no power and are drawn dashed.',
	lmp: 'Locational marginal price: what one more MW of demand at this bus would add to the total cost, per hour.',
	objectiveUnits:
		"The units of the case's generator costs per hour, per MW. MATPOWER files give costs in $/h, so for them this is $/MWh.",
	binding:
		'A limit the optimum sits exactly at: the solver would use more of it if it could, so it holds prices apart.',
	relief:
		'How much the total cost would fall, per hour, if this line could carry one more MW. Zero when the limit does not bind.',
	activePower:
		'Active power (MW): the power that does work, as opposed to reactive power (MVAr), which only sustains voltage.',
	lossless:
		'DC power flow ignores losses, so both ends of a line carry the same MW in opposite directions.'
} as const;

const plural = (n: number, one: string, many = `${one}s`) =>
	`${n.toLocaleString()} ${n === 1 ? one : many}`;

export interface OperatingLeadInput {
	hasResult: boolean;
	/** The calculation solves voltage magnitudes (AC); DC does not. */
	voltageSolved: boolean;
	voltageAvailable: boolean;
	counts: {
		low: number;
		high: number;
		overload: number;
		warning: number;
		atLimit: number;
		loadingUnknown: number;
	};
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
		text = `No line is over its rating, but ${near.toLocaleString()} ${near === 1 ? 'runs' : 'run'} at ${warning}% or more of it.`;
		tone = 'warning';
	} else {
		text = `Every rated line runs below ${warning}% of its rating.`;
		tone = 'ok';
	}
	if (input.voltageAvailable) {
		if (voltageOut > 0) {
			text += ` ${plural(voltageOut, 'bus is', 'buses are')} outside ${voltageOut === 1 ? 'its' : 'their'} voltage limits.`;
			tone = 'violation';
		} else text += ' Every bus voltage is within its limits.';
	}
	if (counts.loadingUnknown > 0)
		text += ` ${plural(counts.loadingUnknown, 'line has', 'lines have')} no rating to check.`;
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
	prices: number[];
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
	if (!input.prices.length)
		return {
			text: 'This result has no prices.',
			why: 'Solve the case again, or choose another OPF calculation, to see locational marginal prices.',
			tone: 'neutral'
		};
	const bindingText = input.thermalAvailable
		? `${plural(input.binding, 'line limit binds', 'line limits bind')}`
		: '';
	if (input.uniform !== null)
		return {
			text: `Every bus has the same price: ${economicAmount(input.uniform)}.`,
			why:
				input.thermalAvailable && input.binding > 0
					? 'A line limit binds here, but relaxing it would not lower the cost, so prices stay equal. When a binding limit costs something, prices separate on either side of it.'
					: 'No line limit holds prices apart, so the cheapest generators can reach every bus and one marginal generator sets the price everywhere. When a line reaches its limit, prices separate on either side of it.',
			tone: 'ok'
		};
	const lo = Math.min(...input.prices);
	const hi = Math.max(...input.prices);
	const text = `Prices range from ${economicAmount(lo)} to ${economicAmount(hi)}.`;
	const why =
		input.thermalAvailable && input.binding > 0
			? `${bindingText} (wide purple): power cannot flow further across ${input.binding === 1 ? 'it' : 'them'}, so cheaper generation cannot reach the dearer side and prices separate.`
			: input.ac
				? 'No thermal limit binds; in AC, losses and voltage limits also make prices differ from bus to bus.'
				: 'No thermal limit binds; other constraints in the case hold the prices apart. See the constraint list.';
	return { text, why, tone: 'warning' };
}

export interface TransferLeadInput {
	hasResult: boolean;
	/** The busiest branch, when any carries power. */
	top: { id: number; mw: string; direction: string } | null;
	/** Width cap in MW. */
	scale: number;
}

export function transferLead(input: TransferLeadInput): Lead {
	if (!input.hasResult)
		return {
			text: 'No result yet.',
			why: 'Solve the case to see how power moves from generators to loads.',
			tone: 'neutral'
		};
	if (!input.top)
		return {
			text: 'No line carries power in this result.',
			why: 'Check that the case has demand and in-service generation.',
			tone: 'neutral'
		};
	return {
		text: `The busiest line, branch ${input.top.id}, carries ${input.top.mw} (${input.top.direction}).`,
		why: `Wider lines carry more power, up to ${input.scale.toLocaleString()} MW; zoom in to see chevrons pointing the way it flows. Power runs from generators towards loads; Operating limits shows which lines run close to their rating.`,
		tone: 'neutral'
	};
}
