import { readdirSync, readFileSync } from 'node:fs';
import { join, relative } from 'node:path';
import { describe, expect, it } from 'vitest';

const root = join(import.meta.dirname, '../src/lib');
const FLOOR_PX = 12;

function sources(dir: string): string[] {
	return readdirSync(dir, { withFileTypes: true }).flatMap((entry) => {
		const path = join(dir, entry.name);
		if (entry.isDirectory()) return sources(path);
		return /\.(svelte|css)$/.test(entry.name) ? [path] : [];
	});
}

/** A literal font size in CSS (`font-size: 11px`, `font: 11px/1.4 …`) or an
 * SVG attribute (`font-size="11"`). Sizes from the `--fs-*` scale, all at
 * least 12px, and relative sizes floored with max() do not match. */
const LITERAL =
	/font-size\s*:\s*(\d+(?:\.\d+)?)px|font\s*:\s*(?:[a-z0-9-]+\s+)*?(\d+(?:\.\d+)?)px|font-size="(\d+(?:\.\d+)?)"/g;
/** A bare relative size such as `font-size: 0.85em`, which can fall under
 * the floor inside small text. */
const RELATIVE = /font-size\s*:\s*(0?\.\d+|0)(em|rem)\b/g;

describe('text size floor', () => {
	it('sets no literal font size under 12px in the kit', () => {
		const small: string[] = [];
		for (const file of sources(root)) {
			const text = readFileSync(file, 'utf8');
			for (const match of text.matchAll(LITERAL)) {
				const px = Number(match[1] ?? match[2] ?? match[3]);
				if (px < FLOOR_PX) small.push(`${relative(root, file)}: ${match[0]}`);
			}
			for (const match of text.matchAll(RELATIVE))
				small.push(`${relative(root, file)}: ${match[0]}`);
		}
		expect(small).toEqual([]);
	});
});
