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
import { allRmsEvents, markers, MAX_EVENTS, title, validate, type DynEvent } from './events.js';
import {
	DynamicsFrame,
	openBranchText,
	type FrameSummary,
	type LayerCtors,
	type OpenBranch
} from './layers.js';
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
	/** The prepared case the next run uses: its options and network. The
	 * inspection and the flat check below describe that case. */
	inspection: RmsInspection | null;
	/** The network {@link inspection} was prepared on: the case's own, or
	 * (for a validated scenario on the demo) the validation study's. */
	network: 'case' | 'validation';
	/** Re-preparing after the options or the network changed. */
	preparing: boolean;
	/** Why the case could not be prepared with the current options. */
	prepareError: string | null;
	error: string | null;
	flat: RmsFlatCheck | null;
	flatError: string | null;
	/** A few steps from rest: the voltages and speeds the map shows before
	 * the first run. */
	rest: RmsRun | null;
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
	badge: { validated: boolean; text: string; detail?: string } | null;
	/** The network the run used: the case's own, or (for a validated
	 * scenario on the demo) the validation study's. */
	network: 'case' | 'validation';
	/** Units the run's case held at their power-flow output. */
	heldUnits?: string[];
	/** The run's case branches, [from, to, circuit]. */
	branchKeys?: [number, number, string][];
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
/** The name of the bundled WECC 240 demo case. */
export const DEMO_LABEL = 'WECC 240 (dynamics demo)';
/** How `runJson` names the validation study's network. */
export const VALIDATION_NETWORK =
	'wecc240 case-validation.raw (the PSID validation study loads; examples/wecc240)';

/** Seconds simulated from rest to draw the map before the first run. */
const REST_SECONDS = 0.02;

const optionsKey = (o: RmsCaseOptions, network: string) => JSON.stringify([network, o]);

/** Why dynamics is unavailable while the experimental setting is off. */
export const DYNAMICS_OFF = 'Dynamics is off: turn on Experimental features in Settings';

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
	/** The step, duration and options before a validated scenario replaced
	 * them; Clear events restores them. */
	beforePreset: { options: RmsCaseOptions; dt: number; tEnd: number } | null = $state.raw(null);
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
		this.#free(local.id);
		this.#described.delete(local.id);
		this.#set(local.id, {
			localId: local.id,
			fileName,
			text,
			demo,
			synthetic,
			status: 'opening',
			inspection: null,
			network: 'case',
			preparing: false,
			prepareError: null,
			error: null,
			flat: null,
			flatError: null,
			rest: null,
			openMs: null
		});
		this.modes = null;
		const started = performance.now();
		const validation = this.#useValidation(local.id);
		try {
			const c = await this.#case(local, validation);
			this.#patch(local.id, {
				status: 'ready',
				openMs: performance.now() - started
			});
			await this.#describe(local.id, validation, c);
		} catch (e) {
			this.#patch(local.id, { status: 'failed', error: (e as Error).message });
			return;
		}
		// The options or the scenario may have changed while it opened.
		await this.refresh();
	}

	/** The prepared-case key of `localId` under the current options. */
	#key(localId: string, validationNetwork: boolean): string {
		return `${localId}|${optionsKey(this.options, validationNetwork ? 'validation' : 'case')}`;
	}
	/** The case each attachment's inspection describes, by key. */
	#described = new Map<string, string>();

	/** Whether runs of `localId` use the validation network: a validated
	 * scenario picked on the demo. */
	#useValidation(localId: string): boolean {
		return !!this.presetId && !!this.attachments[localId]?.demo;
	}

	/** Show what `c` (prepared as `validation` says) resolves to, and check
	 * that it starts at rest. Once per prepared case. */
	async #describe(localId: string, validation: boolean, c: BrowserRmsCase): Promise<void> {
		const key = `${localId}|${optionsKey(c.options, validation ? 'validation' : 'case')}`;
		if (this.#described.get(localId) === key) return;
		this.#described.set(localId, key);
		this.#patch(localId, {
			inspection: c.inspection,
			network: validation ? 'validation' : 'case',
			prepareError: null,
			flat: null,
			flatError: null,
			rest: null
		});
		void this.#loadCtors();
		try {
			// The initial operating point, for the map before a run.
			const rest = await c.run({
				events: [],
				t_end: REST_SECONDS,
				dt: 0.005,
				output_step: 0.01
			});
			if (this.#described.get(localId) === key) this.#patch(localId, { rest });
		} catch {
			// The map keeps the static view; the flat check reports the cause.
		}
		try {
			const flat = await c.flatCheck(2);
			if (this.#described.get(localId) === key) this.#patch(localId, { flat });
		} catch (e) {
			if (this.#described.get(localId) === key)
				this.#patch(localId, { flatError: (e as Error).message });
		}
	}

	/** Prepare the active case for the current options and network, so the
	 * panel describes the case the next run uses. */
	async refresh(): Promise<void> {
		const local = this.app.activeLocal;
		const a = this.active;
		if (!local || !a || a.status !== 'ready') return;
		const validation = this.#useValidation(local.id);
		const key = this.#key(local.id, validation);
		if (this.#described.get(local.id) === key) {
			if (a.preparing) this.#patch(local.id, { preparing: false });
			return;
		}
		this.#patch(local.id, { preparing: true, prepareError: null });
		try {
			const c = await this.#case(local, validation);
			if (this.#key(local.id, this.#useValidation(local.id)) !== key) return;
			this.#patch(local.id, { preparing: false });
			await this.#describe(local.id, validation, c);
		} catch (e) {
			if (this.#key(local.id, this.#useValidation(local.id)) === key)
				this.#patch(local.id, {
					preparing: false,
					prepareError: (e as Error).message
				});
		}
	}

	/** Free the worker cases of `localId`. */
	#free(localId: string): void {
		for (const [k, p] of [...this.#cases]) {
			if (!k.startsWith(`${localId}|`)) continue;
			this.#cases.delete(k);
			void p.then(
				(c) => c.free(),
				() => {}
			);
		}
	}

	/** Forget a closed local case: its attachment, runs and worker cases. */
	forget(localId: string): void {
		this.#free(localId);
		this.#described.delete(localId);
		if (!(localId in this.attachments) && !this.runs.some((r) => r.localId === localId)) return;
		this.attachments = Object.fromEntries(
			Object.entries(this.attachments).filter(([id]) => id !== localId)
		);
		this.runs = this.runs.filter((r) => r.localId !== localId);
		if (this.current === null) this.currentRunId = null;
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
		const key = this.#key(local.id, validationNetwork);
		let p = this.#cases.get(key);
		if (!p) {
			// One prepared case per local case: a new network or new options
			// replace the previous one, which the worker frees.
			this.#free(local.id);
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
		if (!this.app.dynamicsEnabled) throw new Error(DYNAMICS_OFF);
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
	/** Remove one event. Removing the last one clears the list as
	 * {@link clearEvents} does, restoring any settings a scenario changed. */
	remove(id: string): void {
		const rest = this.events.filter((e) => e.id !== id);
		if (rest.length === 0) this.clearEvents();
		else this.events = rest;
	}
	clearEvents(): void {
		this.events = [];
		this.presetId = null;
		this.presetEvents = null;
		const before = this.beforePreset;
		if (before) {
			this.beforePreset = null;
			this.setOptions(before.options);
			this.dt = before.dt;
			this.tEnd = before.tEnd;
		}
		void this.refresh();
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
		this.beforePreset ??= {
			options: this.options,
			dt: this.dt,
			tEnd: this.tEnd
		};
		this.events = events;
		this.presetId = id;
		this.presetEvents = events.map((e) => ({ ...e }));
		this.setOptions({ ...this.options, ...VALIDATION_OPTIONS });
		this.dt = VALIDATION_DT;
		this.tEnd = VALIDATION_T_END;
		void this.refresh();
	}

	setOptions(o: RmsCaseOptions): void {
		this.options = o;
		this.modes = null;
		void this.refresh();
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
			this.error = 'No dynamic data';
			return null;
		}
		if (this.running) {
			this.error = 'Run in progress';
			return null;
		}
		if (this.modesBusy) {
			this.error = 'Modes running';
			return null;
		}
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
			network: useValidation ? 'validation' : 'case',
			wallMs: 0,
			agent: !!opts.agent,
			experimental: a.synthetic
		};
		try {
			void this.#loadCtors();
			const c = await this.#case(local, useValidation);
			void this.#describe(local.id, useValidation, c);
			record.heldUnits = c.inspection.held_units ?? [];
			record.branchKeys = c.inspection.branches;
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
				this.error = 'Stopped';
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

	/** The run the map draws: the selected run of the active case, else (in
	 * Dynamics mode, before a run) that case at rest. */
	readonly #shown = $derived.by(() => {
		const local = this.app.activeLocal;
		if (!local?.view) return null;
		const r = this.current;
		if (r?.run && r.localId === local.id) return { record: r, run: r.run, local };
		const a = this.attachments[local.id];
		if (this.app.mode === 'dynamics' && a?.rest) return { record: null, run: a.rest, local };
		return null;
	});

	/** The map shows the case at rest, before any run of it. */
	get atRest(): boolean {
		return !!this.#shown && !this.#shown.record;
	}

	readonly frame: DynamicsFrame | null = $derived.by(() => {
		const shown = this.#shown;
		if (!shown) return null;
		const ins = this.attachments[shown.local.id]?.inspection;
		return new DynamicsFrame(
			shown.run,
			shown.local.view!.buses,
			shown.local.view!.branches,
			shown.record?.heldUnits ?? ins?.held_units ?? [],
			shown.record?.branchKeys ?? ins?.branches ?? []
		);
	});

	/** What the frame shows at the player's time (at rest: its start). */
	readonly summary: FrameSummary | null = $derived.by(() => {
		const f = this.frame;
		if (!f) return null;
		const r = this.#shown?.record;
		const tripped = new Map<string, number>();
		for (const e of r?.events ?? [])
			if (e.kind === 'generator_trip')
				tripped.set(
					e.machineId.trim() === '1' ? String(e.bus) : `${e.bus}#${e.machineId.trim()}`,
					e.at
				);
		return f.update(r ? this.player.t : 0, this.theme, tripped);
	});

	readonly mapLayers: Layer[] = $derived.by(() => {
		const f = this.frame;
		const ctors = this.layerCtors;
		const s = this.summary;
		if (!f || !ctors || !s || this.app.mode !== 'dynamics') return [];
		const hover = this.app.dynamicsHover;
		const hoverBus = hover ? Number(hover.key.split('#')[0]) : null;
		return f.layers(ctors, s.t, this.#shown?.record?.events ?? [], hoverBus, this.theme, (bus) =>
			this.onBusClick?.(bus)
		);
	});

	tooltip = (info: PickingInfo): { html: string } | null => {
		const f = this.frame;
		if (!f) return null;
		const esc = (x: string) => x.replace(/[&<>"']/g, (c) => `&#${c.charCodeAt(0)};`);
		if (info.layer?.id === 'dyn-bus-v' && info.index >= 0) {
			const v = f.vNow[info.index];
			const when = this.atRest ? 'Before any event' : `Time ${this.player.t.toFixed(3)} s`;
			return {
				html: `<b>Bus ${f.busIds[info.index]}</b><br>Voltage ${v.toFixed(4)} pu<br>${when}`
			};
		}
		if (info.layer?.id === 'dyn-halos' && info.index >= 0) {
			const h = f.halos[info.index];
			const df = f.haloDf[info.index];
			const units = h.units
				.map((u) =>
					u.held ? `${esc(u.key)}: not simulated` : esc(u.key)
				)
				.join('<br>');
			return {
				html: `<b>Generators at bus ${h.bus}</b><br>${units}${Number.isFinite(df) ? `<br>Largest speed change ${(df * 1000).toFixed(1)} mHz` : ''}`
			};
		}
		if (
			(info.layer?.id === 'dyn-open-branches' || info.layer?.id === 'dyn-open-branches-gap') &&
			info.object
		) {
			const [head, ...rest] = openBranchText(info.object as OpenBranch).split('; ');
			return { html: [`<b>${esc(head)}</b>`, ...rest.map(esc)].join('<br>') };
		}
		return null;
	};

	// ---- experimental ----

	async computeModes(): Promise<void> {
		const local = this.app.activeLocal;
		if (!local || !this.active || this.modesBusy) return;
		if (this.running) {
			this.modesError = 'Run in progress';
			return;
		}
		this.modesBusy = true;
		this.modesError = null;
		const abort = new AbortController();
		this.#modesAbort = abort;
		try {
			const c = await this.#case(local, this.#useValidation(local.id));
			this.modes = await c.linear({ participation_top: 3 }, { signal: abort.signal });
		} catch (e) {
			const err = e as Error;
			this.modesError = err.name === 'AbortError' ? 'Stopped' : err.message;
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
				network: r.network === 'validation' ? VALIDATION_NETWORK : 'the case as loaded',
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
		return title(e);
	}
}
