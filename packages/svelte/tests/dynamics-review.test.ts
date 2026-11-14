import { describe, expect, it, vi } from 'vitest';
import { AppState } from '../src/lib/state.svelte.js';
import { DynamicsController, VALIDATION_NETWORK } from '../src/lib/dynamics/controller.svelte.js';
import { AT_NOMINAL_HZ, haloColor, haloRadius } from '../src/lib/dynamics/layers.js';
import { DYN_SPEED } from '../src/lib/colors.js';
import { m4 } from '../src/lib/dynamics/decimate.js';
import { pinInto } from '../src/lib/dynamics/series.js';
import { minSpanFor, tickLabel, ticks, yDomain } from '../src/lib/dynamics/axis.js';
import { busFault, sinceEvent } from '../src/lib/dynamics/events.js';
import { tabKey } from '../src/lib/dynamics/tabs.js';
import { announcement, rows } from '../src/lib/dynamics/verdict.js';
import { VALIDATION_DT, VALIDATION_OPTIONS } from '../src/lib/dynamics/presets.js';
import type { RmsScreening } from '@tellegen/engine';

describe('speed rings at rest', () => {
	const pal = DYN_SPEED.light;
	it('draw a neutral minimum ring for a deviation at rounding noise', () => {
		for (const df of [0, 4e-16, -4e-16, AT_NOMINAL_HZ / 2]) {
			expect(haloColor(df, true, pal)).toEqual(pal.rest);
			expect(haloRadius(df)).toBe(9);
		}
	});
	it('colour by sign above the threshold and grow from it', () => {
		expect(haloColor(0.01, true, pal)).toEqual(pal.over);
		expect(haloColor(-0.01, true, pal)).toEqual(pal.under);
		expect(haloRadius(0.01)).toBeGreaterThan(9);
		expect(haloRadius(0.1)).toBeGreaterThan(haloRadius(0.01));
		expect(haloColor(0.5, false, pal).slice(0, 3)).toEqual(pal.held.slice(0, 3));
	});
});

describe('trace gaps', () => {
	it('M4 leaves a break where samples are missing, however short the gap', () => {
		const x = Array.from({ length: 10 }, (_, i) => i);
		const y = [1, 2, 3, NaN, NaN, 6, 7, 8, 9, 10];
		const idx = m4(x, y, 1000, 0, 9, true);
		expect(idx).toContain(-1);
		const k = idx.indexOf(-1);
		expect(idx[k - 1]).toBe(2);
		expect(idx[k + 1]).toBe(5);
		// Without breaks (the bands) the indices stay plain.
		expect(m4(x, y, 1000, 0, 9)).not.toContain(-1);
		// A leading or trailing gap needs no break.
		expect(m4(x, [NaN, 1, 2, 3, 4, 5, 6, 7, 8, NaN], 1000, 0, 9, true)).not.toContain(-1);
	});
});

describe('pinned traces', () => {
	it('never duplicate a drawn key; the drawn one is emphasised', () => {
		const list = [
			{ key: 'f:1', width: 1.2, alpha: 0.8 },
			{ key: 'f:2', width: 1.2, alpha: 0.8 }
		];
		pinInto(list, { key: 'f:2', width: 2 });
		pinInto(list, { key: 'f:3', width: 2 });
		pinInto(list, { key: 'f:3', width: 2 });
		expect(list.map((s) => s.key)).toEqual(['f:1', 'f:2', 'f:3']);
		expect(list[1]).toMatchObject({ width: 2, alpha: 1 });
	});
});

describe('trace axes', () => {
	it('give a flat trace distinct tick labels', () => {
		const [lo, hi] = yDomain(60, 60 + 4e-13, minSpanFor('Hz'));
		const t = ticks(lo, hi, 4);
		const labels = t.map((v) => tickLabel(v, t[1] - t[0]));
		expect(new Set(labels).size).toBe(labels.length);
		expect(labels.length).toBeGreaterThan(1);
		expect(lo).toBeLessThan(60);
		expect(hi).toBeGreaterThan(60);
	});
	it('leave a real range alone', () => {
		expect(yDomain(59.8, 60, minSpanFor('Hz'))[0]).toBeCloseTo(59.8 - 0.012);
	});
});

describe('playback readout', () => {
	it('counts cycles near an event and seconds after', () => {
		expect(sinceEvent(0.05, 60)).toBe('+3.0 cyc');
		expect(sinceEvent(1.9, 60)).toBe('+1.90 s');
	});
});

describe('tab keys', () => {
	it('wrap with the arrows and go to the ends', () => {
		expect(tabKey('ArrowRight', 3, 4)).toBe(0);
		expect(tabKey('ArrowLeft', 0, 4)).toBe(3);
		expect(tabKey('Home', 2, 4)).toBe(0);
		expect(tabKey('End', 0, 4)).toBe(3);
		expect(tabKey('a', 0, 4)).toBeNull();
	});
});

function controller() {
	const app = new AppState();
	return { app, dyn: new DynamicsController(app, async () => {}) };
}

describe('dynamics controller', () => {
	it('restores the user step and options when a scenario is cleared', () => {
		const { dyn } = controller();
		dyn.dt = 0.001;
		dyn.tEnd = 10;
		dyn.setOptions({ load_model: 'constant_power' });
		dyn.usePreset('fault-1101');
		expect(dyn.dt).toBe(VALIDATION_DT);
		expect(dyn.options.load_model).toBe(VALIDATION_OPTIONS.load_model);
		expect(dyn.beforePreset).not.toBeNull();
		// A second scenario keeps the user's settings, not the first scenario's.
		dyn.usePreset('trip-1101-1401');
		dyn.clearEvents();
		expect(dyn.dt).toBe(0.001);
		expect(dyn.tEnd).toBe(10);
		expect(dyn.options).toEqual({ load_model: 'constant_power' });
		expect(dyn.beforePreset).toBeNull();
	});

	it('refuses a run while modes are computed, with a reason', async () => {
		const { app, dyn } = controller();
		vi.spyOn(app, 'activeLocal', 'get').mockReturnValue({
			id: 'local-1'
		} as never);
		dyn.attachments = {
			'local-1': { status: 'ready', inspection: null } as never
		};
		dyn.modesBusy = true;
		expect(await dyn.run()).toBeNull();
		expect(dyn.error).toMatch(/modes/);
		dyn.modesBusy = false;
		dyn.running = true;
		expect(await dyn.run()).toBeNull();
		expect(dyn.error).toMatch(/in progress/);
	});

	it('records the validation network in the run JSON', () => {
		const { dyn } = controller();
		const json = (network: 'case' | 'validation') =>
			JSON.parse(
				dyn.runJson({
					id: 1,
					localId: 'local-1',
					run: null,
					failure: null,
					events: [busFault(1101, 60)],
					options: { load_model: 'constant_impedance' },
					dt: 0.005,
					tEnd: 20,
					badge: null,
					network,
					wallMs: 0,
					agent: false,
					experimental: false
				})
			);
		expect(json('validation').network).toBe(VALIDATION_NETWORK);
		expect(VALIDATION_NETWORK).toContain('case-validation.raw');
		expect(json('case').network).not.toContain('validation');
	});
});
