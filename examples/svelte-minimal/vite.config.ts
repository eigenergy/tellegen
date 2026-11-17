import { fileURLToPath } from 'node:url';
import { svelte } from '@sveltejs/vite-plugin-svelte';
import { defineConfig } from 'vite';

export default defineConfig({
	plugins: [svelte()],
	build: {
		chunkSizeWarningLimit: 1200,
		rollupOptions: {
			input: {
				viewer: fileURLToPath(new URL('index.html', import.meta.url)),
				custom: fileURLToPath(new URL('custom.html', import.meta.url))
			},
			output: {
				manualChunks(id) {
					if (
						id.includes('/node_modules/@deck.gl/') ||
						id.includes('/node_modules/@luma.gl/') ||
						id.includes('/node_modules/@math.gl/') ||
						id.includes('/node_modules/@probe.gl/')
					) {
						return 'deck-vendor';
					}
					if (id.includes('/node_modules/maplibre-gl/')) {
						return 'map-vendor';
					}
				}
			}
		}
	}
});
