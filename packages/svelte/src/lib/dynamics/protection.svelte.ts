/** The protection layer of a local case (experimental): relays set by rule
 * (IEC 60255 functions) with their audit and distance-zone spans, the
 * standard test cases, the relays at the operating point and static fault
 * studies. Switched on, the relays act in every RMS run and in the live
 * session of the case.
 *
 * The numbers are the engine's (`@tellegen/engine` protection calls): the
 * settings and studies run on the engine worker, the test suite on an
 * isolated worker of its own, so neither holds up the RMS worker. */

import {
	auditProtection,
	autoProtection,
	protectionAtOperatingPoint,
	protectionTests,
	runProtectionTests,
	staticProtectionStudy,
	testScheme,
	type Finding,
	type ProtectionScheme,
	type ProtectionTest,
	type Relay,
	type RelayOperation,
	type RelaySeen,
	type RmsEvent,
	type SettingOptions,
	type StaticFault,
	type StaticStudy,
	type SuiteReport,
	type ZoneSpan
} from '@tellegen/engine';
import type { AppState } from '../state.svelte.js';
import type { DynamicsController } from './controller.svelte.js';
import { allRmsEvents } from './events.js';

/** The protection of one local case. */
export interface ProtectionState {
	scheme: ProtectionScheme;
	/** The setting basis, as the rules state it. */
	notes: string[];
	audit: Finding[];
	zones: ZoneSpan[];
	/** Off keeps the settings but takes the relays out of every calculation
	 * and the drawing. */
	enabled: boolean;
	tests: ProtectionTest[];
	suite: SuiteReport | null;
}

/** A static study of one fault on the short circuit, shown until the next
 * run is. */
export interface ShownStudy {
	localId: string;
	fault: StaticFault;
	study: StaticStudy;
}

/** What the relays did in what the map shows: a run (at the player's
 * time), the live session, a static study, or (before a run) nothing yet
 * with the planned events. */
export interface RelayView {
	kind: 'run' | 'live' | 'study' | 'planned';
	ops: RelayOperation[];
	/** Time shown, s (`Infinity`: all of it). */
	t: number;
	/** The run's events (faults inside lines are drawn). */
	events: RmsEvent[];
	label: string;
	/** The run carried the relays' record. */
	recorded: boolean;
}

const BAD = (f: Finding) => f.severity === 'error' || f.severity === 'warning';

export class ProtectionController {
	/** Per local case. */
	states: Record<string, ProtectionState> = $state.raw({});
	study: ShownStudy | null = $state.raw(null);
	/** The relay whose zones are drawn. */
	selectedRelay = $state<string | null>(null);
	/** The relays at the operating point of the active case's scheme. */
	operating: { scheme: ProtectionScheme; pickups: RelayOperation[]; seen: RelaySeen[] } | null =
		$state.raw(null);
	suiteRunning = $state(false);
	#suiteAbort: AbortController | null = null;

	constructor(
		private readonly app: AppState,
		private readonly dyn: DynamicsController
	) {}

	/** The engine build carries the relays. */
	get supported(): boolean {
		return !!this.dyn.support?.protection;
	}

	/** Shown: the experimental switch is on and the build carries the relays. */
	get available(): boolean {
		return this.app.preferences.experimental && this.supported;
	}

	/** The active case's protection, on or off. */
	get active(): ProtectionState | null {
		const id = this.app.activeLocal?.id;
		return id ? (this.states[id] ?? null) : null;
	}

	/** The active case's protection while it is switched on. */
	get on(): ProtectionState | null {
		const p = this.active;
		return this.available && p?.enabled ? p : null;
	}

	/** The scheme the relays of `localId` act with (null: none). */
	schemeFor(localId: string): ProtectionScheme | null {
		const p = this.states[localId];
		return this.available && p?.enabled && p.scheme.relays.length ? p.scheme : null;
	}

	/** The active case as the protection calls take it. */
	#input(): { localId: string; moduleJson: string; dyn: string } {
		const local = this.app.activeLocal;
		const a = this.dyn.active;
		if (!local?.studyInputJson || !a || a.status !== 'ready')
			throw new Error('the case has no dynamic data yet: relays act in the dynamic simulation');
		return { localId: local.id, moduleJson: local.studyInputJson, dyn: this.dyn.dynText(a) };
	}

	#set(localId: string, p: ProtectionState | null): void {
		const next = { ...this.states };
		if (p) next[localId] = p;
		else delete next[localId];
		this.states = next;
		if (this.study?.localId === localId) this.study = null;
		this.#syncLive(localId);
	}

	/** The live session of `localId` follows its scheme. */
	#syncLive(localId: string): void {
		const live = this.dyn.live;
		if (live.active && live.localId === localId) void live.setProtection(this.schemeFor(localId));
	}

	#log(severity: 'note' | 'warning', message: string): void {
		this.app.log.record([
			{
				severity,
				stage: 'Protection',
				source: 'tellegen',
				subject: this.app.activeLocal?.label,
				message
			}
		]);
	}

	/** Relays on every line end, transformer, busbar and unit, set by rule,
	 * with the audit, the zone spans and the standard tests. */
	async setByRule(options: SettingOptions = {}): Promise<ProtectionState> {
		const { localId, moduleJson, dyn } = this.#input();
		const auto = await autoProtection(moduleJson, dyn, options, this.dyn.options);
		const tests = await protectionTests(moduleJson, dyn, auto.scheme, {}, this.dyn.options);
		const p: ProtectionState = {
			scheme: auto.scheme,
			notes: auto.notes,
			audit: auto.audit,
			zones: auto.zones,
			enabled: true,
			tests,
			suite: null
		};
		this.#set(localId, p);
		const bad = auto.audit.filter(BAD).length;
		this.#log(
			bad ? 'warning' : 'note',
			`${auto.scheme.relays.length} relays set by rule; audit: ${bad} warning(s) or error(s); ${tests.length} standard tests`
		);
		return p;
	}

	/** Switch the layer: on sets relays by rule when there are none yet. */
	async setLayer(on: boolean): Promise<void> {
		const p = this.active;
		if (on && !p) {
			await this.setByRule();
			return;
		}
		const id = this.app.activeLocal?.id;
		if (p && id) this.#set(id, { ...p, enabled: on });
	}

	/** Replace the scheme (an edited relay, a loaded file): audit again. */
	async update(scheme: ProtectionScheme): Promise<void> {
		const { localId, moduleJson, dyn } = this.#input();
		const { audit, zones } = await auditProtection(moduleJson, dyn, scheme, this.dyn.options);
		const tests = await protectionTests(moduleJson, dyn, scheme, {}, this.dyn.options);
		const prev = this.states[localId];
		this.#set(localId, {
			scheme,
			notes: prev?.notes ?? [],
			audit,
			zones,
			enabled: prev?.enabled ?? true,
			tests,
			suite: null
		});
	}

	/** Change one relay (settings, in service) and audit again. */
	async updateRelay(relay: Relay): Promise<void> {
		const p = this.active;
		if (!p) return;
		await this.update({
			...p.scheme,
			relays: p.scheme.relays.map((r) => (r.id === relay.id ? relay : r))
		});
	}

	/** Protect one element by rule (its relays from the rule set for the
	 * whole case), keeping every other relay as it is. */
	async protectElement(match: (r: Relay) => boolean): Promise<Relay[]> {
		const { localId, moduleJson, dyn } = this.#input();
		const auto = await autoProtection(moduleJson, dyn, {});
		const add = auto.scheme.relays.filter(match);
		if (!add.length) throw new Error('no relay applies to this element');
		const prev = this.states[localId]?.scheme ?? {
			relays: [],
			stuck_breakers: [],
			measuring_time: auto.scheme.measuring_time
		};
		const ids = new Set(add.map((r) => r.id));
		await this.update({ ...prev, relays: [...prev.relays.filter((r) => !ids.has(r.id)), ...add] });
		const p = this.states[localId];
		if (p && !p.notes.length) this.#set(localId, { ...p, notes: auto.notes });
		return add;
	}

	/** Run test cases in RMS (all standard ones by default) on an isolated
	 * worker; {@link stopSuite} stops it. */
	async runSuite(tests?: ProtectionTest[]): Promise<SuiteReport | null> {
		const p = this.active;
		if (!p) throw new Error('set the protection first');
		const { localId, moduleJson, dyn } = this.#input();
		this.#suiteAbort?.abort();
		const abort = new AbortController();
		this.#suiteAbort = abort;
		this.suiteRunning = true;
		try {
			const report = await runProtectionTests(
				moduleJson,
				dyn,
				p.scheme,
				tests,
				abort.signal,
				this.dyn.options
			);
			const now = this.states[localId];
			if (now) this.#set(localId, { ...now, suite: tests ? mergeSuite(now.suite, report) : report });
			this.#log(
				report.failed ? 'warning' : 'note',
				`protection tests: ${report.passed} passed, ${report.failed} failed`
			);
			return report;
		} catch (e) {
			if (abort.signal.aborted) return null;
			throw e;
		} finally {
			if (this.#suiteAbort === abort) {
				this.#suiteAbort = null;
				this.suiteRunning = false;
			}
		}
	}

	stopSuite(): void {
		this.#suiteAbort?.abort();
	}

	/** Replay one test as a run of the case (plots, map playback, relay
	 * record). */
	async replay(test: ProtectionTest): Promise<void> {
		const p = this.active;
		if (!p) throw new Error('set the protection first');
		const first = test.expect.find((x) => 'relay' in x) as { relay?: string } | undefined;
		await this.dyn.run({
			replay: {
				label: test.name,
				events: test.events,
				tEnd: test.t_end,
				outputStep: 0.005,
				protection: testScheme(p.scheme, test),
				seek: test.fault_time + 0.2
			}
		});
		if (first?.relay) this.selectedRelay = first.relay;
	}

	/** A fault left to the relays in RMS: a bolted fault at `fault` from
	 * 0.1 s, nothing else clears it. */
	async faultRms(fault: StaticFault, label: string): Promise<void> {
		const scheme = this.schemeFor(this.app.activeLocal?.id ?? '');
		if (!scheme) throw new Error('switch the protection layer on first');
		const ev: RmsEvent =
			fault.kind === 'line'
				? {
						kind: 'line_fault',
						time: 0.1,
						from: fault.from,
						to: fault.to,
						circuit: fault.circuit ?? '1',
						location: fault.location,
						r: 0,
						x: 1e-4,
						permanent: true
					}
				: { kind: 'bus_fault', time: 0.1, bus: fault.bus, r: 0, x: 1e-4 };
		await this.dyn.run({
			replay: { label, events: [ev], tEnd: 1.5, outputStep: 0.005, protection: scheme, seek: 0.3 }
		});
	}

	/** A static protection study (load flow layer) of one fault. */
	async studyFault(fault: StaticFault): Promise<StaticStudy> {
		const p = this.active;
		if (!p?.enabled) throw new Error('switch the protection layer on first');
		const { localId, moduleJson, dyn } = this.#input();
		const study = await staticProtectionStudy(moduleJson, dyn, p.scheme, fault, 3, this.dyn.options);
		this.study = { localId, fault, study };
		return study;
	}

	/** The relays at the operating point of the active scheme (once per
	 * scheme). */
	async evaluateOperating(): Promise<void> {
		const p = this.on;
		if (!p || this.operating?.scheme === p.scheme) return;
		const { moduleJson, dyn } = this.#input();
		const scheme = p.scheme;
		const r = await protectionAtOperatingPoint(moduleJson, dyn, scheme, this.dyn.options);
		if (this.active?.scheme === scheme) this.operating = { scheme, ...r };
	}

	/** A scheme from a file (`*.protection.json`). */
	async load(text: string): Promise<void> {
		const scheme = JSON.parse(text) as ProtectionScheme;
		if (!scheme || !Array.isArray(scheme.relays)) throw new Error('not a protection scheme');
		await this.update(scheme);
	}

	/** Forget a closed local case. */
	forget(localId: string): void {
		if (!(localId in this.states) && this.study?.localId !== localId) return;
		this.#set(localId, null);
	}

	/** What the relays did in what the map shows for the active case. */
	get view(): RelayView | null {
		const local = this.app.activeLocal;
		if (!local || !this.on) return null;
		const live = this.dyn.live;
		if (this.app.mode === 'dynamics' && live.run && live.localId === local.id)
			return {
				kind: 'live',
				ops: live.relays,
				t: live.t,
				events: [],
				label: 'Live',
				recorded: true
			};
		const s = this.study;
		if (s?.localId === local.id) {
			const f = s.fault;
			return {
				kind: 'study',
				ops: s.study.operations,
				t: Infinity,
				events:
					f.kind === 'line'
						? [
								{
									kind: 'line_fault',
									time: 0,
									from: f.from,
									to: f.to,
									circuit: f.circuit ?? '1',
									location: f.location,
									r: 0,
									x: 1e-4,
									permanent: true
								}
							]
						: [],
				label: 'Load flow study',
				recorded: true
			};
		}
		const r = this.dyn.current;
		if (r?.run && r.localId === local.id)
			return {
				kind: 'run',
				ops: r.run.meta.protection?.operations ?? [],
				t: this.dyn.player.t,
				events: r.replay?.events ?? allRmsEvents(r.events),
				label: r.replay?.label ?? (r.events.length ? this.dyn.describe(r.events[0]) : 'Run'),
				recorded: !!r.run.meta.protection
			};
		return {
			kind: 'planned',
			ops: [],
			t: 0,
			events: allRmsEvents(this.dyn.events),
			label: '',
			recorded: false
		};
	}
}

/** `add`'s outcomes over `prev`'s, by test name. */
export function mergeSuite(prev: SuiteReport | null, add: SuiteReport): SuiteReport {
	const tests = [
		...(prev?.tests ?? []).filter((t) => !add.tests.some((a) => a.name === t.name)),
		...add.tests
	];
	const passed = tests.filter((t) => t.passed).length;
	return { audit: add.audit, tests, passed, failed: tests.length - passed };
}

/** Function numbers (IEEE C37.2) by function type. */
export const FUNCTION_CODE: Record<string, string> = {
	distance: '21',
	differential: '87',
	overcurrent: '51',
	earth_fault: '51N',
	undervoltage: '27',
	overvoltage: '59',
	neutral_overvoltage: '59N',
	underfrequency: '81U',
	overfrequency: '81O',
	rocof: '81R',
	overexcitation: '24',
	reverse_power: '32',
	loss_of_field: '40',
	out_of_step: '78',
	negative_sequence: '46',
	thermal: '49',
	fault_ride_through: 'FRT'
};

/** The function numbers of a relay, with 50BF and 79. */
export function relayCodes(r: Relay): string {
	const out = new Set<string>();
	for (const f of r.functions)
		out.add('label' in f && f.label ? f.label.split(' ')[0] : (FUNCTION_CODE[f.type] ?? f.type));
	if (r.breaker_failure) out.add('50BF');
	if (r.autoreclose) out.add('79');
	return [...out].join(' ');
}

/** The relays of one element: the line between two buses (both ends), the
 * transformer between them, or a bus (its busbar and units). */
export function elementMatch(
	target: { kind: 'branch'; from: number; to: number; circuit?: string } | { kind: 'bus'; bus: number }
): (r: Relay) => boolean {
	if (target.kind === 'bus')
		return (r) => {
			const l = r.location;
			return (l.kind === 'busbar' || l.kind === 'generator' || l.kind === 'inverter') && l.bus === target.bus;
		};
	const same = (a: number, b: number) =>
		(a === target.from && b === target.to) || (a === target.to && b === target.from);
	const ckt = (c?: string) => !target.circuit || (c ?? '1').trim() === target.circuit.trim();
	return (r) => {
		const l = r.location;
		return (l.kind === 'line' || l.kind === 'transformer') && same(l.from, l.to) && ckt(l.circuit);
	};
}

/** Whether `e` touches the element of {@link elementMatch}'s target. */
export function eventTouches(
	e: RmsEvent,
	target: { kind: 'branch'; from: number; to: number } | { kind: 'bus'; bus: number }
): boolean {
	if (target.kind === 'branch') {
		if (!('from' in e) || !('to' in e)) return false;
		const f = e.from as number;
		const t = e.to as number;
		return (f === target.from && t === target.to) || (f === target.to && t === target.from);
	}
	return 'bus' in e && e.bus === target.bus;
}
