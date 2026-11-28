/** The quantities a dynamics plot shows: what each channel prefix of the
 * engine is, its unit and how it scales
 * from the engine's per unit.
 * @block Plotted quantities */

export type Owner = 'bus' | 'unit' | 'branch' | 'system';

export interface Quantity {
	/** Channel prefix without the colon; for a system channel, its key. */
	q: string;
	short: string;
	title: string;
	unit: string;
	owner: Owner;
	digits: number;
	/** From the engine's value (`base` is the system MVA base). */
	scale: (v: number, base: number) => number;
}

const id = (v: number) => v;
const mw = (v: number, base: number) => v * base;
const deg = (v: number) => (v * 180) / Math.PI;

export const QUANTITIES: Quantity[] = [
	{ q: 'vm', short: '|V|', title: 'voltage magnitude', unit: 'pu', owner: 'bus', digits: 4, scale: id },
	{ q: 'va', short: 'θ', title: 'Bus voltage angle (simulation reference)', unit: 'deg', owner: 'bus', digits: 2, scale: deg },
	{ q: 'f', short: 'f', title: 'Bus frequency (from its angle, filtered 20 ms)', unit: 'Hz', owner: 'bus', digits: 4, scale: id },
	{ q: 'pl', short: 'P load', title: 'load active power', unit: 'MW', owner: 'bus', digits: 1, scale: mw },
	{ q: 'ql', short: 'Q load', title: 'load reactive power', unit: 'Mvar', owner: 'bus', digits: 1, scale: mw },
	{ q: 'omega', short: 'ω', title: 'rotor speed', unit: 'pu', owner: 'unit', digits: 5, scale: id },
	{ q: 'delta', short: 'δ', title: 'Rotor angle (simulation reference)', unit: 'deg', owner: 'unit', digits: 2, scale: deg },
	{ q: 'dcoi', short: 'δ − δCOI', title: "Rotor angle from its island's centre of inertia", unit: 'deg', owner: 'unit', digits: 2, scale: deg },
	{ q: 'pe', short: 'P', title: 'electrical active power', unit: 'MW', owner: 'unit', digits: 1, scale: mw },
	{ q: 'qe', short: 'Q', title: 'reactive power', unit: 'Mvar', owner: 'unit', digits: 1, scale: mw },
	{ q: 'efd', short: 'Efd', title: 'field voltage', unit: 'pu', owner: 'unit', digits: 3, scale: id },
	{ q: 'tm', short: 'Tm', title: 'mechanical torque', unit: 'pu', owner: 'unit', digits: 3, scale: id },
	{ q: 'vs', short: 'Vs', title: 'stabilizer output', unit: 'pu', owner: 'unit', digits: 4, scale: id },
	{ q: 'p_from', short: 'P flow', title: 'branch active flow (from end)', unit: 'MW', owner: 'branch', digits: 1, scale: mw },
	{ q: 'q_from', short: 'Q flow', title: 'branch reactive flow (from end)', unit: 'Mvar', owner: 'branch', digits: 1, scale: mw }
];

/** Channels of the whole system, by key: the engine derives them at each
 * sample as a batch run's screening does (per island, in the moving
 * reference frame). */
export const SYSTEM_QUANTITIES: Quantity[] = [
	{ q: 'coi:f', short: 'f COI', title: 'System frequency (centre of inertia, largest island)', unit: 'Hz', owner: 'system', digits: 4, scale: id },
	{ q: 'coi:sep', short: 'Δδ max', title: 'Largest rotor-angle separation within an island', unit: 'deg', owner: 'system', digits: 2, scale: deg }
];

export const QUANTITY = new Map(QUANTITIES.map((x) => [x.q, x]));
const SYSTEM = new Map(SYSTEM_QUANTITIES.map((x) => [x.q, x]));

/** The quantity of a channel key (`q` names its chart). */
export function quantityOf(key: string): Quantity | undefined {
	return SYSTEM.get(key) ?? QUANTITY.get(key.split(':')[0]);
}

/** A readable name of a channel. */
export function channelLabel(key: string): string {
	const system = SYSTEM.get(key);
	if (system) return system.short;
	const [q, rest = ''] = key.split(/:(.*)/s);
	const x = QUANTITY.get(q);
	if (!x) return key;
	if (x.owner === 'bus') return `bus ${rest} ${x.short}`;
	if (x.owner === 'branch') return `${rest.replace(/[-:]/, '–')} ${x.short}`;
	const [bus, mid] = rest.split('#');
	return `G${bus}${mid ? ` '${mid}'` : ''} ${x.short}`;
}
