import { describe, expect, it } from 'vitest';
import type { DistGraphBus, McPfResult } from '@tellegen/engine';
import {
	busLabel,
	busLimits,
	exceedance,
	GROUND,
	magnitudeUnbalance,
	rankIssues,
	graphInputs,
	sequenceUnbalance,
	snapshotBusesFromTerminals,
	snapshotFromMcPf,
	snapshotSummary,
	tableModel,
	zoneSummary,
	zoneVoltage,
	type BranchSnapshot,
	type PairVoltage,
	type ResultSnapshot
} from '../src/lib/diagram/index.js';

const polar = (m: number, deg: number) => ({
	re: m * Math.cos((deg * Math.PI) / 180),
	im: m * Math.sin((deg * Math.PI) / 180)
});
const add = (a: { re: number; im: number }, b: { re: number; im: number }) => ({
	re: a.re + b.re,
	im: a.im + b.im
});
const pn = (
	t: string,
	volts: number,
	min: number | null = 207,
	max: number | null = 253
): PairVoltage => ({
	pair: [t, 'n'],
	zone: 'phase–neutral',
	volts,
	min,
	max
});
const pp = (a: string, b: string, volts: number): PairVoltage => ({
	pair: [a, b],
	zone: 'phase–phase',
	volts,
	min: 19000,
	max: 21000
});

/** A two-terminal branch snapshot whose loss is the sum of its ends. */
function branch(ends: BranchSnapshot['ends'], loading: number | null): BranchSnapshot {
	const [a, b] = ends;
	return {
		ends,
		loading,
		basis: null,
		primary: true,
		loss: a && b ? { p: a.p + b.p, q: a.q + b.q } : null
	};
}

function snapshot(): ResultSnapshot {
	return {
		kind: 'solved',
		label: 'test',
		iterations: 3,
		buses: [
			{ pairs: [pp('1', '2', 20400), pp('2', '3', 20500), pp('3', '1', 20450)], unbalance: null },
			{ pairs: [pn('1', 230), pn('2', 232), pn('3', 205)], unbalance: null },
			{ pairs: [pn('1', 255), pn('2', 240), pn('3', 240)], unbalance: null },
			{ pairs: [], unbalance: null }
		],
		branches: [
			branch(
				[
					{ p: 100, q: 10, currents: [] },
					{ p: -98, q: -9, currents: [] }
				],
				95
			),
			branch(
				[
					{ p: 50, q: 5, currents: [] },
					{ p: -49.5, q: -4.8, currents: [] }
				],
				120
			),
			branch([null, null], null),
			branch(
				[
					{ p: 10, q: 1, currents: [] },
					{ p: -10, q: -1, currents: [] }
				],
				101
			)
		]
	};
}

describe('sequenceUnbalance', () => {
	it('is zero for a balanced positive-sequence set and recovers an injected negative sequence', () => {
		const balanced = [polar(230, 0), polar(230, -120), polar(230, 120)] as const;
		expect(sequenceUnbalance(...balanced)).toBeCloseTo(0, 10);
		const negative = [polar(4.6, 0), polar(4.6, 120), polar(4.6, -120)];
		const [a, b, c] = balanced.map((v, i) => add(v, negative[i]));
		expect(sequenceUnbalance(a, b, c)).toBeCloseTo(2, 10);
	});

	it('returns null without a positive sequence', () => {
		const zero = { re: 0, im: 0 };
		expect(sequenceUnbalance(zero, zero, zero)).toBeNull();
	});
});

describe('snapshotFromMcPf', () => {
	const terminal = (bus: string, t: string, v: { re: number; im: number }) => ({
		bus,
		terminal: t,
		voltage: v,
		current_into_network: { re: 0, im: 0 },
		power_into_network: { re: 0, im: 0 }
	});
	const port = (element: string, bus: string, t: string, amps: number, kw: number) => ({
		element,
		kind: 'line',
		branch: 0,
		bus,
		terminal: t,
		current_into_element: { re: amps, im: 0 },
		power_into_element: { re: kw * 1000, im: kw * 100 }
	});
	const result = {
		iterations: 4,
		terminals: [
			terminal('A', '1', polar(20000 / Math.sqrt(3), 0)),
			terminal('A', '2', polar(20000 / Math.sqrt(3), -120)),
			terminal('A', '3', polar(20000 / Math.sqrt(3), 120)),
			terminal('B', '1', polar(231, 0)),
			terminal('B', '2', polar(229, -120)),
			terminal('B', '3', polar(205, 120)),
			terminal('B', 'n', polar(1, 0))
		],
		element_ports: [
			port('L1', 'A', '1', 90, 10),
			port('L1', 'A', '2', 110, 11),
			port('L1', 'A', '3', 176, 12),
			port('L1', 'B', '1', 90, -10),
			port('L1', 'B', '2', 110, -10.5),
			port('L1', 'B', '3', 176, -11.4)
		]
	} as unknown as McPfResult;
	const buses = [
		{
			id: 'A',
			terminals: ['1', '2', '3'],
			limits: { phasePhase: [{ pair: null, min: 19000, max: 21000 }] }
		},
		{
			id: 'B',
			terminals: ['1', '2', '3', 'n'],
			limits: { phaseNeutral: { min: [207, 207, 207], max: [253, 253, 253] } }
		}
	];
	const branch = {
		id: 'L1',
		from: 'A',
		to: 'B',
		fromTerminals: ['1', '2', '3'],
		toTerminals: ['1', '2', '3'],
		inService: true,
		rating: { kind: 'current' as const, amps: [200, 200, 200] }
	};

	it('derives declared pair voltages, phasor unbalance and signed end totals', () => {
		const s = snapshotFromMcPf(result, buses, [branch]);
		expect(s.iterations).toBe(4);
		expect(s.buses[0].pairs.map((p) => p.pair.join('-'))).toEqual(['1-2', '2-3', '3-1']);
		expect(s.buses[0].pairs[0].volts).toBeCloseTo(20000, 6);
		expect(s.buses[0].unbalance?.percent).toBeCloseTo(0, 8);
		expect(s.buses[1].pairs.map((p) => p.pair)).toEqual([
			['1', 'n'],
			['2', 'n'],
			['3', 'n']
		]);
		expect(s.buses[1].pairs[2].min).toBe(207);
		expect(s.buses[1].unbalance?.definition).toBe('VUF');
		expect(s.buses[1].unbalance?.percent).toBeGreaterThan(2);
		const [from, to] = s.branches[0].ends;
		expect(from?.p).toBeCloseTo(33, 10);
		expect(to?.p).toBeCloseTo(-31.9, 10);
		expect(from?.currents.map((c) => c.amps)).toEqual([90, 110, 176]);
		expect(s.branches[0].loading).toBeCloseTo(88, 10);
	});

	it('leaves ends null for missing ports and out-of-service branches, and loading unknown', () => {
		const s = snapshotFromMcPf(result, buses, [
			{ ...branch, inService: false },
			{ ...branch, id: 'L2' }
		]);
		expect(s.branches[0].ends).toEqual([null, null]);
		expect(s.branches[1].ends).toEqual([null, null]);
		expect(s.branches[1].loading).toBeNull();
	});

	it('keeps undeclared limits as null rather than inventing a band', () => {
		const s = snapshotFromMcPf(result, [{ id: 'B', terminals: ['1', '2', '3', 'n'] }], []);
		expect(s.buses[0].pairs.every((p) => p.min === null && p.max === null)).toBe(true);
		expect(rankIssues(s)).toEqual([]);
	});
});

describe('issues and summary', () => {
	it('exceedance is relative to the violated limit', () => {
		expect(exceedance(pn('1', 205))).toBeCloseTo((100 * 2) / 207, 12);
		expect(exceedance(pn('1', 230))).toBeLessThan(0);
		expect(exceedance(pn('1', 230, null, null))).toBe(Number.NEGATIVE_INFINITY);
	});

	it('ranks violations before warnings, then by exceedance, with deterministic ties', () => {
		const issues = rankIssues(snapshot());
		expect(issues.map((i) => [i.kind, i.severity, i.target.index])).toEqual([
			['loading', 'violation', 1],
			['loading', 'violation', 3],
			['voltage', 'violation', 1],
			['voltage', 'violation', 2],
			['loading', 'warning', 0]
		]);
		const low = issues.find((i) => i.kind === 'voltage' && i.target.index === 1);
		expect([low?.low, low?.limit, low?.pair]).toEqual([true, 207, ['3', 'n']]);
	});

	it('summarises losses over active branches, per-zone extremes and the worst unbalance', () => {
		const s = snapshotSummary(snapshot());
		expect(s.loss.p).toBeCloseTo(2.5, 12);
		expect(s.loss.q).toBeCloseTo(1.2, 12);
		expect(s.loss.branches).toBe(3);
		expect(s.loading).toEqual({ branch: 1, value: 120 });
		expect(s.counts).toEqual({ voltage: 2, overload: 2, warning: 1 });
		const lv = s.zones.find((z) => z.zone === 'phase–neutral');
		expect([lv?.min.bus, lv?.min.volts, lv?.min.state, lv?.max.volts, lv?.buses]).toEqual([
			1,
			205,
			'violation',
			255,
			2
		]);
		expect(s.unbalance?.bus).toBe(1);
		expect(s.unbalance?.definition).toBe('IEEE PVUR');
	});

	it('prefers phasor unbalance and labels the magnitude fallbacks by standard', () => {
		const s = snapshot();
		s.buses[0].unbalance = { percent: 50, definition: 'VUF' };
		expect(snapshotSummary(s).unbalance).toEqual({ percent: 50, definition: 'VUF', bus: 0 });
		expect(magnitudeUnbalance(s.buses[0].pairs)?.definition).toBe('NEMA LVUR');
		expect(magnitudeUnbalance([pn('1', 230), pn('2', 230)])).toBeNull();
	});
});

describe('graphInputs and live-session terminals', () => {
	const graph = {
		buses: [
			{
				id: 'A',
				terminals: ['1', '2', '3'],
				grounded: [],
				load_kw: 0,
				gen_kw: 0,
				has_source: true,
				voltage_limits: { vpp_min: [19000, 19000, 19000], vpp_max: [21000, 21000, 21000] }
			},
			{
				id: 'B',
				terminals: ['1', '2', '3', 'n'],
				grounded: ['n'],
				neutral_terminal: 'n',
				load_kw: 5,
				gen_kw: 0,
				has_source: false,
				voltage_limits: { vpn_min: [207, 207, 207], vpn_max: [253, 253, 253] }
			},
			{
				id: 'C',
				terminals: ['1', '2', '3'],
				grounded: [],
				load_kw: 0,
				gen_kw: 0,
				has_source: false,
				voltage_limits: { vpp_min: [1, 2, 3], vpp_max: [9, 9, 9] }
			}
		],
		edges: [
			{
				kind: 'line' as const,
				id: 'L1',
				from: 'A',
				to: 'B',
				conductors: [
					['1', '1'],
					['2', '2'],
					['3', '3']
				] as [string, string][],
				closed: true,
				n_phases: 3,
				rating: { i_max: [200, 200, 200] }
			},
			{
				kind: 'transformer' as const,
				id: 'T1',
				from: 'A',
				to: 'c',
				conductors: [['1', '1']] as [string, string][],
				closed: true,
				n_phases: 3,
				rating: { s_rating_by_bus: { A: 4e5, c: 4e5 } }
			},
			{
				kind: 'switch' as const,
				id: 'S1',
				from: 'B',
				to: 'X',
				conductors: [] as [string, string][],
				closed: false,
				n_phases: 3
			}
		]
	};

	it('maps declared limits and ratings, and refuses to guess non-uniform phase–phase bounds', () => {
		const g = graphInputs(graph);
		expect(g.buses.map((b) => b.limits)).toEqual([
			{ phasePhase: [{ pair: null, min: 19000, max: 21000 }] },
			{ phaseNeutral: { min: [207, 207, 207], max: [253, 253, 253] } },
			null
		]);
		expect(g.branches.map((b) => b.rating)).toEqual([
			{ kind: 'current', amps: [200, 200, 200] },
			{ kind: 'apparent', byBus: { A: 4e5, c: 4e5 } },
			null
		]);
		expect(g.branches[2].inService).toBe(false);
		expect(g.names.branches.map((b) => [b.from, b.to])).toEqual([
			[0, 1],
			[0, 2],
			[1, -1]
		]);
		expect(g.entities.filter((e) => e.kind === 'branch').map((e) => e.family)).toEqual([
			'line',
			'transformer',
			'switch'
		]);
	});

	it('builds bus results from interleaved session terminal arrays', () => {
		const g = graphInputs(graph);
		const ids: [string, string][] = [
			['B', '1'],
			['B', '2'],
			['B', '3'],
			['B', 'n']
		];
		const v = [polar(230, 0), polar(230, -120), polar(205, 120), polar(0, 0)].flatMap((z) => [
			z.re,
			z.im
		]);
		const buses = snapshotBusesFromTerminals(ids, Float64Array.from(v), g.buses);
		expect(buses[0].pairs).toEqual([]);
		expect(buses[1].pairs.map((p) => Math.round(p.volts))).toEqual([230, 230, 205]);
		expect(buses[1].pairs[2].min).toBe(207);
		expect(buses[1].unbalance?.definition).toBe('VUF');
	});

	it('matches ports by element kind when a line and a switch share a name', () => {
		const port = (kind: string, bus: string, kw: number) => ({
			element: 'X1',
			kind,
			branch: 0,
			bus,
			terminal: '1',
			current_into_element: { re: 1, im: 0 },
			power_into_element: { re: kw * 1000, im: 0 }
		});
		const result = {
			iterations: 1,
			terminals: [],
			element_ports: [
				port('line', 'A', 5),
				port('switch', 'A', 99),
				port('line', 'B', -4.9),
				port('switch', 'B', -99)
			]
		} as unknown as McPfResult;
		const branch = {
			id: 'X1',
			kind: 'line',
			from: 'A',
			to: 'B',
			fromTerminals: ['1'],
			toTerminals: ['1'],
			inService: true
		};
		const s = snapshotFromMcPf(result, [], [branch]);
		expect(s.branches[0].ends.map((e) => e?.p)).toEqual([5, -4.9]);
	});
});

describe('equipment that is not a plain two-terminal branch', () => {
	const port = (element: string, kind: string, bus: string, t: string, kw: number) => ({
		element,
		kind,
		branch: 0,
		bus,
		terminal: t,
		current_into_element: { re: 1, im: 0 },
		power_into_element: { re: kw * 1000, im: 0 }
	});

	it('takes the neutral from the bus, whatever it is named', () => {
		const terminal = (t: string, v: { re: number; im: number }) => ({
			bus: 'B',
			terminal: t,
			voltage: v,
			current_into_network: { re: 0, im: 0 },
			power_into_network: { re: 0, im: 0 }
		});
		const result = {
			iterations: 1,
			terminals: [
				terminal('1', polar(230, 0)),
				terminal('2', polar(230, -120)),
				terminal('3', polar(200, 120)),
				terminal('4', polar(0, 0))
			],
			element_ports: []
		} as unknown as McPfResult;
		const bus = {
			id: 'B',
			terminals: ['1', '2', '3', '4'],
			neutral: '4',
			limits: { phaseNeutral: { min: [207, 207, 207], max: [253, 253, 253] } }
		};
		const s = snapshotFromMcPf(result, [bus], []);
		expect(s.buses[0].pairs.map((p) => [p.pair.join('-'), p.zone])).toEqual([
			['1-4', 'phase–neutral'],
			['2-4', 'phase–neutral'],
			['3-4', 'phase–neutral']
		]);
		expect(snapshotSummary(s).zones.map((z) => z.zone)).toEqual(['phase–neutral']);
		expect(magnitudeUnbalance(s.buses[0].pairs)?.definition).toBe('IEEE PVUR');
	});

	it('counts a centre-tap transformer once, and judges its loading per bus', () => {
		// 25 kVA, 20 kW in on the primary and 19.5 kW out across both halves of the secondary.
		const result = {
			iterations: 1,
			terminals: [],
			element_ports: [
				port('T1', 'transformer', 'HV', 'a', 20),
				port('T1', 'transformer', 'HV', 'n', 0),
				port('T1', 'transformer', 'LV', 'a', -10),
				port('T1', 'transformer', 'LV', 'b', -9.5),
				port('T1', 'transformer', 'LV', 'n', 0)
			]
		} as unknown as McPfResult;
		const rating = { kind: 'apparent' as const, byBus: { HV: 25e3, LV: 25e3 } };
		const edge = (from: string, to: string, fromTerminals: string[], toTerminals: string[]) => ({
			id: 'T1',
			kind: 'transformer',
			from,
			to,
			fromTerminals,
			toTerminals,
			inService: true,
			rating
		});
		// PowerIO projects three windings to three winding-pair edges.
		const s = snapshotFromMcPf(
			result,
			[],
			[
				edge('HV', 'LV', ['a', 'n'], ['a', 'n']),
				edge('HV', 'LV', ['a', 'n'], ['n', 'b']),
				edge('LV', 'LV', ['a', 'n'], ['n', 'b'])
			]
		);
		expect(s.branches.map((b) => b.primary)).toEqual([true, false, false]);
		expect(s.branches.map((b) => b.loading)).toEqual([80, 80, 80]);
		const summary = snapshotSummary(s);
		expect(summary.loss.p).toBeCloseTo(0.5, 12);
		expect(summary.loss.branches).toBe(1);
		s.branches.forEach((b) => (b.loading = 95));
		expect(rankIssues(s).map((i) => i.target.index)).toEqual([0]);
	});
});

describe('phase–ground limits', () => {
	const bus = (
		id: string,
		voltage_limits: DistGraphBus['voltage_limits'],
		terminals = ['a', 'b', 'c', 'n']
	): DistGraphBus => ({
		id,
		terminals,
		grounded: [],
		neutral_terminal: terminals.includes('n') ? 'n' : null,
		load_kw: 0,
		gen_kw: 0,
		has_source: false,
		voltage_limits
	});
	const toGround = { v_min_by_terminal: { c: 220, a: 220, b: 220 } };
	const graph = {
		buses: [
			// Phase–neutral and phase–ground both declared; the neutral's cap has no minimum.
			bus('G', {
				vpn_min: [207, 207, 207],
				vpn_max: [253, 253, 253],
				...toGround,
				v_max_by_terminal: { a: 245, b: 245, c: 245, n: 10 }
			}),
			// Phase–ground only, so its phase–neutral voltages are not shown.
			bus('H', { ...toGround, v_max_by_terminal: { a: 245, b: 245, c: 245 } }),
			// Only bounds the overview does not rank.
			bus('K', { vn_max: 10, vneg_max: 5, v_min_by_terminal: { a: 220 } })
		],
		edges: []
	};
	const terminal = (b: string, t: string, v: { re: number; im: number }) => ({
		bus: b,
		terminal: t,
		voltage: v,
		current_into_network: { re: 0, im: 0 },
		power_into_network: { re: 0, im: 0 }
	});
	const phases = (b: string) => [
		terminal(b, 'a', polar(240, 0)),
		terminal(b, 'b', polar(230, -120)),
		terminal(b, 'c', polar(215, 120)),
		terminal(b, 'n', polar(3, 30))
	];
	const result = {
		iterations: 3,
		terminals: [...phases('G'), ...phases('H'), ...phases('K')],
		element_ports: []
	} as unknown as McPfResult;
	const inputs = graphInputs(graph);
	const s = snapshotFromMcPf(result, inputs.buses, inputs.branches);

	it('keeps each terminal band in terminal order, and only with both sides declared', () => {
		expect(inputs.buses.map((b) => b.limits)).toEqual([
			{
				phaseNeutral: { min: [207, 207, 207], max: [253, 253, 253] },
				phaseGround: [
					{ terminal: 'a', min: 220, max: 245 },
					{ terminal: 'b', min: 220, max: 245 },
					{ terminal: 'c', min: 220, max: 245 }
				]
			},
			{
				phaseGround: [
					{ terminal: 'a', min: 220, max: 245 },
					{ terminal: 'b', min: 220, max: 245 },
					{ terminal: 'c', min: 220, max: 245 }
				]
			},
			null
		]);
		// Terminal names are looked up as own keys, never as inherited object properties.
		const named = bus('L', { v_min_by_terminal: {}, v_max_by_terminal: {} }, ['constructor']);
		expect(busLimits(named)).toBeNull();
	});

	it('measures each declared reference, phase–ground as the terminal magnitude', () => {
		expect(s.buses[0].pairs.map((p) => [p.pair.join('–'), p.zone])).toEqual([
			['a–n', 'phase–neutral'],
			['b–n', 'phase–neutral'],
			['c–n', 'phase–neutral'],
			[`a–${GROUND}`, 'phase–ground'],
			[`b–${GROUND}`, 'phase–ground'],
			[`c–${GROUND}`, 'phase–ground']
		]);
		const ground = s.buses[0].pairs.slice(3);
		expect(ground.map((p) => p.volts)).toEqual([240, 230, 215].map((v) => expect.closeTo(v, 9)));
		expect(ground.map((p) => [p.min, p.max])).toEqual([
			[220, 245],
			[220, 245],
			[220, 245]
		]);
		expect(s.buses[1].pairs.map((p) => p.pair)).toEqual([
			['a', GROUND],
			['b', GROUND],
			['c', GROUND]
		]);
		// Without a ranked band the bus falls back to undeclared phase–neutral pairs.
		expect(s.buses[2].pairs.map((p) => [p.zone, p.min])).toEqual([
			['phase–neutral', null],
			['phase–neutral', null],
			['phase–neutral', null]
		]);
	});

	it('takes the unbalance from one reference, the same from any', () => {
		const [a, b, c] = [polar(240, 0), polar(230, -120), polar(215, 120)];
		expect(s.buses[0].unbalance?.percent).toBeCloseTo(sequenceUnbalance(a, b, c) as number, 9);
		expect(s.buses[1].unbalance?.percent).toBeCloseTo(sequenceUnbalance(a, b, c) as number, 9);
		expect(magnitudeUnbalance(s.buses[0].pairs)?.definition).toBe('IEEE PVUR');
		// Mean 685 / 3 V; the 215 V phase deviates most.
		expect(magnitudeUnbalance(s.buses[1].pairs)?.percent).toBeCloseTo(
			(100 * (685 / 3 - 215)) / (685 / 3),
			9
		);
	});

	it('ranks, summarises, labels and tabulates phase–ground violations', () => {
		const issues = rankIssues(s);
		expect(issues.map((i) => [i.target.index, i.pair, i.low, i.limit])).toEqual([
			[0, ['c', GROUND], true, 220],
			[1, ['c', GROUND], true, 220]
		]);
		expect(issues[0].score).toBeCloseTo((100 * 5) / 220, 9);
		const zones = snapshotSummary(s, issues).zones;
		const ground = zones.find((z) => z.zone === 'phase–ground');
		expect([ground?.min.volts, ground?.min.state, ground?.buses]).toEqual([
			expect.closeTo(215, 9),
			'violation',
			2
		]);
		expect(busLabel(s, 1)).toMatchObject({ text: '215 V', state: 'violation' });
		const row = tableModel(s, inputs.names, 'bus').row(0);
		expect([row.reference, row.pairs, row.state]).toEqual([
			'phase–neutral, phase–ground',
			'a–n b–n c–n a–ground b–ground c–ground',
			'violation'
		]);
	});

	it('reads a zone level and its lowest voltage from phase–ground bands', () => {
		const zone = { buses: [1], edges: [], transformers: [], hasSource: false };
		expect(zoneVoltage(zone, graph, null)).toBeCloseTo(((220 + 245) / 2) * Math.sqrt(3), 9);
		expect(zoneSummary(zone, s).minVolts).toBeCloseTo(215, 9);
		// A phase–neutral voltage, where the zone has one, comes first even when a phase–ground one
		// is lower.
		const neutral = s.buses[0].pairs.filter((p) => p.zone === 'phase–neutral');
		expect(zoneSummary({ ...zone, buses: [0, 1] }, s).minVolts).toBe(
			Math.min(...neutral.map((p) => p.volts))
		);
		expect(Math.min(...neutral.map((p) => p.volts))).toBeGreaterThan(215);
	});
});

describe('non-uniform phase–phase limits', () => {
	const bus = (id: string, voltage_limits: DistGraphBus['voltage_limits']): DistGraphBus => ({
		id,
		terminals: ['a', 'b', 'c'],
		grounded: [],
		neutral_terminal: null,
		load_kw: 0,
		gen_kw: 0,
		has_source: false,
		voltage_limits
	});
	const bounds = { vpp_min: [390, 395, 400], vpp_max: [410, 415, 420] };
	const graph = {
		buses: [
			// The engine's order (1,2), (2,3), (3,1) of the phase terminals.
			bus('P', {
				...bounds,
				vpp_pairs: [
					['a', 'b'],
					['b', 'c'],
					['c', 'a']
				]
			}),
			// Pairs are matched by their terminals, in either order, not by position.
			bus('Q', {
				...bounds,
				vpp_pairs: [
					['c', 'b'],
					['a', 'c'],
					['b', 'a']
				]
			}),
			// Without named pairs, or with too few, the non-uniform bounds have no place.
			bus('R', bounds),
			bus('S', { ...bounds, vpp_pairs: [['a', 'b']] })
		],
		edges: []
	};
	const terminal = (b: string, t: string, deg: number) => ({
		bus: b,
		terminal: t,
		voltage: polar(230, deg),
		current_into_network: { re: 0, im: 0 },
		power_into_network: { re: 0, im: 0 }
	});
	const result = {
		iterations: 2,
		terminals: ['P', 'Q', 'R', 'S'].flatMap((b) => [
			terminal(b, 'a', 0),
			terminal(b, 'b', -120),
			terminal(b, 'c', 120)
		]),
		element_ports: []
	} as unknown as McPfResult;
	const inputs = graphInputs(graph);
	const s = snapshotFromMcPf(result, inputs.buses, inputs.branches);
	const line = 230 * Math.sqrt(3);

	it('keeps one band per named pair', () => {
		expect(inputs.buses.map((b) => b.limits)).toEqual([
			{
				phasePhase: [
					{ pair: ['a', 'b'], min: 390, max: 410 },
					{ pair: ['b', 'c'], min: 395, max: 415 },
					{ pair: ['c', 'a'], min: 400, max: 420 }
				]
			},
			{
				phasePhase: [
					{ pair: ['c', 'b'], min: 390, max: 410 },
					{ pair: ['a', 'c'], min: 395, max: 415 },
					{ pair: ['b', 'a'], min: 400, max: 420 }
				]
			},
			null,
			null
		]);
		// A uniform band still applies to every pair without names.
		expect(busLimits(bus('U', { vpp_min: [390, 390, 390], vpp_max: [410, 410, 410] }))).toEqual({
			phasePhase: [{ pair: null, min: 390, max: 410 }]
		});
	});

	it('ranks each pair against its own band', () => {
		expect(s.buses[0].pairs.map((p) => [p.pair.join('–'), p.min, p.max])).toEqual([
			['a–b', 390, 410],
			['b–c', 395, 415],
			['c–a', 400, 420]
		]);
		expect(s.buses[1].pairs.map((p) => [p.pair.join('–'), p.min, p.max])).toEqual([
			['a–b', 400, 420],
			['b–c', 390, 410],
			['c–a', 395, 415]
		]);
		expect(s.buses[2].pairs.every((p) => p.min === null && p.max === null)).toBe(true);
		// A balanced 230 V set is 398.4 V line to line: below only a 400 V minimum.
		expect(rankIssues(s).map((i) => [i.target.index, i.pair, i.low, i.limit])).toEqual([
			[0, ['c', 'a'], true, 400],
			[1, ['a', 'b'], true, 400]
		]);
		expect(s.buses[0].pairs[2].volts).toBeCloseTo(line, 9);
	});
});
