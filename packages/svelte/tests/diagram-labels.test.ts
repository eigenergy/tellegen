import { expect, it } from 'vitest';
import { diagramLabels } from '../src/lib/diagram-labels.js';

it('omits overlapping drawing labels and reveals them when zoomed in', () => {
	const buses = [
		{ id: 1, lon: 10, lat: 20 },
		{ id: 2, lon: 15, lat: 20 }
	];
	expect([...diagramLabels(buses, 1, 0, 0, 500, 500, null)]).toEqual([1]);
	expect([...diagramLabels(buses, 1, 0, 0, 500, 500, 2)]).toEqual([2]);
	expect([...diagramLabels(buses, 5, 0, 0, 500, 500, null)]).toEqual([1, 2]);
	expect([...diagramLabels(buses, 1, -500, 0, 500, 500, null)]).toEqual([]);
});
