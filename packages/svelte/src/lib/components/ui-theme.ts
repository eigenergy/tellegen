import { getUiConfig } from '../context.svelte.js';

/** The provider's live theme, read during component initialisation. Components that also render
 * outside a tellegen provider (the exported multiconductor results) fall back to light. */
export function uiTheme(): { readonly current: 'light' | 'dark' } {
	let config: { theme: 'light' | 'dark' } | null = null;
	try {
		config = getUiConfig();
	} catch {
		// Outside a provider.
	}
	return {
		get current() {
			return config?.theme ?? 'light';
		}
	};
}
