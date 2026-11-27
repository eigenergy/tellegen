import { flushSync } from 'svelte';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';

/** The parts of the DOM the reader touches: the root's dataset and a
 * MutationObserver the test can fire by hand. */
class FakeObserver {
	static live = new Set<FakeObserver>();
	constructor(private callback: () => void) {}
	observe() {
		FakeObserver.live.add(this);
	}
	disconnect() {
		FakeObserver.live.delete(this);
	}
	static fire() {
		for (const observer of FakeObserver.live) observer.callback();
	}
}

const root = { dataset: {} as Record<string, string | undefined> };
const globals = globalThis as Record<string, unknown>;

beforeEach(() => {
	globals.document = { documentElement: root };
	globals.MutationObserver = FakeObserver;
	root.dataset = {};
});
afterEach(() => {
	delete globals.document;
	delete globals.MutationObserver;
});

describe('hostRootTheme', () => {
	it('follows a later change to the root theme and stops observing when unused', async () => {
		const { hostRootTheme } = await import('../src/lib/root-theme.svelte');
		root.dataset.theme = 'light';
		const seen: (string | undefined)[] = [];
		const stop = $effect.root(() => {
			$effect(() => {
				seen.push(hostRootTheme.current);
			});
		});
		flushSync();
		root.dataset.theme = 'dark';
		FakeObserver.fire();
		flushSync();
		delete root.dataset.theme;
		FakeObserver.fire();
		flushSync();
		expect(seen).toEqual(['light', 'dark', undefined]);
		stop();
		flushSync();
		await Promise.resolve();
		expect(FakeObserver.live.size).toBe(0);
	});

	it('ignores values other than light and dark', async () => {
		const { hostRootTheme } = await import('../src/lib/root-theme.svelte');
		root.dataset.theme = 'sepia';
		expect(hostRootTheme.current).toBeUndefined();
	});
});
