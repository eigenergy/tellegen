/** Live view (experimental): the active case simulated as it goes, paced to
 * the wall clock, with events applied at the moment they are picked. The
 * last {@link LiveView.windowS} seconds are kept as a run, so the map and the traces
 * draw the live state the same way they draw a finished run.
 *
 * The numbers are the engine's (`BrowserRmsLive` in the RMS worker); this
 * class paces it, schedules an event's follow-ups (a fault's clearing) and
 * derives the centre-of-inertia frequency from the machine speeds. */

import {
	BrowserRmsLive,
	type RmsEvent,
	type RmsGenerator,
	type RmsInspection,
	type RmsRun,
	type RmsRunMeta
} from '@tellegen/engine';
import { toRmsEvents, type DynEvent } from './events.js';

/** Seconds of simulated time the live run can keep. */
export const WINDOWS = [10, 20, 60] as const;
/** Integration steps the live view offers, s. */
export const STEPS = [0.001, 0.0025, 0.005, 0.01] as const;
/** The shortest time between two redraws of the live view, ms. */
const DRAW_MS = 100;
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
	#time: number[] = [];
	#channels = new Map<string, number[]>();
	/** Follow-up events, applied once the simulation reaches their time. */
	#pending: RmsEvent[] = [];
	/** Events to apply at the next slice, each with its follow-ups and how
	 * long after the event itself they come, s. */
	#queued: { ev: RmsEvent; after: [RmsEvent, number][] }[] = [];
	#loop: Promise<void> | null = null;

	get active(): boolean {
		return !!this.#session;
	}

	/** Start (or resume) the live session of `input` for case `localId`. */
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
				this.t = 0;
				this.log = [];
				this.#time = [];
				this.#channels = new Map();
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
		if (this.#inspection && this.#time.length) this.run = this.#asRun();
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
	 * (clearing, breakers) as far after it as the event states. */
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

	async #run(): Promise<void> {
		let last = performance.now();
		let expected = this.t;
		let simAcc = 0;
		let wallAcc = 0;
		let perfAcc = { sim: 0, worker: 0, round: 0, n: 0 };
		try {
			while (this.running && this.#session) {
				// A follow-up counts from the moment its event applies.
				for (const q of this.#queued.splice(0)) {
					await this.#apply({ ...q.ev, time: this.t } as RmsEvent);
					for (const [a, delay] of q.after)
						this.#pending.push({ ...a, time: this.t + delay } as RmsEvent);
					this.#pending.sort((a, b) => a.time - b.time);
				}
				while (this.#pending.length && this.#pending[0].time <= this.t + 1e-9)
					await this.#apply(this.#pending.shift()!);
				const now = performance.now();
				const wall = (now - last) / 1000;
				last = now;
				wallAcc += wall;
				expected = Math.min(expected + wall * this.speed, this.t + 2);
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

	#append(time: number[], keys: string[], series: (k: string) => Float64Array | undefined): void {
		if (!time.length) return;
		this.#time.push(...time);
		for (const k of keys) {
			let col = this.#channels.get(k);
			if (!col) {
				// A channel that appears later starts with gaps.
				col = new Array(this.#time.length - time.length).fill(NaN);
				this.#channels.set(k, col);
			}
			col.push(...series(k)!);
		}
		const drop = this.#time.length - Math.round(this.windowS / OUTPUT_STEP);
		if (drop > 0) {
			this.#time.splice(0, drop);
			for (const col of this.#channels.values()) col.splice(0, drop);
		}
		// The view is rebuilt at most every DRAW_MS: the engine keeps
		// computing while the page draws.
		const now = performance.now();
		if (now - this.#drawn >= DRAW_MS || !this.running) {
			this.#drawn = now;
			this.run = this.#asRun();
		}
	}
	#drawn = 0;

	/** The kept window as an {@link RmsRun}: the recorded channels (bus
	 * angles turned into bus frequencies `f:<bus>`), the centre-of-inertia
	 * frequency `coi:f` and angle `coi:delta` (weighted by H · MBASE, equal
	 * weights without H), and a screening summary of the window for the
	 * traces. */
	#asRun(): RmsRun {
		const ins = this.#inspection!;
		const n = this.#time.length;
		const f0 = ins.base_frequency;
		const out = new Map<string, Float64Array>();
		for (const [k, col] of this.#channels) if (!k.startsWith('va:')) out.set(k, Float64Array.from(col));
		// Bus frequencies from the bus angles: f0 + dθ/dt / 2π.
		for (const [k, col] of this.#channels) {
			if (!k.startsWith('va:')) continue;
			const f = new Float64Array(n);
			for (let i = 0; i < n; i++) {
				const a = Math.max(0, i - 1);
				const b = Math.min(n - 1, i === 0 ? 1 : i);
				const dt = this.#time[b] - this.#time[a];
				let d = col[b] - col[a];
				d = Math.atan2(Math.sin(d), Math.cos(d));
				f[i] = dt > 0 ? f0 + d / dt / (2 * Math.PI) : f0;
			}
			out.set(`f:${k.slice(3)}`, f);
		}
		const gens: RmsGenerator[] = ins.generators;
		const w = gens.map((g) => ((g.h ?? 0) > 0 ? (g.h ?? 0) * g.mbase : 1));
		const key = (g: RmsGenerator) =>
			g.machine_id.trim() === '1' ? `${g.bus}` : `${g.bus}#${g.machine_id.trim()}`;
		const coi = (prefix: string, scale: number) => {
			const cols = gens.map((g) => this.#channels.get(`${prefix}${key(g)}`));
			const r = new Float64Array(n);
			for (let i = 0; i < n; i++) {
				let num = 0;
				let den = 0;
				cols.forEach((c, j) => {
					const v = c?.[i];
					if (v !== undefined && Number.isFinite(v)) {
						num += w[j] * v;
						den += w[j];
					}
				});
				r[i] = den > 0 ? (num / den) * scale : NaN;
			}
			return r;
		};
		const coiF = coi('omega:', f0);
		const coiD = coi('delta:', 1);
		out.set('coi:f', coiF);
		out.set('coi:delta', coiD);

		// Screening summary of the window (the traces' captions).
		let nadir = Infinity;
		let nadirT = 0;
		coiF.forEach((v, i) => {
			if (v < nadir) {
				nadir = v;
				nadirT = this.#time[i];
			}
		});
		let minV = Infinity;
		let minBus: number | null = null;
		let lastMin = Infinity;
		let lastMax = -Infinity;
		let sep = 0;
		for (const [k, v] of out) {
			if (k.startsWith('vm:')) {
				for (const x of v)
					if (x < minV && x > 1e-3) {
						minV = x;
						minBus = Number(k.slice(3).split('#')[0]);
					}
				const e = v[n - 1];
				if (Number.isFinite(e)) {
					lastMin = Math.min(lastMin, e);
					lastMax = Math.max(lastMax, e);
				}
			} else if (k.startsWith('delta:')) {
				for (let i = 0; i < n; i++)
					sep = Math.max(sep, Math.abs(((v[i] - coiD[i]) * 180) / Math.PI));
			}
		}
		const screening = {
			frequency: {
				coi_nadir_hz: Number.isFinite(nadir) ? nadir : f0,
				nadir_t: nadirT,
				final_hz: coiF[n - 1] ?? f0
			},
			angle: { max_separation_deg: sep, pole_slips: [] },
			voltage: {
				min_pu: Number.isFinite(minV) ? minV : 1,
				min_bus: minBus,
				final_min_pu: Number.isFinite(lastMin) ? lastMin : 1,
				final_max_pu: Number.isFinite(lastMax) ? lastMax : 1
			}
		};

		const allKeys = [...out.keys()];
		const values = new Float64Array(allKeys.length * n);
		allKeys.forEach((k, i) => values.set(out.get(k)!, i * n));
		const index = new Map(allKeys.map((k, i) => [k, i] as const));
		const time = Float64Array.from(this.#time);
		const meta = {
			keys: allKeys,
			notes: [],
			events: [],
			generators: gens,
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
			keys: allKeys,
			series: (k: string) => {
				const i = index.get(k);
				return i === undefined ? undefined : values.subarray(i * n, (i + 1) * n);
			},
			meta,
			screening: screening as unknown as RmsRun['screening'],
			notes: [],
			events: [],
			steps: 0,
			newtonIterations: 0,
			outputStep: OUTPUT_STEP,
			workerMs: 0,
			bytes: values.byteLength
		};
	}
}
