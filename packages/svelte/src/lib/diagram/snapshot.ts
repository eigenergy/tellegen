import type { McComplex, McPfResult } from '@tellegen/engine';
import { neutralTerminal } from '../multiconductor.js';

/** Which reference a pair voltage is measured against. */
export type PairZone = 'phase–neutral' | 'phase–phase' | 'phase–ground';

/** The second member of a phase–ground pair. */
export const GROUND = 'ground';

/** One declared terminal-pair voltage magnitude and, when declared, its limits. */
export interface PairVoltage {
	/** Two terminals, or a terminal and {@link GROUND}. */
	pair: [string, string];
	/** Set from the bus's declared neutral, whatever that terminal is named. */
	zone: PairZone;
	volts: number;
	min: number | null;
	max: number | null;
}

export interface UnbalanceValue {
	percent: number;
	/** `VUF` uses phasors (|V2| / |V1|); the magnitude-only fallbacks name their standard. */
	definition: 'VUF' | 'IEEE PVUR' | 'NEMA LVUR';
}

export interface BusSnapshot {
	pairs: PairVoltage[];
	unbalance: UnbalanceValue | null;
}

export interface ConductorCurrent {
	terminal: string;
	amps: number;
}

/** End totals over every terminal of the element, positive into the equipment. */
export interface BranchEnd {
	p: number;
	q: number;
	currents: ConductorCurrent[];
}

export interface BranchSnapshot {
	ends: [BranchEnd | null, BranchEnd | null];
	/** Loading of the whole piece of equipment: every edge of a multi-winding transformer shows the
	 * same value. */
	loading: number | null;
	basis: string | null;
	/** The first edge of its equipment. A multi-winding transformer projects to one edge per
	 * winding pair; only its first edge counts in totals and issue lists. */
	primary: boolean;
	/** Active and reactive power the equipment consumes (kW, kvar), summed once over every port;
	 * on the primary edge only, and null when a port is missing. */
	loss: { p: number; q: number } | null;
}

/** A result snapshot indexed like the drawing: `buses[i]` and `branches[b]` follow the network order. */
export interface ResultSnapshot {
	kind: 'solved' | 'stress';
	label: string;
	iterations: number | null;
	buses: BusSnapshot[];
	branches: BranchSnapshot[];
}

/** Declared limits for one bus, by reference; a bus can declare several. Phase–neutral limits
 * list one band per phase terminal, phase–ground limits one per bounded terminal in terminal
 * order. A phase–phase band applies to its pair of terminals in either order, or to every pair
 * when `pair` is null (a uniform band). */
export interface BusLimits {
	phaseNeutral?: { min: number[]; max: number[] };
	phasePhase?: { pair: [string, string] | null; min: number; max: number }[];
	phaseGround?: { terminal: string; min: number; max: number }[];
}

export interface SnapshotBus {
	id: string;
	terminals: string[];
	/** The declared neutral; `null` when the model declares there is none, absent when it is silent
	 * (the familiar aliases then apply). */
	neutral?: string | null;
	limits?: BusLimits | null;
}

/** Ampacity per conductor, or a transformer's apparent-power rating (VA) at each bus it connects,
 * keyed by bus id. */
export type BranchRating =
	{ kind: 'current'; amps: number[] } | { kind: 'apparent'; byBus: Record<string, number> };

export interface SnapshotBranch {
	id: string;
	from: string;
	to: string;
	fromTerminals: string[];
	toTerminals: string[];
	inService: boolean;
	/** Element kind, matched against port kinds so a line and a switch sharing a name stay apart. */
	kind?: string;
	rating?: BranchRating | null;
}

const magnitude = (z: McComplex) => Math.hypot(z.re, z.im);
const sub = (a: McComplex, b: McComplex): McComplex => ({ re: a.re - b.re, im: a.im - b.im });

/** Voltage unbalance factor |V2| / |V1| in percent, from three phasors in phase order. The ratio
 * is the same for phase–neutral and phase–phase voltages, so either reference works. */
export function sequenceUnbalance(a: McComplex, b: McComplex, c: McComplex): number | null {
	// α = e^{j2π/3}: V1 = (a + α b + α² c) / 3, V2 = (a + α² b + α c) / 3.
	const h = Math.sqrt(3) / 2;
	const rot = (z: McComplex, s: number): McComplex => ({
		re: -0.5 * z.re - s * h * z.im,
		im: s * h * z.re - 0.5 * z.im
	});
	const v1 = { re: a.re + rot(b, 1).re + rot(c, -1).re, im: a.im + rot(b, 1).im + rot(c, -1).im };
	const v2 = { re: a.re + rot(b, -1).re + rot(c, 1).re, im: a.im + rot(b, -1).im + rot(c, 1).im };
	const positive = magnitude(v1);
	return positive > 0 ? (100 * magnitude(v2)) / positive : null;
}

function busSnapshot(bus: SnapshotBus, voltage: (terminal: string) => McComplex | undefined) {
	// A declared neutral (or a declared absence of one) wins; aliases only when nothing is declared.
	const neutral = neutralTerminal(bus.terminals, bus.neutral);
	const phases = bus.terminals.filter((t) => t !== neutral);
	const limits = bus.limits ?? {};
	const pairs: PairVoltage[] = [];
	const phasors: McComplex[] = [];
	const zero: McComplex = { re: 0, im: 0 };
	// Each declared reference that applies here gets its pairs. Without one, phase–neutral where
	// there is a neutral, else phase–phase on three phases.
	const ground = limits.phaseGround ?? [];
	// Phase–phase pairs in the order (1,2), (2,3), (3,1), each with its declared band.
	const triple: [string, string][] =
		phases.length === 3
			? [
					[phases[0], phases[1]],
					[phases[1], phases[2]],
					[phases[2], phases[0]]
				]
			: [];
	const declaredBands = limits.phasePhase;
	const bands = declaredBands
		? triple.map(
				([a, b]) =>
					declaredBands.find(
						({ pair }) =>
							!pair || (pair[0] === a && pair[1] === b) || (pair[0] === b && pair[1] === a)
					) ?? null
			)
		: [];
	const declared =
		(!!neutral && !!limits.phaseNeutral) || bands.some((band) => band) || ground.length > 0;
	if (neutral && (limits.phaseNeutral || !declared)) {
		const vn = voltage(neutral) ?? zero;
		phases.forEach((t, i) => {
			const v = voltage(t);
			if (!v) return;
			pairs.push({
				pair: [t, neutral],
				zone: 'phase–neutral',
				volts: magnitude(sub(v, vn)),
				min: limits.phaseNeutral?.min[i] ?? null,
				max: limits.phaseNeutral?.max[i] ?? null
			});
			phasors.push(sub(v, vn));
		});
	}
	if (triple.length && (bands.some((band) => band) || (!declared && !neutral))) {
		triple.forEach(([a, b], k) => {
			const va = voltage(a);
			const vb = voltage(b);
			if (!va || !vb) return;
			pairs.push({
				pair: [a, b],
				zone: 'phase–phase',
				volts: magnitude(sub(va, vb)),
				min: bands[k]?.min ?? null,
				max: bands[k]?.max ?? null
			});
		});
	}
	// Terminal voltages are to ground, so a phase–ground pair is the terminal's own magnitude.
	for (const { terminal, min, max } of ground) {
		const v = voltage(terminal);
		if (v)
			pairs.push({ pair: [terminal, GROUND], zone: 'phase–ground', volts: magnitude(v), min, max });
	}
	// VUF is the same from any reference: a shared neutral shift is zero sequence, and phase–phase
	// differences scale both sequences alike. Without phase–neutral phasors, the phase terminals'.
	if (!phasors.length && phases.length === 3)
		for (const t of phases) {
			const v = voltage(t);
			if (v) phasors.push(v);
		}
	const vuf = phasors.length === 3 ? sequenceUnbalance(phasors[0], phasors[1], phasors[2]) : null;
	return { pairs, unbalance: vuf === null ? null : { percent: vuf, definition: 'VUF' as const } };
}

/** Project a full multiconductor power-flow result onto the drawing's bus and branch order.
 * Missing ports leave an end `null`; nothing is interpolated. */
export function snapshotFromMcPf(
	result: McPfResult,
	buses: SnapshotBus[],
	branches: SnapshotBranch[],
	label = 'Tellegen multiconductor power flow'
): ResultSnapshot {
	// Nested maps keyed by bus, then element: no composite string keys on a 100k-bus result.
	const voltages = new Map<string, Map<string, McComplex>>();
	for (const t of result.terminals) {
		let bus = voltages.get(t.bus);
		if (!bus) voltages.set(t.bus, (bus = new Map()));
		bus.set(t.terminal, t.voltage);
	}
	type Port = McPfResult['element_ports'][number];
	const ports = new Map<string, Map<string, Port[]>>();
	for (const p of result.element_ports) {
		let element = ports.get(p.element);
		if (!element) ports.set(p.element, (element = new Map()));
		const rows = element.get(p.bus);
		if (rows) rows.push(p);
		else element.set(p.bus, [p]);
	}
	/** The element's port rows, of its own kind when a line and a switch share a name. A closed
	 * switch has no ports of its own (the solver merges its buses), so it must not borrow another
	 * element's rows. */
	const elementPorts = (branch: SnapshotBranch, bus: string): Port[] | undefined => {
		const all = ports.get(branch.id)?.get(bus);
		if (!all || branch.kind === undefined) return all;
		return all.filter((r) => r.kind === branch.kind);
	};
	const seen = new Set<string>();
	const branchSnapshots = branches.map((branch): BranchSnapshot => {
		const key = `${branch.kind ?? ''}\u0000${branch.id}`;
		const primary = !seen.has(key);
		seen.add(key);
		const ends = [branch.from, branch.to].map((bus, side): BranchEnd | null => {
			const rows = elementPorts(branch, bus);
			if (!branch.inService || !rows?.length) return null;
			let p = 0;
			let q = 0;
			const byTerminal = new Map<string, Port>();
			for (const r of rows) {
				p += r.power_into_element.re;
				q += r.power_into_element.im;
				byTerminal.set(r.terminal, r);
			}
			const currents: ConductorCurrent[] = [];
			for (const terminal of side ? branch.toTerminals : branch.fromTerminals) {
				const row = byTerminal.get(terminal);
				if (row) currents.push({ terminal, amps: magnitude(row.current_into_element) });
			}
			return { p: p / 1000, q: q / 1000, currents };
		}) as [BranchEnd | null, BranchEnd | null];
		return {
			ends,
			...loadingOf(branch, ends, elementPorts),
			primary,
			loss: primary ? equipmentLoss(branch, elementPorts) : null
		};
	});
	return {
		kind: 'solved',
		label,
		iterations: result.iterations,
		buses: buses.map((bus) => {
			const terminals = voltages.get(bus.id);
			return busSnapshot(bus, (t) => terminals?.get(t));
		}),
		branches: branchSnapshots
	};
}

/** Bus results from a live session's transferred terminal arrays (`[re, im]` interleaved), for when
 * the full result with element ports has not been fetched. Branches are left without results. */
export function snapshotBusesFromTerminals(
	ids: ArrayLike<[string, string]>,
	voltages: ArrayLike<number>,
	buses: SnapshotBus[]
): BusSnapshot[] {
	const index = new Map<string, number>();
	for (let k = 0; k < ids.length; k++) index.set(`${ids[k][0]}\u0000${ids[k][1]}`, k);
	return buses.map((bus) =>
		busSnapshot(bus, (t) => {
			const k = index.get(`${bus.id}\u0000${t}`);
			return k === undefined ? undefined : { re: voltages[2 * k], im: voltages[2 * k + 1] };
		})
	);
}

type PortRows = (
	branch: SnapshotBranch,
	bus: string
) => McPfResult['element_ports'][number][] | undefined;

/** Power summed over a set of port rows, in kW and kvar. */
function portPower(rows: McPfResult['element_ports'][number][]) {
	let p = 0;
	let q = 0;
	for (const r of rows) {
		p += r.power_into_element.re;
		q += r.power_into_element.im;
	}
	return { p: p / 1000, q: q / 1000 };
}

/** Both ends' ports are signed into the equipment, so their sum over every bus it connects is what
 * it consumes. Each bus counts once, however many winding pairs join it. */
function equipmentLoss(branch: SnapshotBranch, rows: PortRows): BranchSnapshot['loss'] {
	if (!branch.inService) return null;
	const buses = new Set([branch.from, branch.to]);
	if (branch.rating?.kind === 'apparent')
		for (const bus of Object.keys(branch.rating.byBus)) buses.add(bus);
	let p = 0;
	let q = 0;
	for (const bus of buses) {
		const at = rows(branch, bus);
		if (!at?.length) return null;
		const power = portPower(at);
		p += power.p;
		q += power.q;
	}
	return { p, q };
}

function loadingOf(
	branch: SnapshotBranch,
	ends: [BranchEnd | null, BranchEnd | null],
	rows: PortRows
): Pick<BranchSnapshot, 'loading' | 'basis'> {
	const rating = branch.rating;
	if (!rating || !ends[0] || !ends[1]) return { loading: null, basis: null };
	if (rating.kind === 'current') {
		const ratios = ends.flatMap((end, side) =>
			(end?.currents ?? []).map((c) => {
				const k = (side ? branch.toTerminals : branch.fromTerminals).indexOf(c.terminal);
				return rating.amps[k] > 0 ? c.amps / rating.amps[k] : NaN;
			})
		);
		return ratios.length && ratios.every(Number.isFinite)
			? {
					loading: 100 * Math.max(...ratios),
					basis: 'Maximum conductor |I| / rating across both ends'
				}
			: { loading: null, basis: null };
	}
	// Judged per bus against the transformer's rating there, so a winding-pair edge whose two ends
	// are the same bus (a centre tap's halves) is not counted twice.
	let loading: number | null = null;
	for (const [bus, va] of Object.entries(rating.byBus)) {
		const at = rows(branch, bus);
		if (!(va > 0) || !at?.length) continue;
		const { p, q } = portPower(at);
		const percent = (100 * Math.hypot(p, q) * 1000) / va;
		if (loading === null || percent > loading) loading = percent;
	}
	return loading === null
		? { loading: null, basis: null }
		: {
				loading,
				basis: 'Maximum |S| at a connected bus / rating there; aggregate, not per winding'
			};
}
