import { describe, expect, it } from 'vitest';
import {
	docsLink,
	economicsLead,
	operatingLead,
	transferLead,
	TERMS
} from '../src/lib/diagram/transmission-explain.js';
import { operatingExceptions } from '../src/lib/diagram/transmission-inspection.js';
import { assessLoading, operatingContext } from '../src/lib/diagram/transmission-results.js';
import type { Network, Solution } from '@tellegen/engine';

const network: Network = {
	id: 'coverage',
	name: 'Coverage fixture',
	base_mva: 100,
	synthetic_coords: false,
	buses: [1, 2, 3, 4].map((id) => ({
		id,
		lon: 0,
		lat: 0,
		demand_mw: 0,
		gen_mw: 0,
		vmin_pu: 0.95,
		vmax_pu: 1.05
	})),
	branches: [1, 2, 3].map((id) => ({
		id,
		from: id,
		to: id + 1,
		status: 1,
		rate_mw: 100,
		path: []
	}))
};
const result: Solution = {
	objective: 0,
	va: [],
	w: [],
	prices: [],
	dispatch: [],
	flows: network.branches.map((b) => ({
		branch: b.id,
		mw: 10,
		loading: 0.1,
		loading_info: { basis: 'mw', rating: 100, rating_source: 'stated' }
	}))
};
const count = (
	voltages = new Map(network.buses.map((b) => [b.id, 1])),
	solution = result,
	net = network
) =>
	operatingExceptions(
		net,
		voltages,
		new Set(),
		net.branches.map((b) =>
			assessLoading(
				solution.flows.find((f) => f.branch === b.id),
				!!b.status
			)
		),
		new Map(solution.flows.map((f) => [f.branch, f]))
	).counts;
const base = {
	hasResult: true,
	voltageSolved: true,
	counts: count(),
	warning: 90
};

describe('operating coverage', () => {
	it('reports checked equipment and only gives an all-clear tone for complete data', () => {
		const lead = operatingLead(base);
		expect(lead.tone).toBe('ok');
		expect(lead.text).toContain('3 checked lines');
		expect(lead.text).toContain('4 checked buses');
		expect(operatingLead({ ...base, voltageSolved: false }).why).toContain(
			'DC power flow treats every voltage as 1 pu'
		);
	});
	it('does not turn missing voltage, absent bounds or invalid bounds into an all-clear', () => {
		const net = {
			...network,
			buses: network.buses.map((b, i) =>
				i === 1
					? { ...b, vmin_pu: undefined, vmax_pu: undefined }
					: i === 2
						? { ...b, vmin_pu: 1.1, vmax_pu: 0.9 }
						: b
			)
		};
		const counts = count(
			new Map([
				[1, 1],
				[2, 1],
				[3, 1]
			]),
			result,
			net
		);
		expect(counts).toMatchObject({ voltageChecked: 1, voltageUnknown: 3 });
		const lead = operatingLead({ ...base, counts });
		expect(lead.tone).toBe('neutral');
		expect(lead.text).toContain('1 checked bus');
		expect(lead.text).toContain('3 buses could not be assessed');
		expect(lead.text).not.toContain('Every bus');
	});
	it('distinguishes missing flows from unknown rating metadata', () => {
		const partial = {
			...result,
			flows: [result.flows[0], { ...result.flows[1], loading_info: undefined }]
		};
		const counts = count(new Map(), partial);
		expect(counts).toMatchObject({
			loadingChecked: 1,
			loadingUnknown: 2,
			flowMissing: 1,
			ratingUnknown: 1
		});
		const lead = operatingLead({ ...base, counts });
		expect(lead.text).toContain('1 line has no usable flow value');
		expect(lead.text).toContain('1 line has no reliable rating metadata');
		expect(lead.tone).toBe('neutral');
	});
	it('retains violations even when other values cannot be assessed', () => {
		const lead = operatingLead({ ...base, counts: count(new Map([[1, 0.8]])) });
		expect(lead.tone).toBe('violation');
		expect(lead.text).toContain('1 bus is outside its voltage limits');
		expect(lead.text).toContain('3 buses could not be assessed');
	});
	it('does not claim that zero assessed lines are healthy', () => {
		const lead = operatingLead({
			...base,
			counts: count(new Map(), { ...result, flows: [] })
		});
		expect(lead.text).toContain('No line loading could be assessed');
		expect(lead.text).toContain('No bus voltage could be assessed');
		expect(lead.tone).toBe('neutral');
	});
	it('distinguishes warnings and at-rating operation from overload', () => {
		const lead = operatingLead({
			...base,
			counts: { ...base.counts, warning: 1, atLimit: 1 },
			warning: 80
		});
		expect(lead.tone).toBe('warning');
		expect(lead.text).toContain('No checked line is over its rating, but 2 run at 80%');
		expect(operatingLead({ ...base, hasResult: false }).text).toBe('No result yet.');
	});
});

describe('economic explanations', () => {
	const econ = {
		opf: true,
		ac: false,
		range: { lo: 5, hi: 40 },
		uniform: null,
		binding: 0,
		thermalAvailable: true
	};
	for (const ac of [false, true])
		for (const uniform of [null, 6.71]) {
			it(`does not invent thermal status with missing metadata (AC=${ac}, uniform=${uniform})`, () => {
				const lead = economicsLead({
					...econ,
					ac,
					uniform,
					thermalAvailable: false
				});
				expect(lead.why).toContain('Thermal binding information is unavailable');
				expect(lead.why).not.toContain('No thermal limit binds');
				expect(lead.why).not.toContain('No reported thermal limit binds');
			});
		}
	it('does not infer positive or zero relief from binding status or price uniformity', () => {
		for (const uniform of [null, 6.71]) {
			const lead = economicsLead({ ...econ, uniform, binding: 1 });
			expect(lead.why).toContain('1 reported line limit binds');
			expect(lead.why).toContain('not necessarily that relaxing it reduces cost');
			expect(lead.why).not.toContain('relaxing it would not lower');
		}
		expect(TERMS.binding).toContain('zero marginal value');
		expect(TERMS.relief).toContain('additional MW of rating');
	});
	it('keeps possible causes separate from reported facts', () => {
		expect(economicsLead({ ...econ, ac: true }).why).toContain(
			'AC losses, voltage limits and other constraints can'
		);
		expect(economicsLead({ ...econ, uniform: 6.71 }).text).toContain(
			'Reported bus prices are uniform'
		);
		expect(economicsLead({ ...econ, opf: false }).why).toContain('Choose DC OPF, AC OPF or SOCWR');
	});
});

describe('transfer availability', () => {
	const transfer = {
		hasResult: true,
		awaiting: false,
		zero: 0,
		missing: 0,
		top: null,
		scale: 500
	};
	it('distinguishes missing, zero and absent energized branches', () => {
		expect(transferLead({ ...transfer, missing: 3 }).text).toContain(
			'unavailable for 3 in-service lines'
		);
		expect(transferLead({ ...transfer, missing: 3 }).why).toContain('do not mean zero');
		expect(transferLead({ ...transfer, zero: 3 }).text).toContain(
			'3 checked lines carry zero active power'
		);
		expect(transferLead(transfer).text).toBe('No energized in-service lines to assess.');
	});
	it('waits for the chosen formulation rather than describing a retained result as zero', () => {
		const context = operatingContext('acpf', { ...result, formulation: 'dcopf' }, true, false);
		const lead = transferLead({
			...transfer,
			hasResult: !!context.current,
			awaiting: context.solved && !context.current
		});
		expect(lead.text).toBe('Awaiting a result for this calculation.');
		expect(transferLead({ ...transfer, hasResult: false }).text).toBe('No result yet.');
	});
	it('qualifies a largest flow when other flows are missing', () => {
		const lead = transferLead({
			...transfer,
			missing: 2,
			top: { id: 7, mw: '555 MW', direction: 'Bus 189 → bus 187' }
		});
		expect(lead.text).toContain('Among reported flows');
		expect(lead.text).toContain('2 lines have no usable flow value');
		expect(lead.why).toContain('up to 500 MW');
	});
});

it('links to the appropriate documentation section', () => {
	expect(docsLink('https://example.org/docs', 'operating')).toBe(
		'https://example.org/docs/transmission-diagrams.html#results'
	);
});
