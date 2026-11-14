/** Dynamics mode: attach PSS/E dynamic data to a local case, open it in the
 * RMS worker, pick events, run, keep the last runs, and play one back.
 *
 * Everything numerical happens in the engine (`@tellegen/engine` RMS
 * worker); this class holds what the panel, the timeline and the map show,
 * and the ids that tie them together. */

import {
	ingestCase,
	looksLikeDyr,
	openRmsCase,
	rmsSupport,
	type BrowserRmsCase,
	type RmsCaseOptions,
	type RmsFlatCheck,
	type RmsInspection,
	type RmsLinearResult,
	type RmsRun
} from '@tellegen/engine';
import type { Layer, PickingInfo } from '@deck.gl/core';
import type { AppState, LocalCase } from '../state.svelte.js';
import { allRmsEvents, markers, MAX_EVENTS, sentence, validate, type DynEvent } from './events.js';
import { DynamicsFrame, type FrameSummary, type LayerCtors } from './layers.js';
import { DynamicsPlayer, startTime } from './player.svelte.js';
import {
	VALIDATION_DT,
	VALIDATION_OPTIONS,
	VALIDATION_T_END,
	validationBadge,
	WECC240_PRESETS
} from './presets.js';
import { failureTime } from './verdict.js';

export type DynStatus = 'opening' | 'ready' | 'failed';

/** The dynamic data attached to one local case. */
export interface DynAttachment {
	localId: string;
	fileName: string;
	text: string;
	/** The bundled WECC 240 demo (its validated scenarios apply). */
	demo: boolean;
	/** Typical parameters from "Add default dynamics", not the owner's. */
	synthetic: boolean;
	status: DynStatus;
	inspection: RmsInspection | null;
	error: string | null;
	flat: RmsFlatCheck | null;
	flatError: string | null;
	openMs: number | null;
}

/** One finished (or failed) run. */
export interface DynRunRecord {
	id: number;
	localId: string;
	run: RmsRun | null;
	failure: { message: string; at: number | null } | null;
	events: DynEvent[];
	options: RmsCaseOptions;
	dt: number;
	tEnd: number;
	badge: { validated: boolean; text: string } | null;
	/** End-to-end wall time, ms. */
	wallMs: number;
	/** Started by an agent (WebMCP). */
	agent: boolean;
	experimental: boolean;
}

export const STEP_CHOICES = [
	{ dt: 0.0025, label: 'Standard 2.5 ms' },
	{ dt: 0.005, label: 'Fast 5 ms' },
	{ dt: 0.001, label: 'Fine 1 ms' }
] as const;
export const DURATION_CHOICES = [5, 10, 20, 30] as const;
export const MAX_DURATION = 120;
const HISTORY = 3;

const optionsKey = (o: RmsCaseOptions, network: string) => JSON.stringify([network, o]);

export class DynamicsController {
	attachments: Record<string, DynAttachment> = $state.raw({});
	events: DynEvent[] = $state.raw([]);
	options: RmsCaseOptions = $state.raw({ load_model: 'power_flow' });
	tEnd = $state(20);
	dt = $state(0.0025);
	running = $state(false);
	progress: { t: number; tEnd: number } | null = $state.raw(null);
	runs: DynRunRecord[] = $state.raw([]);
	currentRunId = $state<number | null>(null);
	/** The validated scenario picked, with its events as picked. */
	presetId = $state<string | null>(null);
	presetEvents: DynEvent[] | null = $state.raw(null);
	support: { rms: boolean; experimental: boolean } | null = $state.raw(null);
	/** Experimental: the modes of the open case. */
	modes: RmsLinearResult | null = $state.raw(null);
	modesError = $state<string | null>(null);
	modesBusy = $state(false);
	selectedMode = $state<number | null>(null);
	/** Engine refusal of the last run, verbatim. */
	error = $state<string | null>(null);
	theme: 'light' | 'dark' = $state('light');
	readonly player = new DynamicsPlayer();
	layerCtors: LayerCtors | null = $state.raw(null);
	/** Base URL of the bundled examples. */
	examplesBase = '/examples';
	/** Hook for usage measurement (no case content). */
	onRun: ((r: DynRunRecord) => void) | null = null;
	/** A bus clicked on the playback layer. */
	onBusClick: ((bus: number) => void) | null = null;

	#cases = new Map<string, Promise<BrowserRmsCase>>();
	#validationModule: Promise<string> | null = null;
	#abort: AbortController | null = null;
	#seq = 0;

	constructor(
		private readonly app: AppState,
		private readonly ingest: (files: File[]) => Promise<void>
	) {}

	/** The active local case's attachment. */
	get active(): DynAttachment | null {
		const id = this.app.activeLocal?.id;
		return id ? (this.attachments[id] ?? null) : null;
	}

	/** Steady-state edits of the active case that dynamics leaves out. */
	get staticEdits(): number {
		const c = this.app.activeLocal;
		if (!c) return 0;
		const n = (r: Record<number, number>) => Object.values(r).filter((v) => v !== 0).length;
		return n(c.deltas) + n(c.ratings);
	}

	get current(): DynRunRecord | null {
		return this.runs.find((r) => r.id === this.currentRunId) ?? null;
	}

	get f0(): number {
		return this.active?.inspection?.base_frequency ?? 60;
	}

	async probe(): Promise<void> {
		this.support ??= await rmsSupport();
	}

	/** Whether `file` is dynamic data: by extension, or a `.txt` that reads as DYR. */
	async isDyr(file: File): Promise<boolean> {
		if (/\.dyr$/i.test(file.name)) return true;
		if (!/\.txt$/i.test(file.name)) return false;
		try {
			return await looksLikeDyr(await file.text());
		} catch {
			return false;
		}
	}

	/** Attach `text` to `local` and open it in the RMS worker. */
	async attach(
		local: LocalCase,
		fileName: string,
		text: string,
		demo = false,
		synthetic = false
	): Promise<void> {
		if (!local.studyInputJson) throw new Error(`${local.label} has no network to simulate`);
		for (const [k, p] of this.#cases)
			if (k.startsWith(`${local.id}|`)) void p.then((c) => c.free());
		for (const k of [...this.#cases.keys()])
			if (k.startsWith(`${local.id}|`)) this.#cases.delete(k);
		this.#set(local.id, {
			localId: local.id,
			fileName,
			text,
			demo,
			synthetic,
			status: 'opening',
			inspection: null,
			error: null,
			flat: null,
			flatError: null,
			openMs: null
		});
		this.modes = null;
		const started = performance.now();
		try {
			const c = await this.#case(local, false);
			this.#patch(local.id, {
				status: 'ready',
				inspection: c.inspection,
				openMs: performance.now() - started
			});
			try {
				const flat = await c.flatCheck(2);
				this.#patch(local.id, { flat });
			} catch (e) {
				this.#patch(local.id, { flatError: (e as Error).message });
			}
		} catch (e) {
			this.#patch(local.id, { status: 'failed', error: (e as Error).message });
		}
	}

	#set(id: string, a: DynAttachment) {
		this.attachments = { ...this.attachments, [id]: a };
	}
	#patch(id: string, p: Partial<DynAttachment>) {
		const a = this.attachments[id];
		if (a) this.#set(id, { ...a, ...p });
	}

	/** The prepared case for `local` with the current options. */
	#case(local: LocalCase, validationNetwork: boolean): Promise<BrowserRmsCase> {
		const a = this.attachments[local.id];
		if (!a) throw new Error('no dynamic data attached');
		const key = `${local.id}|${optionsKey(this.options, validationNetwork ? 'validation' : 'case')}`;
		let p = this.#cases.get(key);
		if (!p) {
			const module = validationNetwork
				? this.#validationNetwork()
				: Promise.resolve(local.studyInputJson!);
			p = module.then((m) => openRmsCase(m, a.text, this.options));
			p.catch(() => this.#cases.delete(key));
			this.#cases.set(key, p);
		}
		return p;
	}

	/** The WECC 240 network with the validation study's loads. */
	#validationNetwork(): Promise<string> {
		this.#validationModule ??= fetch(`${this.examplesBase}/wecc240/case-validation.raw`)
			.then(async (r) => {
				if (!r.ok) throw new Error(`the validation network is missing (HTTP ${r.status})`);
				const parsed = await ingestCase(new Uint8Array(await r.arrayBuffer()), 'raw');
				return parsed.module_json;
			})
			.catch((e) => {
				this.#validationModule = null;
				throw e;
			});
		return this.#validationModule;
	}

	/** Open the bundled WECC 240 demo: its case, dynamic data and coordinates. */
	async openDemo(): Promise<void> {
		this.app.mode = 'dynamics';
		const base = `${this.examplesBase}/wecc240`;
		const names = ['case.raw', 'case.dyr', 'case.geo.csv'];
		const files = await Promise.all(
			names.map(async (n) => {
				const r = await fetch(`${base}/${n}`);
				if (!r.ok) throw new Error(`${n}: HTTP ${r.status}`);
				return new File(
					[await r.blob()],
					n === 'case.raw' ? 'wecc240.raw' : n === 'case.dyr' ? 'wecc240.dyr' : 'wecc240.geo.csv'
				);
			})
		);
		this.#demoNext = true;
		try {
			await this.ingest(files);
		} finally {
			this.#demoNext = false;
		}
	}
	#demoNext = false;
	get attachingDemo(): boolean {
		return this.#demoNext;
	}

	// ---- events ----

	add(e: DynEvent): string | null {
		if (this.events.length >= MAX_EVENTS) return `at most ${MAX_EVENTS} events`;
		this.events = [...this.events, e];
		return null;
	}
	update(id: string, patch: Partial<DynEvent>): void {
		this.events = this.events.map((e) => (e.id === id ? ({ ...e, ...patch } as DynEvent) : e));
	}
	remove(id: string): void {
		this.events = this.events.filter((e) => e.id !== id);
	}
	clearEvents(): void {
		this.events = [];
		this.presetId = null;
		this.presetEvents = null;
	}

	/** Errors per event id. */
	get eventErrors(): Map<string, string> {
		const out = new Map<string, string>();
		const inspection = this.active?.inspection;
		if (!inspection) return out;
		for (const e of this.events) {
			const err = validate(e, { tEnd: this.tEnd, inspection });
			if (err) out.set(e.id, err);
		}
		return out;
	}

	usePreset(id: string): void {
		const p = WECC240_PRESETS.find((x) => x.id === id);
		if (!p) return;
		const events = p.events();
		this.events = events;
		this.presetId = id;
		this.presetEvents = events.map((e) => ({ ...e }));
		this.setOptions({ ...this.options, ...VALIDATION_OPTIONS });
		this.dt = VALIDATION_DT;
		this.tEnd = VALIDATION_T_END;
	}

	setOptions(o: RmsCaseOptions): void {
		this.options = o;
		this.modes = null;
	}

	get badge(): { validated: boolean; text: string } | null {
		return validationBadge({
			presetId: this.active?.demo ? this.presetId : null,
			presetEvents: this.presetEvents,
			events: this.events,
			options: this.options,
			dt: this.dt,
			tEnd: this.tEnd,
			validationNetwork: true
		});
	}

	// ---- runs ----

	async run(opts: { agent?: boolean } = {}): Promise<DynRunRecord | null> {
		const local = this.app.activeLocal;
		const a = this.active;
		if (!local || !a || a.status !== 'ready') {
			this.error = 'Attach dynamic data (.dyr) to the case first.';
			return null;
		}
		if (this.running) return null;
		const errs = this.eventErrors;
		if (errs.size) {
			this.error = [...errs.values()][0];
			return null;
		}
		this.error = null;
		this.running = true;
		this.progress = { t: 0, tEnd: this.tEnd };
		this.player.pause();
		const abort = new AbortController();
		this.#abort = abort;
		const started = performance.now();
		const events = this.events.map((e) => ({ ...e }));
		const options = { ...this.options };
		const dt = this.dt;
		const tEnd = this.tEnd;
		const badge = this.badge;
		const useValidation = !!badge && a.demo;
		const record: DynRunRecord = {
			id: ++this.#seq,
			localId: local.id,
			run: null,
			failure: null,
			events,
			options,
			dt,
			tEnd,
			badge,
			wallMs: 0,
			agent: !!opts.agent,
			experimental: a.synthetic
		};
		try {
			void this.#loadCtors();
			const c = await this.#case(local, useValidation);
			record.run = await c.run(
				{ events: allRmsEvents(events), t_end: tEnd, dt },
				{
					signal: abort.signal,
					onProgress: (p) => {
						this.progress = p;
					}
				}
			);
		} catch (e) {
			const err = e as Error;
			if (err.name === 'AbortError') {
				this.running = false;
				this.progress = null;
				this.error = 'Run stopped.';
				return null;
			}
			record.failure = { message: err.message, at: failureTime(err.message) };
		} finally {
			this.running = false;
			this.progress = null;
			this.#abort = null;
		}
		record.wallMs = performance.now() - started;
		this.runs = [record, ...this.runs].slice(0, HISTORY);
		this.select(record.id);
		this.onRun?.(record);
		return record;
	}

	stop(): void {
		this.#abort?.abort();
	}

	/** Show run `id`: load the player and start playback. */
	select(id: number): void {
		this.currentRunId = id;
		const r = this.current;
		if (!r?.run) return;
		const times = markers(r.events, this.f0).map((m) => m.t);
		this.player.load(r.run.time[r.run.time.length - 1], times);
		const reduced =
			typeof matchMedia !== 'undefined' && matchMedia('(prefers-reduced-motion: reduce)').matches;
		const start = startTime(
			r.events.map((e) => e.at),
			r.run.meta.base_frequency,
			reduced
		);
		this.player.seek(start.t);
		if (start.play) this.player.play();
	}

	async #loadCtors(): Promise<void> {
		if (this.layerCtors) return;
		const { ScatterplotLayer, PathLayer } = await import('@deck.gl/layers');
		this.layerCtors = { ScatterplotLayer, PathLayer };
	}

	// ---- map ----

	readonly frame: DynamicsFrame | null = $derived.by(() => {
		const r = this.current;
		const local = this.app.localCases.find((c) => c.id === r?.localId);
		if (!r?.run || !local?.view) return null;
		return new DynamicsFrame(
			r.run,
			local.view.buses,
			local.view.branches,
			this.attachments[r.localId]?.inspection?.held_units ?? []
		);
	});

	/** What the frame shows at the player's time. */
	readonly summary: FrameSummary | null = $derived.by(() => {
		const f = this.frame;
		if (!f) return null;
		const r = this.current!;
		const tripped = new Map<string, number>();
		for (const e of r.events)
			if (e.kind === 'generator_trip')
				tripped.set(
					e.machineId.trim() === '1' ? String(e.bus) : `${e.bus}#${e.machineId.trim()}`,
					e.at
				);
		return f.update(this.player.t, this.theme, tripped);
	});

	readonly mapLayers: Layer[] = $derived.by(() => {
		const f = this.frame;
		const ctors = this.layerCtors;
		const s = this.summary;
		if (!f || !ctors || !s || this.app.mode !== 'dynamics') return [];
		if (this.current?.localId !== this.app.activeLocal?.id) return [];
		const hover = this.app.dynamicsHover;
		const hoverBus = hover ? Number(hover.key.split('#')[0]) : null;
		return f.layers(ctors, s.t, this.current!.events, hoverBus, this.theme, (bus) =>
			this.onBusClick?.(bus)
		);
	});

	tooltip = (info: PickingInfo): { html: string } | null => {
		const f = this.frame;
		if (!f) return null;
		const esc = (x: string) => x.replace(/[&<>"']/g, (c) => `&#${c.charCodeAt(0)};`);
		if (info.layer?.id === 'dyn-bus-v' && info.index >= 0) {
			const v = f.vNow[info.index];
			return {
				html: `<b>Bus ${f.busIds[info.index]}</b><br>|V| ${v.toFixed(4)} pu at t = ${this.player.t.toFixed(3)} s`
			};
		}
		if (info.layer?.id === 'dyn-halos' && info.index >= 0) {
			const h = f.halos[info.index];
			const df = f.haloDf[info.index];
			const units = h.units
				.map((u) =>
					u.held ? `${esc(u.key)}: held at power-flow output, not simulated` : esc(u.key)
				)
				.join('<br>');
			return {
				html: `<b>Units at bus ${h.bus}</b><br>${units}${Number.isFinite(df) ? `<br>largest speed deviation ${(df * 1000).toFixed(1)} mHz` : ''}`
			};
		}
		if (info.layer?.id === 'dyn-open-branches') {
			const b = info.object as { from: number; to: number };
			return { html: `<b>Branch ${b.from}–${b.to}</b><br>open` };
		}
		return null;
	};

	// ---- experimental ----

	async computeModes(): Promise<void> {
		const local = this.app.activeLocal;
		if (!local || !this.active) return;
		this.modesBusy = true;
		this.modesError = null;
		const abort = new AbortController();
		this.#modesAbort = abort;
		try {
			const c = await this.#case(local, false);
			this.modes = await c.linear({ participation_top: 3 }, { signal: abort.signal });
		} catch (e) {
			const err = e as Error;
			this.modesError = err.name === 'AbortError' ? 'Stopped.' : err.message;
		} finally {
			this.modesBusy = false;
			this.#modesAbort = null;
		}
	}
	#modesAbort: AbortController | null = null;

	stopModes(): void {
		this.#modesAbort?.abort();
	}

	/** Dynamic states of the active case's machines and their controls: the
	 * size of a dense small-signal analysis, before network and plant states. */
	get dynamicStates(): number {
		const ins = this.active?.inspection;
		return ins ? ins.generators.reduce((n, g) => n + g.states.length, 0) : 0;
	}

	/** A run as JSON that reproduces it with the Rust or Python API. */
	runJson(r: DynRunRecord): string {
		return JSON.stringify(
			{
				about: 'tellegen RMS run: RmsRequest and the RmsOptions it was prepared with',
				...(r.badge && !r.badge.validated ? {} : {}),
				experimental: r.experimental,
				options: r.options,
				request: {
					t_end: r.tEnd,
					dt: r.dt,
					output_step: r.run?.outputStep ?? 0.01,
					load_model: r.options.load_model ?? 'power_flow',
					events: allRmsEvents(r.events)
				},
				criteria: r.run?.screening.criteria ?? null
			},
			null,
			2
		);
	}

	describe(e: DynEvent): string {
		return sentence(e, this.f0);
	}
}
