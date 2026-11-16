/** The quantities a dynamics plot shows: what each channel prefix of the
 * engine (and the derived `f:` and `coi:`) is, its unit and how it scales
 * from the engine's per unit. */

export type Owner = 'bus' | 'unit' | 'branch' | 'system';

export interface Quantity {
	/** Channel prefix without the colon. */
	q: string;
	short: string;
	title: string;
	unit: string;
	owner: Owner;
	digits: number;
	/** From the engine's value (`base` is the system MVA base, `f0` the frequency). */
	scale: (v: number, base: number) => number;
}

const id = (v: number) => v;
const mw = (v: number, base: number) => v * base;
const deg = (v: number) => (v * 180) / Math.PI;

export const QUANTITIES: Quantity[] = [
	{ q: 'vm', short: '|V|', title: 'voltage magnitude', unit: 'pu', owner: 'bus', digits: 4, scale: id },
	{ q: 'f', short: 'f', title: 'bus frequency', unit: 'Hz', owner: 'bus', digits: 4, scale: id },
	{ q: 'pl', short: 'P load', title: 'load active power', unit: 'MW', owner: 'bus', digits: 1, scale: mw },
	{ q: 'ql', short: 'Q load', title: 'load reactive power', unit: 'Mvar', owner: 'bus', digits: 1, scale: mw },
	{ q: 'omega', short: 'ω', title: 'rotor speed', unit: 'pu', owner: 'unit', digits: 5, scale: id },
	{ q: 'delta', short: 'δ', title: 'rotor angle', unit: 'deg', owner: 'unit', digits: 2, scale: deg },
	{ q: 'pe', short: 'P', title: 'electrical active power', unit: 'MW', owner: 'unit', digits: 1, scale: mw },
	{ q: 'qe', short: 'Q', title: 'reactive power', unit: 'Mvar', owner: 'unit', digits: 1, scale: mw },
	{ q: 'efd', short: 'Efd', title: 'field voltage', unit: 'pu', owner: 'unit', digits: 3, scale: id },
	{ q: 'tm', short: 'Tm', title: 'mechanical torque', unit: 'pu', owner: 'unit', digits: 3, scale: id },
	{ q: 'vs', short: 'Vs', title: 'stabilizer output', unit: 'pu', owner: 'unit', digits: 4, scale: id },
	{ q: 'p_from', short: 'P flow', title: 'branch active flow (from end)', unit: 'MW', owner: 'branch', digits: 1, scale: mw },
	{ q: 'q_from', short: 'Q flow', title: 'branch reactive flow (from end)', unit: 'Mvar', owner: 'branch', digits: 1, scale: mw },
	{ q: 'coi', short: 'COI', title: 'centre-of-inertia frequency', unit: 'Hz', owner: 'system', digits: 4, scale: id }
];

export const QUANTITY = new Map(QUANTITIES.map((x) => [x.q, x]));

/** The quantity of a channel key (`coi:f` is the COI frequency). */
export function quantityOf(key: string): Quantity | undefined {
	return QUANTITY.get(key.split(':')[0]);
}

/** A readable name of a channel. */
export function channelLabel(key: string): string {
	const [q, rest = ''] = key.split(/:(.*)/s);
	const x = QUANTITY.get(q);
	if (q === 'coi') return 'system frequency (COI)';
	if (!x) return key;
	if (x.owner === 'bus') return `bus ${rest} ${x.short}`;
	if (x.owner === 'branch') return `${rest.replace(/[-:]/, '–')} ${x.short}`;
	const [bus, mid] = rest.split('#');
	return `G${bus}${mid ? ` '${mid}'` : ''} ${x.short}`;
}
