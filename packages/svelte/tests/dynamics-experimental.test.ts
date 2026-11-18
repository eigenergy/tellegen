import { describe, expect, it, vi } from 'vitest';
import { AppState } from '../src/lib/state.svelte.js';
import { UserPreferences } from '../src/lib/settings.svelte.js';

// Dynamics (RMS simulation) is always on; Settings → Experimental features
// gates only the tools still being validated, not the mode itself.
describe('dynamics and the experimental setting', () => {
	it('is on by default, with the experimental setting off', () => {
		const app = new AppState();
		expect(app.preferences.experimental).toBe(false);
		expect(app.dynamicsEnabled).toBe(true);
		app.mode = 'dynamics';
		expect(app.mode).toBe('dynamics');
	});

	it('does not depend on the experimental setting', () => {
		const app = new AppState();
		app.mode = 'dynamics';
		app.preferences.update({ experimental: true }, false);
		expect(app.dynamicsEnabled).toBe(true);
		expect(app.mode).toBe('dynamics');
		app.preferences.update({ experimental: false }, false);
		expect(app.dynamicsEnabled).toBe(true);
		expect(app.mode).toBe('dynamics');
	});

	it('preference listeners hear every change until they unsubscribe', () => {
		const p = new UserPreferences();
		const listener = vi.fn();
		const unsubscribe = p.subscribe(listener);
		p.update({ experimental: true }, false);
		p.update({ zen: true }, false);
		expect(listener).toHaveBeenCalledTimes(2);
		unsubscribe();
		p.update({ experimental: false }, false);
		expect(listener).toHaveBeenCalledTimes(2);
	});
});
