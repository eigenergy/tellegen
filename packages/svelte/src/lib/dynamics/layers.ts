/** The map layers of dynamics playback: bus |V| on a fixed absolute scale,
 * unit speed halos, and the events (a faulted bus while the fault is on,
 * tripped branches from their trip). A frame writes into preallocated typed
 * arrays and builds three light layer objects; the static layers are not
 * touched. */

import type { Layer } from '@deck.gl/core';
import type { NetworkBranch, NetworkBus, RmsRun } from '@tellegen/engine';
import { DYN_SPEED, dynVoltageColorInto, type RGBA } from '../colors.js';
import type { DynEvent } from './events.js';
import { sampleAt } from './series.js';

export interface LayerCtors {
	ScatterplotLayer: typeof import('@deck.gl/layers').ScatterplotLayer;
	PathLayer: typeof import('@deck.gl/layers').PathLayer;
}

/** Below this speed deviation a unit counts as at nominal speed: a neutral
 * ring, neither teal nor magenta (1 mHz, far above the numerical drift of a
 * case at rest). */
export const AT_NOMINAL_HZ = 1e-3;

/** Ring radius in pixels for a speed deviation of `dfHz`: the minimum ring
 * up to {@link AT_NOMINAL_HZ}, then ∝ √(|Δf| − threshold), capped. */
export const haloRadius = (dfHz: number): number =>
	Math.min(44, 9 + 60 * Math.sqrt(Math.max(0, Math.abs(dfHz) - AT_NOMINAL_HZ)));

/** The ring colour of a bus: grey when no unit there is simulated (held or
 * tripped), faint neutral at nominal speed, else teal below and magenta above. */
export function haloColor(dfHz: number, live: boolean, pal: (typeof DYN_SPEED)['light']): RGBA {
	if (!live) return [pal.held[0], pal.held[1], pal.held[2], 150];
	if (!(Math.abs(dfHz) >= AT_NOMINAL_HZ)) return [...pal.rest];
	return [...(dfHz < 0 ? pal.under : pal.over)];
}

/** A branch row drawn open: which circuit, and how many of the parallel
 * circuits between its buses are still in service. */
export interface OpenBranch {
	path: [number, number][];
	from: number;
	to: number;
	/** The circuit id, or null when the view row could not be matched to one. */
	circuit: string | null;
	/** In-service rows between the two buses, this one included. */
	parallel: number;
	/** Of those, the ones still closed. */
	live: number;
}

const pairKey = (a: number, b: number) => (a < b ? `${a}-${b}` : `${b}-${a}`);

/** The circuit id of every view branch row: per bus pair, the view's rows in
 * id order against the RMS case's branches ([from, to, circuit], in the
 * network's row order). A pair whose row counts disagree gets null, never a
 * guess. */
export function branchCircuits(
	branches: readonly NetworkBranch[],
	keys: readonly (readonly [number, number, string])[]
): (string | null)[] {
	const circuits = new Map<string, string[]>();
	for (const [f, t, c] of keys) {
		const k = pairKey(f, t);
		const list = circuits.get(k) ?? [];
		list.push(c.trim());
		circuits.set(k, list);
	}
	const rows = (live: boolean) => {
		const m = new Map<string, number[]>();
		branches.forEach((b, i) => {
			if (live && !b.status) return;
			const k = pairKey(b.from, b.to);
			const list = m.get(k) ?? [];
			list.push(i);
			m.set(k, list);
		});
		return m;
	};
	const all = rows(false);
	const live = rows(true);
	const out: (string | null)[] = branches.map(() => null);
	for (const [k, cs] of circuits) {
		// The RMS case may list every row or only those in service.
		const idx = [all.get(k) ?? [], live.get(k) ?? []].find((r) => r.length === cs.length);
		if (!idx) continue;
		[...idx].sort((a, b) => branches[a].id - branches[b].id).forEach((i, n) => (out[i] = cs[n]));
	}
	return out;
}

/** The rows of `branches` open at time `t`: tripped branches from their
 * trip, faulted lines from their clearing. Only the circuit an event names
 * opens; a row whose circuit is unknown opens with every event on its pair. */
export function openBranches(
	branches: readonly NetworkBranch[],
	circuits: readonly (string | null)[],
	events: readonly DynEvent[],
	t: number
): OpenBranch[] {
	const opened = new Map<string, Set<string>>();
	for (const e of events) {
		const on =
			(e.kind === 'branch_trip' && t >= e.at) ||
			(e.kind === 'line_fault' && t >= e.at + e.clearAfter);
		if (!on) continue;
		const k = pairKey(e.from, e.to);
		const set = opened.get(k) ?? new Set<string>();
		set.add(e.circuit.trim());
		opened.set(k, set);
	}
	if (!opened.size) return [];
	const open: number[] = [];
	const inService = new Map<string, number>();
	branches.forEach((b, i) => {
		const k = pairKey(b.from, b.to);
		const set = opened.get(k);
		if (!set) return;
		if (b.status) inService.set(k, (inService.get(k) ?? 0) + 1);
		const c = circuits[i];
		if (c === null || set.has(c)) open.push(i);
	});
	const openCount = new Map<string, number>();
	for (const i of open) {
		const k = pairKey(branches[i].from, branches[i].to);
		openCount.set(k, (openCount.get(k) ?? 0) + 1);
	}
	return open.map((i) => {
		const b = branches[i];
		const k = pairKey(b.from, b.to);
		const parallel = Math.max(inService.get(k) ?? 0, openCount.get(k) ?? 0);
		return {
			path: b.path,
			from: b.from,
			to: b.to,
			circuit: circuits[i],
			parallel,
			live: Math.max(0, parallel - (openCount.get(k) ?? 0))
		};
	});
}

/** The tooltip of an open branch row. */
export function openBranchText(b: OpenBranch): string {
	const name =
		b.circuit === null
			? `Branch ${b.from}–${b.to}`
			: `Branch ${b.from}–${b.to} circuit ${b.circuit}`;
	if (b.parallel <= 1) return `${name} open`;
	if (b.circuit === null)
		return `${name} open (its ${b.parallel} parallel circuits are drawn together)`;
	return `${name} open; ${b.live} of ${b.parallel} parallel circuits still in service`;
}

interface UnitAt {
	key: string;
	omega: Float64Array | undefined;
	held: boolean;
}

interface HaloBus {
	bus: number;
	units: UnitAt[];
}

/** What the layers show at one time, besides the colours (for tooltips and
 * the aria text). */
export interface FrameSummary {
	t: number;
	minV: number;
	minBus: number | null;
	coiHz: number | null;
}

export class DynamicsFrame {
	readonly n: number;
	readonly busIds: number[];
	readonly positions: Float32Array;
	readonly colors: Uint8Array;
	readonly vm: (Float64Array | undefined)[];
	readonly halos: HaloBus[];
	readonly haloPositions: Float32Array;
	readonly haloRadii: Float32Array;
	readonly haloColors: Uint8Array;
	readonly f0: number;
	readonly branches: NetworkBranch[];
	/** The circuit of each of {@link branches}, null where unknown. */
	readonly circuits: (string | null)[];
	readonly busPos: Map<number, [number, number]>;
	frame = 0;
	/** Per bus, the voltage at the current frame (for tooltips). */
	readonly vNow: Float64Array;
	readonly haloDf: Float64Array;

	constructor(
		readonly run: RmsRun,
		buses: readonly NetworkBus[],
		branches: readonly NetworkBranch[],
		heldUnits: readonly string[],
		branchKeys: readonly (readonly [number, number, string])[] = []
	) {
		const placed = buses.filter((b) => run.series(`vm:${b.id}`));
		this.n = placed.length;
		this.busIds = placed.map((b) => b.id);
		this.positions = new Float32Array(placed.flatMap((b) => [b.lon, b.lat]));
		this.busPos = new Map(placed.map((b) => [b.id, [b.lon, b.lat]]));
		this.colors = new Uint8Array(4 * this.n);
		this.vNow = new Float64Array(this.n);
		this.vm = placed.map((b) => run.series(`vm:${b.id}`));
		this.f0 = run.meta.base_frequency;
		this.branches = [...branches];
		this.circuits = branchCircuits(this.branches, branchKeys);
		const byBus = new Map<number, UnitAt[]>();
		for (const g of run.meta.generators) {
			const key = g.machine_id.trim() === '1' ? String(g.bus) : `${g.bus}#${g.machine_id.trim()}`;
			const list = byBus.get(g.bus) ?? [];
			list.push({ key, omega: run.series(`omega:${key}`), held: false });
			byBus.set(g.bus, list);
		}
		for (const key of heldUnits) {
			const bus = Number(key.split('#')[0]);
			const list = byBus.get(bus) ?? [];
			list.push({ key, omega: undefined, held: true });
			byBus.set(bus, list);
		}
		this.halos = [...byBus.entries()]
			.filter(([bus]) => this.busPos.has(bus))
			.map(([bus, units]) => ({ bus, units }));
		this.haloPositions = new Float32Array(this.halos.flatMap((h) => this.busPos.get(h.bus)!));
		this.haloRadii = new Float32Array(this.halos.length);
		this.haloColors = new Uint8Array(4 * this.halos.length);
		this.haloDf = new Float64Array(this.halos.length);
	}

	/** Write the colours and radii at time `t`. */
	update(t: number, theme: 'light' | 'dark', tripped: ReadonlyMap<string, number>): FrameSummary {
		const { i, f } = sampleAt(this.run.time, t);
		const at = (s: Float64Array) =>
			f === 0 || i + 1 >= s.length ? s[i] : s[i] + (s[i + 1] - s[i]) * f;
		let minV = Infinity;
		let minBus: number | null = null;
		for (let b = 0; b < this.n; b++) {
			const s = this.vm[b];
			const v = s ? at(s) : NaN;
			this.vNow[b] = v;
			dynVoltageColorInto(v, theme, this.colors, 4 * b);
			if (v < minV && v > 1e-3) {
				minV = v;
				minBus = this.busIds[b];
			}
		}
		const pal = DYN_SPEED[theme];
		for (let h = 0; h < this.halos.length; h++) {
			let df = 0;
			let live = false;
			let held = true;
			for (const u of this.halos[h].units) {
				const trip = tripped.get(u.key);
				if (trip !== undefined && t > trip) continue;
				if (u.held || !u.omega) continue;
				held = false;
				live = true;
				const d = this.f0 * (at(u.omega) - 1);
				if (Math.abs(d) > Math.abs(df)) df = d;
			}
			this.haloDf[h] = live ? df : NaN;
			this.haloRadii[h] = live ? haloRadius(df) : 9;
			this.haloColors.set(haloColor(df, live && !held, pal), 4 * h);
		}
		const coi = this.run.series('coi:f');
		this.frame++;
		return { t, minV, minBus, coiHz: coi ? at(coi) : null };
	}

	layers(
		ctors: LayerCtors,
		t: number,
		events: readonly DynEvent[],
		hoverBus: number | null,
		theme: 'light' | 'dark',
		onBusClick?: (bus: number, x?: number, y?: number) => void
	): Layer[] {
		const { ScatterplotLayer, PathLayer } = ctors;
		const frame = this.frame;
		const ink: [number, number, number, number] =
			theme === 'dark' ? [232, 238, 244, 220] : [32, 36, 43, 200];
		const out: Layer[] = [];
		// Tripped and opened circuits, from their opening on. A corridor with
		// a parallel circuit still closed keeps its colour under the dash.
		const openRows = openBranches(this.branches, this.circuits, events, t);
		const allOpen = openRows.filter((b) => b.live === 0);
		if (allOpen.length)
			out.push(
				new PathLayer<OpenBranch>({
					id: 'dyn-open-branches-gap',
					data: allOpen,
					getPath: (b) => b.path,
					getColor: theme === 'dark' ? [20, 28, 40, 230] : [250, 249, 245, 235],
					getWidth: 5,
					widthUnits: 'pixels',
					pickable: true
				}) as unknown as Layer
			);
		if (openRows.length)
			out.push(
				new PathLayer<OpenBranch>({
					id: 'dyn-open-branches',
					data: openRows,
					getPath: (b) => b.path,
					getColor: ink,
					getWidth: (b) => (b.live === 0 ? 1.5 : 2.5),
					widthUnits: 'pixels',
					widthMinPixels: 1.5,
					pickable: true
				}) as unknown as Layer
			);
		out.push(
			new ScatterplotLayer({
				id: 'dyn-bus-v',
				data: {
					length: this.n,
					attributes: {
						getPosition: { value: this.positions, size: 2 },
						getFillColor: { value: this.colors, size: 4, normalized: true }
					}
				},
				getRadius: 7,
				radiusUnits: 'pixels',
				stroked: true,
				getLineColor: theme === 'dark' ? [10, 20, 31, 220] : [46, 42, 34, 170],
				lineWidthMinPixels: 1,
				pickable: true,
				onClick: (info: { index: number; x: number; y: number }) => {
					if (info.index < 0) return false;
					onBusClick?.(this.busIds[info.index], info.x, info.y);
					return true;
				},
				updateTriggers: { getFillColor: frame }
			}) as unknown as Layer,
			new ScatterplotLayer({
				id: 'dyn-halos',
				data: {
					length: this.halos.length,
					attributes: {
						getPosition: { value: this.haloPositions, size: 2 },
						getRadius: { value: this.haloRadii, size: 1 },
						getLineColor: { value: this.haloColors, size: 4, normalized: true }
					}
				},
				radiusUnits: 'pixels',
				stroked: true,
				filled: false,
				lineWidthUnits: 'pixels',
				getLineWidth: 2,
				pickable: true,
				// A ring sits over its bus: a click on it is a click on the bus.
				onClick: (info: { index: number; x: number; y: number }) => {
					if (info.index < 0) return false;
					onBusClick?.(this.halos[info.index].bus, info.x, info.y);
					return true;
				},
				updateTriggers: { getRadius: frame, getLineColor: frame }
			}) as unknown as Layer
		);
		const faulted: [number, number][] = [];
		for (const e of events) {
			const on =
				t >= e.at &&
				e.kind !== 'branch_trip' &&
				e.kind !== 'generator_trip' &&
				e.kind !== 'load_change' &&
				t < e.at + e.clearAfter;
			if (!on) continue;
			const bus = e.kind === 'bus_fault' ? e.bus : e.nearBus;
			const p = this.busPos.get(bus);
			if (p) faulted.push(p);
		}
		if (faulted.length)
			out.push(
				new ScatterplotLayer<[number, number]>({
					id: 'dyn-fault',
					data: faulted,
					getPosition: (p) => p,
					getRadius: 16,
					radiusUnits: 'pixels',
					stroked: true,
					filled: true,
					getFillColor: [255, 176, 0, 90],
					getLineColor: [179, 38, 30, 255],
					lineWidthUnits: 'pixels',
					getLineWidth: 3
				}) as unknown as Layer
			);
		if (hoverBus != null) {
			const p = this.busPos.get(hoverBus);
			if (p)
				out.push(
					new ScatterplotLayer<[number, number]>({
						id: 'dyn-hover',
						data: [p],
						getPosition: (q) => q,
						getRadius: 11,
						radiusUnits: 'pixels',
						stroked: true,
						filled: false,
						getLineColor: ink,
						lineWidthUnits: 'pixels',
						getLineWidth: 2
					}) as unknown as Layer
				);
		}
		return out;
	}
}
