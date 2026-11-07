import { describe, expect, it } from 'vitest';
import {
	fitBounds,
	openingCamera,
	showsAnyPosition,
	type DiagramCamera
} from '../src/lib/diagram/camera.js';

const viewport = { width: 1000, height: 800 };
const insets = { left: 300, right: 40, top: 100, bottom: 80 };
const fit: DiagramCamera = { target: [0, 0, 0], zoom: 1 };

describe('opening a schematic in its free area', () => {
	it('fits a feeder even when the distant map contains one of its buses', () => {
		const distant = { ...fit, zoom: -5 };
		expect(showsAnyPosition([0, 0, 0], distant, viewport, insets)).toBe(true);
		expect(openingCamera(distant, fit, () => true)).toBe(fit);
	});

	it('fits when the only bus is behind a dock or toolbar', () => {
		const from = { ...fit, zoom: 0 };
		for (const positions of [
			[-250, 0, 0],
			[0, -350, 0],
			[480, 0, 0],
			[0, 350, 0]
		]) {
			expect(showsAnyPosition(positions, from, viewport, insets)).toBe(false);
			expect(
				openingCamera(from, fit, (c) => showsAnyPosition(positions, c, viewport, insets))
			).toBe(fit);
		}
	});

	it('keeps a close view of part of a large feeder and allows one zoom step of slack', () => {
		for (const zoom of [0, 1, 6]) {
			const from = { ...fit, zoom };
			expect(
				openingCamera(from, fit, (c) => showsAnyPosition([0, 0, 0], c, viewport, insets))
			).toBe(from);
		}
	});

	it('centers a small drawing in the free area after capping its zoom', () => {
		const camera = fitBounds([-1, -1, 1, 1], viewport, insets, 1);
		expect(camera.zoom).toBe(1);
		const x = viewport.width / 2 - camera.target[0] * 2 ** camera.zoom;
		const y = viewport.height / 2 - camera.target[1] * 2 ** camera.zoom;
		expect(x).toBe((insets.left + viewport.width - insets.right) / 2);
		expect(y).toBe((insets.top + viewport.height - insets.bottom) / 2);
	});
});
