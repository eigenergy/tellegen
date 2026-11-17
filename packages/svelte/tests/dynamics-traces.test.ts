import { render } from 'svelte/server';
import { expect, it, vi } from 'vitest';
import type { RmsRun } from '@tellegen/engine';
import DynamicsTraces from '../src/lib/components/dynamics/DynamicsTraces.svelte';

vi.mock('../src/lib/context.svelte.js', () => ({
	getAppState: () => ({ dynamicsHover: null }),
	getController: () => ({ dynamics: { player: { t: 0 } } })
}));

it('renders a trace chart when no frequency extrema are available', () => {
	// Only fields read by the renderer are needed; transport validation has its own fixtures.
	const run = {
		time: new Float64Array([0, 1]),
		keys: ['coi:f'],
		series: (key: string) => key === 'coi:f' ? new Float64Array([NaN, NaN]) : undefined,
		meta: { base_frequency: 60, base_mva: 100, generators: [] },
		screening: { frequency: { coi_nadir_hz: null }, voltage: { min_pu: null } }
	} as unknown as RmsRun;
	const { body } = render(DynamicsTraces, { props: { run, events: [] } });
	expect(body).toContain('Frequency, lowest Unavailable');
	expect(body).toContain('role="img"');
});
