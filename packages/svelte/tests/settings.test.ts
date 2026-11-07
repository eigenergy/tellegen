import { expect, it } from 'vitest';
import { UserPreferences, recordedCalculationSettings } from '../src/lib/settings.svelte.js';
it('display preference patches are atomic and storage errors retain current session choices', () => {
	const preferences = new UserPreferences();
	preferences.initialize({
		getItem: () => null,
		setItem: () => {
			throw new Error('quota');
		}
	} as unknown as Storage);
	expect(() => preferences.update({ theme: 'dark', busSizing: 'invalid' })).toThrow(
		'Invalid display'
	);
	expect(preferences.theme).toBeNull();
	preferences.update({ theme: 'dark' });
	expect(preferences.theme).toBe('dark');
	expect(preferences.error).toContain('could not be saved');
	preferences.reset();
	expect(preferences.theme).toBeNull();
});
it('a corrupt preference record does not partially change the defaults', () => {
	const preferences = new UserPreferences();
	preferences.initialize({
		getItem: () => JSON.stringify({ version: 1, values: { theme: 'dark', busSizing: 'invalid' } })
	} as unknown as Storage);
	expect(preferences.theme).toBeNull();
	expect(preferences.busSizing).toBe('power');
	expect(preferences.error).toContain('could not be restored');
});

it('saved settings read as overrides of the engine defaults, dropping options this engine lacks', () => {
	const saved = (record: unknown) =>
		JSON.stringify({ extensions: { 'tellegen.calculation_settings': record } });
	const recorded = recordedCalculationSettings(
		saved({
			version: 1,
			calculation: 'dcopf',
			overrides: { thermal: 'rated', future_option: 3 }
		})
	);
	expect(recorded).toEqual({
		calculation: 'dcopf',
		overrides: { thermal: 'rated' },
		effective: { angle_difference: true, lazy: null, thermal: 'rated' },
		ignored: ['future_option']
	});
	// Unknown record versions and the unreleased full-effective shape are not read.
	expect(
		recordedCalculationSettings(saved({ version: 2, calculation: 'dcopf', overrides: {} }))
	).toBeNull();
	expect(
		recordedCalculationSettings(saved({ calculation: 'dcopf', effective: { thermal: 'rated' } }))
	).toBeNull();
	expect(recordedCalculationSettings('not json')).toBeNull();
});
