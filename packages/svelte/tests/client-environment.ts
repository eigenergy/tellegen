import type { Environment } from 'vitest/environments';

/** Node globals with Vite's client transform, so `.svelte.test.ts` files
 * compile runes for the browser and their effects run. */
export default {
	name: 'svelte-client',
	viteEnvironment: 'client',
	setup: () => ({ teardown: () => {} })
} satisfies Environment;
