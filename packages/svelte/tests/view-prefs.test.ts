import { afterEach, expect, it, vi } from 'vitest';
import { StoredNumber } from '../src/lib/components/schematic/view-prefs.svelte.js';

afterEach(() => vi.unstubAllGlobals());

it('ignores invalid warning preferences and retains a valid value after an invalid edit', () => {
	for (const stored of ['NaN', 'Infinity', '-1', '100']) {
		vi.stubGlobal('localStorage', { getItem: () => stored, setItem: vi.fn() });
		const preference = new StoredNumber('warning', 0, 99, 90);
		expect(preference.value).toBe(90);
		preference.set(75);
		preference.set(NaN);
		preference.set(100);
		expect(preference.value).toBe(75);
	}
});

it('keeps the warning control usable when storage is blocked', () => {
	vi.stubGlobal('localStorage', {
		getItem: () => {
			throw new Error('blocked');
		},
		setItem: () => {
			throw new Error('blocked');
		}
	});
	const preference = new StoredNumber('warning', 0, 99, 90);
	preference.set(65);
	expect(preference.value).toBe(65);
});
