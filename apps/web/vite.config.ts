import adapter from '@sveltejs/adapter-static';
import { vitePreprocess } from '@sveltejs/vite-plugin-svelte';
import { sveltekit } from '@sveltejs/kit/vite';
import { fileURLToPath } from 'node:url';
import { defineConfig, searchForWorkspaceRoot } from 'vite';

const configDir = fileURLToPath(new URL('.', import.meta.url));

export default defineConfig({
	plugins: [
		sveltekit({
			preprocess: vitePreprocess(),
			adapter: adapter({ fallback: '200.html', precompress: true }),
			// The policy travels with the build. `deploy/Caddyfile` is a sample that
			// CI never copies, so a policy that lives only there protects nothing.
			// Hash mode works with a prerendered static build: kit hashes its own
			// inline bootstrap, so `script-src` needs no `unsafe-inline`.
			csp: {
				mode: 'hash',
				directives: {
					'default-src': ['self'],
					'base-uri': ['self'],
					'object-src': ['none'],
					'frame-ancestors': ['none'],
					'form-action': ['self'],
					// The engine is wasm, which needs `wasm-unsafe-eval`. That allows
					// wasm compilation only. Never widen it to `unsafe-eval`.
					'script-src': ['self', 'wasm-unsafe-eval', 'https://cloud.umami.is'],
					// Svelte and maplibre both write inline style attributes.
					'style-src': ['self', 'unsafe-inline'],
					'img-src': ['self', 'data:', 'blob:', 'https://*.cartocdn.com'],
					'connect-src': ['self', 'https://*.cartocdn.com', 'https://gateway.umami.is'],
					// The engine worker and maplibre's workers load from blob URLs.
					'worker-src': ['self', 'blob:'],
					'child-src': ['self', 'blob:'],
					'font-src': ['self'],
					'upgrade-insecure-requests': true
				}
			}
		})
	],
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
			'/api': process.env.TELLEGEN_API_ORIGIN ?? 'http://localhost:8000'
		}
	}
});
