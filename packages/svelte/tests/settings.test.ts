import { expect, it } from 'vitest';
import { UserPreferences } from '../src/lib/settings.svelte.js';
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
