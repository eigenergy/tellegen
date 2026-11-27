import { createSubscriber } from 'svelte/reactivity';

export type RootTheme = 'light' | 'dark';

/** The host page's `data-theme` on its root element, read live: a host that
 * switches it later is followed without a reload. */
class HostRootTheme {
	#subscribe = createSubscriber((update) => {
		const observer = new MutationObserver(update);
		observer.observe(document.documentElement, {
			attributes: true,
			attributeFilter: ['data-theme']
		});
		return () => observer.disconnect();
	});

	/** `'light'` or `'dark'` when the root names one; undefined otherwise and
	 * during server rendering. */
	get current(): RootTheme | undefined {
		if (typeof document === 'undefined') return undefined;
		this.#subscribe();
		const value = document.documentElement.dataset.theme;
		return value === 'dark' || value === 'light' ? value : undefined;
	}
}

export const hostRootTheme = new HostRootTheme();
