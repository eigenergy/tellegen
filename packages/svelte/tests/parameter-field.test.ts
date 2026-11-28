import { describe, expect, it } from 'vitest';
import { BLOCK_PARAMS } from '../src/lib/dynamics/block-params.js';
import { numberError, parseDrafts } from '../src/lib/dynamics/live-logic.js';

describe('the parameter check both editors share', () => {
	it('accepts numbers and names what is wrong with the rest', () => {
		for (const ok of ['1', '-0.5', '.2', '3e-2', ' 4 ']) expect(numberError(ok), ok).toBeNull();
		expect(numberError('')).toBe('enter a number');
		expect(numberError('1,5')).toBe('"1,5" is not a number');
		expect(numberError('-0.1', 0)).toBe('must be at least 0');
		expect(numberError('0', 0)).toBeNull();
	});

	it('is the check the unit editor applies to its drafts', () => {
		const records = [{ line: 3, model: 'SEXS', parameters: [{ name: 'K', text: '100' }] }];
		expect(parseDrafts({ '3:0': 'x' }, records).errors).toEqual([`SEXS K: ${numberError('x')}`]);
	});
});

describe('block parameters', () => {
	it('each have a label, a unit or none, and a description', () => {
		for (const [type, params] of Object.entries(BLOCK_PARAMS)) {
			const keys = params!.map((p) => p.key);
			expect(new Set(keys).size, type).toBe(keys.length);
			for (const p of params!) {
				expect(p.label, `${type}.${p.key}`).toBeTruthy();
				expect(p.description, `${type}.${p.key}`).toBeTruthy();
				// A lag's time constant is never negative; the engine refuses it.
				if (p.unit === 's' && ['t', 't2'].includes(p.key) && type !== 'lead_lag2')
					expect(p.min, `${type}.${p.key}`).toBe(0);
			}
		}
	});
});
