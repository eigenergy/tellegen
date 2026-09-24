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
	plugins: [sveltekit(), experimentalAcOpf()],
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
