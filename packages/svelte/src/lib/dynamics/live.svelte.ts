/** Live view (experimental): the active case simulated as it goes, paced to
 * the wall clock, with events applied at the moment they are picked. The
 * last {@link LiveView.windowS} seconds are kept as a run, so the map and the traces
 * draw the live state the same way they draw a finished run.
 *
 * The numbers are the engine's (`BrowserRmsLive` in the RMS worker); this
 * class paces it, schedules an event's follow-ups (a fault's clearing) and
 * publishes the channels recorded by the engine.
 * @block Live view */

import {
	BrowserRmsLive,
	type ProtectionScheme,
	type RelayOperation,
	type RmsCaseOptions,
	type RmsEvent,
	type RmsInspection,
	type RmsLoadModel
} from '@tellegen/engine';
import { toRmsEvents, unitKey, type DynEvent } from './events.js';
import { paceTarget } from './live-logic.js';
import type { MapSamples } from './layers.js';

/** Seconds of simulated time the live run can keep. */
export const WINDOWS = [10, 20, 60] as const;
/** Integration steps the live view offers, s. */
export const STEPS = [0.001, 0.0025, 0.005, 0.01] as const;
/** The shortest time between two redraws of the live view, ms (about 30
 * per second; a redraw also waits for the browser's next frame). */
const DRAW_MS = 33;
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
	#keys: string[] = [];
	get keys(): readonly string[] {
		if (this.#keys.length !== this.cols.size) this.#keys = [...this.cols.keys()];
		return this.#keys;
	}
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
/** Channels the live session records: what the map and the traces read.
 * `coi:`, `f:` and `dcoi:` are derived by the engine at each sample, as the
 * screening of a batch run derives them. */
const RECORD = [
	'vm:',
	'va:',
	'f:',
	'coi:',
	'dcoi:',
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
	loadModel: RmsLoadModel;
	/** The case options of the Dynamics runs (reactive limits, held limits). */
	options?: RmsCaseOptions | null;
	/** Relays acting from the start (an engine build with `protection`). */
	protection?: ProtectionScheme | null;
}

/** Relay operations the live view keeps, newest last. */
const MAX_RELAY_OPS = 500;
/** Relay operations that go to the event log (pickups and resets do not). */
const LOGGED = new Set<RelayOperation['kind']>([
	'trip',
	'breaker_open',
	'breaker_stuck',
	'breaker_failure',
	'reclose',
	'lockout',
	'blocked',
	'bypassed',
	'reinserted',
	'sync_held'
]);

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
	error = $state<string | null>(null);
	/** What the engine did, newest last: `[t, text]`. */
	log: [number, string][] = $state.raw([]);
	/** What the relays did, newest last (with relays acting). */
	relays: RelayOperation[] = $state.raw([]);
	/** The kept window as a run (null before the first slice). */
	run: MapSamples | null = $state.raw(null);
	/** The local case the session belongs to. */
	localId = $state<string | null>(null);
	/** Counts the sessions opened: controls that mirror the session's
	 * state (the reference sliders) reset when it changes. */
	generation = $state(0);
	/** The channels the live plots show (`quantity:id`). */
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
			this.localId = localId;
			const generation = this.generation;
			let opening: BrowserRmsLive | null = null;
			try {
				opening = await BrowserRmsLive.open(input.moduleJson, input.dynText, {
					loadModel: input.loadModel,
					caseOptions: input.options ?? null,
					dt: this.dt,
					outputStep: OUTPUT_STEP
				});
				if (generation !== this.generation) return;
				await opening.setRecord({ channels: RECORD, frames: [], frame_step: 0 });
				if (generation !== this.generation) return;
				if (input.protection) {
					try {
						await opening.setProtection(input.protection);
					} catch (e) {
						this.error = `protection: ${(e as Error).message}`;
					}
					if (generation !== this.generation) return;
				}
				this.#session = opening;
				opening = null;
				this.#inspection = input.inspection;
				const firstBus = input.inspection.buses[0];
				const firstMachine = input.inspection.generators[0];
				this.plotted = [
					...(firstBus === undefined ? [] : [`vm:${firstBus}`]),
					...(firstMachine ? [`omega:${unitKey(firstMachine.bus, firstMachine.machine_id)}`] : [])
				];
				this.t = 0;
				this.log = [];
				this.relays = [];
				this.#samples = new Samples();
				this.#pending = [];
			} catch (e) {
				if (generation === this.generation) this.error = (e as Error).message;
				return;
			} finally {
				opening?.free();
				if (generation === this.generation) this.starting = false;
			}
		}
		this.running = true;
		// A pause/resume during an outstanding advance reuses its loop.
		if (!this.#loop && this.#session) {
			const session = this.#session;
			this.#loop = this.#run(session).finally(() => {
				if (this.#session === session) this.#loop = null;
			});
		}
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
		this.generation += 1;
		this.starting = false;
		this.#loop = null;
		this.#inspection = null;
		this.t = 0;
		this.log = [];
		this.relays = [];
		this.error = null;
		this.running = false;
		this.#session?.free();
		this.#session = null;
		this.run = null;
		this.localId = null;
		this.achieved = null;
		this.perf = null;
		this.#pending = [];
		this.#queued = [];
		this.#samples = new Samples();
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

	/** Let the relays of `scheme` act from now on (null: none). */
	async setProtection(scheme: ProtectionScheme | null): Promise<void> {
		if (!this.#session?.alive) return;
		try {
			await this.#session.setProtection(scheme);
		} catch (e) {
			this.error = `protection: ${(e as Error).message}`;
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

	async #apply(session: BrowserRmsLive, ev: RmsEvent): Promise<boolean> {
		try {
			const text = await session.apply(ev);
			if (session !== this.#session) return false;
			this.log = [...this.log, [this.t, text || ev.kind] as [number, string]].slice(-40);
			return true;
		} catch (e) {
			if (session !== this.#session) return false;
			this.log = [...this.log, [this.t, `refused: ${(e as Error).message}`] as [number, string]].slice(
				-40
			);
			return false;
		}
	}

	/** The pacing loop: queued events and due follow-ups first, then a slice
	 * of simulated time sized to keep up with the clock at the chosen speed
	 * ({@link paceTarget}; unpaced, as fast as the engine goes), then the
	 * slice's samples.
	 * @block Pacing loop */
	async #run(session: BrowserRmsLive): Promise<void> {
		let last = performance.now();
		let expected = this.t;
		let simAcc = 0;
		let wallAcc = 0;
		let perfAcc = { sim: 0, worker: 0, round: 0, n: 0 };
		try {
			while (this.running && session === this.#session) {
				// @block Apply picked events
				// Events picked since the last slice apply at the current time; their
				// follow-ups (a fault's clearing) wait for their own time.

				// A follow-up counts from the moment its event applies.
				for (const q of this.#queued.splice(0)) {
					if (session !== this.#session) return;
					if (!(await this.#apply(session, { ...q.ev, time: this.t }))) continue;
					for (const [a, delay] of q.after)
						this.#pending.push({ ...a, time: this.t + delay } as RmsEvent);
					this.#pending.sort((a, b) => a.time - b.time);
				}
				while (session === this.#session && this.#pending.length && this.#pending[0].time <= this.t + 1e-9)
					await this.#apply(session, this.#pending.shift()!);
				if (!this.running || session !== this.#session) break;

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
				if (this.#pending.length) adv = Math.min(adv, this.#pending[0].time - this.t);

				// @block Advance one slice
				// The engine integrates the slice in its worker; the samples are
				// appended and the performance figures updated.
				const started = performance.now();
				const slice = await session.advance(adv);
				if (session !== this.#session) return;
				const round = performance.now() - started;
				const sim = slice.t - this.t;
				simAcc += sim;
				perfAcc.sim += sim;
				perfAcc.worker += slice.workerMs;
				perfAcc.round += round;
				perfAcc.n += 1;
				this.t = slice.t;
				if (slice.relays.length) this.#relays(slice.relays);
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
					simAcc = wallAcc = 0;
					perfAcc = { sim: 0, worker: 0, round: 0, n: 0 };
				}
			}
		} catch (e) {
			if (session !== this.#session) return;
			this.error = (e as Error).message;
			this.running = false;
		}
	}

	/** Keep the relays' operations and log what they switched. */
	#relays(ops: RelayOperation[]): void {
		this.relays = [...this.relays, ...ops].slice(-MAX_RELAY_OPS);
		const logged = ops
			.filter((o) => LOGGED.has(o.kind))
			.map(
				(o) =>
					[
						o.time,
						`${o.relay} ${o.function} ${o.kind.replace(/_/g, ' ')}${o.detail ? ` (${o.detail})` : ''}`
					] as [number, string]
			);
		if (logged.length) this.log = [...this.log, ...logged].slice(-40);
	}

	/** Add a slice: write its samples, derive the new samples' bus
	 * frequencies, centre of inertia and screening columns, drop what left
	 * the window and schedule a redraw. Work is per new sample, not per
	 * kept sample, so it does not grow with the window or the redraw rate. */
	/** Append the recorded samples, keeping gaps when a channel is absent.
	 * @block Append a slice */
	#append(time: number[], keys: string[], series: (k: string) => Float64Array | undefined): void {
		if (!time.length) return;
		const s = this.#samples;
		const keep = Math.max(1, Math.round(this.windowS / OUTPUT_STEP));
		s.reserve(time.length, keep);
		const start = s.end;
		const end = start + time.length;
		s.time.set(time, start);
		const seen = new Set<string>();
		for (const key of keys) {
			const values = series(key);
			if (!values) continue;
			const col = s.col(key);
			col.set(values.subarray(0, time.length), start);
			if (values.length < time.length) col.fill(NaN, start + values.length, end);
			seen.add(key);
		}
		if (seen.size < s.cols.size)
			for (const [key, col] of s.cols) if (!seen.has(key)) col.fill(NaN, start, end);
		s.end = end;
		s.start = Math.max(s.start, end - keep);
		this.#scheduleDraw();
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

	/** A window of the engine's samples for the map and plots.
	 * @block Live samples */
	#asRun(): MapSamples {
		const { cols, start, end, time } = this.#samples;
		return {
			keys: this.#samples.keys,
			time: time.subarray(start, end),
			series: (key) => cols.get(key)?.subarray(start, end),
			meta: this.#inspection!
		};
	}
}
