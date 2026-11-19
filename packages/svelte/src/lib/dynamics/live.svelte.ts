/** Live view (experimental): the active case simulated as it goes, paced to
 * the wall clock, with events applied at the moment they are picked. The
 * last {@link LiveView.windowS} seconds are kept as a run, so the map and the traces
 * draw the live state the same way they draw a finished run.
 *
 * The numbers are the engine's (`BrowserRmsLive` in the RMS worker); this
 * class paces it, schedules an event's follow-ups (a fault's clearing) and
 * derives the centre-of-inertia frequency from the machine speeds.
 * @block Live view */

import {
	BrowserRmsLive,
	type RmsEvent,
	type RmsGenerator,
	type RmsInspection,
	type RmsRun,
	type RmsRunMeta
} from '@tellegen/engine';
import { toRmsEvents, type DynEvent } from './events.js';
import { paceTarget } from './live-logic.js';

/** Seconds of simulated time the live run can keep. */
export const WINDOWS = [10, 20, 60] as const;
/** Integration steps the live view offers, s. */
export const STEPS = [0.001, 0.0025, 0.005, 0.01] as const;
/** The shortest time between two redraws of the live view, ms (about 30
 * per second; a redraw also waits for the browser's next frame). */
const DRAW_MS = 33;
/** Internal per-sample columns (not channels): the lowest bus voltage, its
 * bus and the largest rotor angle from the COI at each sample. */
const VMIN = '\u0000vmin';
const VMIN_BUS = '\u0000vmin_bus';
const SEP = '\u0000sep';

/** The kept samples: one Float64Array per channel, all sharing the index
 * range [start, end) of `time`. Appends write past `end`; dropping old
 * samples only moves `start`. When the arrays run out of room, the kept
 * range moves into other arrays (the ones of two moves back, else fresh
 * ones), so the arrays a published run views are not written again while
 * that run is shown. A run is views into these arrays: no copies. */
class Samples {
	cap = 0;
	start = 0;
	end = 0;
	time: Float64Array = new Float64Array(0);
	cols = new Map<string, Float64Array>();
	#spare: { time: Float64Array; cols: Map<string, Float64Array> } | null = null;

	get length(): number {
		return this.end - this.start;
	}

	/** Room for `n` more samples when at most `keep` are kept. */
	reserve(n: number, keep: number): void {
		if (this.end + n <= this.cap) return;
		const kept = Math.min(this.length, keep);
		const from = this.end - kept;
		const cap = keep + Math.max(Math.ceil(keep / 2), n, 16);
		const spare = this.#spare;
		const take = (old: Float64Array | undefined) =>
			old && old.length === cap ? old : new Float64Array(cap);
		const time = take(spare?.time);
		time.set(this.time.subarray(from, this.end));
		const cols = new Map<string, Float64Array>();
		for (const [k, c] of this.cols) {
			const a = take(spare?.cols.get(k));
			a.set(c.subarray(from, this.end));
			cols.set(k, a);
		}
		this.#spare = { time: this.time, cols: this.cols };
		this.time = time;
		this.cols = cols;
		this.cap = cap;
		this.start = 0;
		this.end = kept;
	}

	/** The column of `k`, created with gaps (NaN) before the current end. */
	col(k: string): Float64Array {
		let c = this.cols.get(k);
		if (!c) {
			c = new Float64Array(this.cap).fill(NaN);
			this.cols.set(k, c);
		}
		return c;
	}
}
/** The live output step, s. */
const OUTPUT_STEP = 0.02;
/** Channels the live session records: what the map and the traces read. */
const RECORD = [
	'vm:',
	'va:',
	'omega:',
	'delta:',
	'pe:',
	'qe:',
	'efd:',
	'tm:',
	'vs:',
	'pl:',
	'ql:',
	'p_from:',
	'q_from:',
	'sig:'
];
/** Paced speeds; `Infinity` runs unpaced, as fast as the engine goes. */
export const SPEEDS = [0.25, 0.5, 1, 2, 4, Infinity] as const;

/** How fast the engine is, over the last second of wall time. */
export interface LivePerf {
	/** Simulated seconds per second the engine computes (wasm only). */
	engineX: number;
	/** The same with the transfer to the page (round trip per slice). */
	roundX: number;
	/** Share of the real-time budget at the chosen speed the round trips use. */
	load: number;
	/** How far the simulation trails the clock at the chosen speed, ms. */
	lagMs: number;
	/** Mean round trip per slice, ms, and simulated seconds per slice. */
	sliceMs: number;
	sliceSim: number;
}

interface Input {
	moduleJson: string;
	dynText: string;
	inspection: RmsInspection;
	loadModel: string;
}

/** The live session of one case: the engine paced to the clock, the events
 * picked while it runs, and the kept window published as a run.
 * @block LiveView */
export class LiveView {
	/** The live workbench is open (docked at the bottom). */
	open = $state(false);
	running = $state(false);
	starting = $state(false);
	/** Simulated time reached, s. */
	t = $state(0);
	speed = $state<number>(1);
	/** Seconds the live run keeps. */
	windowS = $state<number>(20);
	/** The integration step the engine takes, s. */
	dt = $state<number>(0.005);
	/** Simulated over wall time in the last second (the pace shown). */
	achieved = $state<number | null>(null);
	/** The engine's speed, independent of the pace. */
	perf: LivePerf | null = $state.raw(null);
	/** The last minute of {@link LivePerf.engineX}, one value a second. */
	perfHistory: number[] = $state.raw([]);
	error = $state<string | null>(null);
	/** What the engine did, newest last: `[t, text]`. */
	log: [number, string][] = $state.raw([]);
	/** The kept window as a run (null before the first slice). */
	run: RmsRun | null = $state.raw(null);
	/** The local case the session belongs to. */
	localId = $state<string | null>(null);
	/** Counts the sessions opened: controls that mirror the session's
	 * state (the reference sliders) reset when it changes. */
	generation = $state(0);
	/** The channels the live plots show (`quantity:id`, derived ones too). */
	plotted: string[] = $state.raw([]);

	/** Show or hide `key` in the live plots. */
	toggle(key: string): void {
		this.plotted = this.plotted.includes(key)
			? this.plotted.filter((k) => k !== key)
			: [...this.plotted, key];
	}

	#session: BrowserRmsLive | null = $state.raw(null);
	#inspection: RmsInspection | null = null;
	#samples = new Samples();
	/** The channel set the derived columns and the run's keys were built
	 * for; rebuilt when a slice brings a new channel. */
	#layout: Layout | null = null;
	/** Follow-up events, applied once the simulation reaches their time. */
	#pending: RmsEvent[] = [];
	/** Events to apply at the next slice, each with its follow-ups and how
	 * long after the event itself they come, s. */
	#queued: { ev: RmsEvent; after: [RmsEvent, number][] }[] = [];
	#loop: Promise<void> | null = null;

	get active(): boolean {
		return !!this.#session;
	}

	/** Start (or resume) the live session of `input` for case `localId`.
	 * @block Start a live session */
	async start(localId: string, input: Input): Promise<void> {
		if (this.running || this.starting) return;
		this.error = null;
		if (!this.#session?.alive || this.localId !== localId) {
			this.stop();
			this.starting = true;
			try {
				const s = await BrowserRmsLive.open(input.moduleJson, input.dynText, {
					loadModel: input.loadModel as never,
					dt: this.dt,
					outputStep: OUTPUT_STEP
				});
				await s.setRecord({ channels: RECORD, frames: [], frame_step: 0 } as never);
				this.#session = s;
				this.#inspection = input.inspection;
				this.localId = localId;
				this.generation += 1;
				this.t = 0;
				this.log = [];
				this.#samples = new Samples();
				this.#layout = null;
				this.#pending = [];
			} catch (e) {
				this.error = (e as Error).message;
				return;
			} finally {
				this.starting = false;
			}
		}
		this.running = true;
		this.#loop = this.#run();
	}

	pause(): void {
		this.running = false;
		if (this.#inspection && this.#samples.length) this.run = this.#asRun();
	}

	/** End the session and close the workbench. */
	close(): void {
		this.stop();
		this.open = false;
	}

	/** End the session and forget its samples. */
	stop(): void {
		this.running = false;
		this.#session?.free();
		this.#session = null;
		this.run = null;
		this.localId = null;
		this.achieved = null;
		this.perf = null;
		this.perfHistory = [];
		this.#pending = [];
		this.#queued = [];
		this.#samples = new Samples();
		this.#layout = null;
	}

	/** Integrate with a step of about `dt` from now on. */
	async setStep(dt: number): Promise<void> {
		this.dt = dt;
		if (!this.#session?.alive) return;
		try {
			this.dt = await this.#session.setStep(dt);
		} catch (e) {
			this.error = (e as Error).message;
		}
	}

	/** Apply engine events now (a reference, a breaker); `after` (s) delays
	 * the ones that follow, such as a fault's clearing. */
	applyNow(ev: RmsEvent, after: RmsEvent[] = [], delay = 0): void {
		this.#queued.push({ ev, after: after.map((a) => [a, delay] as [RmsEvent, number]) });
	}

	/** Apply `e` now: its first engine event at the current time, the rest
	 * (clearing, breakers) as far after it as the event states.
	 * @block Apply an event now */
	now(e: DynEvent): void {
		const evs = toRmsEvents({ ...e, at: 0 } as DynEvent);
		const first = evs.filter((ev) => ev.time <= 1e-12);
		const rest = evs.filter((ev) => ev.time > 1e-12).map((ev) => [ev, ev.time] as [RmsEvent, number]);
		first.forEach((ev, i) => this.#queued.push({ ev, after: i === 0 ? rest : [] }));
	}

	async #apply(ev: RmsEvent): Promise<void> {
		if (!this.#session) return;
		try {
			const text = await this.#session.apply(ev);
			this.log = [...this.log, [this.t, text || ev.kind] as [number, string]].slice(-40);
		} catch (e) {
			this.log = [...this.log, [this.t, `refused: ${(e as Error).message}`] as [number, string]].slice(
				-40
			);
		}
	}

	/** The pacing loop: queued events and due follow-ups first, then a slice
	 * of simulated time sized to keep up with the clock at the chosen speed
	 * ({@link paceTarget}; unpaced, as fast as the engine goes), then the
	 * slice's samples.
	 * @block Pacing loop */
	async #run(): Promise<void> {
		let last = performance.now();
		let expected = this.t;
		let simAcc = 0;
		let wallAcc = 0;
		let perfAcc = { sim: 0, worker: 0, round: 0, n: 0 };
		try {
			while (this.running && this.#session) {
				// @block Apply picked events
				// Events picked since the last slice apply at the current time; their
				// follow-ups (a fault's clearing) wait for their own time.

				// A follow-up counts from the moment its event applies.
				for (const q of this.#queued.splice(0)) {
					await this.#apply({ ...q.ev, time: this.t } as RmsEvent);
					for (const [a, delay] of q.after)
						this.#pending.push({ ...a, time: this.t + delay } as RmsEvent);
					this.#pending.sort((a, b) => a.time - b.time);
				}
				while (this.#pending.length && this.#pending[0].time <= this.t + 1e-9)
					await this.#apply(this.#pending.shift()!);

				// @block Pace to the clock
				// The simulated time to aim for after the wall time since the last
				// slice; ahead of it, wait.
				const now = performance.now();
				const wall = (now - last) / 1000;
				last = now;
				wallAcc += wall;
				expected = paceTarget(expected, wall, this.speed, this.t);
				const unpaced = !Number.isFinite(this.speed);
				const behind = expected - this.t;
				// Ahead of the clock: wait for it.
				if (!unpaced && behind < 0.02 * this.speed) {
					await new Promise((r) => setTimeout(r, 15));
					continue;
				}
				let adv = unpaced ? 0.2 : Math.min(0.25 * this.speed, Math.max(0.04 * this.speed, behind));
				// Stop at the next follow-up so it applies on time.
				if (this.#pending.length) adv = Math.max(this.dt, Math.min(adv, this.#pending[0].time - this.t));

				// @block Advance one slice
				// The engine integrates the slice in its worker; the samples are
				// appended and the performance figures updated.
				const started = performance.now();
				const slice = await this.#session.advance(adv);
				const round = performance.now() - started;
				const sim = slice.t - this.t;
				simAcc += sim;
				perfAcc.sim += sim;
				perfAcc.worker += slice.workerMs;
				perfAcc.round += round;
				perfAcc.n += 1;
				this.t = slice.t;
				// Unpaced, the clock target stays at the time reached.
				if (unpaced) expected = this.t;
				this.#append(slice.time, slice.keys, slice.series.bind(slice));
				if (wallAcc > 1) {
					this.achieved = simAcc / wallAcc;
					const p = perfAcc;
					this.perf = {
						engineX: p.worker > 0 ? p.sim / (p.worker / 1000) : Infinity,
						roundX: p.round > 0 ? p.sim / (p.round / 1000) : Infinity,
						load: unpaced ? 1 : p.round / 1000 / (p.sim / this.speed),
						lagMs: unpaced ? 0 : Math.max(0, (expected - this.t) * 1000),
						sliceMs: p.round / p.n,
						sliceSim: p.sim / p.n
					};
					this.perfHistory = [...this.perfHistory, this.perf.engineX].slice(-60);
					simAcc = wallAcc = 0;
					perfAcc = { sim: 0, worker: 0, round: 0, n: 0 };
				}
			}
		} catch (e) {
			this.error = (e as Error).message;
			this.running = false;
		}
	}

	/** Add a slice: write its samples, derive the new samples' bus
	 * frequencies, centre of inertia and screening columns, drop what left
	 * the window and schedule a redraw. Work is per new sample, not per
	 * kept sample, so it does not grow with the window or the redraw rate.
	 * @block Append a slice */
	#append(time: number[], keys: string[], series: (k: string) => Float64Array | undefined): void {
		const n = time.length;
		if (!n) return;
		const s = this.#samples;
		const keep = Math.max(1, Math.round(this.windowS / OUTPUT_STEP));
		s.reserve(n, keep);
		const i0 = s.end;
		s.time.set(time, i0);
		const seen = new Set<Float64Array>();
		for (const k of keys) {
			const v = series(k);
			if (!v) continue;
			const c = s.col(k);
			c.set(v.length > n ? v.subarray(0, n) : v, i0);
			if (v.length < n) c.fill(NaN, i0 + v.length, i0 + n);
			seen.add(c);
		}
		s.end = i0 + n;
		const L = this.#ensureLayout();
		// A channel this slice did not bring has a gap here.
		if (seen.size < L.recorded.length)
			for (const c of L.recorded) if (!seen.has(c)) c.fill(NaN, i0, i0 + n);
		this.#derive(L, i0, s.end);
		if (s.length > keep) s.start = s.end - keep;
		this.#scheduleDraw();
	}

	/** The derived and internal columns and the run's keys for the current
	 * channel set; rebuilt when a channel appears or the arrays move. */
	#ensureLayout(): Layout {
		const s = this.#samples;
		const old = this.#layout;
		if (old && old.colsRef === s.cols && old.size === s.cols.size) return old;
		const ins = this.#inspection!;
		const derived = new Set<string>(['coi:f', 'coi:delta', VMIN, VMIN_BUS, SEP]);
		const recordedKeys = [...s.cols.keys()].filter((k) => !derived.has(k) && !k.startsWith('f:'));
		const va: [Float64Array, Float64Array][] = [];
		const fKeys: string[] = [];
		for (const k of recordedKeys) {
			if (!k.startsWith('va:')) continue;
			const fk = `f:${k.slice(3)}`;
			fKeys.push(fk);
			va.push([s.cols.get(k)!, s.col(fk)]);
		}
		const vm: [Float64Array, number][] = [];
		const delta: Float64Array[] = [];
		for (const k of recordedKeys) {
			if (k.startsWith('vm:')) vm.push([s.cols.get(k)!, Number(k.slice(3).split('#')[0])]);
			else if (k.startsWith('delta:')) delta.push(s.cols.get(k)!);
		}
		const gens = ins.generators;
		const unit = (g: RmsGenerator) =>
			g.machine_id.trim() === '1' ? `${g.bus}` : `${g.bus}#${g.machine_id.trim()}`;
		const keys = [...recordedKeys.filter((k) => !k.startsWith('va:')), ...fKeys, 'coi:f', 'coi:delta'];
		const layout: Layout = {
			size: 0,
			colsRef: s.cols,
			keys,
			keySet: new Set(keys),
			recorded: recordedKeys.map((k) => s.cols.get(k)!),
			va,
			vm,
			delta,
			omega: gens.map((g) => s.cols.get(`omega:${unit(g)}`)),
			gDelta: gens.map((g) => s.cols.get(`delta:${unit(g)}`)),
			weights: gens.map((g) => ((g.h ?? 0) > 0 ? (g.h ?? 0) * g.mbase : 1)),
			coiF: s.col('coi:f'),
			coiD: s.col('coi:delta'),
			vmin: s.col(VMIN),
			vminBus: s.col(VMIN_BUS),
			sep: s.col(SEP)
		};
		layout.size = s.cols.size;
		this.#layout = layout;
		return layout;
	}

	/** Bus frequencies (from the bus angles: f0 + dθ/dt / 2π), the centre of
	 * inertia (frequency and angle, weighted by H · MBASE, equal weights
	 * without H) and the per-sample screening columns for samples [a, b).
	 * @block Derived columns */
	#derive(L: Layout, a: number, b: number): void {
		const t = this.#samples.time;
		const f0 = this.#inspection!.base_frequency;
		for (const [va, f] of L.va) {
			for (let i = a; i < b; i++) {
				if (i === 0) {
					f[i] = f0;
					continue;
				}
				const dt = t[i] - t[i - 1];
				let d = va[i] - va[i - 1];
				d = Math.atan2(Math.sin(d), Math.cos(d));
				f[i] = dt > 0 ? f0 + d / dt / (2 * Math.PI) : f0;
			}
		}
		const coi = (cols: (Float64Array | undefined)[], out: Float64Array, scale: number) => {
			for (let i = a; i < b; i++) {
				let num = 0;
				let den = 0;
				for (let j = 0; j < cols.length; j++) {
					const v = cols[j]?.[i];
					if (v !== undefined && Number.isFinite(v)) {
						num += L.weights[j] * v;
						den += L.weights[j];
					}
				}
				out[i] = den > 0 ? (num / den) * scale : NaN;
			}
		};
		coi(L.omega, L.coiF, f0);
		coi(L.gDelta, L.coiD, 1);
		L.vmin.fill(Infinity, a, b);
		L.vminBus.fill(NaN, a, b);
		for (const [c, bus] of L.vm) {
			for (let i = a; i < b; i++) {
				const x = c[i];
				if (x < L.vmin[i] && x > 1e-3) {
					L.vmin[i] = x;
					L.vminBus[i] = bus;
				}
			}
		}
		L.sep.fill(0, a, b);
		const k = 180 / Math.PI;
		for (const c of L.delta) {
			for (let i = a; i < b; i++) {
				const x = Math.abs((c[i] - L.coiD[i]) * k);
				if (x > L.sep[i]) L.sep[i] = x;
			}
		}
	}

	#drawn = 0;
	#drawQueued = false;
	/** Publish the window as {@link run} at the next animation frame, at
	 * most every {@link DRAW_MS}: the engine keeps computing while the page
	 * draws, and a slow frame delays the next publish instead of piling up.
	 * @block Publish at the next frame */
	#scheduleDraw(): void {
		if (this.#drawQueued) return;
		this.#drawQueued = true;
		let done = false;
		const draw = () => {
			if (done) return;
			done = true;
			this.#drawQueued = false;
			if (!this.#inspection || !this.#samples.length) return;
			this.#drawn = performance.now();
			this.run = this.#asRun();
		};
		const wait = Math.max(0, DRAW_MS - (performance.now() - this.#drawn));
		setTimeout(() => {
			if (typeof requestAnimationFrame === 'function' && typeof document !== 'undefined' && !document.hidden) {
				requestAnimationFrame(draw);
				// A page that is not painted (an embedding that paused its
				// frames) still gets the newest window now and then.
				setTimeout(draw, 250);
			} else draw();
		}, wait);
	}

	/** The kept window as an {@link RmsRun}: views into the kept columns
	 * (bus angles appear as bus frequencies `f:<bus>`), the centre-of-inertia
	 * frequency `coi:f` and angle `coi:delta`, and a screening summary of the
	 * window for the traces. Nothing is copied.
	 * @block The window as a run */
	#asRun(): RmsRun {
		const ins = this.#inspection!;
		const s = this.#samples;
		const L = this.#ensureLayout();
		const a = s.start;
		const b = s.end;
		const n = b - a;
		const f0 = ins.base_frequency;
		const cols = s.cols;
		const time = s.time.subarray(a, b);

		// Screening summary of the window (the traces' captions).
		let nadir = Infinity;
		let nadirT = 0;
		let minV = Infinity;
		let minBus: number | null = null;
		let sep = 0;
		for (let i = a; i < b; i++) {
			const f = L.coiF[i];
			if (f < nadir) {
				nadir = f;
				nadirT = s.time[i];
			}
			if (L.vmin[i] < minV) {
				minV = L.vmin[i];
				minBus = L.vminBus[i];
			}
			if (L.sep[i] > sep) sep = L.sep[i];
		}
		let lastMin = Infinity;
		let lastMax = -Infinity;
		for (const [c] of L.vm) {
			const e = c[b - 1];
			if (Number.isFinite(e)) {
				lastMin = Math.min(lastMin, e);
				lastMax = Math.max(lastMax, e);
			}
		}
		const screening = {
			frequency: {
				coi_nadir_hz: Number.isFinite(nadir) ? nadir : f0,
				nadir_t: nadirT,
				final_hz: n ? L.coiF[b - 1] : f0
			},
			angle: { max_separation_deg: sep, pole_slips: [] },
			voltage: {
				min_pu: Number.isFinite(minV) ? minV : 1,
				min_bus: minBus,
				final_min_pu: Number.isFinite(lastMin) ? lastMin : 1,
				final_max_pu: Number.isFinite(lastMax) ? lastMax : 1
			}
		};

		const { keys, keySet } = L;
		const meta = {
			keys,
			notes: [],
			events: [],
			generators: ins.generators,
			sources: ins.sources,
			buses: ins.buses,
			base_mva: ins.base_mva,
			base_frequency: f0,
			steps: 0,
			newton_iterations: 0,
			output_step: OUTPUT_STEP,
			dt: 0,
			t_end: this.t,
			screening
		} as unknown as RmsRunMeta;
		return {
			request: { events: [] },
			time,
			keys,
			series: (k: string) => (keySet.has(k) ? cols.get(k)?.subarray(a, b) : undefined),
			meta,
			screening: screening as unknown as RmsRun['screening'],
			notes: [],
			events: [],
			steps: 0,
			newtonIterations: 0,
			outputStep: OUTPUT_STEP,
			workerMs: 0,
			bytes: keys.length * n * 8
		};
	}
}

/** See {@link LiveView.#ensureLayout}. */
interface Layout {
	/** The column count and the column map the layout was built for. */
	size: number;
	colsRef: Map<string, Float64Array>;
	keys: string[];
	keySet: Set<string>;
	recorded: Float64Array[];
	/** Bus angle and the bus frequency derived from it. */
	va: [Float64Array, Float64Array][];
	/** Bus voltage magnitude and its bus. */
	vm: [Float64Array, number][];
	delta: Float64Array[];
	/** Per generator of the inspection: speed, rotor angle, COI weight. */
	omega: (Float64Array | undefined)[];
	gDelta: (Float64Array | undefined)[];
	weights: number[];
	coiF: Float64Array;
	coiD: Float64Array;
	vmin: Float64Array;
	vminBus: Float64Array;
	sep: Float64Array;
}
