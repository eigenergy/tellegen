import { fileURLToPath } from 'node:url';
import { defineConfig, searchForWorkspaceRoot } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';

const repoRoot = fileURLToPath(new URL('../../../', import.meta.url));

export default defineConfig({
	// Playwright starts this fixture and the SvelteKit dev server together.
	// Their different dependency graphs must not overwrite one optimizer cache.
	cacheDir: fileURLToPath(new URL('../node_modules/.vite-mc-pf/', import.meta.url)),
	plugins: [svelte({ configFile: false })],
	root: fileURLToPath(new URL('./fixtures/mc-pf-browser/', import.meta.url)),
	publicDir: fileURLToPath(new URL('./fixtures/mc-pf-browser/public/', import.meta.url)),
	resolve: {
		alias: {
			'@tellegen/engine': fileURLToPath(
				new URL('../../../packages/engine/src/index.ts', import.meta.url)
			)
		}
	},
	server: {
		fs: { allow: [searchForWorkspaceRoot(repoRoot)] }
	}
});
