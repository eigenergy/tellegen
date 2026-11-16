import { describe, expect, it } from 'vitest';
import {
	docsLink,
	economicsLead,
	operatingLead,
	transferLead
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

const stat = (lead: ReturnType<typeof operatingLead>, label: string) =>
	lead.stats.find((s) => s.label === label)?.value;

describe('operating limits', () => {
	it('shows one short line, and no counts, when nothing is near or over a limit', () => {
		const lead = operatingLead(base);
		expect(lead).toEqual({ stats: [], empty: 'All lines below 90%' });
	});
	it('counts the lines and buses that need attention, and leaves out zeros', () => {
		const lead = operatingLead({ ...base, counts: count(new Map([[1, 0.8]])) });
		expect(stat(lead, 'Low voltage')).toBe('1');
		expect(stat(lead, 'High voltage')).toBeUndefined();
		expect(stat(lead, 'At limit')).toBeUndefined();
		expect(stat(lead, 'Over limit')).toBeUndefined();
	});
	it('does not count voltage violations for DC, which solves no voltage', () => {
		const lead = operatingLead({
			...base,
			voltageSolved: false,
			counts: count(new Map([[1, 0.8]]))
		});
		expect(stat(lead, 'Low voltage')).toBeUndefined();
		expect(stat(lead, 'Voltage unchecked')).toBeUndefined();
		expect(lead).toEqual({ stats: [], empty: 'All lines below 90%' });
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
		expect(lead.empty).toBeUndefined();
		expect(lead.stats.some((s) => s.tone === 'ok')).toBe(false);
		expect(stat(lead, 'Voltage unchecked')).toBe('3');
	});
	it('retains violations even when other values cannot be assessed', () => {
		const lead = operatingLead({ ...base, counts: count(new Map([[1, 0.8]])) });
		expect(lead.stats[0]).toEqual({ label: 'Low voltage', value: '1', tone: 'failure' });
		expect(stat(lead, 'Voltage unchecked')).toBe('3');
	});
	it('keeps missing flows and unknown ratings apart in the counts', () => {
		const partial = {
			...result,
			flows: [result.flows[0], { ...result.flows[1], loading_info: undefined }]
		};
		const counts = count(new Map(network.buses.map((b) => [b.id, 1])), partial);
		expect(counts).toMatchObject({
			loadingChecked: 1,
			loadingUnknown: 2,
			flowMissing: 1,
			ratingUnknown: 1
		});
		const lead = operatingLead({ ...base, counts });
		expect(stat(lead, 'No flow')).toBe('1');
		expect(stat(lead, 'Unrated')).toBe('1');
		expect(lead.stats.some((s) => s.tone === 'ok')).toBe(false);
	});
	it('does not call zero assessed lines healthy', () => {
		const lead = operatingLead({ ...base, counts: count(new Map(), { ...result, flows: [] }) });
		expect(lead.empty).toBeUndefined();
		expect(lead.stats.some((s) => s.tone === 'ok')).toBe(false);
		expect(stat(lead, 'No flow')).toBe('3');
		const none = { ...base.counts, loadingChecked: 0, loadingUnknown: 0, flowMissing: 0 };
		expect(operatingLead({ ...base, voltageSolved: false, counts: none })).toEqual({
			stats: [],
			empty: 'No line loading'
		});
	});
	it('counts lines over, at and near their rating apart', () => {
		const lead = operatingLead({
			...base,
			counts: { ...base.counts, warning: 1, atLimit: 1, overload: 2 },
			warning: 80
		});
		expect(lead.stats.slice(0, 3)).toEqual([
			{ label: 'Over limit', value: '2', tone: 'failure' },
			{ label: 'At limit', value: '1', tone: 'failure' },
			{ label: '≥80%', value: '1', tone: 'warning' }
		]);
		expect(operatingLead({ ...base, hasResult: false })).toEqual({ stats: [], empty: 'No result' });
	});
	it('counts out-of-service lines only when there are some', () => {
		expect(stat(operatingLead(base), 'Out of service')).toBeUndefined();
		const lead = operatingLead({ ...base, counts: { ...base.counts, inactive: 2 } });
		expect(stat(lead, 'Out of service')).toBe('2');
	});
});

describe('prices', () => {
	const econ = { opf: true, range: { lo: 4.531, hi: 39.3 }, uniform: null };
	it('shows the lowest and highest price to the same precision, with the unit under the value', () => {
		expect(economicsLead(econ).stats).toEqual([
			{ label: 'Lowest', value: '4.53', sub: '$/MWh' },
			{ label: 'Highest', value: '39.30', sub: '$/MWh' }
		]);
		expect(
			economicsLead({ ...econ, range: { lo: 12, hi: 1250 } }).stats.map((s) => s.value)
		).toEqual(['12', '1,250']);
		expect(economicsLead({ ...econ, uniform: 6.71 }).stats).toEqual([
			{ label: 'Price', value: '6.71', sub: '$/MWh' }
		]);
	});
	it('does not repeat the lines at their limit, which the Network panel shows', () => {
		expect(economicsLead(econ).stats.some((s) => s.label === 'Lines at limit')).toBe(false);
	});
	it('says when there are no prices to show', () => {
		expect(economicsLead({ ...econ, opf: false }).empty).toBe('Needs an OPF');
		expect(economicsLead({ ...econ, range: null }).empty).toBe('No prices');
	});
});

describe('flows', () => {
	const transfer = { hasResult: true, zero: 0, missing: 0, top: null };
	it('distinguishes missing, zero and absent energized branches', () => {
		expect(transferLead({ ...transfer, missing: 3 }).stats).toEqual([
			{ label: 'No flow', value: '3' }
		]);
		expect(transferLead({ ...transfer, zero: 3 }).stats).toEqual([
			{ label: 'Zero flow', value: '3' }
		]);
		expect(transferLead(transfer).empty).toBe('No flows');
	});
	it('waits for the chosen formulation rather than describing a retained result as zero', () => {
		const context = operatingContext('acpf', { ...result, formulation: 'dcopf' }, true, false);
		expect(transferLead({ ...transfer, hasResult: !!context.current }).empty).toBe('No result');
	});
	it('names the largest flow and still counts missing ones', () => {
		const lead = transferLead({ ...transfer, missing: 2, top: { id: 7, mw: '555 MW' } });
		expect(lead.stats[0]).toEqual({ label: 'Largest flow', value: '555 MW', sub: 'Line 7' });
		expect(lead.stats[1]).toEqual({ label: 'No flow', value: '2' });
	});
});

it('links to the appropriate documentation section', () => {
	expect(docsLink('https://example.org/docs', 'operating')).toBe(
		'https://example.org/docs/transmission-diagrams.html#results'
	);
});
