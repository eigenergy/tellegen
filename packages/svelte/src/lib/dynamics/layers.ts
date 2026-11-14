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
	readonly busPos: Map<number, [number, number]>;
	frame = 0;
	/** Per bus, the voltage at the current frame (for tooltips). */
	readonly vNow: Float64Array;
	readonly haloDf: Float64Array;

	constructor(
		readonly run: RmsRun,
		buses: readonly NetworkBus[],
		branches: readonly NetworkBranch[],
		heldUnits: readonly string[]
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
		onBusClick?: (bus: number) => void
	): Layer[] {
		const { ScatterplotLayer, PathLayer } = ctors;
		const frame = this.frame;
		const ink: [number, number, number, number] =
			theme === 'dark' ? [232, 238, 244, 220] : [32, 36, 43, 200];
		const out: Layer[] = [];
		// Tripped and opened branches, from their opening on.
		const open = new Set<string>();
		for (const e of events) {
			if (e.kind === 'branch_trip' && t >= e.at) open.add(`${e.from}-${e.to}`);
			if (e.kind === 'line_fault' && t >= e.at + e.clearAfter) open.add(`${e.from}-${e.to}`);
		}
		const openPaths = this.branches.filter(
			(b) => open.has(`${b.from}-${b.to}`) || open.has(`${b.to}-${b.from}`)
		);
		if (openPaths.length)
			out.push(
				new PathLayer<NetworkBranch>({
					id: 'dyn-open-branches',
					data: openPaths,
					getPath: (b) => b.path,
					getColor: theme === 'dark' ? [20, 28, 40, 230] : [250, 249, 245, 235],
					getWidth: 5,
					widthUnits: 'pixels',
					pickable: true
				}) as unknown as Layer,
				new PathLayer<NetworkBranch>({
					id: 'dyn-open-branches-dash',
					data: openPaths,
					getPath: (b) => b.path,
					getColor: ink,
					getWidth: 1.5,
					widthUnits: 'pixels'
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
				onClick: (info: { index: number }) => {
					if (info.index < 0) return false;
					onBusClick?.(this.busIds[info.index]);
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
