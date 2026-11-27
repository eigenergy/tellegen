import { render } from 'svelte/server';
import { describe, expect, it, vi } from 'vitest';
import type { Formulation } from '@tellegen/engine';
import FormulationSelector, { rovingIndex } from '../src/lib/components/FormulationSelector.svelte';
import { transmissionSelectable } from '../src/lib/components/CalculationSettings.svelte';
import type { SolvableCase } from '../src/lib/state.svelte.js';

const offered = new Set<Formulation>(['dcopf', 'acpf', 'acopf']);

vi.mock('../src/lib/context.svelte.js', () => ({
	getController: () => ({
		activeSolvable: null,
		acOpfLoading: false,
		calculationOffered: (_c: unknown, f: Formulation) => offered.has(f),
		canCancelSettings: () => false
	})
}));

const fakeCase = {
	solving: false,
	formulation: 'dcopf',
	settings: { calculation: null },
	solution: null
} as unknown as SolvableCase;

describe('FormulationSelector segmented variant', () => {
	it('is a toolbar of toggle buttons with one tab stop on the chosen calculation', () => {
		const { body } = render(FormulationSelector, {
			props: { currentCase: fakeCase, variant: 'segmented', id: 'calc' }
		});
		expect(body).toContain('role="toolbar"');
		expect(body).not.toContain('role="radio');
		expect(body).not.toContain('aria-checked');
		const pressed = [...body.matchAll(/aria-pressed="true"/g)];
		expect(pressed).toHaveLength(1);
		const stops = [...body.matchAll(/data-formulation="([a-z_]+)"[^>]*tabindex="0"/g)];
		expect(stops.map((m) => m[1])).toEqual(['dcopf']);
	});
});

describe('rovingIndex', () => {
	it('moves with the arrows and wraps at either end', () => {
		expect(rovingIndex('ArrowRight', 0, 3)).toBe(1);
		expect(rovingIndex('ArrowDown', 2, 3)).toBe(0);
		expect(rovingIndex('ArrowLeft', 0, 3)).toBe(2);
		expect(rovingIndex('ArrowUp', 1, 3)).toBe(0);
	});

	it('jumps with Home and End and starts from the first when the stop is on none', () => {
		expect(rovingIndex('Home', 2, 3)).toBe(0);
		expect(rovingIndex('End', 0, 3)).toBe(2);
		expect(rovingIndex('ArrowRight', -1, 3)).toBe(1);
	});

	it('leaves Space, Enter and other keys to the button', () => {
		expect(rovingIndex(' ', 0, 3)).toBeNull();
		expect(rovingIndex('Enter', 0, 3)).toBeNull();
		expect(rovingIndex('Tab', 0, 3)).toBeNull();
		expect(rovingIndex('ArrowRight', 0, 0)).toBeNull();
	});
});

describe('transmissionSelectable', () => {
	it('offers AC OPF when it is configured but not yet loaded', () => {
		expect(transmissionSelectable('acopf', false, true)).toBe(true);
	});

	it('hides AC OPF after a failed load and other calculations the engine lacks', () => {
		expect(transmissionSelectable('acopf', false, false)).toBe(false);
		expect(transmissionSelectable('acpf', false, true)).toBe(false);
		expect(transmissionSelectable('acpf', true, true)).toBe(true);
	});
});
