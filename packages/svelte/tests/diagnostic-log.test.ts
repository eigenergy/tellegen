import { afterEach, describe, expect, it, vi } from 'vitest';
import type { BrowserStudy, IngestedCase, PowerIoDiagnostic } from '@tellegen/engine';

const engine = vi.hoisted(() => ({ createStudy: vi.fn() }));
vi.mock('@tellegen/engine', async (importOriginal) => {
	const actual = await importOriginal<typeof import('@tellegen/engine')>();
	return { ...actual, createStudy: engine.createStudy };
});

import {
	DiagnosticLog,
	errorStage,
	powerIoEntry,
	solveEntry,
	type LogEntryInput
} from '../src/lib/log.svelte.js';
import { AppState } from '../src/lib/state.svelte.js';
import { Controller } from '../src/lib/controller.svelte.js';

const note = (message: string, extra: Partial<LogEntryInput> = {}): LogEntryInput => ({
	severity: 'note',
	stage: 'Open',
	source: 'PowerIO',
	message,
	...extra
});

const dropped: PowerIoDiagnostic = {
	code: 'EMIT.MATPOWER.FIELD_DROPPED',
	severity: 'warning',
	message: '95 bus location(s) dropped: MATPOWER .m has no coordinate field',
	target: 'bus',
	suggested_action: 'emit a .geo.json sidecar to keep them',
	details: { count: 95 }
};

describe('DiagnosticLog', () => {
	it('keeps findings in order and counts a repeat in place', () => {
		let now = 1000;
		const log = new DiagnosticLog(() => now);
		log.record([note('first'), note('second', { severity: 'warning' })]);
		now = 2000;
		log.record(note('first'));
		expect(log.entries.map((e) => [e.message, e.count])).toEqual([
			['first', 2],
			['second', 1]
		]);
		expect(log.entries[0]).toMatchObject({ first: 1000, last: 2000 });
		expect(log.counts).toEqual({ error: 0, warning: 1, note: 1 });
	});

	it('treats the same message from another step or file as a new finding', () => {
		const log = new DiagnosticLog();
		log.record([note('same'), note('same', { stage: 'Export' }), note('same', { subject: 'b.m' })]);
		expect(log.entries).toHaveLength(3);
	});

	it('restates without counting when repeat is false, and takes the new details', () => {
		const log = new DiagnosticLog();
		log.record(note('island'), { repeat: false });
		log.record(note('island', { details: { detail: 'why' } }), {
			repeat: false
		});
		expect(log.entries).toHaveLength(1);
		expect(log.entries[0]).toMatchObject({
			count: 1,
			details: { detail: 'why' }
		});
	});

	it('drops empty fields and empty messages', () => {
		const log = new DiagnosticLog();
		log.record([note(''), note('kept', { buses: [], details: {}, code: '' })]);
		expect(log.entries).toHaveLength(1);
		expect(Object.keys(log.entries[0]).sort()).toEqual(
			['count', 'first', 'id', 'last', 'message', 'severity', 'source', 'stage'].sort()
		);
	});

	it('writes a dated document and clears', () => {
		const log = new DiagnosticLog(() => Date.UTC(2026, 9, 30, 12));
		log.record(powerIoEntry(dropped, { stage: 'Export', subject: 'Portugal.m' }));
		const doc = log.toJSON();
		expect(doc).toMatchObject({
			format: 'tellegen.log',
			version: 1,
			written_at: '2026-10-30T12:00:00.000Z',
			counts: { error: 0, warning: 1, note: 0 }
		});
		expect(doc.entries[0]).toMatchObject({
			code: 'EMIT.MATPOWER.FIELD_DROPPED',
			subject: 'Portugal.m',
			hint: 'emit a .geo.json sidecar to keep them',
			first: '2026-10-30T12:00:00.000Z'
		});
		log.clear();
		expect(log.entries).toEqual([]);
	});
});

describe('entry builders', () => {
	it('reads PowerIO severities, with remarks as notes', () => {
		const severity = (s: PowerIoDiagnostic['severity']) =>
			powerIoEntry({ ...dropped, severity: s }, { stage: 'Open' }).severity;
		expect([severity('error'), severity('warning'), severity('remark'), severity('note')]).toEqual([
			'error',
			'warning',
			'note',
			'note'
		]);
	});

	it('marks solver findings that change the numbers as warnings', () => {
		const entry = (code: string) => solveEntry({ code, message: 'm', buses: [3, 4] }, {});
		expect(entry('BUILD.HVDC.TERMINAL_INACTIVE')).toMatchObject({
			severity: 'warning',
			stage: 'Solve',
			source: 'tellegen',
			buses: [3, 4]
		});
		expect(entry('island_deenergized').severity).toBe('warning');
		expect(entry('BUILD.HVDC.FIXED_INJECTION').severity).toBe('note');
	});

	it('places app errors in the step their wording names', () => {
		expect(errorStage('Portugal: could not export study: boom')).toBe('Export');
		expect(errorStage('Portugal: DC OPF has no feasible solution at this demand')).toBe('Solve');
		expect(errorStage('case.m: unexpected token while parsing')).toBe('Open');
		expect(errorStage('No coordinates matched the selected case')).toBe('Coordinates');
		expect(errorStage('Too many requests')).toBe('App');
	});
});

describe('AppState and Controller record into the log', () => {
	afterEach(() => vi.unstubAllGlobals());

	it('records every error once per write, with its detail', () => {
		const app = new AppState();
		app.error = 'Portugal: the browser study is unavailable';
		app.errorDetail = 'WebAssembly.instantiate failed';
		app.error = null;
		app.error = 'Portugal: the browser study is unavailable';
		expect(app.log.entries).toHaveLength(1);
		expect(app.log.entries[0]).toMatchObject({
			severity: 'error',
			stage: 'Solve',
			count: 2,
			details: { detail: 'WebAssembly.instantiate failed' }
		});
	});

	const payload: IngestedCase = {
		name: 'case',
		base_mva: 100,
		n_bus: 2,
		n_branch: 1,
		n_gen: 1,
		load_mw: 50,
		gen_mw: 50,
		has_coords: false,
		coords_kind: 'synthetic_pending',
		diagnostics: [{ code: 'READ.PIO.NOTE', severity: 'note', message: 'retained finding' }],
		warnings: ['1 unlocated bus left off the map'],
		module_json: '{"module":"stub"}',
		topology: {
			buses: [
				{ id: 1, uid: null, demand_mw: 0, gen_mw: 50 },
				{ id: 2, uid: null, demand_mw: 50, gen_mw: 0 }
			],
			branches: [{ id: 0, uid: null, from: 1, to: 2, rate_mw: 100 }]
		},
		view: null
	};

	it('logs what opening, solving and exporting a case reported', async () => {
		vi.stubGlobal('document', {
			createElement: () => ({ click() {}, remove() {} }),
			body: { appendChild() {} }
		});
		vi.stubGlobal('URL', {
			createObjectURL: () => 'blob:x',
			revokeObjectURL() {}
		});
		const app = new AppState();
		const ctrl = new Controller(app, {
			apiBase: null,
			loadDefaultCases: false
		});
		vi.spyOn(app, 'requestFrame').mockResolvedValue();
		const c = await ctrl.openBalancedModule({
			name: 'Portugal',
			payload,
			activate: false
		});
		app.addLocal(c);
		expect(app.log.entries.map((e) => [e.stage, e.source, e.subject, e.message])).toEqual([
			['Open', 'PowerIO', 'Portugal.pio.json', 'retained finding'],
			['Open', 'tellegen', 'Portugal.pio.json', '1 unlocated bus left off the map']
		]);

		const hvdc = {
			code: 'BUILD.HVDC.FIXED_INJECTION',
			message: 'line 0 carries 80 MW',
			buses: [1, 2]
		};
		for (const seq of [1, 2]) {
			c.solveSeq = seq;
			c.solution = {
				objective: 1,
				prices: [],
				va: [],
				w: [],
				flows: [],
				dispatch: [],
				diagnostics: [hvdc]
			};
			ctrl.finishSolve(c, seq, null);
		}
		const solve = app.log.entries.filter((e) => e.stage === 'Solve');
		expect(solve).toHaveLength(1);
		expect(solve[0]).toMatchObject({
			count: 1,
			buses: [1, 2],
			subject: 'Portugal'
		});

		const study = {
			currentSolution: vi.fn(async () => c.solution),
			commit: vi.fn(async () => ({})),
			export: vi.fn(async () => ({
				text: 'mpc',
				diagnostics: [dropped],
				format: 'matpower',
				extension: 'm'
			})),
			free: vi.fn()
		} as unknown as BrowserStudy;
		engine.createStudy.mockResolvedValue(study);
		expect(await ctrl.exportCaseAs(c, 'matpower')).toEqual([dropped]);
		await ctrl.exportCaseAs(c, 'matpower');
		const exported = app.log.entries.filter((e) => e.stage === 'Export');
		expect(exported).toHaveLength(1);
		expect(exported[0]).toMatchObject({
			severity: 'warning',
			subject: 'Portugal.m',
			code: 'EMIT.MATPOWER.FIELD_DROPPED',
			count: 1
		});
	});
});
