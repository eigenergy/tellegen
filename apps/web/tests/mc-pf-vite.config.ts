import { fileURLToPath } from 'node:url';
import { defineConfig, searchForWorkspaceRoot } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';

const repoRoot = fileURLToPath(new URL('../../../', import.meta.url));

export default defineConfig({
	// Playwright serves this fixture and the Kit app simultaneously. They use
	// different Svelte plugins and must not replace each other's optimized deps.
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
