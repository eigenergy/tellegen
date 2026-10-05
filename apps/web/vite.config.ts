import adapter from '@sveltejs/adapter-static';
import { vitePreprocess } from '@sveltejs/vite-plugin-svelte';
import { sveltekit } from '@sveltejs/kit/vite';
import { existsSync, readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { defineConfig, searchForWorkspaceRoot, type Plugin } from 'vite';

const configDir = fileURLToPath(new URL('.', import.meta.url));
const experimentalAcOpfAsset = fileURLToPath(
	new URL('../../target/experimental-acopf/tellegen_acopf_wasi.wasm', import.meta.url)
);

function experimentalAcOpf(): Plugin {
	return {
		name: 'tellegen-experimental-acopf',
		apply: 'build' as const,
		buildStart() {
			if (!process.env.PUBLIC_TELLEGEN_ACOPF_WASM_URL) return;
			if (!existsSync(experimentalAcOpfAsset)) {
				this.error('PUBLIC_TELLEGEN_ACOPF_WASM_URL requires `npm run wasm:acopf` first');
			}
		},
		generateBundle() {
			if (!process.env.PUBLIC_TELLEGEN_ACOPF_WASM_URL) return;
			this.emitFile({
				type: 'asset',
				fileName: 'experimental-acopf/tellegen_acopf_wasi.wasm',
				source: readFileSync(experimentalAcOpfAsset)
			});
		}
	};
}

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
		}),
		experimentalAcOpf()
	],
	build: {
		// The map is loaded on the client only, but deck.gl/luma.gl are a large
		// coupled WebGL stack. Keep them together so Rolldown does not split
		// circular luma.gl modules across chunks, and set the warning threshold
		// for this known async vendor chunk.
		chunkSizeWarningLimit: 1200,
		rolldownOptions: {
			output: {
				// Kit 3 sets its own codeSplitting groups, which disables manualChunks.
				codeSplitting: {
					groups: [
						{ name: 'deck-vendor', test: /\/node_modules\/@(deck|luma|math|probe)\.gl\// },
						{ name: 'map-vendor', test: /\/node_modules\/maplibre-gl\// }
					]
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
