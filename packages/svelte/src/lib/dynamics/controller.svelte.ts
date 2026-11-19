/** Dynamics mode: attach PSS/E dynamic data to a local case, open it in the
 * RMS worker, pick events, run, keep the last runs, and play one back.
 *
 * Everything numerical happens in the engine (`@tellegen/engine` RMS
 * worker); this class holds what the panel, the timeline and the map show,
 * and the ids that tie them together.
 * @block Dynamics controller */

import {
	dynamicInput,
	ingestCase,
	looksLikeDyr,
	openRmsCase,
	rmsSupport,
	type BrowserRmsCase,
	type ModelOverride,
	type ProtectionScheme,
	type RmsCaseOptions,
	type RmsEvent,
	type RmsFlatCheck,
	type RmsInspection,
	type RmsLinearRequest,
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
import { LiveView } from './live.svelte.js';
import { UnitCatalog, unitGlyphLayer, unitSummary, unitKey as catalogKey } from './units.svelte.js';
import { ProtectionController } from './protection.svelte.js';
import { protectionLayers, RELAY_LAYER } from './protection-layers.js';
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

/** One model of the hosted model library (examples/models/index.json,
 * written at deploy time from a library config). */
export interface LibraryModel {
	id: string;
	/** The library (group) it belongs to. */
	library: string;
	title: string;
	/** Short facts shown beside the title, e.g. "10 buses". */
	info?: string | null;
	/** A short label shown with the title, such as a restriction of use. */
	tag?: string | null;
	licence?: string | null;
	/** Files relative to examples/models/: case files, dynamic data (.dyr),
	 * coordinates (.geo.csv, .geojson) and controller graphs (.models.json). */
	files: { path: string; kind: 'case' | 'dyr' | 'geo' | 'models' }[];
	/** It carries dynamic data: it opens in Dynamics, else in steady state. */
	dynamics: boolean;
}
export interface ModelLibrary {
	title: string;
	libraries: { id: string; title: string; tag?: string | null; note?: string | null }[];
	cases: LibraryModel[];
}

/** The dynamic data attached to one local case. */
export interface DynAttachment {
	localId: string;
	fileName: string;
	text: string;
	/** The bundled WECC 240 demo (its validated scenarios apply). */
	demo: boolean;
	/** Typical parameters from "Add default dynamics", not the owner's. */
	synthetic: boolean;
	/** Controller block graphs that replace or add a unit's controller. */
	models: ModelOverride[];
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
	/** Increases with every attach, so an edit computed from older data
	 * can tell that the data moved on while it ran. */
	revision: number;
}

/** What an edit of the dynamic data starts from, taken before it awaits
 * the worker: the local case, the attachment revision and the network it
 * read. {@link DynamicsController.editDynamics} applies the result only
 * while all three still hold.
 * @block Edit origin */
export interface EditOrigin {
	localId: string;
	revision: number;
	/** The network JSON the edit read (`studyInputJson`). */
	network: string | null;
	/** The DYR text and controller graphs the edit starts from. */
	text: string;
	models: ModelOverride[];
}

/** An edit whose case changed (another case active, the data or the
 * network replaced) while the worker computed it; nothing was applied. */
export class StaleEditError extends Error {
	constructor(label: string) {
		super(`${label} changed while the edit was computed; nothing was applied. Try again.`);
		this.name = 'StaleEditError';
	}
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
	/** A protection test or a fault left to the relays: its name and the
	 * engine's events (the run has no {@link events} of its own). */
	replay?: { label: string; events: RmsEvent[] };
	/** The relays that acted during the run. */
	protection?: ProtectionScheme;
}

/** A run of given engine events instead of the picked ones (a protection
 * test, a fault left to the relays). */
export interface DynReplay {
	label: string;
	events: RmsEvent[];
	tEnd: number;
	outputStep?: number;
	/** The relays acting (null: none). */
	protection: ProtectionScheme | null;
	/** Where playback starts, s. */
	seek?: number;
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

/** Why dynamics is unavailable while it is turned off. */
export const DYNAMICS_OFF = 'Dynamics is turned off in this build';

/** The dynamics state of the app: the dynamic data attached to each case,
 * the events and options of the next run, the runs, playback ({@link
 * DynamicsPlayer}), the live view ({@link LiveView}) and the map layers.
 * @block DynamicsController */
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
	support: { rms: boolean; experimental: boolean; protection?: boolean } | null =
		$state.raw(null);
	/** Experimental: the modes of the open case. */
	modes: RmsLinearResult | null = $state.raw(null);
	modesError = $state<string | null>(null);
	modesBusy = $state(false);
	selectedMode = $state<number | null>(null);
	/** Engine refusal of the last run, verbatim. */
	error = $state<string | null>(null);
	theme: 'light' | 'dark' = $state('light');
	readonly player = new DynamicsPlayer();
	/** Experimental: the active case simulated live. */
	readonly live = new LiveView();
	/** Experimental: the active case's units for the unit editor. */
	readonly catalog = new UnitCatalog();
	/** The Models tab consumes the selection made on the map. */
	inspectUnit: { caseId: string; key: string } | null = $state.raw(null);
	/** Experimental: the relays of the local cases. */
	readonly protection: ProtectionController;
	#textLayer: typeof import('@deck.gl/layers').TextLayer | null = $state.raw(null);
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
	#revision = 0;

	constructor(
		private readonly app: AppState,
		private readonly ingest: (files: File[]) => Promise<void>
	) {
		this.protection = new ProtectionController(app, this);
	}

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

	/** Attach `text` to `local` and open it in the RMS worker.
	 * @block Attach dynamic data */
	async attach(
		local: LocalCase,
		fileName: string,
		text: string,
		demo = false,
		synthetic = false,
		models: ModelOverride[] = this.#exampleModels ?? []
	): Promise<void> {
		if (!local.studyInputJson) throw new Error(`${local.label} has no network to simulate`);
		const revision = ++this.#revision;
		this.#free(local.id);
		this.#described.delete(local.id);
		this.#set(local.id, {
			localId: local.id,
			fileName,
			text,
			demo,
			synthetic,
			models,
			status: 'opening',
			inspection: null,
			network: 'case',
			preparing: false,
			prepareError: null,
			error: null,
			flat: null,
			flatError: null,
			rest: null,
			openMs: null,
			revision
		});
		this.modes = null;
		const started = performance.now();
		const validation = this.#useValidation(local.id);
		// A later attach of the same case owns what follows.
		const current = () => this.attachments[local.id]?.revision === revision;
		try {
			const c = await this.#case(local, validation);
			if (!current()) return;
			this.#patch(local.id, {
				status: 'ready',
				openMs: performance.now() - started
			});
			await this.#describe(local.id, validation, c);
		} catch (e) {
			if (current()) this.#patch(local.id, { status: 'failed', error: (e as Error).message });
			return;
		}
		if (!current()) return;
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
	 * panel describes the case the next run uses.
	 * @block Refresh the attachment */
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

	/** The dynamic data of `a` as the engine reads it: DYR, with its
	 * controller graphs when it has any. */
	dynText(a: DynAttachment): string {
		return dynamicInput(a.text, a.models);
	}

	/** Where an edit of the active case's dynamic data starts. Take it
	 * before the first `await` of the edit and pass it to
	 * {@link editDynamics}. */
	editOrigin(): EditOrigin {
		const local = this.app.activeLocal;
		const a = this.active;
		if (!local || !a) throw new Error('no dynamic data attached');
		return {
			localId: local.id,
			revision: a.revision,
			network: local.studyInputJson ?? null,
			text: a.text,
			models: a.models
		};
	}

	/** Replace the dynamic data of the case `origin` names (edited
	 * parameters or models, controller graphs) and prepare it again; that
	 * case's live view ends. Refused with {@link StaleEditError}, changing
	 * nothing, when the case was closed, its data attached again or its
	 * network changed since `origin` was taken: the edit was computed from
	 * data that is no longer there. Which case is active does not matter.
	 * @block Edit the dynamic data */
	async editDynamics(origin: EditOrigin, text: string, models?: ModelOverride[]): Promise<void> {
		const local = this.app.localCases.find((c) => c.id === origin.localId);
		const a = this.attachments[origin.localId];
		if (
			!local ||
			!a ||
			a.revision !== origin.revision ||
			(local.studyInputJson ?? null) !== origin.network
		)
			throw new StaleEditError(local?.label ?? 'The case');
		if (this.live.localId === local.id) this.live.stop();
		await this.attach(local, a.fileName, text, a.demo, a.synthetic, models ?? a.models);
		const now = this.attachments[local.id];
		if (now?.status === 'failed') throw new Error(now.error ?? 'the case does not initialize');
	}

	/** Forget a closed local case: its attachment, runs and worker cases. */
	forget(localId: string): void {
		if (this.live.localId === localId) this.live.stop();
		this.protection.forget(localId);
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
			p = module.then((m) => openRmsCase(m, dynamicInput(a.text, a.models), this.options));
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

	/** Open the bundled WECC 240 demo: its case, dynamic data and coordinates.
	 * @block Open the demo */
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

	// @block Hosted model library
	// The catalog a deployment serves (examples/models/index.json) and
	// opening one of its models as the active case.

	/** The hosted model library; null until loaded, empty when none is hosted. */
	examples: ModelLibrary | null = $state.raw(null);
	#examplesLoad: Promise<ModelLibrary> | null = null;
	#exampleLabel: string | null = null;
	#exampleModels: ModelOverride[] | null = null;

	/** Load examples/models/index.json once; an absent library is empty. */
	loadExamples(): Promise<ModelLibrary> {
		const empty = (): ModelLibrary => ({ title: 'Model library', libraries: [], cases: [] });
		this.#examplesLoad ??= fetch(`${this.examplesBase}/models/index.json`)
			.then(async (r) => {
				const json = (r.headers.get('content-type') ?? '').includes('json');
				const c = r.ok && json ? ((await r.json()) as ModelLibrary) : empty();
				const lib: ModelLibrary = {
					title: c.title || 'Model library',
					libraries: Array.isArray(c.libraries) ? c.libraries : [],
					cases: Array.isArray(c.cases) ? c.cases.filter((m) => Array.isArray(m.files)) : []
				};
				this.examples = lib;
				return lib;
			})
			.catch(() => {
				const lib = empty();
				this.examples = lib;
				return lib;
			});
		return this.#examplesLoad;
	}

	/** How a library model is named: its title and tag. */
	exampleName(m: LibraryModel): string {
		return m.tag ? `${m.title} (${m.tag})` : m.title;
	}

	/** The label the next ingested case takes when a demo or library model opens it. */
	get exampleLabel(): string | null {
		return this.#demoNext ? DEMO_LABEL : this.#exampleLabel;
	}

	/** Open a library model: its case files, and with them its dynamic data,
	 * coordinates and controller graphs. A model with dynamic data opens in
	 * Dynamics, any other in steady state.
	 * @block Open a library model */
	async openExample(id: string): Promise<void> {
		const library = await this.loadExamples();
		const m = library.cases.find((c) => c.id === id);
		if (!m) throw new Error(`no hosted model ${id}`);
		if (!m.files.some((f) => f.kind === 'case')) throw new Error(`${m.title} has no case file`);
		const base = `${this.examplesBase}/models`;
		const get = async (path: string): Promise<Blob> => {
			const r = await fetch(`${base}/${path.split('/').map(encodeURIComponent).join('/')}`);
			if (!r.ok) throw new Error(`${path}: HTTP ${r.status}`);
			return r.blob();
		};
		const files: File[] = [];
		let models: ModelOverride[] = [];
		for (const f of m.files) {
			const blob = await get(f.path);
			if (f.kind === 'models') {
				const doc = JSON.parse(await blob.text());
				models = Array.isArray(doc) ? doc : (doc.models ?? []);
			} else files.push(new File([blob], f.path.split('/').pop()!));
		}
		this.app.mode = m.dynamics ? 'dynamics' : 'steady';
		this.#exampleLabel = this.exampleName(m);
		this.#exampleModels = models.length ? models : null;
		try {
			await this.ingest(files);
		} finally {
			this.#exampleLabel = null;
			this.#exampleModels = null;
		}
	}

	// @block Events of the next run
	// Adding, changing and removing events, the validated scenarios, the
	// options, and the checks that name an event the engine would refuse.

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

	/** Errors per event id.
	 * @block Check the events */
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

	// @block Runs
	// A run in the RMS worker, its record (run, screening, notes) and the
	// last runs kept for comparison and playback.

	/** Run the picked events, or (`replay`) given engine events. The relays
	 * of the case act when its protection layer is on. */
	async run(opts: { agent?: boolean; replay?: DynReplay } = {}): Promise<DynRunRecord | null> {
		const replay = opts.replay ?? null;
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
		const errs = replay ? new Map<string, string>() : this.eventErrors;
		if (errs.size) {
			this.error = [...errs.values()][0];
			return null;
		}
		this.error = null;
		this.running = true;
		this.progress = { t: 0, tEnd: replay ? replay.tEnd : this.tEnd };
		this.player.pause();
		const abort = new AbortController();
		this.#abort = abort;
		const started = performance.now();
		const events: DynEvent[] = replay ? [] : this.events.map((e) => ({ ...e }));
		const options = { ...this.options };
		const dt = this.dt;
		const tEnd = replay ? replay.tEnd : this.tEnd;
		const badge = replay ? null : this.badge;
		const protection = replay ? replay.protection : this.protection.schemeFor(local.id);
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
			experimental: a.synthetic,
			...(replay ? { replay: { label: replay.label, events: replay.events } } : {}),
			...(protection ? { protection } : {})
		};
		try {
			void this.#loadCtors();
			const c = await this.#case(local, useValidation);
			void this.#describe(local.id, useValidation, c);
			record.heldUnits = c.inspection.held_units ?? [];
			record.branchKeys = c.inspection.branches;
			record.run = await c.run(
				{
					events: replay ? replay.events : allRmsEvents(events),
					t_end: tEnd,
					dt,
					...(replay?.outputStep ? { output_step: replay.outputStep } : {}),
					...(protection ? { protection } : {})
				},
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
		if (replay?.seek !== undefined && record.run) {
			this.player.pause();
			this.player.seek(Math.min(replay.seek, record.run.time[record.run.time.length - 1]));
		}
		this.onRun?.(record);
		return record;
	}

	stop(): void {
		this.#abort?.abort();
	}

	/** Show run `id`: load the player and start playback. */
	select(id: number): void {
		this.currentRunId = id;
		// A static protection study gives way to the run.
		this.protection.study = null;
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
		const { ScatterplotLayer, PathLayer, TextLayer } = await import('@deck.gl/layers');
		this.#textLayer = TextLayer;
		this.layerCtors = { ScatterplotLayer, PathLayer };
	}

	// @block Map
	// What the map shows of a run or the live view: the layers of the
	// current frame and the tooltips of buses, branches and units.

	/** The run the map draws: the selected run of the active case, else (in
	 * Dynamics mode, before a run) that case at rest. */
	readonly #shown = $derived.by(() => {
		const local = this.app.activeLocal;
		if (!local?.view) return null;
		if (this.app.mode === 'dynamics' && this.live.run && this.live.localId === local.id)
			return { record: null, run: this.live.run, local, live: true };
		const r = this.current;
		if (r?.run && r.localId === local.id) return { record: r, run: r.run, local, live: false };
		const a = this.attachments[local.id];
		if (this.app.mode === 'dynamics' && a?.rest)
			return { record: null, run: a.rest, local, live: false };
		return null;
	});

	/** The map shows the case at rest, before any run of it. */
	get atRest(): boolean {
		return !!this.#shown && !this.#shown.record && !this.#shown.live;
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
		const t = this.#shown?.live ? f.run.time[f.run.time.length - 1] : r ? this.player.t : 0;
		return f.update(t, this.theme, tripped);
	});

	readonly mapLayers: Layer[] = $derived.by(() => {
		const f = this.frame;
		const ctors = this.layerCtors;
		const s = this.summary;
		if (!f || !ctors || !s || this.app.mode !== 'dynamics') return [];
		const hover = this.app.dynamicsHover;
		const hoverBus = hover ? Number(hover.key.split('#')[0]) : null;
		const layers = f.layers(
			ctors,
			s.t,
			this.#shown?.record?.events ?? [],
			hoverBus,
			this.theme,
			(bus) => this.onBusClick?.(bus)
		);
		// Experimental: the relays, their zones and what they switched.
		const local = this.app.activeLocal;
		const prot = this.protection.on;
		const view = prot ? this.protection.view : null;
		if (prot && view && local?.view)
			layers.push(
				...protectionLayers(
					prot,
					view,
					this.protection.selectedRelay,
					local.view.buses,
					local.view.branches,
					ctors,
					(id) => {
						this.protection.selectedRelay = this.protection.selectedRelay === id ? null : id;
					}
				)
			);
		// Experimental: a clickable chip per unit opens its editor.
		if (this.app.preferences.experimental && this.#textLayer && local?.view && this.catalog.units.length)
			layers.push(
				unitGlyphLayer(this.#textLayer, this.catalog.units, local.view.buses, this.theme, (u) => {
					this.inspectUnit = { caseId: local.id, key: catalogKey(u) };
				})
			);
		return layers;
	});

	tooltip = (info: PickingInfo): { html: string } | null => {
		const f = this.frame;
		if (!f) return null;
		const esc = (x: string) => x.replace(/[&<>"']/g, (c) => `&#${c.charCodeAt(0)};`);
		if (info.layer?.id === 'dyn-bus-v' && info.index >= 0) {
			const v = f.vNow[info.index];
			const when = this.atRest ? 'Before any event' : `Time ${(this.summary?.t ?? this.player.t).toFixed(3)} s`;
			return {
				html: `<b>Bus ${f.busIds[info.index]}</b><br>Voltage ${v.toFixed(4)} pu<br>${when}`
			};
		}
		if (info.layer?.id === RELAY_LAYER && info.object) {
			const m = info.object as { id: string; state: string };
			return { html: `<b>Relay ${esc(m.id)}</b><br>${esc(m.state)}<br>click to draw its zones` };
		}
		if (info.layer?.id === 'dyn-units' && info.object) {
			const u = (info.object as { unit: import('@tellegen/engine').DyrGeneratorView }).unit;
			return {
				html: `<b>Unit ${u.bus} '${esc(u.machine_id.trim())}'</b><br>${esc(unitSummary(u))}<br>click to edit its models`
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

	// @block Experimental tools
	// Behind Settings, Experimental features: small-signal modes, frequency
	// responses and the export of a run.

	async startLive() {
		const a = this.active;
		const l = this.app.activeLocal;
		if (!a?.inspection || !l?.studyInputJson || this.running || !this.app.preferences.experimental) return;
		this.live.open = true;
		await this.live.start(l.id, {
			moduleJson: l.studyInputJson,
			dynText: this.dynText(a),
			inspection: a.inspection,
			loadModel: this.options.load_model ?? 'power_flow',
			protection: this.protection.schemeFor(l.id)
		});
	}

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

	/** A small-signal analysis of the active case's prepared case (the
	 * frequency responses of `request.bode`). */
	async linearAnalysis(request: RmsLinearRequest): Promise<RmsLinearResult> {
		const local = this.app.activeLocal;
		if (!local || !this.active) throw new Error('no dynamic data attached');
		if (this.running) throw new Error('A run is in progress.');
		const c = await this.#case(local, this.#useValidation(local.id));
		return c.linear(request);
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
					events: r.replay?.events ?? allRmsEvents(r.events),
					...(r.protection ? { protection: r.protection } : {})
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
