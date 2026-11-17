import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readdirSync, readFileSync, statSync } from 'node:fs';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { TELLEGEN_ERROR_CODES } from '../../../packages/webmcp/src/error-codes.ts';

const root = fileURLToPath(new URL('../../..', import.meta.url));
// Where tool errors are raised: the WebMCP package, the Svelte adapter and this app.
const SOURCES = ['packages/webmcp/src', 'packages/svelte/src/lib', 'apps/web/src'];
// `TellegenToolError('CODE', ...)`, and the `code: 'CODE'` of an error response.
const RAISED = /(?:TellegenToolError\(\s*|\bcode:\s*)["']([A-Z][A-Z_]{2,})["']/g;
// Not a tool error: the placeholder for an import finding without a code.
const IGNORED = new Set(['UNCODED']);

function files(dir) {
	return readdirSync(dir).flatMap((name) => {
		const path = join(dir, name);
		if (statSync(path).isDirectory()) return files(path);
		return /\.(ts|svelte)$/.test(name) && !/\.test\.ts$/.test(name) ? [path] : [];
	});
}

const raised = new Set();
for (const source of SOURCES)
	for (const file of files(join(root, source)))
		for (const match of readFileSync(file, 'utf8').matchAll(RAISED))
			if (!IGNORED.has(match[1])) raised.add(match[1]);
const registered = new Set(Object.keys(TELLEGEN_ERROR_CODES));

test('TELLEGEN_ERROR_CODES lists every code the tools raise', () => {
	assert.deepEqual(
		[...raised].filter((code) => !registered.has(code)),
		[]
	);
});

test('TELLEGEN_ERROR_CODES lists no code that nothing raises', () => {
	assert.deepEqual(
		[...registered].filter((code) => !raised.has(code)),
		[]
	);
});
