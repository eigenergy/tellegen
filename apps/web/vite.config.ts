import { sveltekit } from '@sveltejs/kit/vite';
import { fileURLToPath } from 'node:url';
import { defineConfig, searchForWorkspaceRoot } from 'vite';

const configDir = fileURLToPath(new URL('.', import.meta.url));

export default defineConfig({
	plugins: [sveltekit()],
	build: {
		// The map is loaded on the client only, but deck.gl/luma.gl are a large
		// coupled WebGL stack. Keep them together so Rollup does not split
		// circular luma.gl modules across chunks, and set the warning threshold
		// for this known async vendor chunk.
		chunkSizeWarningLimit: 1200,
		rollupOptions: {
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
	},
	server: {
		fs: {
			allow: [searchForWorkspaceRoot(configDir)]
		},
		proxy: {
			'/api': 'http://localhost:8000'
		}
	}
});
