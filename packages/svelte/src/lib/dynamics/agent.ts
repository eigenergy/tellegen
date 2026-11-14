/** `simulate_dynamics` for agents: element ids as an agent writes them,
 * resolved against the case's inspection, and a compact summary of the
 * screened run that fits the WebMCP output budget. Pure. */

import type { RmsInspection, RmsRun } from '@tellegen/engine';
import {
	branchTrip,
	busFault,
	circuitsBetween,
	generatorTrip,
	lineFault,
	type DynEvent
} from './events.js';
import { MAX_TRACE_SAMPLES } from '@tellegen/webmcp';
import { coiRelativeDeg } from './series.js';

type JsonValue = string | number | boolean | null | JsonValue[] | { [key: string]: JsonValue };

export type AgentEvent =
	| {
			kind: 'bus_fault';
			bus: number;
			atS: number;
			clearAfterS: number;
			xPu: number;
			rPu: number;
	  }
	| {
			kind: 'line_fault';
			branch: string;
			nearBus?: number;
			atS: number;
			clearAfterS: number;
	  }
	| { kind: 'branch_trip'; branch: string; atS: number }
	| { kind: 'generator_trip'; unit: string; atS: number }
	| { kind: 'load_change'; bus: number; atS: number; scale: number };

/** A refusal naming what the agent can use instead. */
export class AgentInputError extends Error {}

function branch(ins: RmsInspection, id: string): { from: number; to: number; circuit: string } {
	const m = /^(\d+)-(\d+)(?:#(\w+))?$/.exec(id);
	if (!m) throw new AgentInputError(`branch ${id} must read from-to or from-to#circuit`);
	const from = Number(m[1]);
	const to = Number(m[2]);
	const circuits = circuitsBetween(ins.branches, from, to);
	if (circuits.length === 0) throw new AgentInputError(`no in-service branch ${from}-${to}`);
	if (m[3] !== undefined) {
		if (!circuits.includes(m[3]))
			throw new AgentInputError(
				`branch ${from}-${to} has circuits ${circuits.join(' and ')}, not ${m[3]}`
			);
		return { from, to, circuit: m[3] };
	}
	if (circuits.length > 1)
		throw new AgentInputError(
			`branch ${from}-${to} has circuits ${circuits.join(' and ')}; name one, such as ${from}-${to}#${circuits[0]}`
		);
	return { from, to, circuit: circuits[0] };
}

function bus(ins: RmsInspection, b: number): number {
	if (!ins.buses.includes(b)) throw new AgentInputError(`no bus ${b} in the simulated network`);
	return b;
}

/** The panel's events for the agent's. */
export function resolveAgentEvents(ins: RmsInspection, events: readonly AgentEvent[]): DynEvent[] {
	const f0 = ins.base_frequency;
	return events.map((e) => {
		switch (e.kind) {
			case 'bus_fault':
				return busFault(bus(ins, e.bus), f0, {
					at: e.atS,
					clearAfter: e.clearAfterS,
					x: e.xPu,
					r: e.rPu
				} as Partial<DynEvent>);
			case 'line_fault': {
				const b = branch(ins, e.branch);
				const near = e.nearBus ?? b.from;
				if (near !== b.from && near !== b.to)
					throw new AgentInputError(`near_bus ${near} is not an end of ${b.from}-${b.to}`);
				return {
					...lineFault(b.from, b.to, b.circuit, near, f0),
					at: e.atS,
					clearAfter: e.clearAfterS
				};
			}
			case 'branch_trip': {
				const b = branch(ins, e.branch);
				return { ...branchTrip(b.from, b.to, b.circuit), at: e.atS };
			}
			case 'generator_trip': {
				const [busPart, idPart] = e.unit.split('#');
				const b = Number(busPart);
				const id = idPart ?? '1';
				const atBus = [
					...ins.generators.filter((g) => g.bus === b).map((g) => g.machine_id.trim()),
					...[...(ins.held_units ?? []), ...(ins.plants ?? [])]
						.filter((k) => Number(k.split('#')[0]) === b)
						.map((k) => k.split('#')[1] ?? '1')
				];
				if (!atBus.includes(id))
					throw new AgentInputError(
						atBus.length
							? `no unit ${e.unit}; bus ${b} has units ${atBus.map((x) => `${b}#${x}`).join(', ')}`
							: `no generating unit at bus ${b}`
					);
				return { ...generatorTrip(b, id), at: e.atS };
			}
			case 'load_change':
				if (!ins.load_buses.includes(e.bus))
					throw new AgentInputError(`bus ${e.bus} carries no load`);
				return {
					id: `agent-${Math.random().toString(36).slice(2)}`,
					kind: 'load_change',
					bus: e.bus,
					at: e.atS,
					scale: e.scale
				};
		}
	});
}

const r = (v: number | null | undefined, digits = 5): number | null =>
	v == null || !Number.isFinite(v) ? null : Number(v.toPrecision(digits));
/** A time, s, as the traces give it: to 0.1 ms, so a sample's time and a
 * screening figure's time compare equal. */
const ts = (t: number | null | undefined): number | null =>
	t == null || !Number.isFinite(t) ? null : Number(t.toFixed(4));

/** One trace for an agent: `coi:f`, `f:`, `vm:`, `omega:`, `dcoi:` (degrees
 * from the COI) or `pl:` (MW). */
function trace(run: RmsRun, key: string): ArrayLike<number> | undefined {
	if (key.startsWith('dcoi:')) return coiRelativeDeg(run, key.slice(5));
	if (key.startsWith('pl:')) {
		const s = run.series(key);
		return s?.map((v) => v * run.meta.base_mva);
	}
	return run.series(key);
}

/** The indices of at most `n` samples of `d` (over `time`) that keep its
 * shape: its lowest and highest finite samples first, then the samples
 * around each event time (the last one at or before it and the first one
 * after), the ends, and then the lowest and highest sample of each of as
 * many equal time buckets as fit (M4-style). Sorted, without repeats. */
export function decimate(
	time: ArrayLike<number>,
	d: ArrayLike<number>,
	n: number,
	eventTimes: readonly number[] = []
): number[] {
	const len = Math.min(time.length, d.length);
	if (len === 0 || n <= 0) return [];
	const extrema = (lo: number, hi: number): number[] => {
		let iMin = -1;
		let iMax = -1;
		for (let i = lo; i < hi; i++) {
			const v = d[i];
			if (!Number.isFinite(v)) continue;
			if (iMin < 0 || v < d[iMin]) iMin = i;
			if (iMax < 0 || v > d[iMax]) iMax = i;
		}
		return iMin < 0 ? [] : [iMin, iMax];
	};
	const tol = 1e-9;
	const firstAfter = (t: number): number => {
		let lo = 0;
		let hi = len;
		while (lo < hi) {
			const mid = (lo + hi) >> 1;
			if (time[mid] <= t + tol) lo = mid + 1;
			else hi = mid;
		}
		return lo;
	};
	const must: number[] = [...extrema(0, len)];
	for (const te of [...eventTimes].sort((a, b) => a - b)) {
		const k = firstAfter(te);
		if (k > 0) must.push(k - 1);
		if (k < len) must.push(k);
	}
	must.push(0, len - 1);
	const kept = [...new Set(must)].slice(0, n);
	if (kept.length >= n) return kept.sort((a, b) => a - b);
	const t0 = time[0];
	const span = time[len - 1] - t0;
	let best = kept;
	for (let buckets = 1; buckets <= n; buckets++) {
		const pick = new Set(kept);
		let lo = 0;
		for (let b = 1; b <= buckets; b++) {
			const hi = b === buckets ? len : firstAfter(t0 + (span * b) / buckets - 2 * tol);
			for (const i of extrema(lo, Math.max(lo, hi))) pick.add(i);
			lo = Math.max(lo, hi);
		}
		if (pick.size > n) {
			// Fill the last slots from the finer grid, earliest first.
			const extra = [...pick].filter((i) => !best.includes(i)).sort((a, b) => a - b);
			best = [...best, ...extra.slice(0, n - best.length)];
			break;
		}
		best = [...pick];
	}
	return best.sort((a, b) => a - b);
}

/** The compact summary `simulate_dynamics` returns. `budget` is the most
 * characters its JSON may take: traces shrink (fewer samples each, then
 * fewer channels) until it fits, and `traces_truncated` says so. */
export function agentSummary(
	run: RmsRun,
	opts: {
		channels: readonly string[];
		tracePoints: number;
		wallMs: number;
		loadModel: string;
		dtS: number;
		budget?: number;
	}
): Record<string, JsonValue> {
	const s = run.screening;
	const tEnd = run.time[run.time.length - 1];
	const out: Record<string, JsonValue> = {
		verdict: s.verdict,
		reasons: s.reasons.map((x) => x.text.slice(0, 120)).slice(0, 3),
		criteria: s.criteria_name,
		frequency: {
			nominal_hz: s.frequency.nominal_hz,
			coi_nadir_hz: r(s.frequency.coi_nadir_hz),
			nadir_t_s: ts(s.frequency.nadir_t),
			max_rocof_hz_s: r(s.frequency.max_rocof_hz_s, 3),
			final_hz: r(s.frequency.final_hz),
			settled: s.frequency.settled,
			lowest_bus_hz: r(s.frequency.lowest_bus_hz),
			lowest_bus: s.frequency.lowest_bus ?? null
		},
		angle: {
			max_separation_deg: r(s.angle.max_separation_deg, 4),
			pair: [...s.angle.pair],
			pole_slips: s.angle.pole_slips.length,
			trend: s.angle.trend ?? null
		},
		voltage: {
			min_pu: r(s.voltage.min_pu),
			min_bus: s.voltage.min_bus ?? null,
			recovered_s: r(s.voltage.recovered_s, 3)
		},
		islands: s.islands.length,
		held_units: s.held_units,
		notes: run.notes.length,
		...(run.notes.length ? { note_head: run.notes[0].slice(0, 120) } : {}),
		run: {
			t_end_s: tEnd,
			dt_s: opts.dtS,
			steps: run.steps,
			wall_ms: Math.round(opts.wallMs),
			load_model: opts.loadModel
		}
	};
	if (opts.tracePoints <= 0 || !opts.channels.length) return out;
	const data = opts.channels
		.map((key) => ({ key, d: trace(run, key) }))
		.filter((c): c is { key: string; d: ArrayLike<number> } => c.d !== undefined);
	if (!data.length) return out;
	const eventTimes = (run.request?.events ?? []).map((e) => e.time);
	const build = (channels: typeof data, n: number) => {
		const traces: Record<string, JsonValue> = {};
		for (const { key, d } of channels) {
			const idx = decimate(run.time, d, n, eventTimes);
			traces[key] = {
				t_s: idx.map((i) => ts(run.time[i])),
				v: idx.map((i) => r(d[i]))
			};
		}
		return traces;
	};
	const budget = opts.budget ?? Infinity;
	const fits = (extra: Record<string, JsonValue>) =>
		JSON.stringify({ ...out, ...extra }).length <= budget;
	let n = Math.min(opts.tracePoints, Math.floor(MAX_TRACE_SAMPLES / data.length) || 1);
	let channels = data;
	let truncated = n < opts.tracePoints;
	for (;;) {
		const traces = build(channels, n);
		const extra: Record<string, JsonValue> = truncated
			? { traces, traces_truncated: true }
			: { traces };
		if (fits(extra)) return { ...out, ...extra };
		truncated = true;
		if (n > 2) n--;
		else if (channels.length > 1) channels = channels.slice(0, -1);
		else return { ...out, traces_truncated: true };
	}
}
