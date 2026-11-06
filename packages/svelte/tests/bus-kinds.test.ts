import { describe, expect, it } from 'vitest';
import type { NetworkBus } from '../src/lib/api.js';
import { tapBusIds, withoutTaps } from '../src/lib/bus-kinds.js';
import { LocalCase } from '../src/lib/state.svelte.js';

// A generated grid in the PowerIO IR layout: tellegen-synth records each
// bus's kind under `extras.tellegen_synth`. Bus 2 taps the line from 1 to 3.
function synthModule(kinds: Record<number, string>): string {
	return JSON.stringify({
		schema: 'pio-ir',
		version: 2,
		value: {
			type: 'powerio.BalancedNetwork',
			data: {
				buses: Object.entries(kinds).map(([id, kind]) => ({
					id: Number(id),
					kind: 'PQ',
					extras: { tellegen_synth: { kind, tier: 0 } }
				}))
			}
		}
	});
}

const bus = (id: number): NetworkBus => ({ id, lon: id, lat: 0, demand_mw: 0, gen_mw: 0 });
const buses = [bus(1), bus(2), bus(3)];

describe('tapBusIds', () => {
	it('reads the tap buses a generated grid records', () => {
		const ids = tapBusIds(synthModule({ 1: 'substation', 2: 'tap', 3: 'load', 4: 'tap' }));
		expect([...ids]).toEqual([2, 4]);
	});

	it('finds none in a module without tellegen-synth kinds', () => {
		expect(tapBusIds(synthModule({ 1: 'substation', 2: 'load' })).size).toBe(0);
		const plain = JSON.stringify({
			value: { data: { buses: [{ id: 1, name: 'tap', extras: {} }] } }
		});
		expect(tapBusIds(plain).size).toBe(0);
		expect(tapBusIds(undefined).size).toBe(0);
	});

	it('finds none in text that does not parse', () => {
		expect(tapBusIds('{"kind":"tap"').size).toBe(0);
	});
});

describe('withoutTaps', () => {
	it('drops the taps and keeps the rest in order', () => {
		expect(withoutTaps(buses, new Set([2])).map((b) => b.id)).toEqual([1, 3]);
	});

	it('returns the same array when there is no tap, so the map layer keeps its data', () => {
		expect(withoutTaps(buses, new Set())).toBe(buses);
	});
});

describe('LocalCase markers', () => {
	it('leaves the taps of a generated grid out of the map markers but not out of the view', () => {
		const c = new LocalCase({
			id: 'local-1',
			label: 'US-CA',
			fileName: 'us-ca.pio.json',
			studyInputJson: synthModule({ 1: 'substation', 2: 'tap', 3: 'load' }),
			view: { buses, branches: [] }
		});
		expect([...c.tapBuses]).toEqual([2]);
		expect(c.markerBuses.map((b) => b.id)).toEqual([1, 3]);
		expect(c.view?.buses).toHaveLength(3);
	});

	it('draws every bus of any other case', () => {
		const c = new LocalCase({
			id: 'local-2',
			label: 'case3',
			fileName: 'case3.m',
			studyInputJson: synthModule({ 1: 'substation' }),
			view: { buses, branches: [] }
		});
		expect(c.markerBuses).toBe(buses);
	});
});
