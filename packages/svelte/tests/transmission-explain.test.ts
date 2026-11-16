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
	it('shows a single clear count when nothing is near or over a limit', () => {
		const lead = operatingLead(base);
		expect(lead.stats).toEqual([{ label: 'Over limit', value: '0', tone: 'ok' }]);
	});
	it('counts the lines and buses that need attention, and leaves out zeros', () => {
		const lead = operatingLead({ ...base, counts: count(new Map([[1, 0.8]])) });
		expect(stat(lead, 'Low voltage')).toBe('1');
		expect(stat(lead, 'High voltage')).toBeUndefined();
		expect(stat(lead, 'Overloaded')).toBeUndefined();
	});
	it('does not count voltage violations for DC, which solves no voltage', () => {
		const lead = operatingLead({
			...base,
			voltageSolved: false,
			counts: count(new Map([[1, 0.8]]))
		});
		expect(stat(lead, 'Low voltage')).toBeUndefined();
	});
	it('keeps missing flows and unknown ratings apart in the counts', () => {
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
		expect(stat(lead, 'No data')).toBe('1');
		expect(stat(lead, 'Unrated')).toBe('1');
	});
	it('does not call zero assessed lines healthy', () => {
		const lead = operatingLead({ ...base, counts: count(new Map(), { ...result, flows: [] }) });
		expect(lead.stats[0]).toEqual({ label: 'Over limit', value: '—' });
		expect(stat(lead, 'No data')).toBe('3');
	});
	it('separates warnings and at-rating lines from overloads', () => {
		const lead = operatingLead({
			...base,
			counts: { ...base.counts, warning: 1, atLimit: 1 },
			warning: 80
		});
		expect(stat(lead, '≥80%')).toBe('1');
		expect(stat(lead, 'At rating')).toBe('1');
		expect(stat(lead, 'Overloaded')).toBeUndefined();
		expect(operatingLead({ ...base, hasResult: false })).toEqual({ stats: [], empty: 'No result' });
	});
});

describe('prices', () => {
	const econ = {
		opf: true,
		range: { lo: 5, hi: 40 },
		uniform: null,
		binding: 0,
		thermalAvailable: true
	};
	it('shows the price range and the lines at their limit', () => {
		const lead = economicsLead({ ...econ, binding: 2 });
		expect(lead.stats.map((s) => s.label)).toEqual(['Price', 'Lines at limit']);
		expect(lead.stats[1].value).toBe('2');
	});
	it('does not count lines at their limit without thermal data', () => {
		const lead = economicsLead({ ...econ, thermalAvailable: false });
		expect(lead.stats.map((s) => s.label)).toEqual(['Price']);
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
			{ label: 'No data', value: '3' }
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
		expect(lead.stats[1]).toEqual({ label: 'No data', value: '2' });
	});
});

it('links to the appropriate documentation section', () => {
	expect(docsLink('https://example.org/docs', 'operating')).toBe(
		'https://example.org/docs/transmission-diagrams.html#results'
	);
});
