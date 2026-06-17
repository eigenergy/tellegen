import { existsSync, readdirSync, readFileSync, statSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join, relative } from 'node:path';

const scriptDir = dirname(fileURLToPath(import.meta.url));
const buildDir = join(scriptDir, '..', 'build');
const appDir = join(buildDir, '_app');
const indexPath = join(buildDir, 'index.html');
const fallbackPath = join(buildDir, '200.html');

function fail(message) {
	console.error(`build smoke failed: ${message}`);
	process.exit(1);
}

function read(path) {
	if (!existsSync(path)) fail(`${relative(buildDir, path)} is missing`);
	return readFileSync(path, 'utf8');
}

function walk(dir, files = []) {
	for (const entry of readdirSync(dir)) {
		const path = join(dir, entry);
		if (statSync(path).isDirectory()) walk(path, files);
		else files.push(path);
	}
	return files;
}

function svelteKitIds(text) {
	return [...text.matchAll(/__sveltekit_[A-Za-z0-9_$]+/g)].map((match) => match[0]);
}

if (!existsSync(buildDir)) fail('build directory is missing');
if (!existsSync(appDir)) fail('_app directory is missing');

const htmlFiles = [fallbackPath, ...(existsSync(indexPath) ? [indexPath] : [])];
const html = htmlFiles.map(read);
const htmlIds = new Set(html.flatMap(svelteKitIds));
if (htmlIds.size !== 1)
	fail(`expected one SvelteKit bootstrap id, found ${[...htmlIds].join(', ')}`);
const [bootstrapId] = htmlIds;

const jsRefs = new Set();
for (const file of walk(appDir).filter((path) => path.endsWith('.js'))) {
	for (const id of svelteKitIds(read(file))) jsRefs.add(id);
}
if (!jsRefs.has(bootstrapId)) fail(`runtime chunks do not reference ${bootstrapId}`);
for (const id of jsRefs) {
	if (id !== bootstrapId) fail(`runtime chunk references stale SvelteKit id ${id}`);
}

const assetRefs = html
	.flatMap((text) => [...text.matchAll(/(?:href|src)="([^"]+)"|import\("([^"]+)"\)/g)])
	.map((match) => match[1] ?? match[2])
	.filter((ref) => ref && !/^[a-z]+:\/\//i.test(ref))
	.map((ref) => ref.replace(/^\.\//, '').replace(/^\//, ''));

for (const ref of assetRefs) {
	if (!ref.startsWith('_app/')) continue;
	const [path] = ref.split(/[?#]/);
	if (!existsSync(join(buildDir, path))) fail(`referenced asset is missing: ${path}`);
}

console.log(`build smoke passed: ${bootstrapId}`);
