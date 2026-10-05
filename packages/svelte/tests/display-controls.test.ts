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
		activeSolvable: { solving: false }
	}
}));

vi.mock('../src/lib/context.svelte.js', () => ({
	getAppState: () => state.app,
	getController: () => state.ctrl
}));

beforeEach(() => {
	state.ctrl.activeSolvable.solving = false;
});

describe('display controls without a result', () => {
	it('shows progress only while the calculation is still running', () => {
		state.ctrl.activeSolvable.solving = true;
		const { body } = render(DisplayControls);
		expect(body).toContain('Solving AC power flow');
		expect(body).not.toContain('results available');
	});
	it('stops reporting progress when the calculation ends without a result', () => {
		const { body } = render(DisplayControls);
		expect(body).not.toContain('Solving AC power flow');
		expect(body).toContain('No AC power flow results available.');
		expect(body).not.toContain('blink');
	});
});
