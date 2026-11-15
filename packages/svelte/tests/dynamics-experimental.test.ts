import { describe, expect, it, vi } from 'vitest';
import { AppState } from '../src/lib/state.svelte.js';
import { UserPreferences } from '../src/lib/settings.svelte.js';

// Dynamics (RMS) simulation is experimental: off until the user turns on
// Settings → Experimental features.
describe('dynamics behind the experimental setting', () => {
	it('is off by default and the mode reads as steady state', () => {
		const app = new AppState();
		expect(app.preferences.experimental).toBe(false);
		expect(app.dynamicsEnabled).toBe(false);
		app.mode = 'dynamics';
		expect(app.mode).toBe('steady');
	});

	it('comes back when the setting is on', () => {
		const app = new AppState();
		app.preferences.update({ experimental: true }, false);
		expect(app.dynamicsEnabled).toBe(true);
		app.mode = 'dynamics';
		expect(app.mode).toBe('dynamics');
		// Turning the setting off falls back to steady state.
		app.preferences.update({ experimental: false }, false);
		expect(app.mode).toBe('steady');
		// A request for dynamics while it is off does not survive turning it on.
		app.mode = 'dynamics';
		app.preferences.update({ experimental: true }, false);
		expect(app.mode).toBe('steady');
	});

	it('a saved dynamics mode falls back to steady state while the setting is off', () => {
		const app = new AppState();
		app.preferences.update({ experimental: true }, false);
		app.mode = 'dynamics';
		app.preferences.update({ experimental: false }, false);
		expect(app.mode).toBe('steady');
		app.preferences.update({ experimental: true }, false);
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
