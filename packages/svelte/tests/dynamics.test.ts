import { describe, expect, it } from 'vitest';
import {
	allRmsEvents,
	busFault,
	branchTrip,
	circuitsBetween,
	cyclesToSeconds,
	defaultClearing,
	fmtCycles,
	generatorTrip,
	lineFault,
	loadStep,
	markers,
	secondsToCycles,
	sentence,
	toRmsEvents,
	validate
} from '../src/lib/dynamics/events.js';
import { announcement, failureTime, headline, rows } from '../src/lib/dynamics/verdict.js';
import { groupNotes } from '../src/lib/dynamics/notes.js';
import { m4 } from '../src/lib/dynamics/decimate.js';
import { advance, rateAt, startTime } from '../src/lib/dynamics/player.svelte.js';
import {
	sampleAt,
	valueAt,
	sumPrefix,
	envelope,
	coiRelativeDeg
} from '../src/lib/dynamics/series.js';
import { DYN_V_STOPS, dynVoltageColor, luminance } from '../src/lib/colors.js';
import {
	validationBadge,
	VALIDATION_OPTIONS,
	WECC240_PRESETS
} from '../src/lib/dynamics/presets.js';
import { UserPreferences } from '../src/lib/settings.svelte.js';
import type { RmsScreening } from '@tellegen/engine';

const inspection = {
	buses: [1101, 1401, 2604],
	branches: [
		[1101, 1401, '1'],
		[1101, 1401, '2']
	] as [number, number, string][],
	generators: [
		{
			bus: 1431,
			machine_id: 'N ',
			models: [],
			states: [],
			initial_states: [],
			v_ref: 1,
			p_ref: 1,
			mbase: 2500,
			h: 4
		}
	],
	load_buses: [2604],
	held_units: [],
	plants: ['1101#S']
};

describe('dynamics events', () => {
	it('clears after 6 cycles at the base frequency', () => {
		expect(defaultClearing(60)).toBeCloseTo(0.1, 12);
		expect(defaultClearing(50)).toBeCloseTo(0.12, 12);
		expect(secondsToCycles(cyclesToSeconds(4.5, 50), 50)).toBeCloseTo(4.5, 12);
		expect(fmtCycles(0.1, 60)).toBe('6 cycles');
		expect(fmtCycles(0.075, 60)).toBe('4.5 cycles');
	});

	it('maps a fault to apply and clear events, a line fault to its breakers', () => {
		const f = busFault(1101, 60);
		expect(toRmsEvents(f)).toEqual([
			{ kind: 'bus_fault', time: 1, bus: 1101, r: 0, x: 1e-4 },
			{ kind: 'clear_fault', time: 1.1, bus: 1101 }
		]);
		const lf = toRmsEvents(lineFault(1101, 1401, '2', 1401, 60));
		expect(lf[0]).toMatchObject({
			kind: 'line_fault',
			location: 0.99,
			permanent: true,
			circuit: '2'
		});
		expect(lf.slice(1).map((e) => (e as { at: number }).at)).toEqual([1101, 1401]);
		expect(
			allRmsEvents([loadStep(2604, -20), generatorTrip(1431, 'N')]).map((e) => e.kind)
		).toEqual(['load_change', 'generator_trip']);
		expect(toRmsEvents(loadStep(2604, 10))[0]).toMatchObject({ scale: 1.1 });
	});

	it('says each event in one sentence', () => {
		expect(sentence(busFault(1101, 60), 60, () => 'CORONADO')).toBe(
			'Three-phase fault at bus 1101 CORONADO at 1.000 s, cleared after 6 cycles'
		);
		expect(sentence(branchTrip(1101, 1401, '1'), 60)).toBe(
			'Trip of branch 1101–1401 circuit 1 at 1.000 s'
		);
		expect(sentence(loadStep(2604, -15), 60)).toBe('Load at bus 2604 -15 % at 1.000 s');
	});

	it('checks events as the engine does', () => {
		const ctx = { tEnd: 20, inspection };
		expect(validate(busFault(1101, 60), ctx)).toBeNull();
		expect(validate(busFault(9, 60), ctx)).toBe('no bus 9');
		expect(validate({ ...busFault(1101, 60), at: 20 }, ctx)).toMatch(/\[0, 20\)/);
		expect(validate({ ...busFault(1101, 60), clearAfter: 0 } as never, ctx)).toMatch(/clear after/);
		expect(validate({ ...busFault(1101, 60), x: -1 } as never, ctx)).toMatch(/r ≥ 0/);
		expect(validate(branchTrip(1401, 1101, '2'), ctx)).toBeNull();
		expect(validate(branchTrip(1101, 1401, '3'), ctx)).toMatch(/no branch/);
		expect(validate(generatorTrip(1431, 'N'), ctx)).toBeNull();
		expect(validate(generatorTrip(1101, 'S'), ctx)).toBeNull();
		expect(validate(loadStep(1101), ctx)).toMatch(/no load/);
		expect(circuitsBetween(inspection.branches, 1401, 1101)).toEqual(['1', '2']);
	});

	it('marks fault, clearing and trips on the timeline in order', () => {
		const m = markers([branchTrip(1101, 1401, '1'), { ...busFault(1101, 60), at: 0.5 }], 60);
		expect(m.map((x) => [x.t, x.kind])).toEqual([
			[0.5, 'fault'],
			[0.6, 'clear'],
			[1, 'trip']
		]);
	});
});

const screening = (over: Partial<RmsScreening> = {}): RmsScreening =>
	({
		criteria_name: 'tellegen-rms-screening-1',
		criteria: {
			version: 1,
			pole_slip_deg: 180,
			settle_window_s: 2,
			settle_frequency_hz: 0.01,
			settle_angle_deg: 1,
			voltage_recovery_pu: 0.8,
			voltage_recovery_s: 1,
			rocof_window_s: 0.5,
			runaway_pu: 0.05,
			bus_frequency_filter_s: 0.02,
			bus_frequency_min_pu: 0.2
		},
		verdict: 'stable',
		reasons: [],
		frequency: {
			nominal_hz: 60,
			coi_nadir_hz: 59.842,
			nadir_t: 4.21,
			coi_peak_hz: 60,
			peak_t: 0,
			max_rocof_hz_s: 0.31,
			rocof_t: 1.2,
			final_hz: 59.93,
			settled: true,
			lowest_bus_hz: 59.8,
			lowest_bus: 1431,
			lowest_bus_t: 2,
			lowest_unit_hz: 59.81,
			lowest_unit: '1431#G',
			lowest_unit_t: 2
		},
		angle: {
			max_separation_deg: 63.2,
			pair: ['1431#N', '6533#C'],
			separation_t: 1.3,
			max_coi_angle_deg: 40,
			max_coi_angle_t: 1.3,
			first_swing_deg: 20,
			first_swing_unit: '1431#N',
			pole_slips: [],
			trend: 'decaying',
			settled: true
		},
		voltage: {
			min_pu: 0.41,
			min_bus: 1101,
			min_t: 1.05,
			last_event_t: 1.1,
			recovered_s: 0.18,
			recovered: true,
			final_min_pu: 0.96,
			final_max_pu: 1.1
		},
		islands: [{ buses: 243, machines: 103, inertia_mws: 1, final_hz: 59.93, runaway: false }],
		held_units: 0,
		...over
	}) as RmsScreening;

describe('dynamics verdict wording', () => {
	it('leads with an icon and a word', () => {
		expect(headline(screening())).toMatchObject({ icon: '✓', word: 'Stable', tone: 'ok' });
		expect(headline(screening({ verdict: 'loss_of_synchronism' })).word).toBe(
			'Unstable: loss of synchronism'
		);
		expect(headline(screening({ verdict: 'island_runaway' })).word).toBe(
			'Unstable: island frequency runs away'
		);
		expect(headline(screening({ verdict: 'not_settled' }), null, 20).word).toBe(
			'Inconclusive: not settled by 20 s'
		);
		expect(
			headline(null, {
				message: 'integration failed at t = 3.4200 s: x',
				at: failureTime('integration failed at t = 3.4200 s: x')
			}).word
		).toBe('Run failed at t = 3.42 s');
		const s = screening();
		expect(headline({ ...s, voltage: { ...s.voltage, recovered: false } })).toMatchObject({
			tone: 'warning'
		});
	});

	it('states every row with its criterion', () => {
		const r = rows(screening());
		expect(r.map((x) => x.metric)).toEqual([
			'Pole slip',
			'First swing',
			'Max angle separation',
			'Frequency nadir',
			'RoCoF',
			'Settling',
			'Voltage recovery'
		]);
		expect(r[3].value).toBe('59.842 Hz at 4.21 s (COI)');
		// The value compared with the ±180° criterion is |δ − δ_COI|, not the
		// first swing.
		expect(r[0].status).toBe('✓');
		expect(r[0].value).toContain('40.0°');
		expect(r[1].status).toBe('–');
		expect(announcement(screening())).toBe(
			'Stable. COI frequency nadir 59.842 Hz at 4.21 s; lowest voltage 0.41 pu.'
		);
		// A 5 mHz dip is not announced as 60.00 Hz.
		const s = screening();
		expect(announcement({ ...s, frequency: { ...s.frequency, coi_nadir_hz: 59.995 } })).toContain(
			'59.995 Hz'
		);
	});

	it('words the separation against its start and the trend in plain words', () => {
		const s = screening();
		const sep = (angle: Partial<RmsScreening['angle']>, verdict = s.verdict) =>
			rows({ ...s, verdict, angle: { ...s.angle, ...angle } })[2].value;
		expect(sep({ initial_separation_deg: 86.1 })).toContain('86.1° before the event');
		expect(sep({ trend: 'undetermined' })).toContain('too few large swing peaks');
		expect(sep({ trend: 'damped_out' })).toContain('damped out');
		expect(sep({ trend: 'growing' })).toContain('extend the run to confirm');
		expect(rows(s)[5].criterion).toContain('of its final value');
	});
});

describe('notes and decimation', () => {
	it('groups notes and keeps every one verbatim', () => {
		const notes = [
			'dyr: 18 record(s) name out-of-service generators and are not simulated',
			'generator bus 3731 starts at its reactive limit (Q = -121.00 Mvar)',
			'2 generator(s) with only unsupported dynamic models are held as constant-current injections at their power flow output',
			'power flow converged in 3 iterations'
		];
		const g = groupNotes(notes);
		expect(g.flatMap((x) => x.notes).sort()).toEqual([...notes].sort());
		expect(g.find((x) => x.id === 'other')!.notes).toEqual([
			'power flow converged in 3 iterations'
		]);
	});

	it('M4 keeps the extrema and the ends', () => {
		const n = 10000;
		const x = Float64Array.from({ length: n }, (_, i) => i / 1000);
		const y = Float64Array.from(
			{ length: n },
			(_, i) => Math.sin(i / 37) + (i === 4321 ? -5 : 0) + (i === 777 ? 3 : 0)
		);
		const idx = m4(x, y, 100, 0, x[n - 1]);
		expect(idx.length).toBeLessThanOrEqual(400);
		expect(idx).toContain(0);
		expect(idx).toContain(n - 1);
		expect(idx).toContain(4321);
		expect(idx).toContain(777);
		expect(idx).toEqual([...idx].sort((a, b) => a - b));
	});
});

describe('playback', () => {
	it('interpolates between samples', () => {
		const t = [0, 0.01, 0.02];
		expect(sampleAt(t, 0.015)).toEqual({ i: 1, f: expect.closeTo(0.5, 10) });
		expect(valueAt(t, [1, 2, 4], 0.015)).toBeCloseTo(3, 10);
		expect(valueAt(t, [1, 2, 4], 5)).toBe(4);
	});

	it('slows around events and respects reduced motion', () => {
		expect(rateAt(1.05, 4, [1], true)).toBe(0.25);
		expect(rateAt(3, 4, [1], true)).toBe(4);
		expect(rateAt(1.05, 4, [1], false)).toBe(4);
		expect(rateAt(0.95, 0.1, [1], true)).toBe(0.1);
		expect(advance(0, 1, 1, [], true, 20)).toBeCloseTo(0.25, 9); // one call caps at 0.25 s of wall time
		expect(advance(19.99, 0.25, 4, [], false, 20)).toBe(20);
		expect(startTime([1, 2], 60, false)).toEqual({ t: 0.5, play: true });
		expect(startTime([1], 60, true)).toEqual({ t: 1 + 1 / 60, play: false });
	});
});

describe('derived series', () => {
	const time = Float64Array.from([0, 1]);
	const data: Record<string, Float64Array> = {
		'pl:1': Float64Array.from([0.5, 0.4]),
		'pl:2': Float64Array.from([1, 1.2]),
		'vm:1': Float64Array.from([1, 0.5]),
		'vm:2': Float64Array.from([0.9, 0.95]),
		'delta:7': Float64Array.from([0.1, Math.PI / 2]),
		'coi:delta': Float64Array.from([0.1, 0])
	};
	const run = { time, keys: Object.keys(data), series: (k: string) => data[k] };
	it('sums loads and takes envelopes and COI-relative angles', () => {
		expect(Array.from(sumPrefix(run, 'pl:', 100))).toEqual([150, 160]);
		const e = envelope(run, 'vm:');
		expect(Array.from(e.lo)).toEqual([0.9, 0.5]);
		expect(Array.from(e.hi)).toEqual([1, 0.95]);
		expect(Array.from(coiRelativeDeg(run, '7')!)).toEqual([0, 90]);
	});
});

describe('dynamics voltage colours', () => {
	it('darken monotonically away from the neutral band', () => {
		for (const theme of ['light', 'dark'] as const) {
			const low = [0.5, 0.6, 0.7, 0.8, 0.85, 0.9, 0.95, 0.97].map((v) =>
				luminance(dynVoltageColor(v, theme))
			);
			for (let i = 1; i < low.length; i++) expect(low[i]).toBeGreaterThanOrEqual(low[i - 1]);
			const high = [1.03, 1.05, 1.08, 1.1, 1.15, 1.2].map((v) =>
				luminance(dynVoltageColor(v, theme))
			);
			for (let i = 1; i < high.length; i++) expect(high[i]).toBeLessThanOrEqual(high[i - 1]);
		}
		expect(dynVoltageColor(1.0)).toEqual(DYN_V_STOPS[4].light);
	});
});

describe('validated scenarios', () => {
	it('claims validation only when everything matches the study', () => {
		const p = WECC240_PRESETS[1];
		const events = p.events();
		const base = {
			presetId: p.id,
			presetEvents: events,
			events,
			options: { ...VALIDATION_OPTIONS },
			dt: 0.005,
			tEnd: 20,
			validationNetwork: true
		};
		expect(validationBadge(base)?.validated).toBe(true);
		const differs = validationBadge({
			...base,
			options: { ...VALIDATION_OPTIONS, load_model: 'power_flow' },
			dt: 0.0025
		});
		expect(differs).toEqual({
			validated: false,
			text: 'Scenario of the validation study; settings differ: load model, step'
		});
		expect(validationBadge({ ...base, presetId: null })).toBeNull();
	});
});

describe('experimental preference', () => {
	it('is validated and saved with the other preferences', () => {
		const stored: Record<string, string> = {};
		const p = new UserPreferences();
		p.initialize({
			getItem: (k: string) => stored[k] ?? null,
			setItem: (k: string, v: string) => (stored[k] = v)
		} as unknown as Storage);
		expect(p.experimental).toBe(false);
		expect(() => p.update({ experimental: 'yes' })).toThrow('Invalid display');
		p.update({ experimental: true });
		const q = new UserPreferences();
		q.initialize({
			getItem: (k: string) => stored[k] ?? null,
			setItem: () => {}
		} as unknown as Storage);
		expect(q.experimental).toBe(true);
	});
});

import { agentSummary, AgentInputError, resolveAgentEvents } from '../src/lib/dynamics/agent.js';

describe('agent ids', () => {
	const ins = {
		...inspection,
		base_mva: 100,
		base_frequency: 60,
		sources: [],
		dyr_records: 1,
		eigenvalues: [],
		notes: [],
		supported_models: []
	} as never;
	it('names the parallel circuits of an ambiguous branch', () => {
		expect(() =>
			resolveAgentEvents(ins, [{ kind: 'branch_trip', branch: '1101-1401', atS: 1 }])
		).toThrow(/circuits 1 and 2/);
		const [e] = resolveAgentEvents(ins, [{ kind: 'branch_trip', branch: '1401-1101#2', atS: 1 }]);
		expect(e).toMatchObject({ kind: 'branch_trip', circuit: '2', at: 1 });
	});
	it('names the units at a bus', () => {
		expect(() =>
			resolveAgentEvents(ins, [{ kind: 'generator_trip', unit: '1431#X', atS: 1 }])
		).toThrow(/bus 1431 has units 1431#N/);
		expect(() =>
			resolveAgentEvents(ins, [{ kind: 'load_change', bus: 1101, atS: 1, scale: 1.1 }])
		).toThrow(AgentInputError);
	});
	it('fits the summary in the output budget', () => {
		const n = 2001;
		const time = Float64Array.from({ length: n }, (_, i) => i * 0.01);
		const coi = Float64Array.from({ length: n }, (_, i) => 60 - 0.1 * Math.sin(i / 100));
		const run = {
			time,
			keys: ['coi:f'],
			series: (k: string) => (k === 'coi:f' ? coi : undefined),
			screening: screening(),
			notes: ['x'.repeat(500)],
			steps: 8000,
			meta: { base_mva: 100 }
		} as never;
		const out = agentSummary(run, {
			channels: ['coi:f'],
			tracePoints: 40,
			wallMs: 1340,
			loadModel: 'power_flow',
			dtS: 0.0025
		});
		expect(
			JSON.stringify({ ok: true, data: { case_id: 'local-1', revision: 'abc:r12', ...out } }).length
		).toBeLessThan(1450);
		expect((out.traces as Record<string, number[]>)['coi:f']).toHaveLength(40);
	});
});
