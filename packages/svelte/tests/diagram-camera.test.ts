import { describe, expect, it } from 'vitest';
import {
	busBounds,
	fitBounds,
	freeInsets,
	openingCamera,
	maxZoomFor,
	MAX_ZOOM,
	revealPoint,
	showsAnyPosition,
	toScreen
} from '../src/lib/diagram/index.js';

const view = { width: 1000, height: 600 };
const none = { left: 0, right: 0, top: 0, bottom: 0 };

describe('diagram camera', () => {
	it('fits bounds into the free area, centred between the panels', () => {
		const cam = fitBounds([0, 0, 100, 50], view, none);
		expect(cam.zoom).toBeCloseTo(Math.log2(10), 9);
		expect(cam.target.slice(0, 2)).toEqual([50, 25]);
		// A left dock shifts the drawing right on screen: the target moves left.
		const docked = fitBounds([0, 0, 100, 50], view, { ...none, left: 200 });
		expect(docked.target[0]).toBeLessThan(50);
		const [sx] = toScreen([50, 25], docked, view);
		expect(sx).toBeCloseTo(200 + (1000 - 200) / 2, 6);
	});

	it('caps the zoom', () => {
		expect(fitBounds([0, 0, 1, 1], view, none, 2).zoom).toBe(2);
	});

	it('reveals a point only when it is off screen or too small', () => {
		const cam = { target: [0, 0, 0] as [number, number, number], zoom: 0 };
		expect(revealPoint([10, 10], cam, view, none)).toBeNull();
		const moved = revealPoint([2000, 0], cam, view, none)!;
		expect(moved.target[0]).toBe(2000);
		const zoomed = revealPoint([10, 10], cam, view, none, 3)!;
		expect(zoomed.zoom).toBe(3);
		expect(revealPoint([NaN, 0], cam, view, none)).toBeNull();
	});

	it('tells whether a camera shows any position', () => {
		const positions = Float32Array.from([0, 0, 0, 5000, 0, 0]);
		const cam = { target: [5000, 0, 0] as [number, number, number], zoom: 0 };
		expect(showsAnyPosition(positions, cam, view)).toBe(true);
		expect(showsAnyPosition(positions, cam, view, { ...none, top: view.height / 2 + 1 })).toBe(
			false
		);
		expect(showsAnyPosition(positions, cam, view, { ...none, left: view.width / 2 + 1 })).toBe(
			false
		);
		expect(showsAnyPosition(positions, { ...cam, target: [9000, 9000, 0] }, view)).toBe(false);
	});

	it('bounds a set of buses with padding', () => {
		const positions = Float32Array.from([0, 0, 0, 10, 4, 0, NaN, 1, 0]);
		expect(busBounds(positions, [0, 1, 2], 1)).toEqual([-1, -1, 11, 5]);
		expect(busBounds(positions, [2])).toBeNull();
	});

	it('leaves room for docked panels and the compact sheet', () => {
		const docked = freeInsets({ compact: false, top: 10, dockWidth: () => 300, sheetInset: 0 });
		expect(docked).toEqual({ left: 344, right: 344, top: 74, bottom: 88 });
		const sheet = freeInsets({ compact: true, top: 0, dockWidth: () => 300, sheetInset: 200 });
		expect(sheet.bottom).toBe(224);
	});
});

describe('opening camera', () => {
	const fitted = { target: [0, 0, 0] as [number, number, number], zoom: 4 };
	const shows = () => true;

	it('keeps the map view when it shows the drawing at a useful size', () => {
		const closer = { target: [5, 5, 0] as [number, number, number], zoom: 6 };
		expect(openingCamera(closer, fitted, shows)).toBe(closer);
		const slightlyOut = { ...closer, zoom: 3.5 };
		expect(openingCamera(slightlyOut, fitted, shows)).toBe(slightlyOut);
	});

	it('fits when the map view would draw the case small, off screen or not at all', () => {
		expect(openingCamera({ ...fitted, zoom: 2 }, fitted, shows)).toBe(fitted);
		expect(openingCamera({ ...fitted, zoom: 6 }, fitted, () => false)).toBe(fitted);
		expect(openingCamera(null, fitted, shows)).toBe(fitted);
	});
});

describe('zoom limit', () => {
	it('keeps the fixed limit for drawings in metres', () => {
		expect(maxZoomFor([0, 0, 500_000, 300_000])).toBe(MAX_ZOOM);
	});

	it('lets a unit-box drawing fit a large canvas and zoom in', () => {
		const zoom = maxZoomFor([0, 0, 1, 1]);
		expect(zoom).toBe(16);
		const fitted = fitBounds([0, 0, 1, 1], view, none, zoom);
		// 600 px of height for one unit: well past the old 256 px cap.
		expect(2 ** fitted.zoom).toBeCloseTo(600);
	});

	it('falls back to the fixed limit for empty bounds', () => {
		expect(maxZoomFor([0, 0, 0, 0])).toBe(MAX_ZOOM);
	});
});
