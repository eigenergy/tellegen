import { describe, expect, it } from 'vitest';
import { displayMetaFor, displaySeriesFor } from '../src/lib/display.js';
import type { SolvableCase } from '../src/lib/state.svelte.js';

// A solved local case in the shape the display helpers read: two buses, DC OPF prices and angles.
function solvedCase(deltas: Record<number, number> = {}): SolvableCase {
	return {
		formulation: 'dcopf',
		deltas,
		network: {
			buses: [
				{ id: 1, demand_mw: 120, gen_mw: 0 },
				{ id: 2, demand_mw: 0, gen_mw: 300 }
			],
			branches: []
		},
		solution: {
			prices: [
				{ bus: 1, value: 31 },
				{ bus: 2, value: 29 }
			],
			va: [
				{ bus: 1, value: -0.1 },
				{ bus: 2, value: 0 }
			],
			w: [],
			flows: []
		}
	} as unknown as SolvableCase;
}

describe('demand display', () => {
	it('offers demand after the result variables, so a fallback still lands on a result', () => {
		const modes = displayMetaFor(solvedCase()).map((option) => option.mode);
		expect(modes).toEqual(['price', 'angle', 'demand']);
		expect(displayMetaFor(solvedCase()).find((o) => o.mode === 'demand')?.unit).toBe('MW');
	});

	it('reads bus demand with the committed changes', () => {
		expect(displaySeriesFor(solvedCase(), 'demand')).toEqual([
			{ bus: 1, value: 120 },
			{ bus: 2, value: 0 }
		]);
		expect(displaySeriesFor(solvedCase({ 1: -20, 2: 15 }), 'demand')).toEqual([
			{ bus: 1, value: 100 },
			{ bus: 2, value: 15 }
		]);
	});

	it('has nothing to show before a result', () => {
		const unsolved = { ...solvedCase(), solution: null } as unknown as SolvableCase;
		expect(displayMetaFor(unsolved)).toEqual([]);
		expect(displaySeriesFor(unsolved, 'demand')).toEqual([]);
	});
});

describe('legend gradients', () => {
	it('draws voltage with the theme ramp, like price', () => {
		const c = solvedCase();
		const solved = {
			...c,
			solution: { ...c.solution, vm: [{ bus: 1, value: 1.01 }] }
		} as unknown as SolvableCase;
		const options = displayMetaFor(solved);
		const price = options.find((o) => o.mode === 'price')?.gradient;
		const voltage = options.find((o) => o.mode === 'voltage')?.gradient;
		expect(voltage).toMatch(/^var\(--tellegen-price-ramp, /);
		expect(voltage).toBe(price);
	});
});
