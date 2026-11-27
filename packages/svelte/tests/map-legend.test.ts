import { describe, expect, it } from 'vitest';
import {
	LOADING_AMBER,
	LOADING_ATTENTION,
	LOADING_BASE,
	LOADING_RED,
	branchColor,
	demandColor,
	priceColor,
	ramp,
	scalarGradient,
	scalarStops
} from '../src/lib/colors.js';
import { busColorScale, displayRange, lineLoadingKey } from '../src/lib/map-legend.js';

describe('line loading key', () => {
	it('matches the colours branchColor draws at each step', () => {
		const key = lineLoadingKey('dcopf');
		expect(key.map((step) => step.color)).toEqual([LOADING_BASE, LOADING_AMBER, LOADING_RED]);
		for (const step of key) expect(branchColor(step.from, true)).toEqual(step.color);
		expect(branchColor(LOADING_ATTENTION - 0.01, true)).toEqual(LOADING_BASE);
		expect(key.map((step) => step.label)).toEqual(['Below 90%', 'Near limit', 'At limit']);
	});

	it('says over limit for a power flow', () => {
		expect(lineLoadingKey('acpf')[2].label).toBe('Over limit');
	});
});

describe('bus colour scale', () => {
	it('places the stops where the map colours buses', () => {
		for (const theme of ['light', 'dark'] as const) {
			const price = scalarStops('price', theme);
			expect(price[0]).toEqual({ offset: 0, color: priceColor(0, theme) });
			expect(price.at(-1)).toEqual({ offset: 1, color: priceColor(1, theme) });
		}
		// Demand is drawn on a square-root scale: the middle stop sits at a quarter.
		const demand = scalarStops('demand');
		expect(demand[2].offset).toBeCloseTo(0.25);
		expect(demand[2].color).toEqual(demandColor(0.25));
		expect(
			ramp(
				demand.map((s) => s.color),
				1
			)
		).toEqual(demandColor(1));
	});

	it('describes the domain, its clamped ends and the theme ramp', () => {
		const values = [...Array.from({ length: 98 }, (_, i) => 20 + i * 0.1), 5, 90];
		const scale = busColorScale(
			{
				mode: 'price',
				label: 'Price',
				unit: '$/MWh',
				values: values.map((value, bus) => ({ bus, value }))
			},
			'dark'
		);
		expect(scale).not.toBeNull();
		expect(scale?.unit).toBe('$/MWh');
		expect(scale?.lo.clamped).toBe(true);
		expect(scale?.hi.clamped).toBe(true);
		expect(scale?.uniform).toBeNull();
		expect(scale?.gradient).toBe(scalarGradient('price', 'dark'));
		expect(scale?.stops).toEqual(scalarStops('price', 'dark'));
	});

	it('reports one value at every bus, and nothing without a finite value', () => {
		expect(displayRange('voltage', [1.02, 1.02, 1.020001])?.uniform).toBe(1.02);
		expect(displayRange('price', [Number.NaN])).toBeNull();
		expect(busColorScale({ mode: 'angle', label: 'Angle', unit: 'rad', values: [] })).toBeNull();
	});
});
