/** The events of a dynamics run as a person states them ("a fault at bus
 * 1101, cleared after 6 cycles") and as the engine runs them (`RmsEvent`).
 * Pure: the defaults, the cycle arithmetic, the sentences and the checks are
 * tested without a renderer. The engine checks every event again; its
 * refusal is shown verbatim when it disagrees. */

import type { RmsEvent, RmsInspection } from '@tellegen/engine';

export const MAX_EVENTS = 16;

/** Defaults: everything starts at 1 s, a fault clears after 6 cycles. */
export const DEFAULT_EVENT_TIME = 1.0;
export const DEFAULT_CLEARING_CYCLES = 6;
/** A bolted fault: through j1e-4 pu (system base). */
export const DEFAULT_FAULT_X = 1e-4;
/** A line fault sits this fraction of the line from the near end. */
export const LINE_FAULT_OFFSET = 0.01;

export type DynEvent =
	| {
			id: string;
			kind: 'bus_fault';
			bus: number;
			at: number;
			clearAfter: number;
			r: number;
			x: number;
	  }
	| {
			id: string;
			kind: 'line_fault';
			from: number;
			to: number;
			circuit: string;
			nearBus: number;
			at: number;
			clearAfter: number;
	  }
	| { id: string; kind: 'branch_trip'; from: number; to: number; circuit: string; at: number }
	| { id: string; kind: 'generator_trip'; bus: number; machineId: string; at: number }
	| { id: string; kind: 'load_change'; bus: number; at: number; scale: number };

export type DynEventKind = DynEvent['kind'];

let seq = 0;
const nextId = () => `ev-${++seq}`;

export const cyclesToSeconds = (cycles: number, f0: number): number => cycles / f0;
export const secondsToCycles = (seconds: number, f0: number): number => seconds * f0;

/** The default clearing time at `f0` Hz: 6 cycles (0.1 s at 60 Hz, 0.12 s at 50 Hz). */
export const defaultClearing = (f0: number): number => cyclesToSeconds(DEFAULT_CLEARING_CYCLES, f0);

export function busFault(bus: number, f0: number, over: Partial<DynEvent> = {}): DynEvent {
	return {
		id: nextId(),
		kind: 'bus_fault',
		bus,
		at: DEFAULT_EVENT_TIME,
		clearAfter: defaultClearing(f0),
		r: 0,
		x: DEFAULT_FAULT_X,
		...(over as object)
	} as DynEvent;
}

export function lineFault(
	from: number,
	to: number,
	circuit: string,
	nearBus: number,
	f0: number
): DynEvent {
	return {
		id: nextId(),
		kind: 'line_fault',
		from,
		to,
		circuit,
		nearBus,
		at: DEFAULT_EVENT_TIME,
		clearAfter: defaultClearing(f0)
	};
}

export const branchTrip = (from: number, to: number, circuit: string): DynEvent => ({
	id: nextId(),
	kind: 'branch_trip',
	from,
	to,
	circuit,
	at: DEFAULT_EVENT_TIME
});

export const generatorTrip = (bus: number, machineId: string): DynEvent => ({
	id: nextId(),
	kind: 'generator_trip',
	bus,
	machineId,
	at: DEFAULT_EVENT_TIME
});

/** A load step of `percent` (−100…+100 %). */
export const loadStep = (bus: number, percent = 10): DynEvent => ({
	id: nextId(),
	kind: 'load_change',
	bus,
	at: DEFAULT_EVENT_TIME,
	scale: 1 + percent / 100
});

/** The engine's events for one logical event. */
export function toRmsEvents(e: DynEvent): RmsEvent[] {
	switch (e.kind) {
		case 'bus_fault':
			return [
				{ kind: 'bus_fault', time: e.at, bus: e.bus, r: e.r, x: e.x },
				{ kind: 'clear_fault', time: e.at + e.clearAfter, bus: e.bus }
			];
		case 'line_fault': {
			const clear = e.at + e.clearAfter;
			const location = e.nearBus === e.from ? LINE_FAULT_OFFSET : 1 - LINE_FAULT_OFFSET;
			const line = { from: e.from, to: e.to, circuit: e.circuit };
			return [
				{
					kind: 'line_fault',
					time: e.at,
					...line,
					location,
					r: 0,
					x: DEFAULT_FAULT_X,
					permanent: true
				},
				{ kind: 'breaker_open', time: clear, ...line, at: e.from },
				{ kind: 'breaker_open', time: clear, ...line, at: e.to }
			];
		}
		case 'branch_trip':
			return [{ kind: 'branch_trip', time: e.at, from: e.from, to: e.to, circuit: e.circuit }];
		case 'generator_trip':
			return [{ kind: 'generator_trip', time: e.at, bus: e.bus, machine_id: e.machineId }];
		case 'load_change':
			return [{ kind: 'load_change', time: e.at, bus: e.bus, scale: e.scale }];
	}
}

export const allRmsEvents = (events: readonly DynEvent[]): RmsEvent[] =>
	events.flatMap(toRmsEvents).sort((a, b) => a.time - b.time);

/** A unit's key as the engine's channels name it: `bus`, or `bus#id`. */
export const unitKey = (bus: number, id: string): string =>
	id.trim() === '1' ? String(bus) : `${bus}#${id.trim()}`;

const fmtS = (s: number) => `${s.toFixed(3)} s`;
export const fmtCycles = (s: number, f0: number) => {
	const c = secondsToCycles(s, f0);
	const r = Math.round(c * 10) / 10;
	return `${Number.isInteger(r) ? r.toFixed(0) : r.toFixed(1)} cycles`;
};

export type BusNames = (bus: number) => string | null | undefined;

const busLabel = (bus: number, names?: BusNames) => {
	const n = names?.(bus);
	return n ? `bus ${bus} ${n}` : `bus ${bus}`;
};

/** The event in one plain sentence. */
export function sentence(e: DynEvent, f0: number, names?: BusNames): string {
	switch (e.kind) {
		case 'bus_fault': {
			const z = e.r === 0 && e.x <= DEFAULT_FAULT_X ? '' : ` through ${e.r} + j${e.x} pu`;
			return `Three-phase fault at ${busLabel(e.bus, names)}${z} at ${fmtS(e.at)}, cleared after ${fmtCycles(e.clearAfter, f0)}`;
		}
		case 'line_fault':
			return `Three-phase fault on line ${e.from}–${e.to} circuit ${e.circuit} near bus ${e.nearBus} at ${fmtS(e.at)}, cleared by opening the line after ${fmtCycles(e.clearAfter, f0)}`;
		case 'branch_trip':
			return `Trip of branch ${e.from}–${e.to} circuit ${e.circuit} at ${fmtS(e.at)}`;
		case 'generator_trip':
			return `Trip of unit ${e.bus} '${e.machineId}' at ${fmtS(e.at)}`;
		case 'load_change': {
			const pct = Math.round((e.scale - 1) * 1000) / 10;
			return `Load at ${busLabel(e.bus, names)} ${pct >= 0 ? '+' : ''}${pct} % at ${fmtS(e.at)}`;
		}
	}
}

export interface EventContext {
	tEnd: number;
	inspection: Pick<RmsInspection, 'buses' | 'branches' | 'generators' | 'load_buses'> &
		Partial<Pick<RmsInspection, 'held_units' | 'plants'>>;
}

const sameBranch = (b: [number, number, string], from: number, to: number, ckt: string) =>
	((b[0] === from && b[1] === to) || (b[0] === to && b[1] === from)) && b[2].trim() === ckt.trim();

/** Why an event cannot run, mirroring the engine's checks; null when it can. */
export function validate(e: DynEvent, ctx: EventContext): string | null {
	const { tEnd, inspection } = ctx;
	if (!Number.isFinite(e.at) || e.at < 0 || e.at >= tEnd)
		return `the time must lie in [0, ${tEnd}) s`;
	if (e.kind === 'bus_fault' || e.kind === 'line_fault') {
		if (!(e.clearAfter > 0)) return 'the fault must clear after it starts';
		if (e.at + e.clearAfter >= tEnd) return 'the fault must clear before the run ends';
	}
	const hasBus = (b: number) => inspection.buses.includes(b);
	switch (e.kind) {
		case 'bus_fault':
			if (!hasBus(e.bus)) return `no bus ${e.bus}`;
			if (!(e.r >= 0 && e.x >= 0)) return 'the fault impedance needs r ≥ 0 and x ≥ 0';
			if (!(Math.hypot(e.r, e.x) > 0)) return 'the fault needs a nonzero impedance';
			return null;
		case 'line_fault':
		case 'branch_trip':
			if (!inspection.branches.some((b) => sameBranch(b, e.from, e.to, e.circuit)))
				return `no branch ${e.from}–${e.to} circuit ${e.circuit}`;
			if (e.kind === 'line_fault' && e.nearBus !== e.from && e.nearBus !== e.to)
				return `bus ${e.nearBus} is not an end of ${e.from}–${e.to}`;
			return null;
		case 'generator_trip': {
			const key = unitKey(e.bus, e.machineId);
			const known =
				inspection.generators.some(
					(g) => g.bus === e.bus && g.machine_id.trim() === e.machineId.trim()
				) ||
				(inspection.held_units ?? []).includes(key) ||
				(inspection.plants ?? []).includes(key);
			return known ? null : `no unit ${e.bus} '${e.machineId}'`;
		}
		case 'load_change':
			if (!inspection.load_buses.includes(e.bus)) return `bus ${e.bus} carries no load`;
			if (!(Number.isFinite(e.scale) && e.scale >= 0)) return 'the load scale must be ≥ 0';
			return null;
	}
}

/** Markers on the timeline: when something happens and what. */
export interface EventMarker {
	t: number;
	kind: 'fault' | 'clear' | 'trip' | 'load';
	label: string;
	eventId: string;
}

export function markers(events: readonly DynEvent[], f0: number, names?: BusNames): EventMarker[] {
	const out: EventMarker[] = [];
	for (const e of events) {
		const label = sentence(e, f0, names);
		if (e.kind === 'bus_fault' || e.kind === 'line_fault') {
			out.push({ t: e.at, kind: 'fault', label, eventId: e.id });
			out.push({
				t: e.at + e.clearAfter,
				kind: 'clear',
				label: `Cleared: ${label}`,
				eventId: e.id
			});
		} else
			out.push({
				t: e.at,
				kind: e.kind === 'load_change' ? 'load' : 'trip',
				label,
				eventId: e.id
			});
	}
	return out.sort((a, b) => a.t - b.t);
}

/** The parallel circuits between two buses, in either order. */
export function circuitsBetween(
	branches: readonly [number, number, string][],
	a: number,
	b: number
): string[] {
	return branches
		.filter((x) => (x[0] === a && x[1] === b) || (x[0] === b && x[1] === a))
		.map((x) => x[2].trim());
}
