import { describe, expect, it } from 'vitest';
import { channelLabel, quantityOf } from '../src/lib/dynamics/quantities.js';

describe('the channels the engine derives in a live session', () => {
	it('chart system channels by their own key, each in its unit', () => {
		expect(quantityOf('coi:f')).toMatchObject({ q: 'coi:f', unit: 'Hz', owner: 'system' });
		expect(quantityOf('coi:sep')).toMatchObject({ q: 'coi:sep', unit: 'deg' });
		expect(quantityOf('coi:sep')!.scale(Math.PI / 3, 100)).toBeCloseTo(60, 12);
		// Frame-dependent and internal ones are not offered.
		expect(quantityOf('coi:delta')).toBeUndefined();
		expect(quantityOf('coi:islands')).toBeUndefined();
		expect(channelLabel('coi:f')).toBe('f COI');
	});

	it('name bus frequencies and angles from the island COI by their element', () => {
		expect(quantityOf('f:1101')).toMatchObject({ q: 'f', unit: 'Hz', owner: 'bus' });
		expect(channelLabel('f:1101')).toBe('bus 1101 f');
		expect(quantityOf('dcoi:1431#N')).toMatchObject({ q: 'dcoi', unit: 'deg', owner: 'unit' });
		expect(channelLabel('dcoi:1431#N')).toBe("G1431 'N' δ − δCOI");
	});
});
