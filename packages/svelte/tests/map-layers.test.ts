import { describe, expect, it } from 'vitest';
import {
	ALL_MAP_LAYERS,
	busesForLayers,
	layerVisibility,
	resolveMapLayers
} from '../src/lib/map-layers.js';

const buses = [{ id: 1 }, { id: 2 }, { id: 3 }, { id: 4 }];
const kinds: ReadonlyMap<number, string> = new Map([
	[1, 'generator'],
	[2, 'substation'],
	[3, 'load'],
	[4, 'substation']
]);

describe('map layers', () => {
	it('shows every layer the host leaves out', () => {
		expect(resolveMapLayers(undefined)).toEqual(ALL_MAP_LAYERS);
		expect(resolveMapLayers({})).toEqual(ALL_MAP_LAYERS);
		expect(resolveMapLayers({ branches: false })).toEqual({
			branches: false,
			buses: true,
			substations: true
		});
	});

	it('keeps the same array when the layers do not split the markers', () => {
		expect(busesForLayers(buses, kinds, { buses: true, substations: true })).toBe(buses);
		expect(busesForLayers(buses, kinds, { buses: false, substations: false })).toBe(buses);
	});

	it('splits a generated grid into substations and the other buses', () => {
		expect(
			busesForLayers(buses, kinds, { buses: true, substations: false }).map((b) => b.id)
		).toEqual([1, 3]);
		expect(
			busesForLayers(buses, kinds, { buses: false, substations: true }).map((b) => b.id)
		).toEqual([2, 4]);
	});

	it('without kinds, no bus is a substation', () => {
		for (const none of [undefined, new Map<number, string>()]) {
			expect(busesForLayers(buses, none, { buses: true, substations: false })).toBe(buses);
			expect(busesForLayers(buses, none, { buses: false, substations: true })).toEqual([]);
		}
	});

	it('maps the layers prop to each layer kind', () => {
		expect(layerVisibility(undefined)).toEqual({
			branches: true,
			busMarkers: true,
			substationPoints: true,
			multiBuses: true
		});
		expect(layerVisibility({ branches: false })).toMatchObject({ branches: false });
		// A bus layer stays visible while either kind shows; its data drops the other.
		expect(layerVisibility({ buses: false })).toEqual({
			branches: true,
			busMarkers: true,
			substationPoints: true,
			multiBuses: false
		});
		expect(layerVisibility({ substations: false })).toEqual({
			branches: true,
			busMarkers: true,
			substationPoints: false,
			multiBuses: true
		});
		expect(layerVisibility({ buses: false, substations: false })).toMatchObject({
			busMarkers: false,
			substationPoints: false,
			multiBuses: false
		});
	});
});
