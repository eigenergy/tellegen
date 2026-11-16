import { render } from 'svelte/server';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import DisplayControls from '../src/lib/components/DisplayControls.svelte';

const state = vi.hoisted(() => ({
	app: { displayMode: 'voltage', sensitivityLoading: false },
	ctrl: {
		displayOptions: [],
		activeDisplay: null,
		displayStats: null,
		activeFormulation: 'acpf',
		activeSolvable: { solving: false } as {
			solving: boolean;
			solveBackend?: string | null;
			solveMs?: number | null;
		}
	}
}));

vi.mock('../src/lib/context.svelte.js', () => ({
	getAppState: () => state.app,
	getController: () => state.ctrl
}));

beforeEach(() => {
	state.ctrl.activeSolvable = { solving: false };
});

describe('display controls without a result', () => {
	it('shows progress only while the calculation is still running', () => {
		state.ctrl.activeSolvable.solving = true;
		const { body } = render(DisplayControls);
		expect(body).toContain('Solving…');
		expect(body).not.toContain('No result');
	});
	it('stops reporting progress when the calculation ends without a result', () => {
		const { body } = render(DisplayControls);
		expect(body).not.toContain('Solving…');
		expect(body).toContain('No result');
		expect(body).not.toContain('blink');
	});
	it('leaves a failed calculation to the status line above', () => {
		state.ctrl.activeSolvable = { solving: false, solveBackend: 'clarabel-wasm', solveMs: null };
		const { body } = render(DisplayControls);
		expect(body).not.toContain('No result');
	});
});
