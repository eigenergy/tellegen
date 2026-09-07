import { fileURLToPath } from 'node:url';
import { defineConfig, searchForWorkspaceRoot } from 'vite';

const repoRoot = fileURLToPath(new URL('../../../', import.meta.url));

export default defineConfig({
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
