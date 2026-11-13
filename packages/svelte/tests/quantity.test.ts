import { describe, expect, it } from 'vitest';
import { degrees, MISSING, quantity, quantityRange, voltsWithPu } from '../src/lib/format.js';

describe('quantity', () => {
	it('keeps four significant figures with thousands separators', () => {
		expect(quantity(123.4567, 'A')).toBe('123.5 A');
		expect(quantity(0.012345, 'A')).toBe('0.012 A');
		expect(quantity(12.3456, 'kW')).toBe('12.35 kW');
		expect(quantity(123456.7, 'kW')).toBe('123,457 kW');
		expect(quantity(-4.2, 'kvar')).toBe('-4.200 kvar');
	});
	it('switches volts to kV at 1 kV', () => {
		expect(quantity(239.61, 'V')).toBe('239.6 V');
		expect(quantity(7199.56, 'V')).toBe('7.200 kV');
		expect(quantity(-12470, 'V')).toBe('-12.47 kV');
		expect(quantity(7199.56, 'V', { exact: true })).toBe('7199.56 V');
	});
	it('attaches percent and degrees, fixes pu to four decimals', () => {
		expect(quantity(12.345, '%')).toBe('12.3%');
		expect(quantity(-119.876, '°')).toBe('-119.88°');
		expect(quantity(1.023456, 'pu')).toBe('1.0235 pu');
		expect(degrees(Math.PI / 2)).toBe('90.00°');
	});
	it('signs on request, never shows negative zero and marks missing values one way', () => {
		expect(quantity(3, 'kW', { signed: true })).toBe('+3.000 kW');
		expect(quantity(-0.00001, 'A')).toBe('0.000 A');
		expect(quantity(null, 'V')).toBe(MISSING);
		expect(quantity(Number.NaN, '%')).toBe('—');
		expect(quantity(undefined, 'pu')).toBe('—');
	});
	it('puts per-unit beside volts when the base is known', () => {
		expect(voltsWithPu(239.6, 240)).toBe('239.6 V (0.9983 pu)');
		expect(voltsWithPu(239.6, null)).toBe('239.6 V');
		expect(voltsWithPu(null, 240)).toBe('—');
	});
});

describe('quantityRange', () => {
	it('writes a shared unit once', () => {
		expect(quantityRange(203.41, 239.64, 'V')).toBe('203.4–239.6 V');
		expect(quantityRange(950, 1050, 'V')).toBe('950.0 V – 1.050 kV');
		expect(quantityRange(0.848, 0.995, 'pu')).toBe('0.8480–0.9950 pu');
		expect(quantityRange(1.5, 12.25, '%')).toBe('1.5–12.3%');
		expect(quantityRange(null, 2, 'A')).toBe('— – 2.000 A');
	});
});
