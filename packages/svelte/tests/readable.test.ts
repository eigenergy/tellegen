import { describe, expect, it } from 'vitest';
import { readable } from '../src/lib/format.js';

describe('readable quantities', () => {
	it('rounds engineering quantities to about three significant figures', () => {
		expect(readable(1796.905364, 'MW').text).toBe('1,797 MW');
		expect(readable(26.699464, 'objective units/MW').text).toBe('26.7 objective units/MW');
		expect(readable(2.3456, 'MVAr').text).toBe('2.35 MVAr');
		expect(readable(63.924061, '%').text).toBe('63.9%');
		expect(readable(1.023456, 'pu').text).toBe('1.0235 pu');
	});
	it('signs on request and keeps the exact value', () => {
		expect(readable(1014.094636, 'MW', true)).toEqual({
			text: '+1,014 MW',
			exact: '+1014.094636 MW'
		});
		expect(readable(-80, 'MW', true).text).toBe('-80 MW');
		expect(readable(0.0001, 'MW', true).text).toBe('0 MW');
		expect(readable(-0.0001, 'MW').text).toBe('0 MW');
		expect(readable(undefined, 'MW').text).toBe('—');
	});
});

describe('small rates', () => {
	it('reads solver noise as 0 and keeps real values to two figures', async () => {
		const { signedSmall, small } = await import('../src/lib/format.js');
		expect(signedSmall(-3.3e-7)).toBe('0');
		expect(small(1.1e-7)).toBe('0');
		expect(signedSmall(0.0017)).toBe('+0.0017');
		expect(signedSmall(-0.42)).toBe('−0.42');
	});
});
