import { flushSync } from 'svelte';
import { describe, expect, it } from 'vitest';
import { PanelLayout, type PanelRegistration } from '../src/lib/panels.svelte';

function registration(id: string, side: 'left' | 'right' = 'left'): PanelRegistration {
	let open = $state(true);
	return {
		id,
		title: id,
		side,
		order: 0,
		width: 320,
		get open() {
			return open;
		},
		setOpen: (value) => (open = value),
		children: (() => {}) as unknown as PanelRegistration['children']
	};
}

function floatingLayout() {
	const layout = new PanelLayout();
	layout.register(registration('region'));
	layout.placements.region = { x: 900, y: 200, width: 360, height: 400, position: 'floating' };
	return layout;
}

describe('PanelLayout.resizeViewport', () => {
	it('can run inside an effect while a panel floats', () => {
		const layout = floatingLayout();
		let runs = 0;
		const stop = $effect.root(() => {
			$effect(() => {
				runs++;
				layout.resizeViewport(1512, 860, 76);
			});
		});
		expect(() => flushSync()).not.toThrow();
		const placement = layout.placements.region;
		layout.resizeViewport(1512, 860, 76);
		flushSync();
		expect(layout.placements.region).toBe(placement);
		expect(runs).toBe(1);
		stop();
	});

	it('clamps a floating panel once when the viewport shrinks', () => {
		const layout = floatingLayout();
		layout.resizeViewport(1512, 860, 76);
		layout.resizeViewport(1000, 700, 76);
		const clamped = layout.placements.region;
		expect(clamped.x + clamped.width).toBeLessThanOrEqual(1000 - 16);
		layout.resizeViewport(1000, 700, 76);
		expect(layout.placements.region).toBe(clamped);
	});

	it('writes compact and the drawer only when they change', () => {
		const layout = floatingLayout();
		layout.resizeViewport(800, 700, 76);
		expect(layout.compact).toBe(true);
		layout.drawer = null;
		layout.resizeViewport(800, 700, 76);
		expect(layout.drawer).toBeNull();
	});
});

describe('PanelLayout.hide', () => {
	it('closes only the given panel', () => {
		const layout = new PanelLayout();
		const front = registration('network');
		const behind = registration('settings');
		layout.register(front);
		layout.register(behind);
		layout.dockActive.left = 'network';
		layout.hide(front);
		expect(front.open).toBe(false);
		expect(behind.open).toBe(true);
	});

	it('clears the drawer when it shows the hidden panel', () => {
		const layout = new PanelLayout();
		const panel = registration('network');
		layout.register(panel);
		layout.drawer = 'network';
		layout.hide(panel);
		expect(layout.drawer).toBeNull();
	});
});
