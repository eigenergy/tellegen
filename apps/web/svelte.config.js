import adapter from '@sveltejs/adapter-static';
import { vitePreprocess } from '@sveltejs/vite-plugin-svelte';

/** @type {import('@sveltejs/kit').Config} */
const config = {
	preprocess: vitePreprocess(),
	kit: {
		adapter: adapter({
			fallback: '200.html',
			precompress: true
		}),
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
				'script-src': ['self', 'wasm-unsafe-eval'],
				// Svelte and maplibre both write inline style attributes.
				'style-src': ['self', 'unsafe-inline'],
				'img-src': ['self', 'data:', 'blob:', 'https://*.cartocdn.com'],
				'connect-src': ['self', 'https://*.cartocdn.com'],
				// The engine worker and maplibre's workers load from blob URLs.
				'worker-src': ['self', 'blob:'],
				'child-src': ['self', 'blob:'],
				'font-src': ['self'],
				'upgrade-insecure-requests': true
			}
		}
	}
};

export default config;
