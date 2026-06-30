import { existsSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const scriptDir = dirname(fileURLToPath(import.meta.url));
const distDir = join(scriptDir, '..', 'dist');
const note = '# Keep wasm-pack output in the tellegen-frontend package.\n';

for (const dir of ['wasm-pkg', 'wasm-sens-pkg']) {
	const packageDir = join(distDir, dir);
	if (existsSync(packageDir)) writeFileSync(join(packageDir, '.npmignore'), note);
}
