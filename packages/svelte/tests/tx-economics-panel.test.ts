import { render } from 'svelte/server';
import { describe, expect, it, vi } from 'vitest';
import type { Solution } from '@tellegen/engine';
import TxEconomicsPanel from '../src/lib/components/schematic/TxEconomicsPanel.svelte';
import { economicsFor } from '../src/lib/diagram/transmission-economics.js';
import { operatingContext } from '../src/lib/diagram/transmission-results.js';

vi.mock('../src/lib/context.svelte.js', () => ({
	getUiConfig: () => ({ docsHref: '/docs/' })
}));

const solution = (constraints?: Solution['branch_constraints']): Solution => ({
	formulation: 'dcopf',
	objective: 100,
	va: [],
	w: [],
	prices: [
		{ bus: 1, value: 10 },
		{ bus: 2, value: 20 }
	],
	dispatch: [],
	flows: [{ branch: 1, mw: 50, loading: 0.5 }],
	...(constraints ? { branch_constraints: constraints } : {})
});

const panel = (result: Solution) =>
	render(TxEconomicsPanel, {
		props: {
			formulation: 'dcopf',
			context: operatingContext('dcopf', result, false, false),
			economics: economicsFor(result, 'dcopf'),
			scale: -1,
			onscale: () => {},
			domain: { lo: 10, hi: 20 },
			onbranch: () => {}
		}
	}).body;

describe('Prices panel line limits', () => {
	it('leaves out the list of lines at limit when the result has no line-limit data', () => {
		const body = panel(solution());
		expect(body).not.toContain('Lines at limit');
		expect(body).not.toContain('None');
	});

	it('lists the lines at limit, and says None for an empty list, when the data is there', () => {
		expect(panel(solution([]))).toContain('Lines at limit (0)');
		const body = panel(solution([{ branch: 1, enforced: true, binding: true, relief_per_mw: 4 }]));
		expect(body).toContain('Lines at limit (1)');
		expect(body).toContain('Line 1');
	});
});
